# gbrain-rs

## 2026-05-04 当前实现状态

- `gbrain embed`、查询流程和 Autopilot 已接入真实 embedding 生成与持久化；当 sqlite-vec 不可用时，会使用 `chunk_embeddings` fallback 表做 cosine 检索。
- `delete` 现在是软删除，新增 `restore` 和 `purge-deleted` 命令；默认读取、列表、搜索、统计和健康检查都会排除软删除页面。
- schema version 已更新到 9，新增 `deleted_at`、chunk 代码元数据字段，以及 `chunk_embeddings`。
- `PageType` 已补齐 `email`、`slack`、`calendar-event`、`code`；`ChunkSource` 已支持 `fenced_code`。
- `put_page` 会从 Markdown fenced code block 生成额外的代码 chunk；搜索支持 include/exclude slug prefix、默认 hard-exclude 和 source boost。
- Markdown import 会校验 frontmatter `slug` 与路径推导 slug 是否一致，不一致时跳过。

[English](./README_EN.md) | 中文

**个人知识大脑引擎** — [gbrain](https://github.com/garrytan/gbrain) 的 Rust 实现。基于 SQLite + sqlite-vec + FTS5 构建的零配置嵌入式知识库，支持混合搜索、知识图谱和 MCP 智能体集成。

> 原始 TypeScript 版本由 [Garry Tan](https://github.com/garrytan) 开发，本项目为 Rust 移植版。本项目采用**Vibe coding**构建。

---

## 特性

- **混合搜索** — BM25 关键词 + 向量余弦相似度 + 模糊三元组，通过倒数排名融合（RRF）合并，支持多查询扩展
- **知识图谱** — Wiki 链接提取、类型化链接、图遍历、反向链接对称性验证
- **MCP 服务器** — 完整的模型上下文协议（JSON-RPC 2.0）服务器，用于 AI 智能体集成
- **零配置** — 嵌入式 SQLite，无需外部服务（嵌入向量可选）
- **分层丰富** — 自动实体检测与提升（提及 → 存根 → 完善）
- **版本历史** — 完整的页面版本管理，支持回滚
- **自动驾驶** — 自维护守护进程，自动嵌入过期内容并执行完整性检查
- **安全防护** — 路径遍历防护、slug 验证、远程调用输入清理

---

## 构建与安装

```bash
cargo build --release          # 构建
cargo install --path .         # 安装到 ~/.cargo/bin/
gbrain install                 # 安装到 ~/.gbrain/bin/
```

可选特性:

```bash
cargo build --features file-server   # 包含 axum 文件服务器
```

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
| `gbrain query <query> [--limit <N>]` | 混合搜索（别名: `ask`） |

### 搜索与图谱

| 命令 | 说明 |
|------|------|
| `gbrain resolve <partial>` | 模糊解析部分 slug |
| `gbrain graph <slug> [--depth <N>]` | 从页面遍历知识图谱 |
| `gbrain graph-query <from> [--to <slug>] [--depth <N>] [--link-type <TYPE>]` | 查询页面间的图谱关系 |

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
| `gbrain import <dir> [--embed] [--auto-link]` | 导入 Markdown 文件；frontmatter slug 与路径不一致时跳过 |
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
| `gbrain mcp` | 作为 MCP stdio 服务器运行 |

---

## MCP 工具

gbrain 提供 28 个 MCP 工具，通过 stdio 上的 JSON-RPC 2.0 协议供 AI 智能体集成使用。

### 搜索

| 工具 | 说明 |
|------|------|
| `query` | 混合搜索（向量 + 关键词 + 扩展），支持详情级别 |
| `search` | 全文搜索 |
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

### 导入与同步

| 工具 | 说明 |
|------|------|
| `log_ingest` | 记录导入事件 |
| `get_ingest_log` | 获取最近的导入日志 |
| `sync_brain` | 从 Git 仓库同步知识库 |

### 健康与统计

| 工具 | 说明 |
|------|------|
| `get_stats` | 知识库统计（页面数、块数等） |
| `get_health` | 健康仪表盘（嵌入覆盖率、孤立页面等） |

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

---

## 环境变量

> **API 兼容性说明**: 本项目仅支持 OpenAI 兼容格式的 API（`/embeddings`、`/chat/completions`、`/audio/transcriptions`），不支持 Anthropic/Claude API。通过设置 `*_BASE_URL` 可接入任何 OpenAI 兼容服务（DeepSeek、Zhipu、DashScope、Ollama 等）。

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

## 测试

```bash
cargo test                    # 所有测试
cargo test --test engine_test # 引擎集成测试
cargo test --test search_test # 搜索集成测试
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

9 步混合搜索流水线:

1. FTS5 BM25 关键词搜索（权重: 标题 10x, compiled_truth 5x, 时间线 2x）
2. sqlite-vec 余弦相似度
3. 向量结果不足 3 条时回退到扩展 OR 查询
4. RRF 融合（k=60），支持多列表
5. compiled_truth 加权提升
6. 反向链接提升
7. 时效性提升（时间衰减）
8. 意图类型提升（实体/时间/事件）
9. 4 层去重（slug → compiled_truth 优先 → 分数排序 → 截断）

---

## 文档

- [TS vs Rust 对比报告](./docs/compare_report.md) / [English](./docs/compare_report_en.md) — TypeScript 与 Rust 版本的全面对比（代码规模、数据库、搜索、MCP、安全等）
- [TS vs Rust 模块级详细对比](./docs/module_detail.md) / [English](./docs/module_detail_en.md) — 逐模块对比（引擎层、操作层、搜索、分块、富化、验证器等）

---

## 许可证

MIT License
