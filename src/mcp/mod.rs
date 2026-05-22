//! MCP stdio server — JSON-RPC 2.0 over stdio
//! Mirrors gbrain's src/mcp/server.ts
//!
//! Implements the Model Context Protocol for agent integration.
//! All operations are dispatched through the Operations layer with
//! OperationContext.remote = true (untrusted callers).

pub mod tool_defs;

use crate::config::Config;
use crate::error::{GBrainError, OperationError, Result};
use crate::mcp::tool_defs::get_operation_def;
use crate::operations::{OpContext, Operations, ParamType};
use crate::sqlite_engine::SqliteEngine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, Write};
use tracing::{debug, info, warn};

/// JSON-RPC 2.0 request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

/// JSON-RPC 2.0 response — or None for notifications (no response should be sent)
enum HandleResult {
    Response(JsonRpcResponse),
    NoResponse, // For JSON-RPC notifications — server must not reply
}
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// MCP 分页参数安全边界：limit <= 0 视为未指定，回退到 default；
/// limit 超过 max 则 clamp 到 max，防止远程调用绕过限制全量 dump。
fn normalize_limit(limit: i64, default: i64, max: i64) -> i64 {
    if limit <= 0 {
        default
    } else {
        limit.min(max)
    }
}

/// offset 不能为负数，否则可能导致 SQL 行为异常或绕过安全限制。
fn normalize_offset(offset: i64) -> i64 {
    offset.max(0)
}

/// 校验参数：仅校验 OperationDef 中 required=true 的参数是否必填，
/// 可选参数仅在传入时校验类型。复用 tool_defs 的 OperationDef/ParamDef，
/// 避免手写校验规则与 schema 漂移。
fn validate_params(tool_name: &str, arguments: &Value) -> Option<String> {
    // 从 OperationDef 查找工具定义
    let op_def = get_operation_def(tool_name)?;

    for param in op_def.params.iter() {
        match arguments.get(param.name) {
            None => {
                // 仅 required 参数缺失时报错
                if param.required {
                    return Some(format!(
                        "缺少必填参数 '{}' (工具 '{}')",
                        param.name, tool_name
                    ));
                }
            }
            Some(val) => {
                // 传入时校验类型是否匹配
                let type_ok = match param.param_type {
                    ParamType::String => val.is_string(),
                    ParamType::Integer => val.is_u64() || val.is_i64(),
                    ParamType::Boolean => val.is_boolean(),
                    ParamType::Number => val.is_f64() || val.is_i64() || val.is_u64(),
                    ParamType::Array => val.is_array(),
                    ParamType::Object => val.is_object(),
                };
                if !type_ok {
                    return Some(format!(
                        "参数 '{}' (工具 '{}') 应为 {}，实际为 {}",
                        param.name,
                        tool_name,
                        param.param_type.json_type_name(),
                        val
                    ));
                }
                // 校验 enum_values（如果有声明），防止 query.mode=grap、review.status=pendng
                // 等拼写错误穿透到 service 层被静默 fallback
                if let Some(enums) = param.enum_values {
                    if let Some(s) = val.as_str() {
                        if !enums.contains(&s) {
                            return Some(format!(
                                "参数 '{}' (工具 '{}') 无效值 '{}'，有效值: {}",
                                param.name,
                                tool_name,
                                s,
                                enums.join(", ")
                            ));
                        }
                    }
                }
            }
        }
    }

    None
}

/// MCP server running over stdio
pub struct McpServer {
    engine: SqliteEngine,
    config: Config,
}

impl McpServer {
    pub fn new(engine: SqliteEngine) -> Self {
        Self::with_config(engine, Config::default())
    }

    pub fn with_config(engine: SqliteEngine, config: Config) -> Self {
        Self { engine, config }
    }

    /// 测试辅助：直接调用 MCP tools/call dispatch 路径
    /// 不走 stdio，直接传入工具名和参数，返回 dispatch 结果。
    /// 仅用于集成测试验证参数映射、内部工具拦截和返回包装。
    pub fn dispatch_tool_call(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });
        self.handle_tool_call(Some(params))
    }

    /// Run the MCP server, reading JSON-RPC from stdin and writing to stdout
    pub fn run(&mut self) -> Result<()> {
        info!("MCP server starting (stdio transport)");
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = stdin.lock();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!("MCP stdin read error: {}", e);
                    break;
                }
            };

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(e) => {
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        }),
                    };
                    if let Ok(resp_str) = serde_json::to_string(&response) {
                        let _ = writeln!(stdout, "{}", resp_str);
                    }
                    let _ = stdout.flush();
                    continue;
                }
            };

            let result = self.handle_request(request);

            match result {
                HandleResult::Response(response) => {
                    if let Ok(resp_str) = serde_json::to_string(&response) {
                        let _ = writeln!(stdout, "{}", resp_str);
                        let _ = stdout.flush();
                    }
                }
                HandleResult::NoResponse => {
                    // JSON-RPC 2.0: "The Server MUST NOT reply to a Notification"
                    // No output for notifications
                }
            }
        }

        Ok(())
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> HandleResult {
        let id = request.id.clone();
        debug!(method = %request.method, "Handling MCP request");

        match request.method.as_str() {
            "initialize" => HandleResult::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": { "listChanged": false },
                    },
                    "serverInfo": {
                        "name": "gbrain",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                })),
                error: None,
            }),

            "tools/list" => {
                let tools = tool_defs::build_tool_defs();
                let tools_json: Vec<Value> = tools
                    .into_iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.input_schema,
                        })
                    })
                    .collect();

                HandleResult::Response(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({ "tools": tools_json })),
                    error: None,
                })
            }

            "tools/call" => {
                let result = self.handle_tool_call(request.params);
                match result {
                    Ok(value) => {
                        info!(tool = "tools/call", "MCP tool call completed successfully");
                        HandleResult::Response(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(serde_json::json!({
                                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                            })),
                            error: None,
                        })
                    }
                    Err(e) => {
                        warn!(error = %e, "MCP tool call failed, sending error response");
                        // P2-9: Convert GBrainError to OperationError for structured response
                        let op_err = e.to_operation_error();
                        let error_data = serde_json::json!({
                            "code": match &op_err {
                                OperationError::NotFound { .. } => "NOT_FOUND",
                                OperationError::Forbidden { .. } => "FORBIDDEN",
                                OperationError::Validation { .. } => "VALIDATION",
                                OperationError::Failed { .. } => "INTERNAL",
                            },
                            "message": op_err.to_string(),
                            "suggestion": match &op_err {
                                OperationError::NotFound { suggestion, .. } => suggestion,
                                OperationError::Forbidden { suggestion, .. } => suggestion,
                                OperationError::Validation { suggestion, .. } => suggestion,
                                OperationError::Failed { suggestion, .. } => suggestion,
                            },
                            "docs_url": match &op_err {
                                OperationError::NotFound { docs_url, .. } => docs_url,
                                OperationError::Forbidden { docs_url, .. } => docs_url,
                                OperationError::Validation { docs_url, .. } => docs_url,
                                OperationError::Failed { docs_url, .. } => docs_url,
                            },
                        });
                        HandleResult::Response(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(serde_json::json!({
                                "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                                "isError": true,
                                "errorData": error_data,
                            })),
                            error: None,
                        })
                    }
                }
            }

            "ping" => HandleResult::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({})),
                error: None,
            }),

            // JSON-RPC 2.0: "The Server MUST NOT reply to a Notification"
            "notifications/initialized" => HandleResult::NoResponse,

            _ => HandleResult::Response(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            }),
        }
    }

    fn handle_tool_call(&mut self, params: Option<Value>) -> Result<Value> {
        let params = params.unwrap_or_default();
        let tool_name = params["name"].as_str().unwrap_or("").to_string();
        let arguments = params.get("arguments").cloned().unwrap_or_default();

        debug!(tool = %tool_name, "Dispatching MCP tool call");

        // Validate required parameters before dispatching
        if let Some(err) = validate_params(&tool_name, &arguments) {
            debug!(tool = %tool_name, error = %err, "Parameter validation failed");
            return Err(crate::error::GBrainError::InvalidInput(err));
        }

        let ctx = OpContext {
            remote: true, // MCP callers are untrusted
            working_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            dry_run: false,
            subagent_id: None,
        };
        let ops = Operations::with_config(&self.engine, ctx.clone(), self.config.clone());

        match tool_name.as_str() {
            // ========================================================================
            // artifact_* facade — 统一知识操作入口（设计文档 §8.2）
            // 默认暴露给用户，参数映射到现有内部 handler
            // ========================================================================

            // artifact_put: 手动写入长期记忆（设计文档 §4.1.2）
            "artifact_put" => {
                let slug = arguments["slug"].as_str().unwrap_or("").to_string();
                let title = arguments["title"].as_str().map(|s| s.to_string());
                let intent = arguments["intent"].as_str().map(|s| s.to_string());
                let dry_run = arguments["dry_run"].as_bool().unwrap_or(false);

                // 安全校验：slug 不能为空
                if slug.is_empty() {
                    return Err(GBrainError::InvalidInput("slug 不能为空".to_string()));
                }

                // 支持 content 和 file 两种输入方式
                let content = if let Some(c) = arguments["content"].as_str() {
                    c.to_string()
                } else if let Some(f) = arguments["file"].as_str() {
                    // 从文件读取内容
                    let file_path = std::path::PathBuf::from(f);
                    // P2-12 修复：artifact_put --file 使用与 put_memory 相同的 1MB 大小上限
                    // 和文本文件专用扩展名白名单，而非 KB 上传的 50MB 上限和 KB 扩展名列表。
                    // 之前使用 kb_max_file_size_mb（默认 50MB），导致 1MB~50MB 的文本文件
                    // 先完整读入，再在 service 层被拒绝。
                    let allowed_extensions: Vec<String> =
                        crate::artifact::service::TEXT_FILE_EXTENSIONS
                            .iter()
                            .map(|s| s.to_string())
                            .collect();
                    let max_file_bytes = crate::artifact::service::MAX_PUT_MEMORY_CONTENT_BYTES;
                    let validated_path = crate::kb::security::validate_upload_source(
                        &file_path,
                        true,
                        &ctx.working_dir,
                        max_file_bytes,
                        &allowed_extensions,
                    )?;
                    let ext = validated_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let raw_content = std::fs::read(&validated_path).map_err(|e| {
                        GBrainError::FileError(format!("读取文件 {} 失败: {}", f, e))
                    })?;
                    let _mime = crate::kb::security::detect_and_validate_mime(&raw_content, &ext)?;
                    // artifact_put 只接受文本内容，需要转为 String
                    String::from_utf8(raw_content).map_err(|e| {
                        GBrainError::InvalidInput(format!("文件内容不是有效 UTF-8 文本: {}", e))
                    })?
                } else {
                    return Err(GBrainError::InvalidInput(
                        "必须提供 content 或 file 参数".to_string(),
                    ));
                };

                // 安全校验：内容不能为空
                if content.is_empty() {
                    return Err(GBrainError::InvalidInput("内容不能为空".to_string()));
                }

                let svc = ops.artifact_service();
                // P1 修复：force 参数允许强制覆盖已被人工修改的页面
                let force = arguments
                    .get("force")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                svc.put_memory(
                    &slug,
                    &content,
                    title.as_deref(),
                    intent.as_deref(),
                    dry_run,
                    force,
                )
            }

            // artifact_upload: 委派到 ArtifactService.upload_file（设计文档 §8.3）
            "artifact_upload" => {
                let path = arguments["path"].as_str().unwrap_or("").to_string();
                let file_path = std::path::PathBuf::from(&path);

                let intent_str = arguments["intent"].as_str().unwrap_or("auto");
                // 使用 FromStr 严格解析，MCP 客户端传错枚举值直接报错返回
                let intent: crate::artifact::types::UploadIntent = match intent_str.parse() {
                    Ok(i) => i,
                    Err(e) => {
                        return Err(crate::error::GBrainError::InvalidInput(e));
                    }
                };

                // 根据扩展名推断 route plan，再选择允许列表
                let ext_for_route = file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let route_plan =
                    crate::artifact::types::infer_route_plan(&ext_for_route, "", &intent);
                let allowed_extensions: Vec<String> = if route_plan.to_file {
                    let mut exts = self.config.kb_allowed_extensions.clone();
                    for extra in &[
                        "png", "jpg", "jpeg", "gif", "bmp", "svg", "webp", "avif", "ico", "tiff",
                        "tif", "zip", "tar", "gz", "json", "xml", "yaml", "yml", "toml",
                    ] {
                        let s = extra.to_string();
                        if !exts.contains(&s) {
                            exts.push(s);
                        }
                    }
                    exts
                } else {
                    self.config.kb_allowed_extensions.clone()
                };
                let max_file_bytes = self.config.kb_max_file_size_mb * 1024 * 1024;

                let validated_path = crate::kb::security::validate_upload_source(
                    &file_path,
                    true,
                    &ctx.working_dir,
                    max_file_bytes,
                    &allowed_extensions,
                )?;

                let ext = validated_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let content = std::fs::read(&validated_path)?;
                let _mime = crate::kb::security::detect_and_validate_mime(&content, &ext)?;

                let original_name = validated_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let promotion_policy: Option<crate::artifact::types::PromotionPolicy> = arguments
                    .get("promotion")
                    .and_then(|v| v.as_str())
                    .map(|s| s.parse())
                    .transpose()
                    .map_err(|e| crate::error::GBrainError::InvalidInput(e))?;

                let input = crate::artifact::types::UploadSourceInput {
                    content,
                    original_name: original_name.clone(),
                    source_kind: crate::artifact::types::SourceKind::Mcp,
                    source_uri: path,
                    intent,
                    target_slug: arguments["target_slug"].as_str().map(|s| s.to_string()),
                    page_slug: arguments["page_slug"].as_str().map(|s| s.to_string()),
                    library_id: arguments["library_id"].as_i64(),
                    folder_id: arguments["folder_id"].as_i64(),
                    promotion_policy,
                    owner_ref: None,
                    metadata: None,
                    path: Some(validated_path.clone()),
                    dry_run: arguments["dry_run"].as_bool().unwrap_or(false),
                };

                let svc = ops.artifact_service();
                let result = svc.upload_file(input)?;
                Ok(serde_json::to_value(result)?)
            }

            // artifact_query: 统一知识查询（设计文档 §7）
            // 返回 ArtifactQueryOutput，隐藏内部 ID
            "artifact_query" => {
                let query = arguments["query"].as_str().unwrap_or("").to_string();
                let mode = arguments["mode"].as_str().map(|s| s.to_string());
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let filter_slug = arguments["filter_slug"].as_str().map(|s| s.to_string());
                let include_sources = arguments["include_sources"].as_bool();

                let input = crate::artifact::types::ArtifactQueryInput {
                    query,
                    mode,
                    limit,
                    filter_slug,
                    include_sources,
                };

                let svc = ops.artifact_service();
                let result = svc.query_facade(&input)?;
                Ok(serde_json::to_value(result)?)
            }

            // P1-1 修复：artifact_list 改走 ArtifactService::list_artifacts DTO，
            // 不再返回 raw SourceArtifact（含内部 id/storage_path/metadata_json 等字段）
            "artifact_list" => {
                // 安全边界：clamp limit/offset 防止远程调用负数绕过
                let limit = normalize_limit(arguments["limit"].as_i64().unwrap_or(50), 50, 100);
                let offset = normalize_offset(arguments["offset"].as_i64().unwrap_or(0));
                let svc = ops.artifact_service();
                let items = svc.list_artifacts(limit, offset)?;
                Ok(serde_json::to_value(items)?)
            }

            // artifact_get: 获取 Artifact 详情（设计文档 §4.1.1）
            "artifact_get" => {
                let id_or_uid = arguments["id_or_uid"].as_str().unwrap_or("").to_string();
                let include_projections =
                    arguments["include_projections"].as_bool().unwrap_or(false);
                let include_sources = arguments["include_sources"].as_bool().unwrap_or(false);
                let include_content = arguments["include_content"].as_bool().unwrap_or(false);

                let svc = ops.artifact_service();
                let detail = svc.get_artifact_detail(
                    &id_or_uid,
                    include_projections,
                    include_sources,
                    include_content,
                )?;

                match detail {
                    Some(d) => Ok(serde_json::to_value(d)?),
                    None => Ok(serde_json::json!({
                        "error": format!("未找到 artifact '{}'", id_or_uid)
                    })),
                }
            }

            // artifact_delete: 委派到现有 artifact_delete handler
            // facade 版本用 id_or_uid，内部版本用 artifact_id
            "artifact_delete" => {
                let id_or_uid = arguments["id_or_uid"].as_str().unwrap_or("").to_string();
                let dry_run = arguments["dry_run"].as_bool().unwrap_or(false);

                let svc = ops.artifact_service();
                if dry_run {
                    // P1-5 修复：MCP dry_run 也使用 delete_artifact_dry_run 影响预览，
                    // 与 CLI 行为一致，让 MCP caller 能看到 occurrence/projection/KB/provenance 影响明细
                    let preview = svc.delete_artifact_dry_run(&id_or_uid)?;
                    Ok(serde_json::to_value(preview)?)
                } else {
                    let artifact_id = svc.resolve_artifact_id(&id_or_uid)?;
                    svc.delete_artifact(artifact_id)?;
                    Ok(serde_json::json!({"artifact_id": artifact_id, "status": "deleted"}))
                }
            }

            // artifact_detach: 移除知识源与页面的关联（设计文档 §4.1.4）
            "artifact_detach" => {
                let id_or_uid = arguments["id_or_uid"].as_str().unwrap_or("").to_string();
                let from_slug = arguments["from"].as_str().unwrap_or("").to_string();
                let dry_run = arguments["dry_run"].as_bool().unwrap_or(false);

                if from_slug.is_empty() {
                    return Err(GBrainError::InvalidInput(
                        "from (目标页面 slug) 不能为空".to_string(),
                    ));
                }

                let svc = ops.artifact_service();
                svc.detach(&id_or_uid, &from_slug, dry_run)
            }

            // artifact_restore: 恢复已软删除的知识源（设计文档 §4.1.4）
            "artifact_restore" => {
                let id_or_uid = arguments["id_or_uid"].as_str().unwrap_or("").to_string();
                let dry_run = arguments["dry_run"].as_bool().unwrap_or(false);

                let svc = ops.artifact_service();
                svc.restore(&id_or_uid, dry_run)
            }

            // artifact_reprocess: 重新处理知识源（设计文档 §4.1.4）
            "artifact_reprocess" => {
                let id_or_uid = arguments["id_or_uid"].as_str().unwrap_or("").to_string();
                let dry_run = arguments["dry_run"].as_bool().unwrap_or(false);

                let svc = ops.artifact_service();
                svc.reprocess(&id_or_uid, dry_run)
            }

            // artifact_health: 委派到 ArtifactService.health_check
            "artifact_health" => {
                let svc = ops.artifact_service();
                let report = svc.health_check()?;
                Ok(serde_json::to_value(report)?)
            }

            // artifact_review_list: 列出建议变更，返回用户友好的 ArtifactReviewItem
            "artifact_review_list" => {
                let status = arguments["status"].as_str();
                let target_slug = arguments["target_slug"].as_str();
                // 安全边界：clamp limit 防止远程调用负数绕过
                let limit = normalize_limit(arguments["limit"].as_i64().unwrap_or(50), 50, 100);

                let svc = ops.artifact_service();
                let items = svc.list_suggested_changes(status, target_slug, limit, 0)?;
                Ok(serde_json::to_value(items)?)
            }

            // artifact_review_get: 获取建议变更详情，返回用户友好的 ArtifactReviewItem
            "artifact_review_get" => {
                let change_id = arguments["change_id"].as_i64().unwrap_or(0);

                let svc = ops.artifact_service();
                let item = svc.get_suggested_change(change_id)?;
                Ok(serde_json::to_value(item)?)
            }

            // artifact_review_apply: 应用建议变更
            "artifact_review_apply" => {
                let change_id = arguments["change_id"].as_i64().unwrap_or(0);

                let svc = ops.artifact_service();
                let result = svc.apply_suggested_change(change_id)?;
                Ok(serde_json::to_value(result)?)
            }

            // artifact_review_reject: 拒绝建议变更
            "artifact_review_reject" => {
                let change_id = arguments["change_id"].as_i64().unwrap_or(0);
                let reviewer = arguments["reviewer"].as_str().unwrap_or("mcp").to_string();
                let reason = arguments["reason"].as_str().map(|s| s.to_string());

                let input = crate::artifact::types::ReviewCandidateInput {
                    candidate_id: change_id,
                    action: "reject".to_string(),
                    reviewer,
                    notes: reason,
                };

                let svc = ops.artifact_service();
                let result = svc.reject_suggested_change(input)?;
                Ok(serde_json::to_value(result)?)
            }

            // artifact_review_rollback: 回滚已应用的建议变更
            "artifact_review_rollback" => {
                let change_id = arguments["change_id"].as_i64().unwrap_or(0);

                let svc = ops.artifact_service();
                let result = svc.rollback_suggested_change(change_id)?;
                Ok(serde_json::to_value(result)?)
            }

            _ => Err(crate::error::GBrainError::InvalidInput(format!(
                "Unknown tool: {}",
                tool_name
            ))),
        }
    }
}
