# gbrain-rs

[English](./README_EN.md) | 中文

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**个人知识大脑引擎** — 基于 [gbrain](https://github.com/garrytan/gbrain) 的 Rust 移植，新增单入口多投影融合架构（Artifact 原件 → KB/影子页面/候选变更/附件多投影 + 溯源审计 + 回滚）、KB 子系统（异步文档处理管线 + RAPTOR 递归摘要树）、中文 NLP 全链路支持（jieba 分词 + 拼音 + FTS5 查询重构）、软删除生命周期（restore/purge-deleted）、时间衰减搜索等特性。基于 SQLite + sqlite-vec + FTS5 构建的零配置嵌入式架构，开箱即用。

> 原始 TypeScript 版本由 [Garry Tan](https://github.com/garrytan) 开发。本项目采用 **Vibe coding** 构建。

---

## 快速开始

```bash
# 1. 构建
cargo build --release

# 2. 初始化知识库
gbrain init

# 3. 写入长期记忆
gbrain put people/alice --title "Alice" --content "一位工程师，擅长 Rust 和系统编程"

# 4. 查询知识
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
- **单入口多投影融合** — 原件上传（Upload）自动路由至多投影（KB 文档 / 影子页面 / 候选变更 / 文件附件 / 链接 / 时间线），溯源审计（Provenance Ledger），候选评审与提升（Promotion），版本链与回滚（Projection Supersede / Rollback），统一记忆查询（Memory Query，4 种策略）
- **MCP 服务器** — 完整的模型上下文协议（JSON-RPC 2.0）服务器，默认暴露 Artifact facade 工具
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
gbrain init                    # 初始化知识库（含安装可执行文件到 ~/.gbrain/bin/）
```


---

## 数据目录

初始化后，`~/.gbrain/` 目录结构如下：

```
~/.gbrain/
  brain.db           # SQLite 数据库（FTS5 + sqlite-vec）
  config.json        # 运行时配置（通过 gbrain config set 生成）
  artifacts/         # Artifact 原件存储（按 SHA256 命名）
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

### 核心命令

| 命令 | 说明 |
|------|------|
| `gbrain init` | 初始化知识库 |
| `gbrain put <slug> [--title] [--content|--file] [--intent] [--dry-run] [--force]` | 手动写入长期记忆 |
| `gbrain upload <path> [--intent] [--target] [--page] [--library] [--folder] [--promotion] [--dry-run]` | 上传文件作为知识源 |
| `gbrain query <query> [--mode] [--limit] [--filter] [--include-sources]` | 统一知识查询 |
| `gbrain list [--limit] [--offset]` | 列出知识源 |
| `gbrain get <id_or_uid> [--include-projections] [--include-sources]` | 获取知识源详情 |
| `gbrain delete <id_or_uid> [--dry-run]` | 软删除知识源 |
| `gbrain detach <id_or_uid> --from <slug> [--dry-run]` | 解除知识源与页面的关联 |
| `gbrain restore <id_or_uid> [--dry-run]` | 恢复已删除的知识源 |
| `gbrain reprocess <id_or_uid> [--dry-run]` | 重新处理知识源的投影 |
| `gbrain health` | 检查知识源一致性 |
| `gbrain review list [--status] [--target] [--limit]` | 列出建议变更 |
| `gbrain review show <change_id>` | 查看建议变更详情 |
| `gbrain review apply <change_id>` | 应用建议变更 |
| `gbrain review reject <change_id> [--reason]` | 拒绝建议变更 |
| `gbrain review rollback <change_id>` | 回滚已应用的建议变更 |
| `gbrain serve` | 运行 MCP stdio 服务器 |
| `gbrain config show\|get\|set` | 配置管理 |

#### 示例

```bash
# 初始化知识库
gbrain init

# 写入长期记忆（默认意图 memory）
gbrain put people/alice --title "Alice" --content "一位工程师，擅长 Rust 和系统编程"

# 查询知识
gbrain query "谁是 Alice"

# 上传文档自动路由
gbrain upload report.pdf --intent evidence

# 预览写入路由（不实际执行）
gbrain put people/bob --content "产品经理" --dry-run

# 强制覆盖已被人工修改的页面
gbrain put people/alice --content "更新内容" --force

# 列出建议变更
gbrain review list --status pending

# 查看建议变更详情
gbrain review show 1

# 应用建议变更
gbrain review apply 1

# 拒绝建议变更
gbrain review reject 2 --reason "信息已过时"

# 启动 MCP 服务器
gbrain serve
```

---

## CLI 命令参数

### `gbrain put`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 目标页面 slug（如 people/alice） |
| `--title` | string | 否 | 页面标题（可选，默认从 slug 推断） |
| `--content` | string | 否 | 直接输入的文本内容（与 --file 二选一） |
| `--file` | path | 否 | 从文件读取内容（与 --content 二选一） |
| `--intent` | string | 否 | 意图: memory(默认), evidence, promote |
| `--force` | flag | 否 | 强制覆盖已被人工修改的页面 |
| `--dry-run` | flag | 否 | 仅返回路由计划，不实际写入 |

### `gbrain upload`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | path | 是 | 本地文件路径 |
| `--intent` | string | 否 | 上传意图: auto(默认), evidence, memory, attachment, promote |
| `--target` | string | 否 | 目标 gbrain 页面 slug（用于生成建议变更） |
| `--page` | string | 否 | 关联页面 slug（用于附件） |
| `--library` | integer | 否 | KB 库 ID |
| `--folder` | integer | 否 | KB 文件夹 ID |
| `--promotion` | string | 否 | 提升策略: none, shadow, candidate, auto-low-risk |
| `--dry-run` | flag | 否 | 仅返回路由计划，不实际执行 |

### `gbrain query`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 查询文本 |
| `--mode` | string | 否 | 查询模式: auto(默认), memory, evidence, timeline |
| `--limit` | integer | 否 | 最大结果数 |
| `--filter` | string | 否 | 按 slug 过滤 |
| `--include-sources` | flag | 否 | 包含来源追溯 |

### `gbrain list`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `--limit` | integer | 否 | 最大结果数（默认 50） |
| `--offset` | integer | 否 | 偏移量 |

### `gbrain get`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `--include-projections` | flag | 否 | 包含投影详情 |
| `--include-sources` | flag | 否 | 包含来源追溯 |

### `gbrain delete`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `--dry-run` | flag | 否 | 预览影响，不实际删除 |

### `gbrain detach`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `--from` | string | 是 | 目标页面 slug |
| `--dry-run` | flag | 否 | 预览影响，不实际执行 |

### `gbrain restore`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `--dry-run` | flag | 否 | 预览恢复影响，不实际执行 |

### `gbrain reprocess`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `--dry-run` | flag | 否 | 预览重新处理影响，不实际执行 |

### `gbrain review list`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `--status` | string | 否 | 过滤状态: pending, accepted, rejected, applied, rolled_back |
| `--target` | string | 否 | 过滤目标页面 slug |
| `--limit` | integer | 否 | 最大结果数（默认 50） |

### `gbrain review show`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | string | 是 | 变更 ID |

### `gbrain review apply`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | string | 是 | 变更 ID |

### `gbrain review reject`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | string | 是 | 变更 ID |
| `--reason` | string | 否 | 拒绝原因 |

### `gbrain review rollback`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | string | 是 | 变更 ID |

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

gbrain 通过 MCP 标准协议暴露 Artifact 统一知识操作 facade 工具（`artifact_*`），供 AI 智能体集成使用。

### Artifact 统一知识操作

| 工具 | 说明 |
|------|------|
| `artifact_put` | 写入手动记忆（slug + content + intent） |
| `artifact_upload` | 上传文件作为知识源（支持 PDF/DOCX/MD 等） |
| `artifact_query` | 统一知识查询（支持 memory/evidence/timeline 模式） |
| `artifact_list` | 列出知识源 |
| `artifact_get` | 获取知识源详情（含 occurrences/projections/sources） |
| `artifact_delete` | 软删除知识源（dry-run 支持影响预览） |
| `artifact_detach` | 解除知识源与特定页面的关联 |
| `artifact_restore` | 恢复已删除的知识源 |
| `artifact_reprocess` | 重新处理知识源的所有投影 |
| `artifact_health` | 知识源健康检查 |
| `artifact_review_list` | 列出建议变更 |
| `artifact_review_get` | 获取建议变更详情 |
| `artifact_review_apply` | 应用建议变更 |
| `artifact_review_reject` | 拒绝建议变更 |
| `artifact_review_rollback` | 回滚已应用的建议变更 |

#### 示例

```json
// 写入手动记忆
{ "tool": "artifact_put", "params": { "slug": "rust-async", "content": "Rust 异步编程使用 async/await 语法...", "intent": "memory" } }
```

```json
// 统一知识查询（含来源追溯）
{ "tool": "artifact_query", "params": { "query": "Rust 异步编程", "mode": "auto", "include_sources": true } }
```

```json
// 获取知识源详情
{ "tool": "artifact_get", "params": { "id_or_uid": "art_abc123", "include_sources": true, "include_projections": true } }
```

```json
// 上传文件
{ "tool": "artifact_upload", "params": { "path": "/path/to/doc.pdf", "intent": "memory", "library_id": 1 } }
```

```json
// 列出建议变更
{ "tool": "artifact_review_list", "params": { "status": "pending" } }
```

---

## MCP 工具参数

### `artifact_put`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 目标页面 slug（如 people/alice） |
| `content` | string | 否 | 直接输入的文本内容（与 file 二选一） |
| `file` | string | 否 | 本地文件路径（与 content 二选一） |
| `title` | string | 否 | 页面标题（可选，默认从 slug 推断） |
| `intent` | string | 否 | 意图: memory / evidence / promote（默认 memory） |
| `force` | boolean | 否 | 强制覆盖已被人工修改的页面（默认 false） |
| `dry_run` | boolean | 否 | 仅返回路由计划，不实际写入 |

### `artifact_upload`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | string | 是 | 本地文件路径 |
| `intent` | string | 否 | 上传意图: auto / evidence / memory / attachment / promote（默认 auto） |
| `target_slug` | string | 否 | 目标 gbrain 页面 slug（用于生成建议变更） |
| `page_slug` | string | 否 | 关联页面 slug（用于附件） |
| `library_id` | integer | 否 | KB 库 ID（可选，默认自动选择 Inbox） |
| `folder_id` | integer | 否 | KB 文件夹 ID |
| `promotion` | string | 否 | 提升策略: none / shadow / candidate / auto-low-risk |
| `dry_run` | boolean | 否 | 仅返回路由计划，不实际写入 |

### `artifact_query`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 查询文本 |
| `mode` | string | 否 | 查询模式: auto / memory / evidence / timeline（默认 auto） |
| `limit` | integer | 否 | 最大结果数 |
| `filter_slug` | string | 否 | 过滤到指定页面 slug |
| `include_sources` | boolean | 否 | 显示来源追溯（artifact 来源和引用） |

### `artifact_list`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `limit` | integer | 否 | 最大结果数（默认 50） |
| `offset` | integer | 否 | 偏移量 |

### `artifact_get`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID（如 '1' 或 'art_ab12cd34ef56'） |
| `include_projections` | boolean | 否 | 包含投影详情 |
| `include_sources` | boolean | 否 | 包含来源追溯 |

### `artifact_delete`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `dry_run` | boolean | 否 | 预览删除影响，不实际执行 |

### `artifact_detach`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `from` | string | 是 | 目标页面 slug |
| `dry_run` | boolean | 否 | 预览影响，不实际执行 |

### `artifact_restore`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `dry_run` | boolean | 否 | 预览恢复影响，不实际执行 |

### `artifact_reprocess`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID |
| `dry_run` | boolean | 否 | 预览重新处理影响，不实际执行 |

### `artifact_health`

无参数。

### `artifact_review_list`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `status` | string | 否 | 过滤状态: pending / accepted / rejected / applied / rolled_back |
| `target_slug` | string | 否 | 过滤目标页面 slug |
| `limit` | integer | 否 | 最大结果数 |

### `artifact_review_get`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |

### `artifact_review_apply`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |

### `artifact_review_reject`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |
| `reason` | string | 否 | 拒绝原因 |

### `artifact_review_rollback`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |

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
| `GBRAIN_ARTIFACT_STORAGE_DIR` | Artifact 原件存储目录 | `$GBRAIN_DIR/artifacts` |

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

## 写入意图

`gbrain put` 通过 `--intent` 参数控制写入的投影目标：

| 意图 | 说明 |
|------|------|
| `memory` | 默认值，写入 gbrain 页面 + KB 知识库 |
| `evidence` | 仅写入 KB 作为证据，不创建 gbrain 页面 |
| `promote` | 提升已有 KB 证据为 gbrain 页面内容 |

## 写入模式

gbrain 内部提供三种页面写入策略（通过配置 `post_write_lint`、`writer.*` 等控制）：

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

- `gbrain delete <id_or_uid>` — 软删除，数据保留
- `gbrain restore <id_or_uid>` — 恢复软删除的页面
- `gbrain health` — 检查知识源一致性

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

通过 `gbrain upload` 上传代码文件时，Tree-sitter 进行 AST 分块，regex 提取符号定义、引用和调用图。

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

### 单入口多投影融合架构（Artifact）

```
上传原件（单一入口）
  │
  ├─ 路由规划器（根据 intent + 文件类型自动决策）
  │
  ├─ Artifact 原件存储（SHA256 去重，按 hash 命名）
  │
  └─ 多投影自动创建：
      ├─ KB Document 投影 → 文档处理管线（解析→拆分→嵌入→RAPTOR→持久化）
      ├─ Shadow Page 投影 → 影子页面（提取内容生成 wiki 页面）
      ├─ Promotion Candidate 投影 → 候选变更（实体提及/链接建议/时间线事件/事实声明）
      ├─ File Attachment 投影 → 文件附件（简单文件引用）
      ├─ Brain Link 投影 → 自动链接
      └─ Brain Timeline 投影 → 自动时间线
```

**核心概念：**

- **Artifact** — 上传的原始文件，含状态（active/deleted/purged）、来源类型（upload/sync/link/mcp）、上传意图（auto/evidence/memory/attachment/promote；`document` 是 `evidence` 的别名）
- **Projection** — 同一 Artifact 在不同子系统中的表示，含版本链（superseded_by）和状态（active/stale/superseded）
- **Candidate** — 从 KB 证据中提取的建议变更，含风险等级（low/medium/high）和评审工作流（pending→accepted→applied / rejected / rolled_back）
- **Provenance** — 溯源审计记录，追踪页面事实的来源 Artifact 和 Candidate

**统一记忆查询（Memory Query）：**

4 种查询策略自动适配不同场景：

| 策略 | 说明 |
|------|------|
| `brain_first` | 优先搜索 gbrain 策展知识，不足时补充 KB 证据 |
| `evidence_first` | 优先搜索 KB 文档证据，适合需要原始来源的场景 |
| `provenance` | 追溯事实来源，返回溯源记录 |
| `timeline_first` | 优先按时间线排序，适合时间相关查询 |

---

## 文档

- [TS vs Rust 对比报告](./docs/compare_report.md) / [English](./docs/compare_report_en.md) — TypeScript 与 Rust 版本的全面对比（代码规模、数据库、搜索、MCP、安全等）
- [TS vs Rust 模块级详细对比](./docs/module_detail.md) / [English](./docs/module_detail_en.md) — 逐模块对比（引擎层、操作层、搜索、分块、富化、验证器等）

---

## 许可证

MIT License
