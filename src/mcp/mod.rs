//! MCP stdio server — JSON-RPC 2.0 over stdio
//! Mirrors gbrain's src/mcp/server.ts
//!
//! Implements the Model Context Protocol for agent integration.
//! All operations are dispatched through the Operations layer with
//! OperationContext.remote = true (untrusted callers).

pub mod tool_defs;

use crate::config::Config;
use crate::engine::BrainEngine;
use crate::error::{GBrainError, OperationError, Result};
use crate::mcp::tool_defs::get_operation_def;
use crate::operations::{OpContext, Operations, ParamType};
use crate::sqlite_engine::SqliteEngine;
use crate::types::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, Write};
use tracing::{debug, info};

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

/// 校验参数：仅校验 OperationDef 中 required=true 的参数是否必填，
/// 可选参数仅在传入时校验类型。复用 tool_defs 的 OperationDef/ParamDef，
/// 避免手写校验规则与 schema 漂移。
fn validate_params(tool_name: &str, arguments: &Value) -> Option<String> {
    // 从 OperationDef 查找工具定义
    let op_def = match get_operation_def(tool_name) {
        Some(def) => def,
        None => return None, // 未定义的工具不做校验
    };

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
                    Ok(value) => HandleResult::Response(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::json!({
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                        })),
                        error: None,
                    }),
                    Err(e) => {
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
            "query" => {
                let query = arguments["query"].as_str().unwrap_or("");
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let offset = arguments["offset"].as_u64().map(|l| l as usize);
                let detail_level = arguments["detail"].as_str().and_then(|d| match d {
                    "low" => Some(DetailLevel::Low),
                    "medium" => Some(DetailLevel::Medium),
                    "high" => Some(DetailLevel::High),
                    _ => None,
                });
                let opts = SearchOpts {
                    limit,
                    offset,
                    detail_level,
                    language: arguments["lang"].as_str().map(ToString::to_string),
                    symbol_kind: arguments["symbol_kind"].as_str().map(ToString::to_string),
                    near_symbol: arguments["near_symbol"].as_str().map(ToString::to_string),
                    walk_depth: arguments["walk_depth"]
                        .as_u64()
                        .map(|d| (d as usize).min(2)),
                    ..Default::default()
                };
                let expand = arguments["expand"].as_bool().unwrap_or(true);
                let with_meta = ops.query_with_meta(query, opts, expand)?;
                if arguments["include_meta"].as_bool().unwrap_or(false) {
                    Ok(serde_json::to_value(with_meta)?)
                } else {
                    Ok(serde_json::to_value(with_meta.results)?)
                }
            }

            "search" => {
                let query = arguments["query"].as_str().unwrap_or("");
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let offset = arguments["offset"].as_u64().map(|l| l as usize);
                let opts = SearchOpts {
                    limit,
                    offset,
                    language: arguments["lang"].as_str().map(ToString::to_string),
                    symbol_kind: arguments["symbol_kind"].as_str().map(ToString::to_string),
                    near_symbol: arguments["near_symbol"].as_str().map(ToString::to_string),
                    walk_depth: arguments["walk_depth"]
                        .as_u64()
                        .map(|d| (d as usize).min(2)),
                    ..Default::default()
                };
                // Use Operations::query() for full hybrid search pipeline
                // (keyword + vector + fallback + RRF fusion + boosts + dedup)
                // instead of raw engine.search_keyword() which only does FTS5.
                let results = ops.query(query, opts)?;
                Ok(serde_json::to_value(results)?)
            }

            "get_page" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                crate::security::validate_page_slug(slug)?;
                // P1-10: Enrich get_page with tags (mirrors TS: { ...page, tags })
                match ops.get_page(slug)? {
                    Some(page) => {
                        let tags = ops.engine.get_tags(slug)?;
                        let mut page_value = serde_json::to_value(page)?;
                        page_value["tags"] = serde_json::to_value(tags)?;
                        Ok(page_value)
                    }
                    None => Err(crate::error::GBrainError::PageNotFound(slug.to_string())),
                }
            }

            "put_page" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers (security boundary)
                crate::security::validate_page_slug(slug)?;
                let content = arguments["content"].as_str().unwrap_or("");
                // Content size limit for remote callers (DoS prevention)
                if content.len() > 1_000_000 {
                    return Err(GBrainError::InvalidInput(
                        "content exceeds 1MB limit for remote callers".into(),
                    ));
                }
                // Parse frontmatter from content to extract title and page_type
                let parsed = crate::markdown::parse_markdown(content);
                // Derive title from slug as fallback
                let slug_title = slug
                    .split('/')
                    .next_back()
                    .unwrap_or(slug)
                    .replace('-', " ")
                    .split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let title = parsed
                    .frontmatter
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&slug_title)
                    .to_string();
                let page_type = parsed
                    .frontmatter
                    .get("page_type")
                    .and_then(|v| v.as_str())
                    .map(PageType::from_str_lossy)
                    .or_else(|| Some(crate::markdown::infer_type(slug)));
                let page = ops.put_page(slug, &title, content, page_type, None)?;
                Ok(serde_json::to_value(page)?)
            }

            "delete_page" => {
                // R3-06: Require explicit confirm=true for MCP delete operations
                // to prevent accidental data loss from LLM agent calls
                let confirm = arguments["confirm"].as_bool().unwrap_or(false);
                if !confirm {
                    return Err(crate::error::GBrainError::InvalidInput(
                        "delete_page requires confirm=true to prevent accidental deletion"
                            .to_string(),
                    ));
                }
                let slug = arguments["slug"].as_str().unwrap_or("");
                crate::security::validate_page_slug(slug)?;
                ops.delete_page(slug)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "list_pages" => {
                let page_type = arguments["type"].as_str().map(PageType::from_str_lossy);
                let tag = arguments["tag"].as_str().map(|s| s.to_string());
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let offset = arguments["offset"].as_u64().map(|l| l as usize);
                let filters = PageFilters {
                    page_type,
                    tag,
                    limit,
                    offset,
                    ..Default::default()
                };
                let pages = ops.list_pages(filters)?;
                Ok(serde_json::to_value(pages)?)
            }

            "add_tag" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                let tag = arguments["tag"].as_str().unwrap_or("");
                crate::security::validate_page_slug(slug)?;
                if tag.is_empty() || tag.len() > 200 {
                    return Err(GBrainError::InvalidInput(
                        "tag must be 1-200 characters".into(),
                    ));
                }
                if tag.contains('\0') || tag.contains('\n') || tag.contains('\r') {
                    return Err(GBrainError::InvalidInput(
                        "tag contains invalid characters".into(),
                    ));
                }
                ops.engine.add_tag(slug, tag)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "remove_tag" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                let tag = arguments["tag"].as_str().unwrap_or("");
                crate::security::validate_page_slug(slug)?;
                if tag.is_empty() || tag.len() > 200 {
                    return Err(GBrainError::InvalidInput(
                        "tag must be 1-200 characters".into(),
                    ));
                }
                if tag.contains('\0') || tag.contains('\n') || tag.contains('\r') {
                    return Err(GBrainError::InvalidInput(
                        "tag contains invalid characters".into(),
                    ));
                }
                ops.engine.remove_tag(slug, tag)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "get_tags" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let tags = ops.engine.get_tags(slug)?;
                Ok(serde_json::to_value(tags)?)
            }

            "add_link" => {
                let from = arguments["from"].as_str().unwrap_or("");
                let to = arguments["to"].as_str().unwrap_or("");
                // Validate slugs for remote callers
                crate::security::validate_page_slug(from)?;
                crate::security::validate_page_slug(to)?;
                // Verify both slugs exist to prevent dead links
                if ops.engine.get_page(from)?.is_none() {
                    return Err(GBrainError::PageNotFound(format!(
                        "Source slug not found: {}",
                        from
                    )));
                }
                if ops.engine.get_page(to)?.is_none() {
                    return Err(GBrainError::PageNotFound(format!(
                        "Target slug not found: {}",
                        to
                    )));
                }
                let link_type = arguments["link_type"].as_str();
                let context = arguments["context"].as_str();
                // Validate link_type and context length for remote callers
                if let Some(lt) = link_type {
                    if lt.len() > 200 || lt.contains('\0') || lt.contains('\n') || lt.contains('\r')
                    {
                        return Err(GBrainError::InvalidInput(
                            "link_type must be ≤200 chars with no control characters".into(),
                        ));
                    }
                }
                if let Some(ctx) = context {
                    if ctx.len() > 2000 || ctx.contains('\0') {
                        return Err(GBrainError::InvalidInput(
                            "context must be ≤2000 chars with no null bytes".into(),
                        ));
                    }
                }
                ops.engine
                    .add_link(from, to, context, link_type, Some("manual"), None, None)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "remove_link" => {
                let from = arguments["from"].as_str().unwrap_or("");
                let to = arguments["to"].as_str().unwrap_or("");
                // Validate slugs for remote callers
                crate::security::validate_page_slug(from)?;
                crate::security::validate_page_slug(to)?;
                let link_type = arguments["link_type"].as_str();
                ops.engine.remove_link(from, to, link_type, None, None)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "get_links" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let links = ops.engine.get_links(slug)?;
                Ok(serde_json::to_value(links)?)
            }

            "get_backlinks" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let links = ops.get_backlinks(slug)?;
                Ok(serde_json::to_value(links)?)
            }

            "traverse_graph" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let depth = arguments["depth"].as_u64().unwrap_or(5) as usize;
                let depth = depth.min(10); // Cap at 10
                let nodes = ops.traverse_graph(slug, depth)?;
                Ok(serde_json::to_value(nodes)?)
            }

            "add_timeline_entry" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let date = arguments["date"].as_str().unwrap_or("");
                let summary = arguments["summary"].as_str().unwrap_or("");
                // Validate date format (YYYY-MM-DD) for LLM callers using proper date parsing
                if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                    return Err(GBrainError::InvalidInput(
                        "date must be valid YYYY-MM-DD format".to_string(),
                    ));
                }
                // Cap summary length to prevent abuse from LLM callers
                let summary_capped = if summary.len() > 500 {
                    summary.chars().take(500).collect()
                } else {
                    summary.to_string()
                };
                let entry = TimelineInput {
                    date: date.to_string(),
                    source: None,
                    summary: summary_capped,
                    detail: None,
                };
                ops.engine.add_timeline_entry(slug, entry, false)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "get_timeline" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let entries = ops.engine.get_timeline(
                    slug,
                    Some(TimelineQueryOpts {
                        limit,
                        after: None,
                        before: None,
                    }),
                )?;
                Ok(serde_json::to_value(entries)?)
            }

            "resolve_slugs" => {
                let partial = arguments["partial"].as_str().unwrap_or("");
                let slugs = ops.resolve_slugs(partial)?;
                Ok(serde_json::to_value(slugs)?)
            }

            "find_by_title_fuzzy" => {
                let query = arguments["query"].as_str().unwrap_or("");
                let dir_prefix = arguments["dir_prefix"].as_str();
                let min_similarity = arguments["min_similarity"].as_f64();
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let matches = ops.find_by_title_fuzzy(query, dir_prefix, min_similarity, limit)?;
                Ok(serde_json::to_value(matches)?)
            }

            "get_stats" => {
                let stats = ops.get_stats()?;
                Ok(serde_json::to_value(stats)?)
            }

            "get_health" => {
                let health = ops.get_health()?;
                Ok(serde_json::to_value(health)?)
            }

            "find_orphans" => {
                let health = ops.get_health()?;
                Ok(serde_json::json!({"orphan_count": health.orphan_pages}))
            }

            "file_upload" => {
                let path = arguments["path"].as_str().unwrap_or("");
                // R3-05: Reject empty path — prevents silent default to empty path
                if path.is_empty() {
                    return Err(crate::error::GBrainError::InvalidInput(
                        "path is required for file_upload".to_string(),
                    ));
                }
                let slug = arguments["page_slug"].as_str().unwrap_or("unsorted");
                crate::security::validate_page_slug(slug)?;
                // Note: path containment validation is handled by ops.file_upload()
                // internally via validate_upload_path + validate_contained with the
                // correct OpContext.working_dir. We only validate the empty path
                // and slug here at the MCP boundary.
                let opts = FileUploadOptions {
                    slug: slug.to_string(),
                    overwrite: false,
                    max_size_bytes: None,
                };
                let record = ops.file_upload(std::path::Path::new(path), slug, opts)?;
                Ok(serde_json::to_value(record)?)
            }

            "file_list" => {
                let slug = arguments["slug"].as_str();
                // Validate slug for remote callers if provided
                if let Some(s) = slug {
                    crate::security::validate_page_slug(s)?;
                }
                let files = ops.file_list(slug, None)?;
                Ok(serde_json::to_value(files)?)
            }

            "get_versions" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let versions = ops.engine.get_versions(slug, limit)?;
                Ok(serde_json::to_value(versions)?)
            }

            "revert_version" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers (write operation)
                crate::security::validate_page_slug(slug)?;
                let version_id = arguments["version_id"].as_i64().ok_or_else(|| {
                    GBrainError::InvalidInput("version_id must be a valid integer".into())
                })?;
                ops.engine.revert_to_version(slug, version_id)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "put_raw_data" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers (write operation)
                crate::security::validate_page_slug(slug)?;
                let source = arguments["source"].as_str().unwrap_or("");
                let data = arguments
                    .get("data")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                // Size limit for remote callers: reject JSON payloads > 1MB
                let json_size = data.to_string().len();
                if json_size > 1_000_000 {
                    return Err(GBrainError::InvalidInput(format!(
                        "Raw data payload too large: {} bytes (max 1MB)",
                        json_size
                    )));
                }
                ops.engine.put_raw_data(slug, source, data)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "get_raw_data" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let source = arguments["source"].as_str();
                let data = ops.engine.get_raw_data(slug, source.unwrap_or(""))?;
                Ok(serde_json::to_value(data)?)
            }

            "get_chunks" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                // Validate slug for remote callers
                crate::security::validate_page_slug(slug)?;
                let chunks = ops.engine.get_chunks(slug)?;
                Ok(serde_json::to_value(chunks)?)
            }

            "log_ingest" => {
                let source_type = arguments["source_type"].as_str().unwrap_or("");
                let source_ref = arguments["source_ref"].as_str().unwrap_or("");
                let summary = arguments["summary"].as_str().unwrap_or("").to_string();
                let pages_updated: Vec<String> = arguments
                    .get("pages_updated")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let status = if pages_updated.is_empty() {
                    "no_changes"
                } else {
                    "success"
                }
                .to_string();
                let entry = IngestLogInput {
                    source_type: source_type.to_string(),
                    source_ref: source_ref.to_string(),
                    summary,
                    pages_updated,
                    status,
                    error: None,
                };
                ops.engine.log_ingest(entry)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "get_ingest_log" => {
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let entries = ops.engine.get_ingest_log(limit)?;
                Ok(serde_json::to_value(entries)?)
            }

            "file_url" => {
                let storage_path = arguments["storage_path"].as_str().unwrap_or("");
                // Validate storage_path for remote callers: reject traversal patterns
                if storage_path.contains("..")
                    || storage_path.contains('\0')
                    || storage_path.contains('\\')
                {
                    return Err(crate::error::GBrainError::Security(
                        "invalid storage path".into(),
                    ));
                }
                // Path containment: verify resolved path stays within file storage directory
                let base_dir = Config::base_dir();
                let files_dir = base_dir.join("files");
                let resolved = files_dir.join(storage_path);
                crate::security::validate_contained(&resolved, &files_dir, true)?;
                let url = ops.file_url_by_path(storage_path)?;
                Ok(serde_json::json!({"url": url, "storage_path": storage_path}))
            }

            "search_code_chunks" => {
                let query = arguments["query"].as_str().unwrap_or("");
                let limit = arguments["limit"].as_u64().map(|l| l as usize);
                let results = ops.search_keyword_chunks(
                    query,
                    SearchOpts {
                        limit,
                        page_type: Some(PageType::Code),
                        language: arguments["lang"].as_str().map(ToString::to_string),
                        symbol_kind: arguments["symbol_kind"].as_str().map(ToString::to_string),
                        ..Default::default()
                    },
                )?;
                Ok(serde_json::to_value(results)?)
            }

            "code_def" => {
                let symbol = arguments["symbol"].as_str().unwrap_or("");
                let limit = arguments["limit"]
                    .as_u64()
                    .map(|l| l as usize)
                    .unwrap_or(20);
                let language = arguments["lang"].as_str();
                let chunks = ops.find_code_definitions(symbol, language, limit)?;
                Ok(serde_json::to_value(chunks)?)
            }

            "code_refs" => {
                let symbol = arguments["symbol"].as_str().unwrap_or("");
                let limit = arguments["limit"]
                    .as_u64()
                    .map(|l| l as usize)
                    .unwrap_or(20);
                let language = arguments["lang"].as_str();
                let refs = ops.find_code_references(symbol, language, limit)?;
                Ok(serde_json::to_value(refs)?)
            }

            "get_callers" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                let symbol = arguments["symbol"].as_str().unwrap_or("");
                crate::security::validate_page_slug(slug)?;
                Ok(serde_json::to_value(ops.get_callers_of(slug, symbol)?)?)
            }

            "get_callees" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                let symbol = arguments["symbol"].as_str().unwrap_or("");
                crate::security::validate_page_slug(slug)?;
                Ok(serde_json::to_value(ops.get_callees_of(slug, symbol)?)?)
            }

            "get_code_edges_by_chunk" => {
                let chunk_id = arguments["chunk_id"].as_i64().unwrap_or(0);
                Ok(serde_json::to_value(ops.get_edges_by_chunk(chunk_id)?)?)
            }

            "reindex_code_page" => {
                let slug = arguments["slug"].as_str().unwrap_or("");
                crate::security::validate_page_slug(slug)?;
                let chunks = ops.reindex_code_page(slug)?;
                Ok(serde_json::json!({"ok": true, "chunks": chunks}))
            }

            // P1-9: sync_brain MCP tool (mirrors TS sync_brain operation)
            "sync_brain" => {
                let repo_path = arguments["repo_path"].as_str().unwrap_or("");
                let force_full = arguments["force_full"].as_bool().unwrap_or(false);
                let path = std::path::Path::new(repo_path);
                // Use canonical security validation instead of inline checks
                let working_dir =
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                crate::security::validate_upload_path(path, true, &working_dir)?;
                crate::security::validate_contained(path, &working_dir, true)?;
                let result = crate::sync::sync_brain(&self.engine, path, force_full, true)?;
                Ok(serde_json::to_value(result)?)
            }

            // --- KB subsystem tools ---
            // 【KB 总入口守卫】⚠️ 此 catch-all 必须在所有具体 kb_* handler 之前！
            // 当 kb_enabled=false 时，拦截所有 kb_* 前缀的工具调用。
            _ if tool_name.starts_with("kb_") && !self.config.kb_enabled => {
                Err(GBrainError::InvalidInput(
                    "KB subsystem is disabled (kb_enabled=false)".to_string(),
                ))
            }

            "kb_list_libraries" => {
                let kb = self.engine.kb_engine()?;
                let libraries = kb.list_libraries_with_stats()?;
                Ok(serde_json::to_value(libraries)?)
            }

            "kb_create_library" => {
                let kb = self.engine.kb_engine()?;
                let input = crate::kb::types::CreateLibraryInput {
                    name: arguments["name"].as_str().unwrap_or("").to_string(),
                    semantic_segmentation_enabled: arguments["semantic_segmentation_enabled"]
                        .as_bool(),
                    raptor_enabled: arguments["raptor_enabled"].as_bool(),
                    raptor_llm_base_url: arguments["raptor_llm_base_url"]
                        .as_str()
                        .map(|s| s.to_string()),
                    raptor_llm_secret_ref: arguments["raptor_llm_secret_ref"]
                        .as_str()
                        .map(|s| s.to_string()),
                    raptor_llm_model: arguments["raptor_llm_model"]
                        .as_str()
                        .map(|s| s.to_string()),
                    chunk_size: arguments["chunk_size"].as_u64().map(|v| v as usize),
                    chunk_overlap: arguments["chunk_overlap"].as_u64().map(|v| v as usize),
                    batch_max_documents: arguments["batch_max_documents"]
                        .as_u64()
                        .map(|v| v as usize),
                    batch_max_chunks: arguments["batch_max_chunks"].as_u64().map(|v| v as usize),
                    // P0-016: 库级治理和模型配置
                    embedding_provider: arguments["embedding_provider"]
                        .as_str()
                        .map(|s| s.to_string()),
                    embedding_model: arguments["embedding_model"].as_str().map(|s| s.to_string()),
                    embedding_dimensions: arguments["embedding_dimensions"]
                        .as_i64()
                        .map(|v| v as i32),
                    search_profile: arguments["search_profile"].as_str().map(|s| s.to_string()),
                    rerank_enabled: arguments["rerank_enabled"].as_bool(),
                    rerank_provider: arguments["rerank_provider"].as_str().map(|s| s.to_string()),
                    summary_enabled: arguments["summary_enabled"].as_bool(),
                    external_embedding_allowed: arguments["external_embedding_allowed"].as_bool(),
                    external_rerank_allowed: arguments["external_rerank_allowed"].as_bool(),
                    external_summary_allowed: arguments["external_summary_allowed"].as_bool(),
                    external_ocr_allowed: arguments["external_ocr_allowed"].as_bool(),
                    redaction_enabled: arguments["redaction_enabled"].as_bool(),
                };
                let id = kb.create_library(&input)?;
                Ok(serde_json::json!({"id": id}))
            }

            "kb_update_library" => {
                let kb = self.engine.kb_engine()?;
                let library_id = arguments["library_id"].as_i64().unwrap_or(0);
                let input = crate::kb::types::UpdateLibraryInput {
                    name: arguments["name"].as_str().map(|s| s.to_string()),
                    semantic_segmentation_enabled: arguments["semantic_segmentation_enabled"]
                        .as_bool(),
                    raptor_enabled: arguments["raptor_enabled"].as_bool(),
                    raptor_llm_base_url: arguments["raptor_llm_base_url"]
                        .as_str()
                        .map(|s| s.to_string()),
                    raptor_llm_secret_ref: arguments["raptor_llm_secret_ref"]
                        .as_str()
                        .map(|s| s.to_string()),
                    raptor_llm_model: arguments["raptor_llm_model"]
                        .as_str()
                        .map(|s| s.to_string()),
                    chunk_size: arguments["chunk_size"].as_u64().map(|v| v as usize),
                    chunk_overlap: arguments["chunk_overlap"].as_u64().map(|v| v as usize),
                    // P0-016: 库级治理和模型配置
                    embedding_provider: arguments["embedding_provider"]
                        .as_str()
                        .map(|s| s.to_string()),
                    embedding_model: arguments["embedding_model"].as_str().map(|s| s.to_string()),
                    embedding_dimensions: arguments["embedding_dimensions"]
                        .as_i64()
                        .map(|v| v as i32),
                    search_profile: arguments["search_profile"].as_str().map(|s| s.to_string()),
                    rerank_enabled: arguments["rerank_enabled"].as_bool(),
                    rerank_provider: arguments["rerank_provider"].as_str().map(|s| s.to_string()),
                    summary_enabled: arguments["summary_enabled"].as_bool(),
                    external_embedding_allowed: arguments["external_embedding_allowed"].as_bool(),
                    external_rerank_allowed: arguments["external_rerank_allowed"].as_bool(),
                    external_summary_allowed: arguments["external_summary_allowed"].as_bool(),
                    external_ocr_allowed: arguments["external_ocr_allowed"].as_bool(),
                    redaction_enabled: arguments["redaction_enabled"].as_bool(),
                };
                kb.update_library(library_id, &input)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "kb_delete_library" => {
                let confirm = arguments["confirm"].as_bool().unwrap_or(false);
                if !confirm {
                    return Err(GBrainError::InvalidInput(
                        "kb_delete_library requires confirm=true to prevent accidental deletion"
                            .to_string(),
                    ));
                }
                let kb = self.engine.kb_engine()?;
                let library_id = arguments["library_id"].as_i64().unwrap_or(0);
                kb.delete_library(library_id)?;
                Ok(serde_json::json!({"ok": true}))
            }

            "kb_upload_document" => {
                let library_id = arguments["library_id"].as_i64().unwrap_or(0);
                let file_path = arguments["file_path"].as_str().unwrap_or("");
                let folder_id = arguments["folder_id"].as_i64();
                let working_dir = ctx.working_dir.clone();
                let max_file_bytes = self.config.kb_max_file_size_mb * 1024 * 1024;

                // Validate source path (remote callers confined to working_dir)
                let validated_path = crate::kb::security::validate_upload_source(
                    std::path::Path::new(file_path),
                    true, // remote
                    &working_dir,
                    max_file_bytes,
                    &self.config.kb_allowed_extensions,
                )?;

                // 扩展名已在 validate_upload_source 中通过 config.kb_allowed_extensions 验证
                let ext = validated_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let file_data = std::fs::read(&validated_path)?;
                let mime = crate::kb::security::detect_and_validate_mime(&file_data, &ext)?;

                // SHA-256 dedup check
                let content_hash = {
                    use sha2::{Digest, Sha256};
                    hex::encode(Sha256::digest(&file_data))
                };

                let kb = self.engine.kb_engine()?;

                // Check for duplicate
                if let Some(existing) = kb.find_document_by_hash(library_id, &content_hash)? {
                    return Ok(serde_json::json!({
                        "id": existing.id,
                        "status": "duplicate",
                        "message": "Document with same content already exists in this library"
                    }));
                }

                // Store file
                let base_dir = self
                    .config
                    .kb_storage_dir
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| crate::config::Config::base_dir().join("kb_files"));
                let storage_path = crate::kb::security::store_kb_file(
                    library_id,
                    &content_hash,
                    &ext,
                    &file_data,
                    &base_dir,
                )?;

                // Tokenize name for FTS5
                let original_name = validated_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let name_tokens = crate::nlp::chinese::tokenize_name(&original_name);

                // Create processing_run_id and enqueue job
                let processing_run_id = crate::kb::jobs::new_run_id();

                // Create document record
                let doc = crate::kb::types::Document {
                    library_id,
                    folder_id,
                    original_name,
                    name_tokens,
                    file_size: file_data.len() as i64,
                    content_hash,
                    extension: ext,
                    mime_type: mime,
                    source_type: "upload".to_string(),
                    storage_path,
                    original_path: validated_path.to_string_lossy().to_string(),
                    job_id: String::new(),
                    processing_run_id,
                    ..Default::default()
                };

                let doc_id = kb.create_document(&doc)?;

                // Update payload with real document_id and enqueue
                let payload = crate::kb::jobs::KbProcessPayload {
                    kind: "kb_process".to_string(),
                    document_id: doc_id,
                    library_id,
                    processing_run_id: doc.processing_run_id.clone(),
                    storage_path: doc.storage_path.clone(),
                    extension: doc.extension.clone(),
                };
                let conn = self.engine.connection()?;
                let job_db_id = crate::kb::jobs::enqueue_kb_process_job(conn, &payload)?;

                // 写回 job_id 到文档记录
                kb.update_document_job_id(doc_id, &job_db_id.to_string())?;

                Ok(serde_json::json!({
                    "id": doc_id,
                    "job_id": job_db_id,
                    "status": "pending",
                }))
            }

            "kb_get_document_status" => {
                let kb = self.engine.kb_engine()?;
                let document_id = arguments["document_id"].as_i64().unwrap_or(0);
                let doc = kb.get_document(document_id)?;
                Ok(serde_json::json!({
                    "id": doc.id,
                    "parsing_status": doc.parsing_status,
                    "parsing_progress": doc.parsing_progress,
                    "parsing_error": doc.parsing_error,
                    "embedding_status": doc.embedding_status,
                    "embedding_progress": doc.embedding_progress,
                    "embedding_error": doc.embedding_error,
                    "word_total": doc.word_total,
                    "split_total": doc.split_total,
                }))
            }

            "kb_retry_document" => {
                let kb = self.engine.kb_engine()?;
                let document_id = arguments["document_id"].as_i64().unwrap_or(0);
                let doc = kb.get_document(document_id)?;

                // Only retry failed documents
                if doc.parsing_status != crate::kb::types::STATUS_FAILED
                    && doc.embedding_status != crate::kb::types::STATUS_FAILED
                {
                    return Err(GBrainError::InvalidInput(
                        "Document is not in a failed state; cannot retry".to_string(),
                    ));
                }

                // Create new processing run
                let processing_run_id = crate::kb::jobs::new_run_id();
                let payload = crate::kb::jobs::KbProcessPayload {
                    kind: "kb_process".to_string(),
                    document_id,
                    library_id: doc.library_id,
                    processing_run_id: processing_run_id.clone(),
                    storage_path: doc.storage_path.clone(),
                    extension: doc.extension.clone(),
                };

                // Reset status and enqueue
                kb.update_document_status(
                    document_id,
                    Some(crate::kb::types::STATUS_PENDING),
                    Some(0),
                    None,
                    Some(crate::kb::types::STATUS_PENDING),
                    Some(0),
                    None,
                )?;

                // 写回新的 processing_run_id
                kb.update_document_run_id(document_id, &processing_run_id)?;

                let conn = self.engine.connection()?;
                let job_db_id = crate::kb::jobs::enqueue_kb_process_job(conn, &payload)?;

                // 写回 job_id
                kb.update_document_job_id(document_id, &job_db_id.to_string())?;

                Ok(serde_json::json!({"id": document_id, "job_id": job_db_id, "status": "pending"}))
            }

            "kb_cancel_document_job" => {
                let kb = self.engine.kb_engine()?;
                let document_id = arguments["document_id"].as_i64().unwrap_or(0);
                let doc = kb.get_document(document_id)?;

                if !doc.job_id.is_empty() {
                    let conn = self.engine.connection()?;
                    if let Ok(job_db_id) = doc.job_id.parse::<i64>() {
                        crate::kb::jobs::cancel_kb_job(conn, job_db_id)?;
                    }
                }

                kb.update_document_status(
                    document_id,
                    Some(crate::kb::types::STATUS_FAILED),
                    None,
                    Some("cancelled"),
                    None,
                    None,
                    None,
                )?;

                Ok(serde_json::json!({"ok": true}))
            }

            "kb_delete_document" => {
                let confirm = arguments["confirm"].as_bool().unwrap_or(false);
                if !confirm {
                    return Err(GBrainError::InvalidInput(
                        "kb_delete_document requires confirm=true to prevent accidental deletion"
                            .to_string(),
                    ));
                }
                let kb = self.engine.kb_engine()?;
                let document_id = arguments["document_id"].as_i64().unwrap_or(0);
                // 软删除：设置 deleted_at，搜索默认过滤
                kb.soft_delete_document(document_id)?;
                Ok(serde_json::json!({"ok": true, "deleted": true}))
            }

            "kb_purge_document" => {
                let confirm = arguments["confirm"].as_bool().unwrap_or(false);
                if !confirm {
                    return Err(GBrainError::InvalidInput(
                        "kb_purge_document requires confirm=true — this permanently destroys data"
                            .to_string(),
                    ));
                }
                let kb = self.engine.kb_engine()?;
                let document_id = arguments["document_id"].as_i64().unwrap_or(0);
                kb.purge_document(document_id)?;
                Ok(serde_json::json!({"ok": true, "purged": true}))
            }

            "kb_list_documents" => {
                let kb = self.engine.kb_engine()?;
                let library_id = arguments["library_id"].as_i64().unwrap_or(0);
                let folder_id = arguments["folder_id"].as_i64();
                let limit = arguments["limit"]
                    .as_u64()
                    .map(|v| v as usize)
                    .unwrap_or(50);
                let offset = arguments["offset"]
                    .as_u64()
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let docs = kb.list_documents(library_id, folder_id, limit, offset)?;
                Ok(serde_json::to_value(docs)?)
            }

            "kb_search" => {
                let query = arguments["query"].as_str().unwrap_or("");
                let library_ids: Vec<i64> = arguments
                    .get("library_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
                    .unwrap_or_default();
                let level = arguments["level"].as_i64().map(|v| v as i32);
                let top_k = arguments["top_k"]
                    .as_u64()
                    .map(|v| v as usize)
                    .unwrap_or(10)
                    .min(50);

                let debug = arguments["debug"].as_bool().unwrap_or(false);
                let profile = arguments["profile"].as_str().map(|s| s.to_string());
                let folder_id = arguments["folder_id"].as_i64();
                let input = crate::kb::types::KbSearchInput {
                    query: query.to_string(),
                    library_ids,
                    level,
                    top_k,
                    profile,
                    debug,
                    folder_id,
                    include_context: arguments["include_context"].as_bool().unwrap_or(false),
                    context_before: arguments["context_before"]
                        .as_u64()
                        .map(|v| v as usize)
                        .unwrap_or(200),
                    context_after: arguments["context_after"]
                        .as_u64()
                        .map(|v| v as usize)
                        .unwrap_or(200),
                    include_highlights: arguments["include_highlights"].as_bool().unwrap_or(false),
                    group_by_document: arguments["group_by_document"].as_bool().unwrap_or(false),
                    rerank_api_key: self.config.openai_api_key.clone(),
                    rerank_base_url: self.config.openai_base_url.clone(),
                    ..Default::default()
                };

                // FIX10-10: 多 library 搜索时按 (provider, model, dimensions, external_embedding_allowed) 分组，
                // 每组生成对应 query vector 并分别检索，再做最终融合。
                // 如果暂时不支持多模型混搜，检测到不一致时禁用 vector retriever 并在 debug 中返回原因。
                let conn_for_embed = self.engine.connection()?;
                let kb = crate::kb::engine::KbEngine::new(&conn_for_embed);

                // 收集每个目标库的 embedding 配置
                let mut lib_configs: Vec<(i64, String, String, i32, bool)> = Vec::new(); // (lib_id, provider, model, dims, ext_allowed)
                if input.library_ids.is_empty() {
                    // 未指定 library_ids，取所有库
                    if let Ok(libs) = kb.list_libraries() {
                        for lib in libs {
                            lib_configs.push((
                                lib.id,
                                lib.embedding_provider.clone(),
                                lib.embedding_model.clone(),
                                lib.embedding_dimensions.unwrap_or(0),
                                lib.external_embedding_allowed,
                            ));
                        }
                    }
                } else {
                    for &lib_id in &input.library_ids {
                        if let Ok(lib) = kb.get_library(lib_id) {
                            lib_configs.push((
                                lib.id,
                                lib.embedding_provider.clone(),
                                lib.embedding_model.clone(),
                                lib.embedding_dimensions.unwrap_or(0),
                                lib.external_embedding_allowed,
                            ));
                        }
                    }
                }

                // 检查是否所有库的 embedding 配置一致
                let all_ext_allowed = lib_configs.iter().all(|(_, _, _, _, ext)| *ext);
                let unique_configs: Vec<(String, String, i32)> = lib_configs
                    .iter()
                    .map(|(_, p, m, d, _)| (p.clone(), m.clone(), *d))
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                let mut debug_reason: Option<String> = None;

                let query_vector: Option<Vec<f32>> = if all_ext_allowed {
                    if unique_configs.len() <= 1 {
                        // 配置一致（或仅一个库），正常生成 query vector
                        let (embed_model, embed_dims) = if let Some(eidx_id) =
                            input.embedding_index_id
                        {
                            conn_for_embed
                                .query_row(
                                    "SELECT model, dimensions FROM kb_embedding_indexes WHERE id = ?1",
                                    rusqlite::params![eidx_id],
                                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
                                )
                                .ok()
                                .unwrap_or_else(|| {
                                    lib_configs.first()
                                        .map(|(_, _, m, d, _)| (m.clone(), *d))
                                        .unwrap_or_else(|| (self.config.embedding_model.clone(), self.config.embedding_dimensions as i32))
                                })
                        } else {
                            lib_configs
                                .first()
                                .map(|(_, _, m, d, _)| (m.clone(), *d))
                                .unwrap_or_else(|| {
                                    (
                                        self.config.embedding_model.clone(),
                                        self.config.embedding_dimensions as i32,
                                    )
                                })
                        };

                        let embed_dims = arguments["embedding_dimensions"]
                            .as_i64()
                            .map(|v| v as i32)
                            .unwrap_or(embed_dims);

                        if let Some(api_key) = self.config.openai_api_key.as_deref() {
                            let embedder = crate::embedding::Embedder::new(
                                api_key,
                                self.config.openai_base_url.as_deref(),
                                Some(&embed_model),
                                Some(embed_dims as usize),
                            );
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .ok();
                            match rt {
                                Some(rt) => rt
                                    .block_on(embedder.embed_batch(&[query]))
                                    .ok()
                                    .and_then(|v| v.into_iter().next()),
                                None => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        // FIX10-10: 多模型混搜暂不支持，禁用 vector retriever
                        debug_reason = Some(format!(
                            "多 library 的 embedding 配置不一致（发现 {} 种），暂不支持多模型混搜，已禁用向量检索",
                            unique_configs.len()
                        ));
                        None
                    }
                } else {
                    // 库级策略禁止外部 embedding，跳过 query vector 创建
                    None
                };

                let conn = self.engine.connection()?;
                let mut results =
                    crate::kb::search::kb_search(conn, &input, query_vector.as_deref())?;
                // FIX10-10: 当多模型混搜被禁用时，在 debug 信息中返回原因
                if let Some(reason) = debug_reason {
                    if input.debug {
                        for r in &mut results {
                            let mut signals =
                                r.debug_signals.clone().unwrap_or(serde_json::json!({}));
                            signals["vector_disabled_reason"] =
                                serde_json::Value::String(reason.clone());
                            r.debug_signals = Some(signals);
                        }
                    }
                }
                Ok(serde_json::to_value(results)?)
            }

            "kb_create_folder" => {
                let kb = self.engine.kb_engine()?;
                let input = crate::kb::types::CreateFolderInput {
                    library_id: arguments["library_id"].as_i64().unwrap_or(0),
                    parent_id: arguments["parent_id"].as_i64(),
                    name: arguments["name"].as_str().unwrap_or("").to_string(),
                };
                let id = kb.create_folder(&input)?;
                Ok(serde_json::json!({"id": id}))
            }

            // --- P5/P6: KB operations & governance tools ---
            "kb_check_index_health" => {
                let conn = self.engine.connection()?;
                let health = crate::kb::health::check_index_health(conn)?;
                Ok(serde_json::to_value(health)?)
            }
            "kb_repair_index" => {
                let conn = self.engine.connection()?;
                let repaired = crate::kb::health::repair_fts(conn)?;
                Ok(serde_json::json!({"repaired_fts_rows": repaired}))
            }
            "kb_backup" => {
                let output = arguments["output"].as_str().unwrap_or("");
                if output.is_empty() {
                    return Err(GBrainError::InvalidInput("output path required".into()));
                }
                let output_dir = std::path::Path::new(output);
                let db_path = self.config.db_path();
                crate::kb::backup::backup_database(&db_path, output_dir)?;

                // FIX9-10: 同时备份 storage 目录，确保上传文件等进入备份
                let storage_dir = self
                    .config
                    .kb_storage_dir
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| crate::config::Config::base_dir().join("kb_files"));
                if storage_dir.exists() {
                    crate::kb::backup::backup_storage(&storage_dir, output_dir)?;
                }

                // 收集真实数据填充 manifest
                let kb = self.engine.kb_engine()?;
                let library_ids: Vec<i64> = kb.list_libraries()?.iter().map(|lib| lib.id).collect();

                // 收集所有库的 embedding index 信息
                let conn = self.engine.connection()?;
                let mut embedding_indexes = Vec::new();
                for &lib_id in &library_ids {
                    if let Ok(indexes) =
                        crate::kb::embedding_index::list_embedding_indexes(conn, lib_id)
                    {
                        for idx in indexes {
                            embedding_indexes.push(crate::kb::backup::EmbeddingIndexInfo {
                                id: idx.id,
                                library_id: idx.library_id,
                                model: idx.model,
                                dimensions: idx.dimensions,
                            });
                        }
                    }
                }

                // 统计文件数量（kb_files 目录下的文件数）
                let base_dir = self
                    .config
                    .kb_storage_dir
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| crate::config::Config::base_dir().join("kb_files"));
                let file_count = count_files_in_dir(&base_dir);

                // 获取 DB 文件大小
                let db_size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

                let manifest = crate::kb::backup::create_manifest(
                    crate::schema::SCHEMA_VERSION,
                    library_ids,
                    embedding_indexes,
                    file_count,
                    db_size_bytes,
                );
                Ok(serde_json::to_value(manifest)?)
            }
            "kb_restore" => {
                let input = arguments["input"].as_str().unwrap_or("");
                if input.is_empty() {
                    return Err(GBrainError::InvalidInput("input path required".into()));
                }
                let input_dir = std::path::Path::new(input);
                let db_path = self.config.db_path();
                crate::kb::backup::restore_database(&input_dir.join("gbrain.db"), &db_path)?;

                // FIX9-10: 同时恢复 storage 目录
                let storage_dir = self
                    .config
                    .kb_storage_dir
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| crate::config::Config::base_dir().join("kb_files"));
                if input_dir.join("storage").exists() {
                    crate::kb::backup::restore_storage(input_dir, &storage_dir)?;
                }

                Ok(serde_json::json!({"ok": true}))
            }
            "kb_add_eval_query" => {
                let conn = self.engine.connection()?;
                let library_id = arguments["library_id"].as_i64().unwrap_or(0);
                let query_text = arguments["query"].as_str().unwrap_or("");
                let query_type = arguments["query_type"].as_str().unwrap_or("manual");
                let expected_ids: Vec<i64> = arguments["expected_document_ids"]
                    .as_str()
                    .map(|s| {
                        s.split(',')
                            .filter_map(|id| id.trim().parse().ok())
                            .collect()
                    })
                    .unwrap_or_default();
                let id = crate::kb::eval::add_eval_query(
                    conn,
                    library_id,
                    query_text,
                    query_type,
                    &expected_ids,
                )?;
                Ok(serde_json::json!({"id": id}))
            }
            "kb_add_search_feedback" => {
                let conn = self.engine.connection()?;
                let search_log_id = arguments["search_log_id"].as_i64();
                let document_id = arguments["document_id"].as_i64();
                let node_id = arguments["node_id"].as_i64();
                let rating = arguments["rating"].as_i64().map(|v| v as i32).unwrap_or(0);
                let comment = arguments["comment"].as_str().unwrap_or("");
                let id = crate::kb::eval::add_search_feedback(
                    conn,
                    search_log_id,
                    document_id,
                    node_id,
                    rating,
                    comment,
                )?;
                Ok(serde_json::json!({"id": id}))
            }

            _ => Err(crate::error::GBrainError::InvalidInput(format!(
                "Unknown tool: {}",
                tool_name
            ))),
        }
    }
}

/// 递归统计目录下的文件数量（不含子目录本身）
fn count_files_in_dir(dir: &std::path::Path) -> usize {
    if !dir.exists() {
        return 0;
    }
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files_in_dir(&path);
            } else {
                count += 1;
            }
        }
    }
    count
}
