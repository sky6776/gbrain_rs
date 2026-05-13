# gbrain-rs

[English](./README_EN.md) | 中文

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**个人知识大脑引擎** — 基于 [gbrain](https://github.com/garrytan/gbrain) 的 Rust 移植，新增 KB 子系统（异步文档处理管线 + RAPTOR 递归摘要树）、中文 NLP 全链路支持（jieba 分词 + 拼音 + FTS5 查询重构）、软删除生命周期（restore/purge-deleted）、时间衰减搜索等特性。基于 SQLite + sqlite-vec + FTS5 构建的零配置嵌入式架构，开箱即用。

> 原始 TypeScript 版本由 [Garry Tan](https://github.com/garrytan) 开发。本项目采用**Vibe coding**构建。

---

## 快速开始

```bash
# 1. 构建
cargo build --release

# 2. 初始化知识库
gbrain init

# 3. 创建页面
gbrain put people/alice --title "Alice" --content "一位工程师，擅长 Rust 和系统编程"

# 4. 搜索
gbrain query "谁是 Alice"

# 5. 启动 MCP 服务器（供 AI 智能体使用）
gbrain serve
```

无需配置数据库或外部服务——开箱即用。嵌入向量、查询扩展等 AI 功能为可选，配置 API Key 后自动启用。

---

## 特性

- **混合搜索** — BM25 关键词 + 向量余弦相似度 + 模糊三元组，通过倒数排名融合（RRF）合并，支持多查询扩展
- **知识图谱** — Wiki 链接提取、类型化链接、图遍历、反向链接对称性验证
- **KB 子系统** — 异步五阶段文档处理管线（解析→拆分→嵌入→RAPTOR→持久化），RAPTOR 递归摘要树，文档上传与处理，多格式解析器（Markdown/PDF/DOCX/XLSX/CSV/HTML/纯文本/代码），语义分块（Savitzky-Golay 平滑 + chunk_overlap 重叠）
- **中文 NLP** — jieba 分词 + 拼音 + 前缀通配符，FTS5 查询自动重构，中文标点断句与分词计数，预分词列自动同步
- **MCP 服务器** — 完整的模型上下文协议（JSON-RPC 2.0）服务器，58 个工具，用于 AI 智能体集成
- **零配置** — 嵌入式 SQLite，无需外部服务（嵌入向量可选）
- **分层丰富** — 自动实体检测与提升（提及 → 存根 → 完善）
- **版本历史** — 完整的页面版本管理，支持回滚
- **自动驾驶** — 自维护守护进程，自动嵌入过期内容并执行完整性检查
- **安全防护** — 路径遍历防护、slug 验证、远程调用输入清理、参数化查询防 SQL 注入
- **代码知识图谱** — Tree-sitter AST 代码分块 + regex 符号索引，支持符号定义、引用和调用图（Rust/TypeScript/JavaScript/Python/Go/Java/C/C++）
- **音频转录** — 支持 Groq Whisper（默认）或 OpenAI Whisper
- **写作模式** — Strict（严格校验）/ Lint（零 LLM 质量检查）/ Off（自由写入）三种写入策略
- **软删除生命周期** — 删除 → 恢复 → 永久清理，支持按时间批量清理

---

## 构建与安装

```bash
cargo build --release          # 构建
cargo install --path .         # 安装到 ~/.cargo/bin/
gbrain install                 # 安装到 ~/.gbrain/bin/
```


---

## 数据目录

初始化后，`~/.gbrain/` 目录结构如下：

```
~/.gbrain/
  brain.db           # SQLite 数据库（FTS5 + sqlite-vec）
  config.json        # 运行时配置（通过 gbrain config set 生成）
  files/             # 上传文件存储
  cache/             # 缓存目录
  logs/              # 日志文件
    gbrain.log
```

可通过 `GBRAIN_DIR` 环境变量自定义根目录。

---

## 命令行命令

### 全局选项

| 选项 | 说明 |
|------|------|
| `--db <PATH>` | 数据库路径 |
| `--json` | 以 JSON 格式输出 |
| `--dry-run` | 预览操作，不实际执行 |

### 核心

| 命令 | 说明 |
|------|------|
| `gbrain init` | 初始化新的知识库 |
| `gbrain get <slug>` | 按 slug 读取页面 |
| `gbrain put <slug> --title <TITLE> [--content <TEXT> \| --file <PATH>]` | 创建或更新页面 |
| `gbrain delete <slug> [--force]` | 软删除页面 |
| `gbrain restore <slug>` | 恢复软删除页面 |
| `gbrain purge-deleted [--older-than-hours <N>]` | 永久清理旧的软删除页面 |
| `gbrain list [--page-type <TYPE>] [--limit <N>]` | 列出页面（可筛选） |
| `gbrain query <query> [--limit <N>] [--lang <LANG>] [--symbol-kind <KIND>]` | 混合搜索（别名: `ask`），支持代码过滤和两阶段检索 |

### 搜索与图谱

| 命令 | 说明 |
|------|------|
| `gbrain resolve <partial>` | 模糊解析部分 slug |
| `gbrain graph <slug> [--depth <N>]` | 从页面遍历知识图谱 |
| `gbrain graph-query <from> [--to <slug>] [--depth <N>] [--link-type <TYPE>]` | 查询页面间的图谱关系 |
| `gbrain code search/def/refs/callers/callees/edges` | 代码 chunk、符号定义/引用和调用图查询 |

### 反向链接

| 命令 | 说明 |
|------|------|
| `gbrain backlinks list <slug>` | 列出页面的反向链接 |
| `gbrain backlinks check [slug]` | 检查缺失的反向链接 |
| `gbrain backlinks fix [slug]` | 修复缺失的反向链接 |

### 数据管理

| 命令 | 说明 |
|------|------|
| `gbrain embed [slugs...] [--batch-size <N>]` | 为 stale chunks 生成并持久化嵌入向量 |
| `gbrain import <dir> [--embed] [--auto-link]` | 导入 Markdown 和支持的代码文件；frontmatter slug 与路径不一致时跳过 |
| `gbrain export [slugs...] [--dir <DIR>] [--page-type <TYPE>]` | 导出页面为 Markdown |
| `gbrain extract [--mode links\|timeline\|all]` | 批量提取链接/时间线 |
| `gbrain lint [slug] [--fix] [--dry-run]` | 零 LLM 质量检查（6 条规则） |

### 文件存储

| 命令 | 说明 |
|------|------|
| `gbrain file upload <path> [--page <slug>]` | 上传文件 |
| `gbrain file list [slug]` | 列出已存储的文件 |
| `gbrain file sync <dir>` | 同步目录到存储 |
| `gbrain file verify` | 验证所有文件记录 |
| `gbrain file url <storage-path>` | 获取文件的本地路径/URL |

### 健康与维护

| 命令 | 说明 |
|------|------|
| `gbrain stats` | 知识库统计信息 |
| `gbrain health` | 健康仪表盘 |
| `gbrain doctor [--fast]` | 综合诊断 |
| `gbrain integrity` | 检查数据完整性 |
| `gbrain orphans` | 检测孤立页面 |
| `gbrain autopilot [--once] [--interval <SECS>]` | 自维护守护进程 |

### 配置与其他

| 命令 | 说明 |
|------|------|
| `gbrain config show` | 显示所有配置值 |
| `gbrain config get <key>` | 获取配置值 |
| `gbrain config set <key> <value>` | 设置配置值 |
| `gbrain report --report-type <TYPE> [--title <TITLE>] [--content <TEXT>]` | 生成知识库报告 |
| `gbrain ingest-log [--limit <N>]` | 查看导入日志 |
| `gbrain tools-json` | 以 JSON 输出 MCP 工具定义 |
| `gbrain serve` | 作为 MCP stdio 服务器运行 |

---

## MCP 集成

gbrain 可作为 MCP 服务器运行，供 Claude、Cursor 等 AI 工具调用。

### 启动服务器

```bash
gbrain serve
```

### Claude Desktop 配置

在 `claude_desktop_config.json` 中添加：

```json
{
  "mcpServers": {
    "gbrain": {
      "command": "gbrain",
      "args": ["serve"]
    }
  }
}
```

### Cursor 配置

在 `.cursor/mcp.json` 中添加：

```json
{
  "mcpServers": {
    "gbrain": {
      "command": "gbrain",
      "args": ["serve"]
    }
  }
}
```

### 安全模型

MCP 服务器对远程调用者设置 `remote=true`，启用额外的安全验证：
- slug 格式校验（防路径遍历）
- 输入内容清理
- 参数化查询（防 SQL 注入）
- 文件名安全检查

CLI 直接使用 `remote=false`，跳过远程安全限制。

---

## MCP 工具

gbrain 提供 58 个 MCP 工具，通过 stdio 上的 JSON-RPC 2.0 协议供 AI 智能体集成使用。

### 搜索

| 工具 | 说明 |
|------|------|
| `query` | 混合搜索（向量 + 关键词 + 扩展），支持详情级别、代码过滤、两阶段检索和搜索元数据 |
| `search` | 全文搜索（向量 + 关键词 + RRF 融合），支持代码过滤 |
| `find_by_title_fuzzy` | 基于三元组相似度的标题模糊搜索 |
| `resolve_slugs` | 模糊解析部分 slug 到匹配页面 |

### 页面增删改查

| 工具 | 说明 |
|------|------|
| `get_page` | 读取页面（支持模糊匹配） |
| `put_page` | 写入/更新页面（Markdown + frontmatter） |
| `delete_page` | 软删除页面 |
| `list_pages` | 列出页面（支持类型/标签/数量筛选） |
| `get_chunks` | 获取页面的内容块 |

### 标签

| 工具 | 说明 |
|------|------|
| `add_tag` | 为页面添加标签 |
| `remove_tag` | 从页面移除标签 |
| `get_tags` | 列出页面的标签 |

### 链接与图谱

| 工具 | 说明 |
|------|------|
| `add_link` | 创建页面间的类型化链接 |
| `remove_link` | 移除页面间的链接 |
| `get_links` | 列出页面的出站链接 |
| `get_backlinks` | 列出页面的入站链接 |
| `traverse_graph` | 从页面遍历链接图谱 |

### 时间线

| 工具 | 说明 |
|------|------|
| `add_timeline_entry` | 为页面添加时间线条目 |
| `get_timeline` | 获取页面的时间线 |

### 版本管理

| 工具 | 说明 |
|------|------|
| `get_versions` | 页面版本历史 |
| `revert_version` | 将页面回滚到先前版本 |

### 原始数据

| 工具 | 说明 |
|------|------|
| `put_raw_data` | 存储页面的原始 API 响应数据 |
| `get_raw_data` | 获取页面的原始数据 |

### 代码知识图谱

| 工具 | 说明 |
|------|------|
| `code_def` | 查找代码符号定义 |
| `code_refs` | 查找引用某符号的代码块 |
| `search_code_chunks` | 按关键词/符号文本搜索代码块 |
| `get_callers` | 获取符号的调用者 |
| `get_callees` | 获取符号的被调用者 |
| `get_code_edges_by_chunk` | 获取代码块关联的代码图边 |
| `reindex_code_page` | 重建代码页的代码块和代码边 |

### 文件存储

| 工具 | 说明 |
|------|------|
| `file_upload` | 上传文件到存储 |
| `file_list` | 列出已存储的文件 |
| `file_url` | 获取文件的 URL/路径 |

### 导入与同步

| 工具 | 说明 |
|------|------|
| `log_ingest` | 记录导入事件 |
| `get_ingest_log` | 获取最近的导入日志 |
| `sync_brain` | 从 Git 仓库同步知识库 |
| `find_orphans` | 查找无入站链接的孤立页面 |

### 健康与统计

| 工具 | 说明 |
|------|------|
| `get_stats` | 知识库统计（页面数、块数等） |
| `get_health` | 健康仪表盘（嵌入覆盖率、孤立页面等） |

### KB 子系统

| 工具 | 说明 |
|------|------|
| `kb_list_libraries` | 列出所有知识库（含文档和块计数） |
| `kb_create_library` | 创建知识库（支持语义分块/RAPTOR/分块参数配置） |
| `kb_update_library` | 更新知识库配置 |
| `kb_delete_library` | 删除知识库 |
| `kb_upload_document` | 上传文档到知识库处理 |
| `kb_get_document_status` | 获取文档处理状态 |
| `kb_retry_document` | 重试处理失败的文档 |
| `kb_cancel_document_job` | 取消文档处理作业 |
| `kb_delete_document` | 从知识库删除文档 |
| `kb_list_documents` | 列出知识库中的文档 |
| `kb_search` | 跨库混合搜索（向量 + 关键词 + RRF 融合） |
| `kb_create_folder` | 在知识库中创建文件夹 |
| `kb_purge_document` | 永久删除文档（需确认） |
| `kb_check_index_health` | 检查知识库索引健康状态 |
| `kb_repair_index` | 修复知识库索引 |
| `kb_backup` | 备份知识库到文件 |
| `kb_restore` | 从备份文件恢复知识库 |
| `kb_add_eval_query` | 添加搜索评估查询 |
| `kb_add_search_feedback` | 添加搜索结果反馈评分 |

---

## MCP 工具参数

### `query`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 搜索查询 |
| `limit` | integer | 否 | 最大结果数（默认 20） |
| `offset` | integer | 否 | 分页偏移量 |
| `expand` | boolean | 否 | 启用多查询扩展（默认 true） |
| `detail` | string | 否 | `low` / `medium` / `high`（默认 medium） |
| `lang` | string | 否 | 按编程语言过滤代码检索 |
| `symbol_kind` | string | 否 | 按符号类型过滤代码检索 |
| `near_symbol` | string | 否 | 锚定两阶段代码图检索的起始符号 |
| `walk_depth` | integer | 否 | 代码图邻居遍历深度（0-2） |
| `include_meta` | boolean | 否 | 返回 `{results, meta}` 含向量/扩展详情 |

### `put_page`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `content` | string | 是 | 完整 Markdown（含 YAML frontmatter） |

### `traverse_graph`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 起始页面 slug |
| `depth` | integer | 否 | 最大遍历深度（默认 5，上限 10） |
| `link_type` | string | 否 | 按链接类型筛选 |
| `direction` | string | 否 | `in` / `out` / `both`（默认 out） |

### `find_by_title_fuzzy`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 要匹配的标题 |
| `dir_prefix` | string | 否 | 限定 slug 前缀 |
| `min_similarity` | number | 否 | 相似度阈值 0.0–1.0（默认 0.55） |
| `limit` | integer | 否 | 最大结果数（默认 10） |

### `sync_brain`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `repo_path` | string | 是 | Git 仓库路径 |
| `force_full` | boolean | 否 | 强制全量同步（默认 false） |

### `kb_create_library`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 知识库名称 |
| `semantic_segmentation_enabled` | boolean | 否 | 启用语义分块 |
| `raptor_enabled` | boolean | 否 | 启用 RAPTOR 摘要树 |
| `raptor_llm_base_url` | string | 否 | RAPTOR LLM 基础 URL 覆盖 |
| `raptor_llm_secret_ref` | string | 否 | RAPTOR LLM API 密钥环境变量名 |
| `raptor_llm_model` | string | 否 | RAPTOR LLM 模型名称 |
| `chunk_size` | integer | 否 | 分块大小（字符数） |
| `chunk_overlap` | integer | 否 | 分块重叠（字符数） |
| `batch_max_documents` | integer | 否 | 每批最大文档数 |
| `batch_max_chunks` | integer | 否 | 每批最大块数 |

### `kb_search`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 搜索查询 |
| `library_ids` | integer[] | 否 | 限定搜索的知识库 ID（空 = 全部） |
| `level` | integer | 否 | RAPTOR 树层级过滤 |
| `top_k` | integer | 否 | 最大结果数（默认 10，上限 50） |

---

## 环境变量

> **API 兼容性说明**: 本项目仅支持 OpenAI 兼容格式的 API（`/embeddings`、`/chat/completions`、`/audio/transcriptions`），不支持 Anthropic/Claude API。通过设置 `*_BASE_URL` 可接入任何 OpenAI 兼容服务（DeepSeek、Zhipu、DashScope、Ollama 等）。

### API Key 回退链

各模块的 API Key 按以下优先级回退：

```
嵌入向量:  GBRAIN_OPENAI_API_KEY
查询扩展:  GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
LLM 分块:  GBRAIN_CHUNKER_API_KEY → GBRAIN_OPENAI_API_KEY
KB RAPTOR: GBRAIN_KB_RAPTOR_API_KEY → GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
```

只需设置 `GBRAIN_OPENAI_API_KEY` 即可启用所有 AI 功能。如需不同模型/提供商，可按模块单独覆盖。

### 基础配置

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_DIR` | 数据存储根目录 | `~/.gbrain` |
| `GBRAIN_DB_PATH` | 数据库文件路径 | `$GBRAIN_DIR/brain.db` |

### 嵌入向量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_OPENAI_API_KEY` | OpenAI API 密钥（嵌入向量，同时作为其他模块的回退密钥） | — |
| `GBRAIN_OPENAI_BASE_URL` | OpenAI 兼容基础 URL（同时作为其他模块的回退 URL） | — |
| `GBRAIN_EMBEDDING_MODEL` | 嵌入模型名称 | `text-embedding-3-large` |
| `GBRAIN_EMBEDDING_DIMENSIONS` | 嵌入向量维度 | `1536` |

### 查询扩展

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_EXPANSION_API_KEY` | 查询扩展 LLM API 密钥 | 回退到 `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_EXPANSION_BASE_URL` | 查询扩展 LLM 基础 URL | 回退到 `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_EXPANSION_MODEL` | 查询扩展模型 | `gpt-4o-mini` |

### LLM 分块

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_CHUNKER_API_KEY` | LLM 分块 API 密钥 | 回退到 `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_CHUNKER_BASE_URL` | LLM 分块基础 URL | 回退到 `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_CHUNKER_MODEL` | LLM 分块模型 | `gpt-4o-mini` |

### 音频转录

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_TRANSCRIPTION_PROVIDER` | 转录服务提供商（`groq` / `openai`） | `groq` |
| `GBRAIN_TRANSCRIPTION_GROQ_API_KEY` | Groq 转录 API 密钥 | — |
| `GBRAIN_TRANSCRIPTION_GROQ_BASE_URL` | Groq 转录基础 URL | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_API_KEY` | OpenAI 转录 API 密钥 | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_BASE_URL` | OpenAI 转录基础 URL | — |

### KB 子系统

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_KB_ENABLED` | 启用 KB 子系统 | `true` |
| `GBRAIN_KB_RAPTOR_API_KEY` | KB RAPTOR LLM API 密钥 | 回退到 `GBRAIN_EXPANSION_API_KEY` |
| `GBRAIN_KB_RAPTOR_BASE_URL` | KB RAPTOR LLM 基础 URL | 回退到 `GBRAIN_EXPANSION_BASE_URL` |
| `GBRAIN_KB_RAPTOR_MODEL` | KB RAPTOR LLM 模型 | `gpt-4o-mini` |
| `GBRAIN_KB_MAX_FILE_SIZE_MB` | KB 文件大小上限（MB） | `50` |
| `GBRAIN_KB_ALLOWED_EXTENSIONS` | KB 允许的文件扩展名（逗号分隔） | `pdf,docx,xlsx,csv,html,htm,txt,md` |
| `GBRAIN_KB_STORAGE_DIR` | KB 文件存储目录 | — |

### 日志

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_LOG_LEVEL` | 日志级别（trace/debug/info/warn/error） | `info` |
| `GBRAIN_LOG_TO_FILE` | 启用文件日志 | `true` |
| `GBRAIN_LOG_FILE_PATH` | 日志文件路径 | `$GBRAIN_DIR/logs/gbrain.log` |
| `GBRAIN_LOG_TO_CONSOLE` | 启用控制台日志 | `true` |

### 行为控制

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_AUTO_LINK` | 写入时自动提取链接 | `true` |
| `GBRAIN_AUTO_TIMELINE` | 写入时自动提取时间线 | `true` |
| `GBRAIN_POST_WRITE_LINT` | 写入后运行 lint | `false` |

### 调试

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_SEARCH_DEBUG` | 启用搜索调试日志（设为 `1` 或 `true`） | — |
| `GBRAIN_PROGRESS_MODE` | 进度显示模式（`human` / `json` / `quiet`） | 自动检测 |
| `GBRAIN_PROGRESS_JSON` | 设为 `"1"` 启用 JSON 进度模式 | — |

---

## 写入模式

gbrain 提供三种写入策略，通过 `put_page` 的 `writer_mode` 参数控制：

| 模式 | 说明 |
|------|------|
| `Strict` | 严格校验——要求 frontmatter、禁止空内容、检查链接引用有效性 |
| `Lint` | 零 LLM 质量检查——运行 6 条规则，自动修复可修复的问题 |
| `Off` | 自由写入——跳过所有校验，直接写入 |

### Lint 规则

| 规则 | 说明 |
|------|------|
| LLM 前言检测 | 检测并移除 AI 生成的典型前言（"Here is..."、"Sure, I'll..."） |
| 占位日期检测 | 检测未替换的日期占位符（如 `YYYY-MM-DD`） |
| 缺失 frontmatter | 检测缺失的 YAML frontmatter |
| 断裂引用 | 检测引用了不存在页面的 wikilink |
| 空章节 | 检测仅有标题无内容的章节 |
| 未闭合代码围栏 | 检测未闭合的 ``` 代码块 |

---

## 软删除生命周期

页面删除遵循软删除机制，防止误删并支持恢复：

```
正常页面 ──delete──→ 软删除状态（仍占存储，不可查询）
                        │
                        ├──restore──→ 恢复为正常页面
                        │
                        └──purge-deleted──→ 永久删除（释放存储）
```

- `gbrain delete <slug>` — 软删除，页面标记为已删除但数据保留
- `gbrain restore <slug>` — 恢复软删除的页面
- `gbrain purge-deleted --older-than-hours 168` — 永久清理 7 天前的软删除页面

---

## 代码知识图谱

基于 Tree-sitter AST 分块 + regex 符号索引，支持以下语言：

| 语言 | Tree-sitter 绑定 |
|------|-----------------|
| Rust | `tree-sitter-rust` |
| TypeScript | `tree-sitter-typescript` |
| JavaScript | `tree-sitter-javascript` |
| Python | `tree-sitter-python` |
| Go | `tree-sitter-go` |
| Java | `tree-sitter-java` |
| C | `tree-sitter-c` |
| C++ | `tree-sitter-cpp` |

通过 `gbrain import` 导入代码文件时，Tree-sitter 进行 AST 分块，regex 提取符号定义、引用和调用图。之后可通过 `gbrain code` 命令或 MCP 工具查询。

---

## 测试

```bash
cargo test                    # 所有测试
cargo test --test engine_test # 引擎集成测试
cargo test --test search_test # 搜索集成测试
cargo test --test fuzzy_test  # 模糊匹配测试
cargo test --test dedup_test  # 去重测试
cargo clippy                  # 代码检查
```

测试使用内存 SQLite（`:memory:`），无需额外配置。

---

## 架构

三层设计:

1. **引擎层** — `BrainEngine` trait → `SqliteEngine`（SQLite + FTS5 + sqlite-vec）。同步，直接数据库操作。

2. **操作层** — 业务逻辑：自动分块、标签提取、链接推断、安全验证、批量操作。

3. **接口层** — CLI + MCP 服务器。CLI 使用 `remote=false`；MCP 对不受信任的调用者设置 `remote=true`。

### 搜索流水线

9 步混合搜索流水线（+ 两阶段代码图扩展 + 去重）:

1. FTS5 BM25 关键词搜索（权重: 标题 10x, compiled_truth 5x, 时间线 2x）
2. sqlite-vec 余弦相似度
3. 向量结果不足 3 条时回退到扩展 OR 查询
4. RRF 融合（k=60），支持多列表
5. compiled_truth 加权提升
6. 反向链接提升
7. 时效性提升（时间衰减）
8. 意图类型提升（实体/时间/事件）
9. 6 层去重（slug top-3 → 跨源去重 → 文本相似度 → 类型多样性 → 每页上限 → compiled_truth 保证）

### KB 子系统架构

异步五阶段文档处理管线:

1. **解析** — 文档解析器（Markdown / PDF / DOCX / XLSX / CSV / HTML / 纯文本 / 代码）
2. **拆分** — 递归拆分器 / 语义拆分器（Savitzky-Golay 平滑 + chunk_overlap 重叠），支持 `semantic_enabled` 标志切换
3. **嵌入** — 向量嵌入生成与持久化
4. **RAPTOR** — 递归摘要树（K-Means++ 聚类 + LLM 摘要，三级回退链：库级配置 → `GBRAIN_EXPANSION_*` → `GBRAIN_CHUNKER_*`）
5. **持久化** — 事务保护的节点/向量写入

### 中文 NLP 模块

- **分词索引** — jieba 分词 + 拼音 + 前缀通配符，FTS5 查询自动重构
- **中文分块** — 中文标点加入句子/子句分隔层级，CJK 标点无需后续空格即可断句
- **预分词列** — schema V16 新增 `_tokens` 列，FTS5 改用 `unicode61` 分词器，写入时自动同步

---

## 文档

- [TS vs Rust 对比报告](./docs/compare_report.md) / [English](./docs/compare_report_en.md) — TypeScript 与 Rust 版本的全面对比（代码规模、数据库、搜索、MCP、安全等）
- [TS vs Rust 模块级详细对比](./docs/module_detail.md) / [English](./docs/module_detail_en.md) — 逐模块对比（引擎层、操作层、搜索、分块、富化、验证器等）

---

## 许可证

MIT License
