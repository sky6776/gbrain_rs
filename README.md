# gbrain-rs

[English](./README_EN.md) | 中文

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

**个人知识大脑引擎** — 基于 [gbrain](https://github.com/garrytan/gbrain) 的 Rust 移植，新增单入口多投影融合架构（Artifact 原件 → KB/影子页面/候选变更/附件多投影 + 溯源审计 + 回滚）、KB 子系统（异步文档处理管线 + 新库默认启用的 RAPTOR 递归摘要树）、中文 NLP 全链路支持（jieba 分词 + 拼音 + FTS5 查询重构）、软删除生命周期、时间衰减搜索等特性。基于 SQLite + FTS5 的嵌入式架构；向量检索在可用时使用 sqlite-vec，否则回退到内置 BLOB 存储检索，无需外部数据库。

> 原始 TypeScript 版本由 [Garry Tan](https://github.com/garrytan) 开发。本项目采用 **Vibe coding** 构建。

---

## 快速开始

```bash
# 1. 构建并安装
cargo install --path .

# 2. 初始化知识库
gbrain init

# 3. 写入长期记忆
gbrain put people/alice --title "Alice" --content "一位工程师，擅长 Rust 和系统编程"

# 4. 查询知识
gbrain query "谁是 Alice"

# 5. 启动 MCP 服务器（供 AI 智能体使用）
gbrain serve
```

无需配置数据库或外部服务即可使用关键词检索和本地存储。嵌入向量、查询扩展与重排序为可选功能，仅在使用相应底层或 KB 检索路径并配置 API Key 后启用。

---

## 特性

- **检索能力** — 统一 facade 查询当前以 FTS5 关键词检索为主；底层 hybrid API 可将关键词与可选向量结果用 RRF 融合并支持多查询扩展，模糊三元组查询作为独立 API 提供
- **知识图谱** — Wiki 链接提取、类型化链接、图遍历、反向链接对称性验证
- **KB 子系统** — 异步文档处理管线，新建库默认启用 RAPTOR（满足模型配置与库策略时执行）；语义分块仍按库配置启用；支持 Markdown/PDF/DOCX/XLSX/CSV/HTML/纯文本解析和 PDF 页级 OCR 自动检测与回写；代码页面索引走独立代码流程
- **中文 NLP** — jieba 分词 + 拼音 + 前缀通配符，FTS5 查询自动重构，中文标点断句与分词计数，预分词列自动同步
- **单入口多投影融合** — 原件上传（Upload）自动创建 KB 文档、影子页面、页面更新或附件等投影，并通过候选评审形成链接或时间线变更；支持溯源审计、提升、版本链与回滚，内部 Memory Query 含 4 种策略
- **MCP 服务器** — 完整的模型上下文协议（JSON-RPC 2.0）服务器，默认暴露 Artifact facade 与 KB OCR 工具
- **零配置** — 嵌入式 SQLite，无需外部服务（嵌入向量可选）
- **分层丰富** — 自动实体检测与提升（提及 → 存根 → 完善）
- **版本历史** — 完整的页面版本管理，支持回滚
- **自动驾驶** — 自维护守护线程，`gbrain serve` 时自动在后台运行，定期嵌入过期内容并执行完整性检查（默认每 3600 秒，可通过 `GBRAIN_AUTOPILOT_INTERVAL` 配置间隔，最小60秒，通过 `GBRAIN_AUTOPILOT_ENABLED` 关闭）
- **安全防护** — 路径遍历防护、slug 验证、远程调用输入清理、参数化查询防 SQL 注入
- **代码知识图谱** — 导入或重新索引为代码页面时，可用 Tree-sitter AST 代码分块 + regex 符号索引生成定义、引用和调用图（Rust/TypeScript/JavaScript/Python/Go/Java/C/C++）
- **音频转录模块** — 库 API 支持 Groq Whisper（默认）或 OpenAI Whisper；当前未暴露 CLI/MCP 命令
- **写入校验库 API** — `BrainWriter` 提供 Strict/Lint/Off；统一 Artifact CLI/MCP 未暴露写入模式选择
- **软删除生命周期** — Artifact 删除/恢复已暴露；永久清理与按时间清理目前仅在引擎层可用

---

## 构建与安装

```bash
cargo build --release          # 构建
cargo install --path .         # 安装到 ~/.cargo/bin/
gbrain init                    # 初始化知识库（含安装可执行文件到 ~/.gbrain/bin/）
```


---

## 数据目录

按功能使用后，`~/.gbrain/` 目录中可能出现：

```
~/.gbrain/
  brain.db           # SQLite 数据库（FTS5 + 向量 BLOB 回退；扩展可用时使用 sqlite-vec）
  config.json        # 运行时配置（通过 gbrain config set 生成）
  bin/               # 可执行文件副本（gbrain init 时复制）
  artifacts/         # Artifact 原件存储（按 SHA256 去重命名，KB 文档也引用此处）
  cache/             # 缓存目录
  logs/              # 日志文件
    gbrain.log
```

Artifact 上传生成的 KB 文档引用 `artifacts/` 中的原件；KB 元数据保存在 `brain.db` 中。可通过 `GBRAIN_DIR` 环境变量自定义根目录。

---

## 命令行命令

### 全局选项

| 选项 | 说明 |
|------|------|
| `--db <PATH>` | 数据库路径 |
| `--json` | 以 JSON 格式输出 |
| `--dry-run` | 预览操作，不实际执行 |

全局选项必须写在子命令之前，例如 `gbrain --json query "Alice"`。

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
| `gbrain kb ocr-status <doc_id>` | 查看 KB 文档 OCR 状态 |
| `gbrain kb ocr-run <doc_id> [--pages]` | 手动触发或排队 OCR |
| `gbrain kb ocr-retry <doc_id> [--pages]` | 重试失败或空结果的 OCR 页 |
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

# ===== 写入 =====
# 写入长期记忆（默认意图 memory）
gbrain put people/alice --title "Alice" --content "一位工程师，擅长 Rust 和系统编程"

# 从文件写入
gbrain put docs/guide --file ./guide.md --intent memory

# 预览写入路由（不实际执行）
gbrain put people/bob --content "产品经理" --dry-run

# 强制覆盖已被人工修改的页面
gbrain put people/alice --content "更新内容" --force

# ===== 上传 =====
# 上传文档自动路由
gbrain upload report.pdf --intent evidence

# 上传并关联到特定页面
gbrain upload note.txt --page people/alice --intent attachment

# 上传到指定 KB 库和文件夹
gbrain upload paper.pdf --library 1 --folder 2 --intent evidence

# 上传并指定提升策略
gbrain upload document.md --intent memory --promotion auto-low-risk

# 预览上传路由
gbrain upload data.csv --dry-run

# ===== 查询 =====
# 统一知识查询
gbrain query "谁是 Alice"

# 按模式查询（仅记忆/仅证据/时间线）
gbrain query "Rust 异步" --mode memory
gbrain query "市场分析" --mode evidence --limit 10
gbrain query "最近更新" --mode timeline

# 过滤到特定页面
gbrain query "性能优化" --filter tech/rust

# 包含来源追溯
gbrain query "项目A进展" --include-sources

# ===== 查看 =====
# 列出知识源
gbrain list --limit 20

# 获取知识源详情
gbrain get 1
gbrain get art_ab12cd34ef56 --include-projections --include-sources

# ===== 生命周期管理 =====
# 软删除知识源
gbrain delete 5

# 预览删除影响
gbrain delete 5 --dry-run

# 解除知识源与页面的关联
gbrain detach 5 --from people/alice

# 恢复已删除的知识源
gbrain restore 5

# 重新处理知识源
gbrain reprocess 5

# 健康检查
gbrain health

# ===== 变更审核 =====
# 列出建议变更
gbrain review list --status pending

# 按状态过滤
gbrain review list --status applied --target people/alice

# 查看建议变更详情
gbrain review show 1

# 应用建议变更
gbrain review apply 1

# 拒绝建议变更
gbrain review reject 2 --reason "信息已过时"

# 回滚已应用的建议变更
gbrain review rollback 1

# ===== 配置管理 =====
# 查看所有配置
gbrain config show

# 获取单个配置
gbrain config get embedding_model

# 设置配置
gbrain config set chunk_size 800
gbrain config set log_level debug

# ===== MCP 服务器 =====
# 启动 MCP stdio 服务器
gbrain serve

# ===== 高级用法 =====
# 使用自定义数据库路径
gbrain --db /path/to/custom/brain.db init
gbrain --db /path/to/custom/brain.db put people/alice --content "Hello"

# JSON 格式输出（便于脚本处理）
gbrain --json query "Alice"
gbrain --json get 1 --include-projections
gbrain --json health
gbrain --json review list --status pending

# 仅预览影响（dry-run，所有支持的命令）
gbrain put people/bob --content "test" --dry-run
gbrain --json upload report.pdf --dry-run
gbrain delete 5 --dry-run
gbrain detach 5 --from people/alice --dry-run
gbrain restore 5 --dry-run
gbrain reprocess 5 --dry-run

# ===== 意图驱动工作流 =====
# evidence 意图：仅存为 KB 证据，不创建脑页
gbrain put research/findings --content "实验数据显示..." --intent evidence

# promote 意图：创建影子页 + KB 投影 + 候选变更（需审核后发布）
gbrain put people/new-hire --content "新员工信息..." --intent promote

# upload promote 意图 + 自动低风险应用
gbrain upload meeting-notes.md --intent promote --promotion auto-low-risk --target people/alice

# ===== 完整审核工作流 =====
# 1. 列出待审核变更
gbrain review list --status pending

# 2. 查看详情（含证据和风险评估）
gbrain review show 1

# 3. 审核通过并应用
gbrain review apply 1

# 4. 若发现问题，回滚已应用的变更
gbrain review rollback 1

# 5. 拒绝不适用的变更
gbrain review reject 2 --reason "信息已过时，参考新来源"

# ===== KB OCR =====
# 查看状态、指定页运行 OCR、重试失败或空结果页面
gbrain kb ocr-status 1
gbrain kb ocr-run 1 --pages "1,3,5-10"
gbrain kb ocr-retry 1
```

---

## CLI 命令参数

### `gbrain put`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 目标页面 slug（如 people/alice） |
| `--title` | string | 否 | 页面标题（可选，默认从 slug 推断） |
| `--content` | string | 否 | 直接输入的文本内容（与 --file 二选一） |
| `--file` | path | 否 | 从文本文件读取内容（与 --content 二选一，仅支持 txt/md/csv/json/yaml 等纯文本格式，最大 1MB） |
| `--intent` | string | 否 | 意图: memory(默认, 稳定脑页+可选KB+低风险自动应用), evidence(仅KB证据), promote(影子页+KB+候选变更) |
| `--force` | flag | 否 | 强制覆盖已被人工修改的页面（默认不覆盖，返回冲突信息） |
| `--dry-run` | flag | 否 | 仅返回路由计划，不实际写入 |

### `gbrain upload`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | path | 是 | 本地文件路径 |
| `--intent` | string | 否 | 上传意图: auto(默认), evidence(别名 document), memory, attachment, promote |
| `--target` | string | 否 | 目标 gbrain 页面 slug（用于生成建议变更） |
| `--page` | string | 否 | 关联页面 slug（用于附件） |
| `--library` | integer | 否 | KB 库 ID |
| `--folder` | integer | 否 | KB 文件夹 ID |
| `--promotion` | string | 否 | 提升策略: none, shadow, candidate, auto-low-risk(alias auto) |
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

### `gbrain kb OCR`

| 命令/参数 | 类型 | 必填 | 说明 |
|-----------|------|------|------|
| `ocr-status <doc_id>` | integer | 是 | 查看指定 KB 文档的 OCR 状态 |
| `ocr-run <doc_id>` | integer | 是 | 手动触发或排队指定文档的 OCR |
| `ocr-retry <doc_id>` | integer | 是 | 重试失败或输出为空的 OCR 页 |
| `--pages <RANGES>` | string | 否 | 运行或重试的页码范围，如 `1,3,5-10` |

### `gbrain review list`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `--status` | string | 否 | 过滤状态: pending, accepted, rejected, applied, rolled_back |
| `--target` | string | 否 | 过滤目标页面 slug |
| `--limit` | integer | 否 | 最大结果数（默认 50） |

### `gbrain review show`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |

### `gbrain review apply`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |

### `gbrain review reject`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |
| `--reason` | string | 否 | 拒绝原因 |

### `gbrain review rollback`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `change_id` | integer | 是 | 变更 ID |

### `gbrain config`

| 子命令 | 说明 |
|--------|------|
| `gbrain config show` | 显示常用配置值（15项核心配置的快速概览） |
| `gbrain config get <key>` | 获取单个配置值（可查询以下全部 29 项 key） |
| `gbrain config set <key> <value>` | 设置配置值（另支持两个仅可设置的 OCR 阈值 key） |

> **注意:** `config show` 仅显示最常用的 15 个核心 key；`config get <key>` 支持查询下表中列出的全部 29 个配置项。`config set` 还接受 `ocr_text_density_threshold` 与 `ocr_timeout_seconds_per_page`，它们会写入 `config.json`，但当前不能通过 `config get` 读取。

**可用配置 key：**

| Key | 类型 | 说明 | 默认值 |
|-----|------|------|--------|
| `embedding_model` | string | 嵌入模型名称 | `text-embedding-3-large` |
| `embedding_dimensions` | integer | 嵌入向量维度 | `1536` |
| `expansion_model` | string | 查询扩展 LLM 模型 | `gpt-4o-mini` |
| `chunker_model` | string | LLM 分块模型（预留；也作为 RAPTOR 回退模型） | `gpt-4o-mini` |
| `chunk_size` | integer | 分块大小（字符数） | `500` |
| `chunk_overlap` | integer | 分块重叠（字符数） | `50` |
| `log_level` | string | 日志级别（trace/debug/info/warn/error） | `info` |
| `log_to_file` | boolean | 启用文件日志 | `true` |
| `log_to_console` | boolean | 启用控制台日志 | `true` |
| `auto_link` | boolean | 写入时自动提取链接 | `true` |
| `auto_timeline` | boolean | 写入时自动提取时间线 | `true` |
| `post_write_lint` | boolean | 写入后运行 validators 检查并记录日志 | `false` |
| `kb_enabled` | boolean | 启用 KB 子系统 | `true` |
| `kb_raptor_model` | string | KB RAPTOR LLM 模型 | `gpt-4o-mini` |
| `kb_max_file_size_mb` | integer | KB 文件大小上限（MB） | `50` |
| `kb_worker_enabled` | boolean | 启用 KB 后台 worker | `true` |
| `kb_worker_poll_interval_secs` | integer | KB worker 轮询间隔（秒） | `30` |
| `upload_default_promotion_policy` | string | 上传默认提升策略: none/shadow/candidate/auto-low-risk | `candidate` |
| `artifact_default_intent` | string | artifact 默认意图: memory/evidence/promote | `memory` |
| `artifact_auto_create_inbox_library` | boolean | artifact_put 无 Inbox 库时自动创建 | `true` |
| `artifact_manual_memory_to_kb` | boolean | memory 意图是否写入 KB | `true` |
| `autopilot_enabled` | boolean | 启用 autopilot 后台维护线程 | `true` |
| `autopilot_interval_secs` | integer | autopilot 维护间隔（秒，最小60） | `3600` |
| `ocr_enabled` | boolean | 启用 PDF OCR | `true` |
| `ocr_base_url` | string | GLM-OCR endpoint | `https://open.bigmodel.cn/api/paas/v4/layout_parsing` |
| `ocr_model` | string | OCR 模型 | `glm-ocr` |
| `ocr_mode` | string | OCR 范围策略: auto/all_pages | `auto` |
| `ocr_submit_mode` | string | OCR 提交策略: pdf_first/pdf_range | `pdf_first` |
| `ocr_profile` | string | OCR 场景: general/table/formula/handwriting | `general` |

**示例：**

```bash
# 查看所有配置
gbrain config show

# 获取单个配置
gbrain config get embedding_model

# 设置配置
gbrain config set chunk_size 800
gbrain config set log_level debug
gbrain config set upload_default_promotion_policy auto-low-risk
```

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

gbrain 通过 MCP 标准协议暴露 Artifact 统一知识操作 facade 工具（`artifact_*`）及 KB OCR 扩展工具，供 AI 智能体集成使用。

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

### KB OCR 扩展

| 工具 | 说明 |
|------|------|
| `kb_document_status` | 查看 KB 文档处理与 OCR 状态 |
| `kb_ocr_run` | 为 KB 文档手动触发或排队 OCR |
| `kb_ocr_retry` | 重试失败或输出为空的 OCR 页 |

#### 示例

```jsonc
// ===== 写入 =====
// 写入手动记忆
{ "tool": "artifact_put", "params": { "slug": "rust-async", "content": "Rust 异步编程使用 async/await 语法...", "intent": "memory" } }

// 从文件写入
{ "tool": "artifact_put", "params": { "slug": "docs/guide", "file": "/path/to/guide.md", "intent": "evidence" } }

// 预览写入路由
{ "tool": "artifact_put", "params": { "slug": "test", "content": "...", "dry_run": true } }

// 强制覆盖
{ "tool": "artifact_put", "params": { "slug": "people/alice", "content": "更新内容", "force": true } }
```

```jsonc
// ===== 上传 =====
// 上传文档（默认自动路由）
{ "tool": "artifact_upload", "params": { "path": "/path/to/report.pdf", "intent": "auto" } }

// 上传作为证据
{ "tool": "artifact_upload", "params": { "path": "/path/to/doc.pdf", "intent": "evidence", "library_id": 1, "folder_id": 2 } }

// 上传并生成建议变更
{ "tool": "artifact_upload", "params": { "path": "/path/to/doc.pdf", "intent": "promote", "target_slug": "people/alice", "promotion": "candidate" } }

// 上传作为附件
{ "tool": "artifact_upload", "params": { "path": "/path/to/image.png", "intent": "attachment", "page_slug": "people/alice" } }

// 预览上传路由
{ "tool": "artifact_upload", "params": { "path": "/path/to/data.csv", "dry_run": true } }
```

```jsonc
// ===== 查询 =====
// 统一知识查询
{ "tool": "artifact_query", "params": { "query": "Rust 异步编程", "mode": "auto", "limit": 10 } }

// 查询含来源追溯
{ "tool": "artifact_query", "params": { "query": "Rust 异步编程", "mode": "memory", "include_sources": true } }

// 查询 KB 证据
{ "tool": "artifact_query", "params": { "query": "市场分析", "mode": "evidence" } }

// 按时间线查询
{ "tool": "artifact_query", "params": { "query": "最近活动", "mode": "timeline" } }

// 过滤到特定页面
{ "tool": "artifact_query", "params": { "query": "性能优化", "filter_slug": "tech/rust" } }
```

```jsonc
// ===== 查看 =====
// 列出知识源
{ "tool": "artifact_list", "params": { "limit": 20, "offset": 0 } }

// 获取知识源详情（含投影和来源）
{ "tool": "artifact_get", "params": { "id_or_uid": "art_abc123", "include_sources": true, "include_projections": true } }

// 通过 ID 获取
{ "tool": "artifact_get", "params": { "id_or_uid": "1" } }
```

```jsonc
// ===== 生命周期管理 =====
// 预览删除影响
{ "tool": "artifact_delete", "params": { "id_or_uid": "5", "dry_run": true } }

// 软删除
{ "tool": "artifact_delete", "params": { "id_or_uid": "5" } }

// 解除知识源与页面的关联
{ "tool": "artifact_detach", "params": { "id_or_uid": "5", "from": "people/alice" } }

// 恢复已删除的知识源
{ "tool": "artifact_restore", "params": { "id_or_uid": "5" } }

// 预览恢复影响
{ "tool": "artifact_restore", "params": { "id_or_uid": "5", "dry_run": true } }

// 重新处理知识源
{ "tool": "artifact_reprocess", "params": { "id_or_uid": "5" } }

// 健康检查
{ "tool": "artifact_health", "params": {} }
```

```jsonc
// ===== 变更审核 =====
// 列出待审核的建议变更
{ "tool": "artifact_review_list", "params": { "status": "pending" } }

// 按目标和状态过滤（5 种状态：pending / accepted / rejected / applied / rolled_back）
{ "tool": "artifact_review_list", "params": { "status": "applied", "target_slug": "people/alice", "limit": 50 } }

// 查看建议变更详情（含证据、风险等级和来源追溯）
{ "tool": "artifact_review_get", "params": { "change_id": 1 } }

// 应用建议变更（写入 gbrain 长期记忆 + 创建来源追溯）
{ "tool": "artifact_review_apply", "params": { "change_id": 1 } }

// 拒绝建议变更（可选注明原因）
{ "tool": "artifact_review_reject", "params": { "change_id": 2, "reason": "信息已过时" } }

// 回滚已应用的建议变更（撤销影子页更新 + 标记来源追溯为 stale）
{ "tool": "artifact_review_rollback", "params": { "change_id": 1 } }
```

### 写入意图说明

`artifact_put` 和 `artifact_upload` 通过 `intent` 参数控制知识进入系统的方式：

| 工具 | intent 可选值 | 默认值 | 行为说明 |
|------|-------------|--------|---------|
| `artifact_put` | `memory` / `evidence` / `promote` | `memory` | memory=稳定脑页+可选KB，evidence=仅KB证据不建脑页，promote=影子页+KB+候选变更 |
| `artifact_upload` | `auto` / `evidence`(别名`document`) / `memory` / `attachment` / `promote` | `auto` | auto=按文件类型自动路由，evidence=KB文档证据，memory=整理进记忆，attachment=仅附件，promote=明确提升含候选 |

### 提升策略说明

`artifact_upload` 的 `promotion` 参数控制从 KB 证据生成建议变更的自动化程度：

| 策略值 | 别名 | 说明 |
|--------|------|------|
| `none` | — | 不自动提升，不生成影子页或候选变更 |
| `shadow` | — | 仅创建影子页，不生成候选变更 |
| `candidate` | — | 生成候选变更，需人工审核（默认行为） |
| `auto-low-risk` | — | 自动接受低风险候选（实体提及、链接建议等），高风险仍需审核 |

---

## MCP 工具参数

### `artifact_put`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 目标页面 slug（如 people/alice） |
| `content` | string | 否 | 直接输入的文本内容（与 file 二选一） |
| `file` | string | 否 | 本地文本文件路径（与 content 二选一，仅支持 txt/md/csv/json/yaml 等纯文本格式，上限 1MB） |
| `title` | string | 否 | 页面标题（可选，默认从 slug 推断） |
| `intent` | string | 否 | 意图: memory(默认, 稳定脑页+可选KB+低风险自动应用) / evidence(仅KB证据) / promote(影子页+KB+候选变更) |
| `force` | boolean | 否 | 强制覆盖已被人工修改的页面（默认 false，冲突时返回 resolution=conflict） |
| `dry_run` | boolean | 否 | 仅返回路由计划，不实际写入 |

### `artifact_upload`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | string | 是 | 本地文件路径 |
| `intent` | string | 否 | 上传意图: auto / evidence(别名 document) / memory / attachment / promote（默认 auto） |
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
| `include_content` | boolean | 否 | 包含原始内容 |

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

### `kb_document_status`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | KB 文档 ID |

### `kb_ocr_run`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | KB 文档 ID |
| `pages` | string | 否 | 页码范围，如 `1,3,5-10` |

### `kb_ocr_retry`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | KB 文档 ID |
| `pages` | string | 否 | 仅重试指定页码范围 |

### 已知限制

- **`artifact_query` mode=graph** 尚未实现。代码图谱（符号定义/引用/调用关系）查询暂不可用。

---

## 环境变量

> **API 兼容性说明**: 除 PDF OCR 使用智谱 GLM-OCR `layout_parsing` endpoint 外，本项目的通用模型与音频调用仅支持 OpenAI 兼容格式的 API（`/embeddings`、`/chat/completions`、`/audio/transcriptions`），不支持 Anthropic/Claude API。通过设置相应 `*_BASE_URL` 可接入兼容服务；OCR endpoint 由单独的安全开关控制。

### LLM 配置分组

LLM 配置按调用类型和功能拆分：

| 配置组 | 环境变量 | 适用功能 |
|--------|----------|----------|
| **Embeddings** | `GBRAIN_OPENAI_API_KEY` / `GBRAIN_OPENAI_BASE_URL` / `GBRAIN_EMBEDDING_MODEL` | 文档分块嵌入（向量化）、语义分块（段落相似度切分）、查询向量 |
| **查询扩展 / 重排序** | `GBRAIN_EXPANSION_API_KEY` / `GBRAIN_EXPANSION_BASE_URL` / `GBRAIN_EXPANSION_MODEL` | 查询扩展、通过 chat/completions 进行搜索重排序 |
| **KB RAPTOR** | 库级 `raptor_llm_*`、`GBRAIN_KB_RAPTOR_*`、`GBRAIN_EXPANSION_*`、`GBRAIN_CHUNKER_*` | RAPTOR 树摘要 |
| **LLM 分块（预留）** | `GBRAIN_CHUNKER_API_KEY` / `GBRAIN_CHUNKER_BASE_URL` / `GBRAIN_CHUNKER_MODEL` | 预留给 LLM 引导分块；当前 KB 文档处理流程未接入，同时可作为 RAPTOR 回退配置 |
| **PDF OCR（GLM-OCR）** | `GBRAIN_OCR_API_KEY` / `GBRAIN_OCR_BASE_URL` / `GBRAIN_OCR_MODEL` | PDF 页级版面识别、文本回写与重嵌入 |

### API Key 回退链

各模块的 API Key 按以下优先级回退：

```
嵌入向量:     GBRAIN_OPENAI_API_KEY
查询扩展:     GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
LLM 分块（预留）: GBRAIN_CHUNKER_API_KEY → GBRAIN_OPENAI_API_KEY
KB RAPTOR:    库级 raptor_llm_secret_ref → GBRAIN_KB_RAPTOR_API_KEY → GBRAIN_EXPANSION_API_KEY → GBRAIN_CHUNKER_API_KEY
搜索重排序:   GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
PDF OCR:      GBRAIN_OCR_API_KEY → ZHIPU_API_KEY
```

设置 `GBRAIN_OPENAI_API_KEY` 可启用嵌入向量、查询扩展和搜索重排序（使用 OpenAI 兼容默认端点/模型）。RAPTOR 需要库级/KB 级 RAPTOR secret，或配置 `GBRAIN_EXPANSION_API_KEY` / `GBRAIN_CHUNKER_API_KEY`。PDF OCR 使用独立的 GLM-OCR 凭证，不会回退到 `GBRAIN_OPENAI_API_KEY`。

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

### 查询扩展 / 重排序（Chat Completions）

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_EXPANSION_API_KEY` | 查询扩展、搜索重排序的 LLM API 密钥 | 回退到 `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_EXPANSION_BASE_URL` | 查询扩展、搜索重排序的 LLM 基础 URL | 回退到 `GBRAIN_OPENAI_BASE_URL`；未设置时使用 OpenAI 默认端点 |
| `GBRAIN_EXPANSION_MODEL` | 查询扩展、搜索重排序的 LLM 模型 | `gpt-4o-mini` |

### LLM 分块

当前 KB 文档处理流程使用 Markdown/递归分块，或基于 Embeddings 的语义分块；尚未接入 `src/chunker/llm.rs` 中的 LLM 引导分块器。以下变量为预留配置，其中 `GBRAIN_CHUNKER_*` 也会作为 RAPTOR 的最后一级回退配置。

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_CHUNKER_API_KEY` | LLM 分块 API 密钥 | 回退到 `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_CHUNKER_BASE_URL` | LLM 分块基础 URL | 回退到 `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_CHUNKER_MODEL` | LLM 分块模型 | `gpt-4o-mini` |

### 音频转录

转录能力由库模块提供，当前没有对应的 CLI 或 MCP 工具。

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_TRANSCRIPTION_PROVIDER` | 转录服务提供商（`groq` / `openai`） | `groq` |
| `GBRAIN_TRANSCRIPTION_GROQ_API_KEY` | Groq 转录 API 密钥 | — |
| `GBRAIN_TRANSCRIPTION_GROQ_BASE_URL` | Groq 转录基础 URL | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_API_KEY` | OpenAI 转录 API 密钥 | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_BASE_URL` | OpenAI 转录基础 URL | — |

### PDF OCR（GLM-OCR）

PDF 文档解析后会进行页级 OCR 判定。`auto` 模式仅将文本层为空或低密度、含图片/矢量或批注风险、含不可见或疑似乱码文本、内容解析失败或页面尺寸无法确认的页面送入 OCR；判定结果会记录为 `ocr_scope`（`none` / `partial` / `full`）、`needs_ocr_pages` 和 `ocr_reasons_by_page`。

外部 OCR 还受库级隐私策略约束：只有 `external_ocr_allowed=true` 且 `redaction_enabled=false` 时，PDF 页面才会发送至外部 OCR 服务。否则系统保留可提取的原始文本层，不向服务端提交文件。

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_OCR_ENABLED` | 启用 PDF OCR 总开关 | `true` |
| `GBRAIN_OCR_EXTERNAL_ALLOWED_DEFAULT` | 新建 KB 库默认允许外部 OCR | `true` |
| `GBRAIN_OCR_API_KEY` | GLM-OCR API 密钥；仅从环境变量读取 | — |
| `ZHIPU_API_KEY` | OCR API 密钥兼容别名，仅在 `GBRAIN_OCR_API_KEY` 为空时使用 | — |
| `GBRAIN_OCR_BASE_URL` | GLM-OCR endpoint；自定义地址还必须显式开启下方安全开关 | `https://open.bigmodel.cn/api/paas/v4/layout_parsing` |
| `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL` | 允许使用自定义 OCR endpoint；仅接受环境变量设置 | `false` |
| `GBRAIN_OCR_MODEL` / `GLMOCR_MODEL` | OCR 模型名称，前者优先 | `glm-ocr` |
| `GBRAIN_OCR_PROFILE` | 后处理 profile：`general` / `table` / `formula` / `handwriting`；不发送给 API | `general` |
| `GBRAIN_OCR_ENABLE_LAYOUT` / `GLMOCR_ENABLE_LAYOUT` | 请求版面识别，前者优先 | `true` |
| `GBRAIN_OCR_MODE` | OCR 选择模式：`auto` / `all_pages` | `auto` |
| `GBRAIN_OCR_SUBMIT_MODE` | PDF 提交模式：`pdf_first` / `pdf_range` | `pdf_first` |
| `GBRAIN_OCR_SYNC_INLINE` | 内联执行 OCR；默认使用后台异步 job | `false` |
| `GBRAIN_OCR_TEXT_DENSITY_THRESHOLD` | 低文本密度页字符数阈值 | `50` |
| `GBRAIN_OCR_MIN_LOW_DENSITY_RATIO` | 低密度比例兼容配置；当前只保留统计信息，不否决单页 OCR 选择 | `0.5` |
| `GBRAIN_OCR_IMAGE_AREA_THRESHOLD` | 图片面积覆盖率触发阈值 | `0.08` |
| `GBRAIN_OCR_IMAGE_COUNT_THRESHOLD` | 嵌入图片数量触发阈值 | `1` |
| `GBRAIN_OCR_TIMEOUT_SECONDS_PER_PAGE` / `GLM_OCR_TIMEOUT` / `GLMOCR_TIMEOUT` | 单页请求超时秒数，按列出顺序优先 | `60` |
| `GBRAIN_OCR_MAX_PAGES_PER_REQUEST` | 单次 OCR 请求最大页数 | `100` |
| `GBRAIN_OCR_MAX_PDF_BYTES_PER_REQUEST` | 单次 OCR 请求最大 PDF 字节数 | `52,428,800`（50 MiB） |
| `GBRAIN_OCR_MAX_PAGES_PER_DOCUMENT` | 单文档最多尝试 OCR 的页数；`0` 会校正为 `1` | `300` |
| `GBRAIN_OCR_MAX_CONCURRENCY` | 进程内 OCR 并发工作上限；`0` 会校正为 `1` | `2` |
| `GBRAIN_OCR_TEMP_DIR_MAX_BYTES` | OCR 拆分页临时文件的进程内总字节预算 | `536,870,912`（512 MiB） |
| `GBRAIN_OCR_RETURN_CROP_IMAGES` | 请求返回裁剪图片 | `false` |
| `GBRAIN_OCR_NEED_LAYOUT_VISUALIZATION` | 请求返回版面可视化结果 | `false` |

**安全说明：**

- `GBRAIN_OCR_API_KEY` 和 `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL` 仅从环境变量生效；配置文件不能打开自定义 endpoint 放行门。
- 未显式设置 `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL=true` 时，即使配置了 `GBRAIN_OCR_BASE_URL`，请求仍会发往官方默认 endpoint。
- 启用自定义 endpoint 时会写入审计警告；日志中的 endpoint 会移除 userinfo、query 和 fragment，错误及持久化 OCR 响应也会对 API key 等敏感值进行脱敏。

### KB 子系统

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_KB_ENABLED` | 启用 KB 子系统 | `true` |
| `GBRAIN_KB_RAPTOR_API_KEY` | KB RAPTOR LLM API 密钥 | 通过默认 `kb_raptor_secret_ref` 使用；之后回退到 `GBRAIN_EXPANSION_API_KEY`，再回退到 `GBRAIN_CHUNKER_API_KEY` |
| `GBRAIN_KB_RAPTOR_BASE_URL` | KB RAPTOR LLM 基础 URL | 回退到 `GBRAIN_EXPANSION_BASE_URL`，再回退到 `GBRAIN_CHUNKER_BASE_URL`，最后使用 OpenAI 默认端点 |
| `GBRAIN_KB_RAPTOR_MODEL` | KB RAPTOR LLM 模型 | `gpt-4o-mini`；若 KB/库级模型为空，resolver 可使用 `GBRAIN_EXPANSION_MODEL`，再使用 `GBRAIN_CHUNKER_MODEL` |
| `GBRAIN_KB_MAX_FILE_SIZE_MB` | KB 文件大小上限（MB） | `50` |
| `GBRAIN_KB_ALLOWED_EXTENSIONS` | KB 允许的文件扩展名（逗号分隔） | `pdf,docx,xlsx,csv,html,htm,txt,md,markdown,rst,json,xml,yaml,yml,toml,tsv` |
| `GBRAIN_KB_STORAGE_DIR` | KB 文件存储目录 | — |
| `GBRAIN_KB_WORKER_ENABLED` | 启用 KB 后台处理 worker | `true` |
| `GBRAIN_KB_WORKER_POLL_INTERVAL` | KB worker 轮询间隔（秒） | `30` |
| `GBRAIN_AUTOPILOT_ENABLED` | 启用 autopilot 后台维护线程（`gbrain serve` 时生效） | `true` |
| `GBRAIN_AUTOPILOT_INTERVAL` | autopilot 维护间隔（秒，默认 3600 = 1 小时，最小60秒） | `3600` |
| `GBRAIN_KB_SYNONYMS_FILE` | 同义词文件路径（用于搜索查询扩展） | — |
| `GBRAIN_KB_ALIASES_FILE` | 别名映射文件路径（用于搜索查询扩展） | — |

**KB 子系统 LLM 用途说明：**

| 功能 | LLM 类型 | API Key / Base URL | 使用模型 |
|------|----------|-------------------|----------|
| 文档分块嵌入（向量化） | Embeddings API | `GBRAIN_OPENAI_API_KEY` / `GBRAIN_OPENAI_BASE_URL` | `GBRAIN_EMBEDDING_MODEL` |
| 语义分块（段落相似度切分） | Embeddings API | `GBRAIN_OPENAI_API_KEY` / `GBRAIN_OPENAI_BASE_URL` | `GBRAIN_EMBEDDING_MODEL` |
| PDF 页级 OCR 与文本回写 | GLM-OCR `layout_parsing` | `GBRAIN_OCR_API_KEY` / `GBRAIN_OCR_BASE_URL`（自定义地址需 `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL=true`） | `GBRAIN_OCR_MODEL` |
| RAPTOR 层级摘要 | Chat Completions | 库级 `raptor_llm_*` → `GBRAIN_KB_RAPTOR_*` → `GBRAIN_EXPANSION_*` → `GBRAIN_CHUNKER_*` | 库级/KB 模型 → `GBRAIN_EXPANSION_MODEL` → `GBRAIN_CHUNKER_MODEL` → 未设置 KB 模型时使用 `gpt-4o-mini` |
| 搜索重排序（Rerank） | Chat Completions | `GBRAIN_EXPANSION_API_KEY` / `GBRAIN_EXPANSION_BASE_URL`，回退到 `GBRAIN_OPENAI_*` | `GBRAIN_EXPANSION_MODEL` / `expansion_model` → `gpt-4o-mini` |

### Artifact 融合架构

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GBRAIN_ARTIFACT_STORAGE_DIR` | Artifact 原件存储目录 | `$GBRAIN_DIR/artifacts` |
| `GBRAIN_DEFAULT_KB_LIBRARY_ID` | 默认 KB 库 ID | — |
| `GBRAIN_UPLOAD_PROMOTION_POLICY` | 上传默认提升策略: none/shadow/candidate/auto-low-risk | `candidate` |
| `GBRAIN_ARTIFACT_DEFAULT_INTENT` | artifact 默认意图: memory/evidence/promote | `memory` |
| `GBRAIN_ARTIFACT_AUTO_CREATE_INBOX_LIBRARY` | artifact_put 无 Inbox 库时自动创建 | `true` |
| `GBRAIN_ARTIFACT_MANUAL_MEMORY_TO_KB` | memory 意图是否写入 KB（设为 `false` 仅写入 gbrain 页面） | `true` |

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
| `GBRAIN_POST_WRITE_LINT` | 写入后运行 validators 检查并记录日志 | `false` |

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

`BrainWriter` 库 API 提供三种页面写入策略。当前 `gbrain put` / `artifact_put` 不提供 `mode` 参数；`post_write_lint` 仅运行 `validators` 检查并写日志，不会执行下方 `lint.rs` 的六条自动修复规则。

| 模式 | 说明 |
|------|------|
| `Strict` | 严格校验——要求 frontmatter、禁止空内容、检查链接引用有效性 |
| `Lint` | 零 LLM 质量检查——运行 6 条规则，自动修复可修复的问题 |
| `Off` | 自由写入——跳过所有校验，直接写入 |

### Lint 规则

`lint.rs` 中已实现的规则（目前未作为 CLI/MCP 命令暴露）：

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

Artifact 删除遵循软删除机制，防止误删并支持恢复：

```
正常页面 ──delete──→ 软删除状态（仍占存储，不可查询）
                        │
                        ├──restore──→ 恢复为正常页面
                        │
                        └──purge-deleted──→ 永久删除（释放存储）[^purge-note]
```

[^purge-note]: `purge-deleted`（永久清理）功能引擎层已实现，暂未暴露为独立 CLI 命令。当前需通过健康检查识别僵尸数据后手动清理。

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

Tree-sitter 索引在代码页面写入或调用 `Operations::reindex_code_page` 时执行。`gbrain upload --intent auto` 对代码扩展名只识别为应走既有 code import/sync 流程，不会自动建立 KB 或代码图投影；当前 `artifact_query` 也未暴露 `graph` 模式。

---

## 测试

```bash
cargo test                    # 所有测试
cargo test --test engine_test # 引擎集成测试
cargo test --test search_test # 搜索集成测试
cargo test --test fuzzy_test  # 模糊匹配测试
cargo test --test dedup_test  # 去重测试
cargo test --test artifact_facade_test  # Artifact 统一接口测试
cargo clippy                  # 代码检查
```

测试使用内存 SQLite（`:memory:`），无需额外配置。

---

## 架构

三层设计:

1. **引擎层** — `BrainEngine` trait → `SqliteEngine`（SQLite + FTS5；sqlite-vec 可用时加速向量检索，否则使用 BLOB 回退）。同步，直接数据库操作。

2. **操作层** — 业务逻辑：自动分块、标签提取、链接推断、安全验证、批量操作。

3. **接口层** — CLI + MCP 服务器。CLI 使用 `remote=false`；MCP 对不受信任的调用者设置 `remote=true`。

### 搜索流水线

底层 hybrid 搜索流水线（供内部或程序化搜索路径使用；公开的 `gbrain query` / `artifact_query` facade 当前不生成查询向量或 LLM 查询扩展）：

1. FTS5 BM25 关键词搜索（权重: 标题 10x, compiled_truth 5x, 时间线 2x）
2. 可选向量搜索（sqlite-vec 可用时使用扩展，否则读取 BLOB 回退）
3. 向量结果不足 3 条时补充扩展 OR 关键词查询
4. RRF 融合（k=60）和归一化，支持已提供的扩展向量结果列表
5. compiled_truth 加权提升
6. 已提供查询向量时，将余弦相似度与归一化 RRF 分数混合重排
7. 反向链接提升
8. 时效性提升（时间衰减）
9. 意图类型提升（实体/时间/事件）
10. 可选两阶段代码图扩展（`walk_depth` / `near_symbol`）
11. 6 层去重（slug top-3 → 跨源去重 → 文本相似度 → 类型多样性 → 每页上限 → compiled_truth 保证）

### KB 子系统架构

异步五阶段文档处理管线:

1. **解析** — 文档解析器（Markdown / PDF / DOCX / XLSX / CSV / HTML / 纯文本）；代码图索引走独立页面流程
2. **拆分** — 递归拆分器；库启用 `semantic_enabled` 且具备嵌入能力时可使用语义拆分器（Savitzky-Golay 平滑 + chunk_overlap 重叠）
3. **嵌入** — 向量嵌入生成与持久化
4. **RAPTOR（新库默认开启）** — `raptor_enabled=true` 且满足运行条件时构建递归摘要树（K-Means++ 聚类 + LLM 摘要，四级回退链：库级配置 → `GBRAIN_KB_RAPTOR_*` → `GBRAIN_EXPANSION_*` → `GBRAIN_CHUNKER_*`）
5. **持久化** — 事务保护的节点/向量写入

`raptor_enabled` 存放在每条 `kb_libraries` 记录中；新建库及自动创建的 `Inbox` 库默认均为 `true`。启用后，文档节点少于 3 个、库禁止外部摘要或无法解析 RAPTOR LLM 凭据时，处理管线会自动跳过摘要树构建。当前 `gbrain config`、CLI 的 `kb` 子命令和 MCP 工具均未暴露关闭开关。

对于 PDF，解析阶段会先生成页级 OCR 判定：空/低密度文本层、图片与矢量内容、批注 appearance 风险、隐藏文本、字体编码疑似异常以及解析/几何不确定页都会保守进入候选集合。需要 OCR 时，系统默认排入异步 OCR job，识别完成后回写文本并触发重嵌入；仅在 `GBRAIN_OCR_SYNC_INLINE=true` 时在处理管线中内联执行。

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
      ├─ KB Document 投影 → 文档处理管线（解析→拆分→嵌入→默认开启的 RAPTOR→持久化）
      ├─ Shadow Page 投影 → 影子页面（提取内容生成 wiki 页面）
      ├─ Brain Page Update 投影 → 已有页面更新
      ├─ File Attachment 投影 → 文件附件（简单文件引用）
      └─ Promotion Candidate → 候选变更（可包含链接/时间线建议）
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

这四种是内部 Memory Query 策略；CLI/MCP facade 当前仅暴露 `auto` / `memory` / `evidence` / `timeline` 模式，不提供 `provenance` 或 `graph` 模式。

---

## 文档

- [TS vs Rust 对比报告](./docs/compare_report.md) / [English](./docs/compare_report_en.md) — TypeScript 与 Rust 版本的全面对比（代码规模、数据库、搜索、MCP、安全等）
- [TS vs Rust 模块级详细对比](./docs/module_detail.md) / [English](./docs/module_detail_en.md) — 逐模块对比（引擎层、操作层、搜索、分块、富化、验证器等）

---

## 许可证

MIT License
