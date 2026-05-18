//! MCP tool definitions — Artifact facade + 内部工具
//!
//! 默认只暴露 artifact_* facade tools（知识操作统一入口）。
//! kb_*、promotion_*、projection_*、get_provenance 等内部工具
//! 通过 config.expose_internal_tools 或 admin-tools feature 暴露。
//!
//! 设计文档 §8.2: MCP tools 收口

use crate::operations::{OperationDef, ParamDef, ParamType};
use serde_json::Value;

/// Tool definition (MCP-compatible)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl From<&OperationDef> for ToolDef {
    fn from(op: &OperationDef) -> Self {
        ToolDef {
            name: op.name.to_string(),
            description: op.description.to_string(),
            input_schema: op.to_mcp_schema(),
        }
    }
}

/// Artifact facade 工具定义 — 统一知识操作入口（设计文档 §4.2）
///
/// 默认暴露给用户，包含所有 artifact_* 命名空间的 MCP tools。
pub(crate) static ARTIFACT_FACADE_DEFS: &[OperationDef] = &[
    OperationDef {
        name: "artifact_put",
        description: "手动写入长期记忆。创建 text/manual artifact 并投影到 gbrain 页面。支持 --content 直接输入或 --file 从文件读取。",
        params: &[
            ParamDef { name: "slug", description: "目标页面 slug（如 people/alice）", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "content", description: "直接输入的文本内容（与 file 二选一）", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "file", description: "本地文本文件路径（与 content 二选一，仅支持 txt/md/csv/json/yaml 等纯文本格式，上限 1MB）", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "title", description: "页面标题（可选，默认从 slug 推断）", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "intent", description: "意图: memory(默认, 稳定brain页面+可选KB+低风险自动应用), evidence(仅KB证据), promote(影子页面+KB+建议变更)", required: false, param_type: ParamType::String, enum_values: Some(&["memory", "evidence", "promote"]), items_type: None },
            ParamDef { name: "dry_run", description: "仅返回路由计划，不实际写入", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "force", description: "强制覆盖已被人工修改的页面（默认不覆盖，返回冲突信息）", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_upload",
        description: "上传文件作为知识源。系统根据文件类型和意图自动路由：文档进KB+影子页，图片/二进制走附件，带--target生成建议变更。",
        params: &[
            ParamDef { name: "path", description: "本地文件路径", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "intent", description: "上传意图: auto(自动路由), evidence(文档证据), memory(整理进记忆), attachment(仅附件), promote(明确提升)", required: false, param_type: ParamType::String, enum_values: Some(&["auto", "evidence", "memory", "attachment", "promote"]), items_type: None },
            ParamDef { name: "target_slug", description: "目标 gbrain 页面 slug（用于生成建议变更）", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "page_slug", description: "关联页面 slug（用于附件）", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "library_id", description: "KB 库 ID（可选，默认自动选择 Inbox）", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "KB 文件夹 ID", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "promotion", description: "提升策略: none, shadow, candidate, auto-low-risk", required: false, param_type: ParamType::String, enum_values: Some(&["none", "shadow", "candidate", "auto-low-risk"]), items_type: None },
            ParamDef { name: "dry_run", description: "仅返回路由计划，不实际写入", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_query",
        description: "统一知识查询。自动混合 gbrain 长期记忆、KB 文档证据、时间线事件和图谱关系。默认隐藏内部 ID。",
        params: &[
            ParamDef { name: "query", description: "查询文本", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "mode", description: "查询模式: auto(自动), memory(优先记忆), evidence(优先证据), timeline(优先时间线), graph(图谱扩展)", required: false, param_type: ParamType::String, enum_values: Some(&["auto", "memory", "evidence", "timeline", "graph"]), items_type: None },
            ParamDef { name: "limit", description: "最大结果数", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "filter_slug", description: "过滤到指定页面 slug", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "include_sources", description: "显示来源追溯（artifact 来源和引用）", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_list",
        description: "列出知识源（source artifacts）",
        params: &[
            ParamDef { name: "limit", description: "最大结果数（默认 50）", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "offset", description: "偏移量", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_get",
        description: "获取知识源详情。支持 ID 或 UID 查询，可选展示投影和来源追溯。",
        params: &[
            ParamDef { name: "id_or_uid", description: "Artifact ID 或 UID（如 '1' 或 'art_ab12cd34ef56'）", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "include_projections", description: "包含投影详情", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "include_sources", description: "包含来源追溯", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_delete",
        description: "软删除知识源。KB 证据和影子页随生命周期 stale，稳定 gbrain 页面内容不默认删除。支持 dry_run 预览。",
        params: &[
            ParamDef { name: "id_or_uid", description: "Artifact ID 或 UID", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "dry_run", description: "预览删除影响，不实际执行", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_detach",
        description: "移除知识源与某次使用的关联（occurrence/projection 级），不删除 source artifact 本身，不影响其它页面使用。",
        params: &[
            ParamDef { name: "id_or_uid", description: "Artifact ID 或 UID", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "from", description: "目标页面 slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "dry_run", description: "预览影响，不实际执行", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_restore",
        description: "恢复已软删除的知识源及其可恢复投影",
        params: &[
            ParamDef { name: "id_or_uid", description: "Artifact ID 或 UID", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "dry_run", description: "预览恢复影响，不实际执行", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_reprocess",
        description: "重新处理知识源：重跑 KB pipeline、投影生成和 promotion 提取",
        params: &[
            ParamDef { name: "id_or_uid", description: "Artifact ID 或 UID", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "dry_run", description: "预览重新处理影响，不实际执行", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_health",
        description: "检查知识源一致性：孤立投影、stale 投影、待审核变更、来源追溯状态",
        params: &[],
    },

    OperationDef {
        name: "artifact_review_list",
        description: "列出建议变更（suggested changes）。对用户展示为来源证据驱动的变更建议，而非内部 promotion candidate。",
        params: &[
            ParamDef { name: "status", description: "过滤状态: pending, accepted, rejected, applied, rolled_back", required: false, param_type: ParamType::String, enum_values: Some(&["pending", "accepted", "rejected", "applied", "rolled_back"]), items_type: None },
            ParamDef { name: "target_slug", description: "过滤目标页面 slug", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "最大结果数", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_review_get",
        description: "获取建议变更详情，包含证据、目标记忆、风险等级和来源追溯",
        params: &[
            ParamDef { name: "change_id", description: "变更 ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_review_apply",
        description: "应用建议变更到 gbrain 长期记忆",
        params: &[
            ParamDef { name: "change_id", description: "变更 ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_review_reject",
        description: "拒绝建议变更",
        params: &[
            ParamDef { name: "change_id", description: "变更 ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "reason", description: "拒绝原因", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_review_rollback",
        description: "回滚已应用的建议变更，撤销影子页更新并标记来源追溯为 stale",
        params: &[
            ParamDef { name: "change_id", description: "变更 ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
];

/// 内部工具定义 — 默认不暴露给用户，仅通过 expose_internal_tools 或 admin-tools feature 可见
///
/// 包含: gbrain 页面操作、KB 子系统、promotion/projection 内部操作、
/// 旧版 upload_source/memory_query（已被 artifact_* facade 取代）
pub(crate) static INTERNAL_DEFS: &[OperationDef] = &[
    // --- gbrain 页面操作（内部，默认不暴露） ---
    OperationDef {
        name: "query",
        description: "Hybrid search with vector + keyword + multi-query expansion",
        params: &[
            ParamDef { name: "query", description: "Search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "offset", description: "Skip first N results (for pagination)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "expand", description: "Enable multi-query expansion (default: true)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "detail", description: "Result detail level: low (compiled truth only), medium (default, all with dedup), high (all chunks)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter code-aware retrieval by language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol_kind", description: "Filter code-aware retrieval by symbol kind", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "near_symbol", description: "Anchor two-pass code graph retrieval near this symbol", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "walk_depth", description: "Walk code graph neighbors up to this depth (0-2)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "include_meta", description: "Return {results, meta} with vector/expansion detail", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_page",
        description: "Read a page by slug (supports optional fuzzy matching)",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "fuzzy", description: "Enable fuzzy slug resolution (default: false)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "put_page",
        description: "Write/update a page (markdown with frontmatter). Chunks, embeds, reconciles tags, and (when auto_link/auto_timeline are enabled) extracts + reconciles graph links and timeline entries.",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "content", description: "Full markdown content with YAML frontmatter", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "delete_page",
        description: "Delete a page (requires confirm=true to prevent accidental deletion)",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm deletion", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "list_pages",
        description: "List pages with optional filters",
        params: &[
            ParamDef { name: "type", description: "Filter by page type", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "tag", description: "Filter by tag", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 50)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "add_tag",
        description: "Add tag to page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "tag", description: "Tag name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "remove_tag",
        description: "Remove tag from page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "tag", description: "Tag name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_tags",
        description: "List tags for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "add_link",
        description: "Create link between pages",
        params: &[
            ParamDef { name: "from", description: "Source slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "to", description: "Target slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "link_type", description: "Link type (e.g., invested_in, works_at)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "context", description: "Context for the link", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "remove_link",
        description: "Remove link between pages",
        params: &[
            ParamDef { name: "from", description: "Source slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "to", description: "Target slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "link_type", description: "Link type to remove", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_links",
        description: "List outgoing links from a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_backlinks",
        description: "List incoming links to a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "traverse_graph",
        description: "Traverse link graph from a page. With link_type/direction, returns edges (GraphPath[]) instead of nodes.",
        params: &[
            ParamDef { name: "slug", description: "Starting slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "depth", description: "Max traversal depth (default 5, capped at 10)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "link_type", description: "Filter to one link type (per-edge filter, traversal only follows matching edges)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "direction", description: "Traversal direction (default out)", required: false, param_type: ParamType::String, enum_values: Some(&["in", "out", "both"]), items_type: None },
        ],
    },
    OperationDef {
        name: "add_timeline_entry",
        description: "Add timeline entry to a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "date", description: "Date (YYYY-MM-DD)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "summary", description: "Event summary", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_timeline",
        description: "Get timeline entries for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_stats",
        description: "Brain statistics (page count, chunk count, pages by type, etc.)",
        params: &[],
    },
    OperationDef {
        name: "get_health",
        description: "Brain health dashboard (embed coverage, stale pages, orphans)",
        params: &[],
    },
    OperationDef {
        name: "get_versions",
        description: "Page version history",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "revert_version",
        description: "Revert page to a previous version",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "version_id", description: "Version ID to revert to", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "put_raw_data",
        description: "Store raw API response data for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "source", description: "Data source (e.g., crustdata, happenstance)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "data", description: "Raw data object", required: true, param_type: ParamType::Object, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_raw_data",
        description: "Retrieve raw data for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "source", description: "Filter by source", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "resolve_slugs",
        description: "Fuzzy-resolve a partial slug to matching page slugs",
        params: &[
            ParamDef { name: "partial", description: "Partial slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "find_by_title_fuzzy",
        description: "Fuzzy search pages by title using trigram similarity",
        params: &[
            ParamDef { name: "query", description: "Search query (title to match)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "dir_prefix", description: "Constrain to slug prefix (e.g., 'people', 'companies')", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "min_similarity", description: "Minimum similarity threshold 0.0-1.0 (default 0.55)", required: false, param_type: ParamType::Number, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 10)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_chunks",
        description: "Get content chunks for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "code_def",
        description: "Find code symbol definitions",
        params: &[
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter by code language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "code_refs",
        description: "Find code chunks referencing a symbol",
        params: &[
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter by code language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "search_code_chunks",
        description: "Search indexed code chunks by keyword/symbol text",
        params: &[
            ParamDef { name: "query", description: "Code search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter by code language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol_kind", description: "Filter by symbol kind", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_callers",
        description: "Get code graph callers of a symbol",
        params: &[
            ParamDef { name: "slug", description: "Code page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_callees",
        description: "Get code graph callees of a symbol",
        params: &[
            ParamDef { name: "slug", description: "Code page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_code_edges_by_chunk",
        description: "Get code graph edges attached to a chunk id",
        params: &[
            ParamDef { name: "chunk_id", description: "Chunk id", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "reindex_code_page",
        description: "Rebuild code chunks and code edges for a code page",
        params: &[
            ParamDef { name: "slug", description: "Code page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "log_ingest",
        description: "Log an ingestion event",
        params: &[
            ParamDef { name: "source_type", description: "Source type (e.g., git, import, api)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "source_ref", description: "Source reference", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "pages_updated", description: "List of updated page slugs", required: true, param_type: ParamType::Array, enum_values: None, items_type: Some(ParamType::String) },
            ParamDef { name: "summary", description: "Human-readable ingestion summary", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_ingest_log",
        description: "Get recent ingestion log entries",
        params: &[
            ParamDef { name: "limit", description: "Max entries (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "file_list",
        description: "List stored files",
        params: &[
            ParamDef { name: "slug", description: "Filter by page slug", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "file_upload",
        description: "Upload a file to storage",
        params: &[
            ParamDef { name: "path", description: "Local file path", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "page_slug", description: "Associate with page", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "file_url",
        description: "Get a URL for a stored file",
        params: &[
            ParamDef { name: "storage_path", description: "Storage path of the file", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "find_orphans",
        description: "Find pages with no inbound wikilinks. Essential for content enrichment cycles.",
        params: &[
            ParamDef { name: "include_pseudo", description: "Include auto-generated and pseudo pages (default: false)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "sync_brain",
        description: "Sync brain from a Git repository. Reads .md files, chunking and embedding new/changed pages, removing deleted ones.",
        params: &[
            ParamDef { name: "repo_path", description: "Path to Git repository to sync from", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "force_full", description: "Force full sync instead of incremental (default: false)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    // --- KB 子系统工具（内部，默认不暴露） ---
    OperationDef {
        name: "kb_list_libraries",
        description: "List all knowledge base libraries with document and chunk counts",
        params: &[],
    },
    OperationDef {
        name: "kb_create_library",
        description: "Create a new knowledge base library",
        params: &[
            ParamDef { name: "name", description: "Library name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "semantic_segmentation_enabled", description: "Enable semantic segmentation", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "raptor_enabled", description: "Enable Raptor tree summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_base_url", description: "Raptor LLM base URL override", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_secret_ref", description: "Raptor LLM API key env var name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_model", description: "Raptor LLM model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "chunk_size", description: "Chunk size in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "chunk_overlap", description: "Chunk overlap in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "batch_max_documents", description: "Max documents per batch", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "batch_max_chunks", description: "Max chunks per batch", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "embedding_provider", description: "Embedding provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_model", description: "Embedding model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_dimensions", description: "Embedding vector dimensions", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "search_profile", description: "Search profile name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "rerank_enabled", description: "Enable reranking", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "rerank_provider", description: "Rerank provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "summary_enabled", description: "Enable summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_embedding_allowed", description: "Allow external embedding calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_rerank_allowed", description: "Allow external rerank calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_summary_allowed", description: "Allow external summary calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_ocr_allowed", description: "Allow external OCR calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "redaction_enabled", description: "Enable redaction of sensitive content", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_update_library",
        description: "Update a knowledge base library configuration",
        params: &[
            ParamDef { name: "library_id", description: "Library ID to update", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "name", description: "New library name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "semantic_segmentation_enabled", description: "Enable semantic segmentation", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "raptor_enabled", description: "Enable Raptor tree summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_base_url", description: "Raptor LLM base URL override", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_secret_ref", description: "Raptor LLM API key env var name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_model", description: "Raptor LLM model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "chunk_size", description: "Chunk size in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "chunk_overlap", description: "Chunk overlap in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "embedding_provider", description: "Embedding provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_model", description: "Embedding model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_dimensions", description: "Embedding vector dimensions", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "search_profile", description: "Search profile name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "rerank_enabled", description: "Enable reranking", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "rerank_provider", description: "Rerank provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "summary_enabled", description: "Enable summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_embedding_allowed", description: "Allow external embedding calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_rerank_allowed", description: "Allow external rerank calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_summary_allowed", description: "Allow external summary calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_ocr_allowed", description: "Allow external OCR calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "redaction_enabled", description: "Enable redaction of sensitive content", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_delete_library",
        description: "Delete a knowledge base library (requires confirm=true)",
        params: &[
            ParamDef { name: "library_id", description: "Library ID to delete", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm deletion", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_upload_document",
        description: "Upload a document file to a knowledge base library for processing",
        params: &[
            ParamDef { name: "library_id", description: "Target library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "file_path", description: "Local file path to upload", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "Optional folder ID", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_get_document_status",
        description: "Get the processing status of a document",
        params: &[
            ParamDef { name: "document_id", description: "Document ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_retry_document",
        description: "Retry processing a failed document",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to retry", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_cancel_document_job",
        description: "Cancel a document processing job",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to cancel", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_delete_document",
        description: "Delete a document from a library (requires confirm=true)",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to delete", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm deletion", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_list_documents",
        description: "List documents in a knowledge base library, optionally filtered by folder",
        params: &[
            ParamDef { name: "library_id", description: "Library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "Optional folder ID to filter documents", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 50)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "offset", description: "Skip first N results (for pagination)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_search",
        description: "Search across knowledge base libraries using hybrid vector + keyword + summary + table + metadata search with RRF fusion and rerank",
        params: &[
            ParamDef { name: "query", description: "Search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "library_ids", description: "Library IDs to search (empty = all)", required: false, param_type: ParamType::Array, enum_values: None, items_type: Some(ParamType::Integer) },
            ParamDef { name: "level", description: "Raptor tree level filter", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "top_k", description: "Max results (default 10, max 50)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "profile", description: "Search profile: fast|balanced|accurate|file_lookup|table", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "debug", description: "Enable debug mode (returns planner/rerank/fallback info)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "include_context", description: "Include context before/after matched nodes", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "context_before", description: "Characters of context before match (default 200)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "context_after", description: "Characters of context after match (default 200)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "include_highlights", description: "Return highlight character ranges", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "group_by_document", description: "Group results by document", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "Filter to folder", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "embedding_dimensions", description: "Override embedding dimensions for query vector", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "embedding_index_id", description: "Specific embedding index ID to use for query vector (must belong to target library)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_create_folder",
        description: "Create a folder in a knowledge base library",
        params: &[
            ParamDef { name: "library_id", description: "Library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "name", description: "Folder name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "parent_id", description: "Parent folder ID (null = root)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_purge_document",
        description: "Permanently destroy a soft-deleted document and all its associated data",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to purge", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm permanent destruction", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_check_index_health",
        description: "Run index health check (orphan nodes/embeddings/summaries, missing FTS, split mismatches)",
        params: &[],
    },
    OperationDef {
        name: "kb_repair_index",
        description: "Repair missing FTS entries for document nodes",
        params: &[],
    },
    OperationDef {
        name: "kb_backup",
        description: "Backup KB database to output directory",
        params: &[
            ParamDef { name: "output", description: "Output directory path", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_restore",
        description: "Restore KB database from backup directory",
        params: &[
            ParamDef { name: "input", description: "Input directory path containing backup", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_add_eval_query",
        description: "Add a search evaluation query with expected document IDs",
        params: &[
            ParamDef { name: "library_id", description: "Library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "query", description: "Evaluation query text", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "query_type", description: "Query type classification", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "expected_document_ids", description: "Comma-separated expected document IDs", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_add_search_feedback",
        description: "Submit relevance feedback for a search result",
        params: &[
            ParamDef { name: "search_log_id", description: "Search log entry ID", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "document_id", description: "Document ID rated", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "node_id", description: "Node ID rated", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "rating", description: "Relevance rating 0-5", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "comment", description: "Optional feedback comment", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },

    // --- 旧版 artifact 内部工具（已被 artifact_* facade 取代） ---
    OperationDef {
        name: "upload_source",
        description: "Upload a source file (unified entry point for gbrain + KB + file storage). The system automatically creates Source Artifact, KB projection, shadow page, and file attachment based on intent.",
        params: &[
            ParamDef { name: "path", description: "Local file path to upload", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "intent", description: "Upload intent: auto, document, attachment, memory, promote", required: false, param_type: ParamType::String, enum_values: Some(&["auto", "document", "attachment", "memory", "promote"]), items_type: None },
            ParamDef { name: "library_id", description: "KB library ID (optional, uses default if not specified)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "target_slug", description: "Target gbrain page slug for promotion", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "page_slug", description: "Target page slug for file attachment", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "KB folder ID", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "promotion", description: "Promotion policy: none, shadow, candidate, auto-low-risk", required: false, param_type: ParamType::String, enum_values: Some(&["none", "shadow", "candidate", "auto-low-risk"]), items_type: None },
            ParamDef { name: "dry_run", description: "Only return route plan without committing", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "memory_query",
        description: "Unified memory query. Searches both gbrain curated knowledge and KB document evidence. The planner automatically selects the best strategy based on query intent.",
        params: &[
            ParamDef { name: "query", description: "Query text", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "strategy", description: "Query strategy: brain_first, evidence_first, provenance, timeline_first", required: false, param_type: ParamType::String, enum_values: Some(&["brain_first", "evidence_first", "provenance", "timeline_first"]), items_type: None },
            ParamDef { name: "limit", description: "Maximum results", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "filter_slug", description: "Filter by brain slug (applies to all strategies: limits brain hits, evidence hits, and timeline hits to this page)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "include_evidence", description: "Include KB evidence hits", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "include_provenance", description: "Include provenance records", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    // --- promotion/projection 内部工具（已被 artifact_review_* facade 取代） ---
    OperationDef {
        name: "promotion_list_candidates",
        description: "List promotion candidates (suggested changes extracted from KB evidence)",
        params: &[
            ParamDef { name: "status", description: "Filter by status: pending, accepted, rejected, applied, rolled_back, stale, superseded", required: false, param_type: ParamType::String, enum_values: Some(&["pending", "accepted", "rejected", "applied", "rolled_back", "stale", "superseded"]), items_type: None },
            ParamDef { name: "candidate_type", description: "Filter by type: document_summary, entity_mention, link_suggestion, timeline_event, fact_claim, page_create, page_update", required: false, param_type: ParamType::String, enum_values: Some(&["document_summary", "entity_mention", "link_suggestion", "timeline_event", "fact_claim", "page_create", "page_update"]), items_type: None },
            ParamDef { name: "target_slug", description: "Filter by target slug", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Maximum results", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "promotion_get_candidate",
        description: "Get details of a promotion candidate",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "promotion_accept_candidate",
        description: "Accept a promotion candidate",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "reviewer", description: "Reviewer name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "notes", description: "Review notes", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "promotion_reject_candidate",
        description: "Reject a promotion candidate",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "reviewer", description: "Reviewer name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "reason", description: "Rejection reason", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "promotion_apply_candidate",
        description: "Apply an approved promotion candidate to gbrain",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "promotion_rollback_candidate",
        description: "Rollback an applied promotion candidate, undoing shadow page updates and marking provenance stale",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID to rollback", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "promotion_batch_apply",
        description: "Batch apply pending promotion candidates, optionally filtered by artifact and risk level",
        params: &[
            ParamDef { name: "artifact_id", description: "Filter by artifact ID (optional)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "risk", description: "Filter by risk level: low, medium, high", required: false, param_type: ParamType::String, enum_values: Some(&["low", "medium", "high"]), items_type: None },
            ParamDef { name: "dry_run", description: "Preview candidates without applying", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "gc_orphan_projections",
        description: "Garbage collect orphaned projections and clean up stale projection records",
        params: &[
            ParamDef { name: "stale_days", description: "Delete projections orphaned/superseded for more than N days (default: 30)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "dry_run", description: "Preview what would be cleaned without making changes", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "projection_supersede",
        description: "Supersede an old projection with a new one, marking the old as superseded and setting superseded_by",
        params: &[
            ParamDef { name: "old_proj_id", description: "Old projection ID to supersede", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "new_proj_id", description: "New projection ID that replaces the old one", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "projection_history",
        description: "Query projection version chain history by projection_key. Supports optional artifact_id and projection_type filters.",
        params: &[
            ParamDef { name: "projection_key", description: "Projection key to query history for", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "artifact_id", description: "Optional artifact ID to filter by (avoids mixing projections from different artifacts)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "projection_type", description: "Optional projection type to filter by (e.g. 'kb_document')", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Maximum history records to return (default: 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_provenance",
        description: "Get provenance records for a brain page (trace where facts came from)",
        params: &[
            ParamDef { name: "brain_slug", description: "Brain page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
];

/// 构建 tool 定义列表。默认只暴露 artifact_* facade tools。
/// 当 expose_internal_tools=true 时，追加内部工具。
pub fn build_tool_defs() -> Vec<ToolDef> {
    build_tool_defs_with_internal(false)
}

/// 构建 tool 定义列表，可选包含内部工具
pub fn build_tool_defs_with_internal(expose_internal: bool) -> Vec<ToolDef> {
    let mut defs: Vec<ToolDef> = ARTIFACT_FACADE_DEFS.iter().map(|op| op.into()).collect();
    if expose_internal {
        defs.extend(INTERNAL_DEFS.iter().map(|op| op.into()));
    }
    defs
}

/// 获取操作定义（在 facade 和 internal 中查找）
pub fn get_operation_def(name: &str) -> Option<&'static OperationDef> {
    ARTIFACT_FACADE_DEFS
        .iter()
        .find(|op| op.name == name)
        .or_else(|| INTERNAL_DEFS.iter().find(|op| op.name == name))
}

/// 获取所有操作定义（facade + internal）
pub fn get_operation_defs() -> Vec<&'static OperationDef> {
    ARTIFACT_FACADE_DEFS
        .iter()
        .chain(INTERNAL_DEFS.iter())
        .collect()
}

/// 获取 artifact facade 操作定义
pub fn get_artifact_facade_defs() -> &'static [OperationDef] {
    ARTIFACT_FACADE_DEFS
}

/// 获取内部操作定义
pub fn get_internal_defs() -> &'static [OperationDef] {
    INTERNAL_DEFS
}

/// 判断工具名是否属于内部工具
///
/// 修复：用于在 tools/call 入口硬拦截，防止 expose_internal_tools=false 时
/// 仍可通过直接调用工具名绕过 tools/list 的隐藏。
pub fn is_internal_tool(name: &str) -> bool {
    INTERNAL_DEFS.iter().any(|op| op.name == name)
}
