# gbrain-rs

[English](./README_EN.md) | 中文

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**个人知识大脑引擎** — 基于 [gbrain](https://github.com/garrytan/gbrain) 的 Rust 移植，新增单入口多投影融合架构（Artifact 原件 → KB/影子页面/候选变更/附件多投影 + 溯源审计 + 回滚）、KB 子系统（异步文档处理管线 + RAPTOR 递归摘要树）、中文 NLP 全链路支持（jieba 分词 + 拼音 + FTS5 查询重构）、软删除生命周期（restore/purge-deleted）、时间衰减搜索等特性。基于 SQLite + sqlite-vec + FTS5 构建的零配置嵌入式架构，开箱即用。

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
- **单入口多投影融合** — 原件上传（Upload）自动路由至多投影（KB 文档 / 影子页面 / 候选变更 / 文件附件 / 链接 / 时间线），溯源审计（Provenance Ledger），候选评审与提升（Promotion），版本链与回滚（Projection Supersede / Rollback），统一记忆查询（Memory Query，4 种策略）
- **MCP 服务器** — 完整的模型上下文协议（JSON-RPC 2.0）服务器，74 个工具，用于 AI 智能体集成
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

### 核心

| 命令 | 说明 |
|------|------|
| `gbrain init` | 初始化新的知识库 |
| `gbrain get <slug>` | 按 slug 读取页面 |
| `gbrain put <slug> --title <TITLE> [--content <TEXT> \| --file <PATH>] [--page-type <TYPE>]` | 创建或更新页面 |
| `gbrain delete <slug> [--force]` | 软删除页面 |
| `gbrain restore <slug>` | 恢复软删除页面 |
| `gbrain purge-deleted [--older-than-hours <N>]` | 永久清理旧的软删除页面 |
| `gbrain list [--page-type <TYPE>] [--limit <N>]` | 列出页面（可筛选） |
| `gbrain query <query> [--limit <N>] [--detail <LEVEL>] [--lang <LANG>] [--symbol-kind <KIND>] [--near-symbol <SYMBOL>] [--walk-depth <DEPTH>] [--expand]` | 混合搜索（别名: `ask`），支持代码过滤和两阶段检索 |

#### 示例

```bash
# 初始化知识库
gbrain init

# 创建页面
gbrain put people/alice --title "Alice" --content "一位工程师，擅长 Rust 和系统编程"

# 搜索
gbrain query "Rust 异步编程" --limit 5 --lang rust

# 恢复误删页面
gbrain restore people/alice

# 更新页面内容
gbrain put people/alice --title "Alice" --content "一位资深工程师，擅长 Rust 和系统编程，目前就职于 Acme Corp"

# 从文件创建页面
gbrain put projects/alpha --title "Project Alpha" --file ./notes/alpha.md

# 创建带类型的页面
gbrain put people/bob --title "Bob" --content "产品经理" --page-type person

# 搜索代码相关内容
gbrain query "异步运行时" --lang rust --limit 10

# 搜索并获取详细信息
gbrain query "Acme Corp" --detail high --limit 5

# 两阶段代码图检索
gbrain query "handle_request" --near-symbol "HttpHandler" --walk-depth 1

# 关键词搜索（不扩展）
gbrain query "Rust" --limit 5 --expand false

# 列出特定类型的页面
gbrain list --page-type person --limit 20

# 永久清理 7 天前的删除页面
gbrain purge-deleted --older-than-hours 168
```

### 搜索与图谱

| 命令 | 说明 |
|------|------|
| `gbrain resolve <partial>` | 模糊解析部分 slug |
| `gbrain graph <slug> [--depth <N>]` | 从页面遍历知识图谱 |
| `gbrain graph-query <from> [--to <slug>] [--depth <N>] [--link-type <TYPE>]` | 查询页面间的图谱关系 |
| `gbrain code search/def/refs/callers/callees/edges` | 代码 chunk、符号定义/引用和调用图查询 |

#### 示例

```bash
# 模糊解析 slug
gbrain resolve ali

# 遍历知识图谱
gbrain graph people/alice --depth 3

# 查询两个页面间的关系路径
gbrain graph-query people/alice --to companies/acme --depth 3

# 查找 Rust 函数定义
gbrain code def --symbol "parse_config" --lang rust

# 查询图谱关系路径
gbrain graph-query people/alice --to projects/alpha --depth 2

# 按链接类型查询
gbrain graph-query people/alice --link-type works_at --depth 1

# 查找代码符号引用
gbrain code refs --symbol "SqliteEngine" --lang rust

# 搜索代码块
gbrain code search "async fn handle" --lang rust

# 查看调用者
gbrain code callers --slug src/engine.rs --symbol "query"

# 查看被调用者
gbrain code callees --slug src/engine.rs --symbol "query"

# 重建代码页索引
gbrain code reindex src/engine.rs
```

### 反向链接

| 命令 | 说明 |
|------|------|
| `gbrain backlinks list <slug>` | 列出页面的反向链接 |
| `gbrain backlinks check [slug]` | 检查缺失的反向链接 |
| `gbrain backlinks fix [slug]` | 修复缺失的反向链接 |

#### 示例

```bash
# 列出反向链接
gbrain backlinks list people/alice

# 检查所有页面的反向链接完整性
gbrain backlinks check

# 修复缺失的反向链接
gbrain backlinks fix people/alice
```

### 数据管理

| 命令 | 说明 |
|------|------|
| `gbrain embed [slugs...] [--batch-size <N>]` | 为 stale chunks 生成并持久化嵌入向量 |
| `gbrain import <dir> [--embed] [--auto-link]` | 导入 Markdown 和支持的代码文件；frontmatter slug 与路径不一致时跳过 |
| `gbrain export [slugs...] [--dir <DIR>] [--page-type <TYPE>]` | 导出页面为 Markdown |
| `gbrain extract [--mode links\|timeline\|all]` | 批量提取链接/时间线 |
| `gbrain lint [slug] [--fix] [--dry-run]` | 零 LLM 质量检查（6 条规则） |

#### 示例

```bash
# 导入目录并生成嵌入
gbrain import ./my-notes --embed --auto-link

# 导出所有页面
gbrain export --dir ./backup

# 批量提取链接和时间线
gbrain extract --mode all

# 运行 lint 检查并自动修复
gbrain lint --fix

# 导出特定页面
gbrain export people/alice companies/acme --dir ./backup

# 按类型导出
gbrain export --dir ./people-backup --page-type person

# 仅提取链接
gbrain extract --mode links

# 仅提取时间线
gbrain extract --mode timeline

# 检查页面质量（不修复）
gbrain lint people/alice

# 预览 lint 修复
gbrain lint --fix --dry-run
```

### 文件存储

| 命令 | 说明 |
|------|------|
| `gbrain file upload <path> [--page <slug>]` | 上传文件 |
| `gbrain file list [slug]` | 列出已存储的文件 |
| `gbrain file sync <dir>` | 同步目录到存储 |
| `gbrain file verify` | 验证所有文件记录 |
| `gbrain file url <storage-path>` | 获取文件的本地路径/URL |

#### 示例

```bash
# 上传文件并关联到页面
gbrain file upload report.pdf --page projects/annual-report

# 列出某页面关联的文件
gbrain file list projects/annual-report

# 获取文件路径
gbrain file url files/report.pdf

# 同步目录到存储
gbrain file sync ./documents

# 验证所有文件记录
gbrain file verify

# 列出所有文件
gbrain file list
```

### 健康与维护

| 命令 | 说明 |
|------|------|
| `gbrain stats` | 知识库统计信息 |
| `gbrain health` | 健康仪表盘 |
| `gbrain doctor [--fast]` | 综合诊断 |
| `gbrain integrity` | 检查数据完整性 |
| `gbrain orphans` | 检测孤立页面 |
| `gbrain autopilot [--once] [--interval <SECS>]` | 自维护守护进程 |

#### 示例

```bash
# 查看知识库统计
gbrain stats

# 快速诊断
gbrain doctor --fast

# 运行一次自维护
gbrain autopilot --once

# 持续自维护（每 10 分钟）
gbrain autopilot --interval 600

# 查看健康仪表盘
gbrain health

# 检查数据完整性
gbrain integrity

# 检测孤立页面
gbrain orphans

# 为特定页面生成嵌入
gbrain embed people/alice companies/acme --batch-size 10
```

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

#### 示例

```bash
# 查看所有配置
gbrain config show

# 设置嵌入模型
gbrain config set embedding_model text-embedding-3-large

# 生成维护报告
gbrain report --report-type maintenance --title "Weekly Check"

# 启动 MCP 服务器
gbrain serve

# 获取单个配置值
gbrain config get embedding_model

# 设置写入后自动 lint
gbrain config set post_write_lint true

# 查看导入日志
gbrain ingest-log --limit 10

# 输出 MCP 工具定义
gbrain tools-json
```

### KB 子系统（CLI）

| 命令 | 说明 |
|------|------|
| `gbrain kb-worker [--once] [--interval <SECS>]` | 运行 KB 文档处理工作器（从队列领取作业） |
| `gbrain kb-eval --library-id <ID>` | 运行 KB 搜索评估 |
| `gbrain kb-backup --output <DIR>` | 备份 KB 数据库和存储 |
| `gbrain kb-restore --input <DIR>` | 从备份恢复 KB |
| `gbrain kb-source-add --library-id <ID> --path <DIR>` | 添加本地目录为 KB 导入源 |
| `gbrain kb-sync-source --source-id <ID>` | 同步 KB 导入源 |
| `gbrain kb-jobs list/pause/resume --library-id <ID>` | KB 作业管理（列出/暂停/恢复） |
| `gbrain kb-export-library --library-id <ID> --output <DIR>` | 导出 KB 库到目录 |
| `gbrain kb-import-library --archive <DIR> [--new-name <NAME>]` | 从导出导入 KB 库 |
| `gbrain kb-reembed --library-id <ID> [--embedding-index-id <ID>]` | 重新嵌入文档（使用新模型） |
| `gbrain kb-eval-compare --index-id-1 <ID> --index-id-2 <ID>` | 比较两个嵌入索引的搜索质量 |
| `gbrain kb-health-check [--library-id <ID>] [--repair]` | 检查 KB 索引健康状态（可选修复） |
| `gbrain kb-rebuild-document --document-id <ID>` | 重建单个文档索引 |
| `gbrain kb-rebuild-library --library-id <ID>` | 重建整个库索引 |
| `gbrain kb-purge-deleted [--library-id <ID>] [--older-than-days <N>]` | 清理已删除的 KB 文档 |

#### 示例

```bash
# 启动 KB 工作器（持续运行）
gbrain kb-worker --interval 30

# 运行一次 KB 工作器
gbrain kb-worker --once

# 备份 KB
gbrain kb-backup --output ./kb-backup

# 从备份恢复
gbrain kb-restore --input ./kb-backup

# 添加导入源并同步
gbrain kb-source-add --library-id 1 --path ./documents
gbrain kb-sync-source --source-id 1

# 导出/导入库
gbrain kb-export-library --library-id 1 --output ./export
gbrain kb-import-library --archive ./export --new-name "Restored Library"

# 重新嵌入文档
gbrain kb-reembed --library-id 1

# 比较两个嵌入索引
gbrain kb-eval-compare --index-id-1 1 --index-id-2 2

# 检查并修复索引
gbrain kb-health-check --library-id 1 --repair

# 重建单个文档索引
gbrain kb-rebuild-document --document-id 42

# 清理 30 天前的已删除文档
gbrain kb-purge-deleted --older-than-days 30
```

### 单入口多投影融合（Artifact）

| 命令 | 说明 |
|------|------|
| `gbrain upload <path> [--intent <INTENT>] [--library-id <ID>] [--target <SLUG>] [--page <SLUG>] [--folder-id <ID>] [--promotion <POLICY>] [--dry-run]` | 上传原件（统一入口），自动路由至多投影。intent: auto/document/attachment/memory/promote；promotion: none/shadow/candidate/auto-low-risk |
| `gbrain memory-query <query> [--strategy <STRATEGY>] [--limit <N>] [--filter-slug <SLUG>] [--include-evidence] [--include-provenance]` | 统一记忆查询（别名: ask-memory）。strategy: brain_first/evidence_first/provenance/timeline_first |
| `gbrain artifact list [--limit <N>] [--offset <N>]` | 列出所有 Artifact |
| `gbrain artifact get <id_or_uid>` | 获取 Artifact 详情（支持 ID 或 UID 如 `art_ab12cd34ef56`） |
| `gbrain artifact delete <artifact_id>` | 软删除 Artifact（标记所有投影为 stale） |
| `gbrain artifact health` | 检查 Artifact 投影一致性与健康状态 |

#### 示例

```bash
# 上传文档自动路由
gbrain upload report.pdf --intent document --library-id 1

# 上传并自动提升低风险候选
gbrain upload notes.md --intent promote --target people/alice --promotion auto-low-risk

# 预览上传路由（不实际执行）
gbrain upload data.xlsx --intent auto --dry-run

# 统一记忆查询
gbrain memory-query "Alice 的项目经历" --strategy evidence_first --limit 10

# 查看 Artifact 详情
gbrain artifact get art_ab12cd34ef56

# 上传文件附件
gbrain upload photo.jpg --intent attachment --page people/alice

# 上传到 KB 指定文件夹
gbrain upload report.pdf --intent document --library-id 1 --folder-id 5

# 统一记忆查询 — 优先搜索 KB 证据
gbrain memory-query "部署流程" --strategy evidence_first --limit 5

# 统一记忆查询 — 追溯事实来源
gbrain memory-query "Alice 的职位" --strategy provenance --include-provenance

# 统一记忆查询 — 按时间线排序
gbrain memory-query "最近的项目进展" --strategy timeline_first

# 列出所有 Artifact
gbrain artifact list --limit 20

# 检查 Artifact 健康状态
gbrain artifact health

# 软删除 Artifact
gbrain artifact delete 42
```

### 候选变更与提升（Promotion）

| 命令 | 说明 |
|------|------|
| `gbrain promotion list [--status <STATUS>] [--candidate-type <TYPE>] [--target-slug <SLUG>]` | 列出候选变更 |
| `gbrain promotion get <candidate_id>` | 获取候选变更详情 |
| `gbrain promotion accept <candidate_id> [--reviewer <NAME>] [--notes <TEXT>]` | 接受候选变更 |
| `gbrain promotion reject <candidate_id> [--reviewer <NAME>] [--notes <TEXT>]` | 拒绝候选变更 |
| `gbrain promotion apply <candidate_id>` | 应用已接受的候选变更到 gbrain |
| `gbrain promotion auto-apply <artifact_id>` | 自动应用低风险候选变更 |
| `gbrain promotion batch-apply [--artifact-id <ID>] [--risk <LEVEL>] [--dry-run]` | 批量应用候选变更 |
| `gbrain promotion rollback <candidate_id>` | 回滚已应用的候选变更 |

#### 示例

```bash
# 列出待审候选变更
gbrain promotion list --status pending

# 接受候选变更
gbrain promotion accept 42 --reviewer alice --notes "信息准确"

# 批量应用低风险候选（预览模式）
gbrain promotion batch-apply --risk low --dry-run

# 回滚已应用的候选
gbrain promotion rollback 42

# 查看候选详情
gbrain promotion get 42

# 拒绝候选变更
gbrain promotion reject 43 --reviewer bob --notes "信息已过时"

# 应用已接受的候选
gbrain promotion apply 42

# 自动应用低风险候选
gbrain promotion auto-apply 7

# 列出已应用的候选
gbrain promotion list --status applied
```

### 投影管理（Projection）

| 命令 | 说明 |
|------|------|
| `gbrain projection supersede <old_proj_id> <new_proj_id>` | 用新投影替代旧投影（版本链） |
| `gbrain projection history <projection_key> [--artifact-id <ID>] [--projection-type <TYPE>] [--limit <N>]` | 查询投影版本链历史 |
| `gbrain gc-orphan-projections [--stale-days <N>] [--dry-run]` | 清理孤立/过期的投影记录 |

#### 示例

```bash
# 替代旧投影
gbrain projection supersede 101 202

# 查询投影版本链
gbrain projection history kb_doc:42 --artifact-id 7 --limit 10

# 预览清理孤立投影
gbrain gc-orphan-projections --stale-days 30 --dry-run
```

---

## CLI 命令参数

### `gbrain put`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug（如 people/alice） |
| `--title` | string | 是 | 页面标题 |
| `--content` | string | 否 | 页面内容（Markdown） |
| `--file` | path | 否 | 从文件读取内容 |
| `--page-type` | string | 否 | 页面类型（如 person、company） |

### `gbrain query`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 搜索查询 |
| `--limit` | integer | 否 | 最大结果数（默认 20） |
| `--detail` | string | 否 | 结果详情级别：low/medium/high（默认 medium） |
| `--lang` | string | 否 | 按编程语言过滤代码检索 |
| `--symbol-kind` | string | 否 | 按符号类型过滤代码检索 |
| `--near-symbol` | string | 否 | 锚定两阶段代码图检索的起始符号 |
| `--walk-depth` | integer | 否 | 代码图邻居遍历深度（0-2，默认 0） |
| `--expand` | flag | 否 | 启用 LLM 查询扩展 |

### `gbrain upload`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | path | 是 | 文件路径 |
| `--intent` | string | 否 | 上传意图：auto/document/attachment/memory/promote（默认 auto） |
| `--library-id` | integer | 否 | KB 知识库 ID |
| `--target` | string | 否 | 目标 gbrain 页面 slug（用于提升） |
| `--page` | string | 否 | 目标页面 slug（用于文件附件） |
| `--folder-id` | integer | 否 | KB 文件夹 ID |
| `--promotion` | string | 否 | 提升策略：none/shadow/candidate/auto-low-risk |
| `--dry-run` | flag | 否 | 仅返回路由计划，不实际执行 |
| `--json` | flag | 否 | 以 JSON 格式输出 |

### `gbrain memory-query`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 查询文本 |
| `--strategy` | string | 否 | 查询策略：brain_first/evidence_first/provenance/timeline_first（默认 brain_first） |
| `--limit` | integer | 否 | 最大结果数（默认 10） |
| `--filter-slug` | string | 否 | 按 slug 过滤 |
| `--include-evidence` | flag | 否 | 包含 KB 证据（默认 true） |
| `--include-provenance` | flag | 否 | 包含溯源记录（默认 false） |
| `--json` | flag | 否 | 以 JSON 格式输出 |

### `gbrain embed`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slugs` | string[] | 否 | 指定页面 slug（空 = 所有过期内容） |
| `--batch-size` | integer | 否 | 嵌入 API 批量大小（默认 20） |

### `gbrain import`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `dir` | string | 是 | 扫描 .md 文件的目录 |
| `--embed` | flag | 否 | 为导入内容生成嵌入 |
| `--auto-link` | flag | 否 | 自动链接导入页面到已有页面 |

### `gbrain export`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slugs` | string[] | 否 | 指定导出的页面 slug |
| `--dir` | string | 否 | 输出目录 |
| `--page-type` | string | 否 | 按页面类型筛选 |

### `gbrain autopilot`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `--once` | flag | 否 | 运行一次后退出 |
| `--interval` | integer | 否 | 循环间隔秒数（默认 3600） |

### `gbrain purge-deleted`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `--older-than-hours` | integer | 否 | 清理多少小时前的删除页面（默认 72） |

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

gbrain 提供 74 个 MCP 工具，通过 stdio 上的 JSON-RPC 2.0 协议供 AI 智能体集成使用。

### 搜索

| 工具 | 说明 |
|------|------|
| `query` | 混合搜索（向量 + 关键词 + 扩展），支持详情级别、代码过滤、两阶段检索和搜索元数据 |
| `find_by_title_fuzzy` | 基于三元组相似度的标题模糊搜索 |
| `resolve_slugs` | 模糊解析部分 slug 到匹配页面 |

#### 示例

```json
// MCP 调用示例
{ "tool": "query", "params": { "query": "Rust 异步编程", "limit": 5, "lang": "rust" } }
```

```json
// 标题模糊搜索
{ "tool": "find_by_title_fuzzy", "params": { "query": "Alice", "min_similarity": 0.6 } }
```

```json
// 模糊解析 slug
{ "tool": "resolve_slugs", "params": { "partial": "ali" } }
```

```json
// 搜索并返回元数据
{ "tool": "query", "params": { "query": "Rust 异步", "limit": 5, "include_meta": true } }
```

```json
// 两阶段代码图检索
{ "tool": "query", "params": { "query": "handle_request", "near_symbol": "HttpHandler", "walk_depth": 1 } }
```

### 页面增删改查

| 工具 | 说明 |
|------|------|
| `get_page` | 读取页面（支持模糊匹配） |
| `put_page` | 写入/更新页面（Markdown + frontmatter） |
| `delete_page` | 软删除页面（需 confirm=true 确认） |
| `list_pages` | 列出页面（支持类型/标签/数量筛选） |
| `get_chunks` | 获取页面的内容块 |

#### 示例

```json
// 读取页面
{ "tool": "get_page", "params": { "slug": "people/alice" } }
```

```json
// 创建/更新页面
{ "tool": "put_page", "params": { "slug": "people/alice", "content": "---\ntitle: Alice\n---\n一位工程师" } }
```

```json
// 软删除页面（需确认）
{ "tool": "delete_page", "params": { "slug": "people/alice", "confirm": true } }
```

```json
// 列出页面
{ "tool": "list_pages", "params": { "type": "person", "limit": 10 } }
```

```json
// 模糊匹配读取页面
{ "tool": "get_page", "params": { "slug": "ali", "fuzzy": true } }
```

```json
// 获取页面内容块
{ "tool": "get_chunks", "params": { "slug": "people/alice" } }
```

### 标签

| 工具 | 说明 |
|------|------|
| `add_tag` | 为页面添加标签 |
| `remove_tag` | 从页面移除标签 |
| `get_tags` | 列出页面的标签 |

#### 示例

```json
// 添加标签
{ "tool": "add_tag", "params": { "slug": "people/alice", "tag": "engineer" } }
```

```json
// 列出标签
{ "tool": "get_tags", "params": { "slug": "people/alice" } }
```

```json
// 移除标签
{ "tool": "remove_tag", "params": { "slug": "people/alice", "tag": "engineer" } }
```

### 链接与图谱

| 工具 | 说明 |
|------|------|
| `add_link` | 创建页面间的类型化链接 |
| `remove_link` | 移除页面间的链接 |
| `get_links` | 列出页面的出站链接 |
| `get_backlinks` | 列出页面的入站链接 |
| `traverse_graph` | 从页面遍历链接图谱 |

#### 示例

```json
// 创建类型化链接
{ "tool": "add_link", "params": { "from": "people/alice", "to": "companies/acme", "link_type": "works_at" } }
```

```json
// 遍历图谱
{ "tool": "traverse_graph", "params": { "slug": "people/alice", "depth": 3, "direction": "both" } }
```

```json
// 获取反向链接
{ "tool": "get_backlinks", "params": { "slug": "people/alice" } }
```

```json
// 移除链接
{ "tool": "remove_link", "params": { "from": "people/alice", "to": "companies/acme", "link_type": "works_at" } }
```

```json
// 按链接类型遍历图谱
{ "tool": "traverse_graph", "params": { "slug": "people/alice", "depth": 3, "link_type": "works_at", "direction": "out" } }
```

### 时间线

| 工具 | 说明 |
|------|------|
| `add_timeline_entry` | 为页面添加时间线条目 |
| `get_timeline` | 获取页面的时间线 |

#### 示例

```json
// 添加时间线
{ "tool": "add_timeline_entry", "params": { "slug": "people/alice", "date": "2024-01-15", "summary": "加入 Acme 公司" } }
```

```json
// 获取时间线
{ "tool": "get_timeline", "params": { "slug": "people/alice" } }
```

### 版本管理

| 工具 | 说明 |
|------|------|
| `get_versions` | 页面版本历史 |
| `revert_version` | 将页面回滚到先前版本 |

#### 示例

```json
// 查看版本历史
{ "tool": "get_versions", "params": { "slug": "people/alice" } }
```

```json
// 回滚到指定版本（创建新版本记录）
{ "tool": "revert_version", "params": { "slug": "people/alice", "version_id": 3 } }
```

### 原始数据

| 工具 | 说明 |
|------|------|
| `put_raw_data` | 存储页面的原始 API 响应数据 |
| `get_raw_data` | 获取页面的原始数据 |

#### 示例

```json
// 存储原始数据
{ "tool": "put_raw_data", "params": { "slug": "companies/acme", "source": "crustdata", "data": { "founded": "2020" } } }
```

```json
// 获取原始数据
{ "tool": "get_raw_data", "params": { "slug": "companies/acme", "source": "crustdata" } }
```

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

#### 示例

```json
// 查找符号定义
{ "tool": "code_def", "params": { "symbol": "parse_config", "lang": "rust" } }
```

```json
// 查找符号引用
{ "tool": "code_refs", "params": { "symbol": "SqliteEngine", "lang": "rust" } }
```

```json
// 获取调用者
{ "tool": "get_callers", "params": { "slug": "src/engine.rs", "symbol": "query" } }
```

```json
// 重建代码页索引
{ "tool": "reindex_code_page", "params": { "slug": "src/engine.rs" } }
```

```json
// 搜索代码块
{ "tool": "search_code_chunks", "params": { "query": "async fn handle", "lang": "rust" } }
```

```json
// 获取代码边
{ "tool": "get_code_edges_by_chunk", "params": { "chunk_id": 42 } }
```

### 文件存储

| 工具 | 说明 |
|------|------|
| `file_upload` | 上传文件到存储 |
| `file_list` | 列出已存储的文件 |
| `file_url` | 获取文件的 URL/路径 |

#### 示例

```json
// 上传文件
{ "tool": "file_upload", "params": { "path": "/path/to/report.pdf", "page_slug": "projects/annual-report" } }
```

```json
// 列出文件
{ "tool": "file_list", "params": { "slug": "projects/annual-report" } }
```

### 导入与同步

| 工具 | 说明 |
|------|------|
| `log_ingest` | 记录导入事件 |
| `get_ingest_log` | 获取最近的导入日志 |
| `sync_brain` | 从 Git 仓库同步知识库 |
| `find_orphans` | 查找无入站链接的孤立页面 |

#### 示例

```json
// 同步 Git 仓库
{ "tool": "sync_brain", "params": { "repo_path": "/path/to/repo", "force_full": false } }
```

```json
// 查找孤立页面
{ "tool": "find_orphans", "params": { "include_pseudo": false } }
```

```json
// 获取导入日志
{ "tool": "get_ingest_log", "params": { "limit": 10 } }
```

### 健康与统计

| 工具 | 说明 |
|------|------|
| `get_stats` | 知识库统计（页面数、块数等） |
| `get_health` | 健康仪表盘（嵌入覆盖率、孤立页面等） |

#### 示例

```json
// 获取统计信息
{ "tool": "get_stats", "params": {} }
```

```json
// 获取健康状态
{ "tool": "get_health", "params": {} }
```

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

#### 示例

```json
// 列出知识库
{ "tool": "kb_list_libraries", "params": {} }
```

```json
// 创建知识库
{ "tool": "kb_create_library", "params": { "name": "项目文档", "raptor_enabled": true, "embedding_model": "text-embedding-3-large" } }
```

```json
// 上传文档
{ "tool": "kb_upload_document", "params": { "library_id": 1, "file_path": "/path/to/doc.pdf" } }
```

```json
// KB 搜索
{ "tool": "kb_search", "params": { "query": "部署流程", "library_ids": [1], "top_k": 10, "profile": "accurate" } }
```

```json
// 备份与恢复
{ "tool": "kb_backup", "params": { "output": "/path/to/backup" } }
```

### 单入口多投影融合（Artifact）

| 工具 | 说明 |
|------|------|
| `upload_source` | 上传原件（统一入口），自动创建 Artifact、KB 投影、影子页面和文件附件 |
| `memory_query` | 统一记忆查询，同时搜索 gbrain 策展知识和 KB 文档证据，4 种策略自动选择 |
| `artifact_list` | 列出所有 Artifact |
| `artifact_get` | 获取 Artifact 详情（支持 ID 或 UID） |
| `artifact_delete` | 软删除 Artifact（标记所有投影为 stale） |
| `artifact_health` | 检查 Artifact 投影一致性与健康状态 |
| `get_provenance` | 获取页面的溯源记录（追踪事实来源） |

#### 示例

```json
// 上传原件
{ "tool": "upload_source", "params": { "path": "/path/to/report.pdf", "intent": "document", "library_id": 1 } }
```

```json
// 统一记忆查询
{ "tool": "memory_query", "params": { "query": "Alice 的项目经历", "strategy": "evidence_first", "limit": 10 } }
```

```json
// 获取溯源记录
{ "tool": "get_provenance", "params": { "brain_slug": "people/alice" } }
```

```json
// 查看 Artifact 健康状态
{ "tool": "artifact_health", "params": {} }
```

```json
// 上传文件附件
{ "tool": "upload_source", "params": { "path": "/path/to/photo.jpg", "intent": "attachment", "page_slug": "people/alice" } }
```

```json
// 预览上传路由
{ "tool": "upload_source", "params": { "path": "/path/to/report.pdf", "intent": "auto", "dry_run": true } }
```

```json
// 记忆查询 — 追溯来源
{ "tool": "memory_query", "params": { "query": "Alice 的职位", "strategy": "provenance", "include_provenance": true } }
```

### 候选变更与提升（Promotion）

| 工具 | 说明 |
|------|------|
| `promotion_list_candidates` | 列出候选变更（从 KB 证据提取的建议修改） |
| `promotion_get_candidate` | 获取候选变更详情 |
| `promotion_accept_candidate` | 接受候选变更 |
| `promotion_reject_candidate` | 拒绝候选变更（含 reason 参数说明拒绝原因） |
| `promotion_apply_candidate` | 应用已接受的候选变更到 gbrain |
| `promotion_batch_apply` | 批量应用候选变更，可按 artifact 和风险等级筛选 |
| `promotion_rollback_candidate` | 回滚已应用的候选变更，撤销影子页面更新并标记溯源为 stale |

#### 示例

```json
// 列出待审候选
{ "tool": "promotion_list_candidates", "params": { "status": "pending", "limit": 20 } }
```

```json
// 接受候选
{ "tool": "promotion_accept_candidate", "params": { "candidate_id": 42, "reviewer": "alice", "notes": "信息准确" } }
```

```json
// 拒绝候选（含原因）
{ "tool": "promotion_reject_candidate", "params": { "candidate_id": 43, "reason": "信息过时" } }
```

```json
// 批量应用低风险候选（预览）
{ "tool": "promotion_batch_apply", "params": { "risk": "low", "dry_run": true } }
```

```json
// 回滚候选
{ "tool": "promotion_rollback_candidate", "params": { "candidate_id": 42 } }
```

### 投影管理（Projection）

| 工具 | 说明 |
|------|------|
| `gc_orphan_projections` | 清理孤立/过期的投影记录 |
| `projection_supersede` | 用新投影替代旧投影（版本链） |
| `projection_history` | 查询投影版本链历史 |

#### 示例

```json
// 替代旧投影
{ "tool": "projection_supersede", "params": { "old_proj_id": 101, "new_proj_id": 202 } }
```

```json
// 查询投影版本链
{ "tool": "projection_history", "params": { "projection_key": "kb_doc:42", "artifact_id": 7, "limit": 10 } }
```

```json
// 清理孤立投影（预览）
{ "tool": "gc_orphan_projections", "params": { "stale_days": 30, "dry_run": true } }
```

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

### `get_page`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `fuzzy` | boolean | 否 | 启用模糊 slug 解析（默认 false） |

### `put_page`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `content` | string | 是 | 完整 Markdown（含 YAML frontmatter） |

### `add_link`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `from` | string | 是 | 源页面 slug |
| `to` | string | 是 | 目标页面 slug |
| `link_type` | string | 否 | 链接类型（如 works_at、invested_in） |
| `context` | string | 否 | 链接上下文描述 |

### `list_pages`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `type` | string | 否 | 按页面类型筛选 |
| `tag` | string | 否 | 按标签筛选 |
| `limit` | integer | 否 | 最大结果数（默认 50） |

### `delete_page`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `confirm` | boolean | 是 | 必须为 true 以确认删除 |

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
| `embedding_provider` | string | 否 | 嵌入提供商名称 |
| `embedding_model` | string | 否 | 嵌入模型名称 |
| `embedding_dimensions` | integer | 否 | 嵌入向量维度 |
| `search_profile` | string | 否 | 搜索配置名称 |
| `rerank_enabled` | boolean | 否 | 启用重排序 |
| `rerank_provider` | string | 否 | 重排序提供商名称 |
| `summary_enabled` | boolean | 否 | 启用摘要 |
| `external_embedding_allowed` | boolean | 否 | 允许外部嵌入调用 |
| `external_rerank_allowed` | boolean | 否 | 允许外部重排序调用 |
| `external_summary_allowed` | boolean | 否 | 允许外部摘要调用 |
| `external_ocr_allowed` | boolean | 否 | 允许外部 OCR 调用 |
| `redaction_enabled` | boolean | 否 | 启用敏感内容脱敏 |

### `kb_list_documents`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `library_id` | integer | 是 | 知识库 ID |
| `folder_id` | integer | 否 | 按文件夹筛选文档 |
| `limit` | integer | 否 | 最大结果数（默认 50） |
| `offset` | integer | 否 | 分页偏移量 |

### `kb_search`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 搜索查询 |
| `library_ids` | integer[] | 否 | 限定搜索的知识库 ID（空 = 全部） |
| `level` | integer | 否 | RAPTOR 树层级过滤 |
| `top_k` | integer | 否 | 最大结果数（默认 10，上限 50） |
| `profile` | string | 否 | 搜索配置: fast/balanced/accurate/file_lookup/table |
| `debug` | boolean | 否 | 启用调试模式（返回规划器/重排序/回退信息） |
| `include_context` | boolean | 否 | 包含匹配节点前后上下文 |
| `context_before` | integer | 否 | 匹配前上下文字符数（默认 200） |
| `context_after` | integer | 否 | 匹配后上下文字符数（默认 200） |
| `include_highlights` | boolean | 否 | 返回高亮字符范围 |
| `group_by_document` | boolean | 否 | 按文档分组结果 |
| `folder_id` | integer | 否 | 按文件夹筛选 |
| `embedding_dimensions` | integer | 否 | 覆盖查询向量的嵌入维度 |
| `embedding_index_id` | integer | 否 | 使用特定嵌入索引的模型配置 |

### `upload_source`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | string | 是 | 本地文件路径 |
| `intent` | string | 否 | 上传意图: auto/document/attachment/memory/promote（默认 auto） |
| `library_id` | integer | 否 | KB 知识库 ID |
| `target_slug` | string | 否 | 目标 gbrain 页面 slug（用于提升） |
| `page_slug` | string | 否 | 目标页面 slug（用于文件附件） |
| `folder_id` | integer | 否 | KB 文件夹 ID |
| `promotion` | string | 否 | 提升策略: none/shadow/candidate/auto-low-risk |
| `dry_run` | boolean | 否 | 仅返回路由计划，不实际执行 |

### `memory_query`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 查询文本 |
| `strategy` | string | 否 | 查询策略: brain_first/evidence_first/provenance/timeline_first（默认 brain_first） |
| `limit` | integer | 否 | 最大结果数 |
| `filter_slug` | string | 否 | 按 slug 过滤（适用于所有策略） |
| `include_evidence` | boolean | 否 | 包含 KB 证据结果 |
| `include_provenance` | boolean | 否 | 包含溯源记录 |

### `promotion_list_candidates`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `status` | string | 否 | 按状态过滤: pending/accepted/rejected/applied/rolled_back/stale/superseded |
| `candidate_type` | string | 否 | 按类型过滤: document_summary/entity_mention/link_suggestion/timeline_event/fact_claim/page_create/page_update |
| `target_slug` | string | 否 | 按目标 slug 过滤 |
| `limit` | integer | 否 | 最大结果数 |

### `promotion_batch_apply`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `artifact_id` | integer | 否 | 按 Artifact ID 筛选 |
| `risk` | string | 否 | 按风险等级筛选: low/medium/high |
| `dry_run` | boolean | 否 | 仅预览，不实际应用 |

### `add_tag`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `tag` | string | 是 | 标签名称 |

### `remove_tag`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `tag` | string | 是 | 标签名称 |

### `add_timeline_entry`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `date` | string | 是 | 日期（YYYY-MM-DD） |
| `summary` | string | 是 | 事件摘要 |

### `remove_link`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `from` | string | 是 | 源页面 slug |
| `to` | string | 是 | 目标页面 slug |
| `link_type` | string | 否 | 要移除的链接类型 |

### `revert_version`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `version_id` | integer | 是 | 要回滚到的版本 ID |

### `put_raw_data`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `source` | string | 是 | 数据来源标识 |
| `data` | object | 是 | 原始数据对象 |

### `get_raw_data`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 页面 slug |
| `source` | string | 否 | 按来源筛选 |

### `resolve_slugs`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `partial` | string | 是 | 部分 slug |

### `log_ingest`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `source_type` | string | 是 | 来源类型（如 git、import、api） |
| `source_ref` | string | 是 | 来源引用 |
| `pages_updated` | string[] | 是 | 更新的页面 slug 列表 |
| `summary` | string | 是 | 导入摘要 |

### `kb_update_library`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `library_id` | integer | 是 | 要更新的知识库 ID |
| `name` | string | 否 | 新知识库名称 |
| `semantic_segmentation_enabled` | boolean | 否 | 启用语义分块 |
| `raptor_enabled` | boolean | 否 | 启用 RAPTOR 摘要树 |
| `chunk_size` | integer | 否 | 分块大小（字符数） |
| `chunk_overlap` | integer | 否 | 分块重叠（字符数） |
| `embedding_provider` | string | 否 | 嵌入提供商名称 |
| `embedding_model` | string | 否 | 嵌入模型名称 |
| `embedding_dimensions` | integer | 否 | 嵌入向量维度 |
| `search_profile` | string | 否 | 搜索配置名称 |
| `rerank_enabled` | boolean | 否 | 启用重排序 |
| `rerank_provider` | string | 否 | 重排序提供商名称 |
| `summary_enabled` | boolean | 否 | 启用摘要 |
| `redaction_enabled` | boolean | 否 | 启用敏感内容脱敏 |

### `kb_delete_library`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `library_id` | integer | 是 | 要删除的知识库 ID |
| `confirm` | boolean | 是 | 必须为 true 以确认删除 |

### `kb_upload_document`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `library_id` | integer | 是 | 目标知识库 ID |
| `file_path` | string | 是 | 本地文件路径 |
| `folder_id` | integer | 否 | 文件夹 ID |

### `kb_get_document_status`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | 文档 ID |

### `kb_retry_document`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | 要重试的文档 ID |

### `kb_cancel_document_job`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | 要取消的文档 ID |

### `kb_delete_document`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | 要删除的文档 ID |
| `confirm` | boolean | 是 | 必须为 true 以确认删除 |

### `kb_purge_document`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `document_id` | integer | 是 | 要永久销毁的文档 ID |
| `confirm` | boolean | 是 | 必须为 true 以确认永久销毁 |

### `kb_create_folder`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `library_id` | integer | 是 | 知识库 ID |
| `name` | string | 是 | 文件夹名称 |
| `parent_id` | integer | 否 | 父文件夹 ID（null = 根目录） |

### `kb_add_eval_query`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `library_id` | integer | 是 | 知识库 ID |
| `query` | string | 是 | 评估查询文本 |
| `query_type` | string | 否 | 查询类型分类 |
| `expected_document_ids` | string | 否 | 逗号分隔的期望文档 ID |

### `kb_add_search_feedback`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `search_log_id` | integer | 否 | 搜索日志 ID |
| `document_id` | integer | 否 | 被评分的文档 ID |
| `node_id` | integer | 否 | 被评分的节点 ID |
| `rating` | integer | 是 | 相关性评分 0-5 |
| `comment` | string | 否 | 反馈评论 |

### `code_def`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `symbol` | string | 是 | 限定或局部符号名 |
| `lang` | string | 否 | 按编程语言过滤 |
| `limit` | integer | 否 | 最大结果数（默认 20） |

### `code_refs`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `symbol` | string | 是 | 限定或局部符号名 |
| `lang` | string | 否 | 按编程语言过滤 |
| `limit` | integer | 否 | 最大结果数（默认 20） |

### `search_code_chunks`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `query` | string | 是 | 代码搜索查询 |
| `limit` | integer | 否 | 最大结果数（默认 20） |
| `lang` | string | 否 | 按编程语言过滤 |
| `symbol_kind` | string | 否 | 按符号类型过滤 |

### `get_callers`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 代码页面 slug |
| `symbol` | string | 是 | 限定或局部符号名 |

### `get_callees`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 代码页面 slug |
| `symbol` | string | 是 | 限定或局部符号名 |

### `get_code_edges_by_chunk`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `chunk_id` | integer | 是 | 代码块 ID |

### `reindex_code_page`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `slug` | string | 是 | 代码页面 slug |

### `file_upload`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | string | 是 | 本地文件路径 |
| `page_slug` | string | 否 | 关联的页面 slug |

### `promotion_get_candidate`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `candidate_id` | integer | 是 | 候选 ID |

### `promotion_accept_candidate`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `candidate_id` | integer | 是 | 候选 ID |
| `reviewer` | string | 否 | 审阅者名称 |
| `notes` | string | 否 | 审阅备注 |

### `promotion_reject_candidate`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `candidate_id` | integer | 是 | 候选 ID |
| `reviewer` | string | 否 | 审阅者名称 |
| `reason` | string | 否 | 拒绝原因 |

### `promotion_apply_candidate`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `candidate_id` | integer | 是 | 候选 ID |

### `promotion_rollback_candidate`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `candidate_id` | integer | 是 | 要回滚的候选 ID |

### `gc_orphan_projections`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `stale_days` | integer | 否 | 清理超过 N 天的孤立投影（默认 30） |
| `dry_run` | boolean | 否 | 仅预览，不实际清理 |

### `projection_supersede`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `old_proj_id` | integer | 是 | 旧投影 ID |
| `new_proj_id` | integer | 是 | 新投影 ID |

### `projection_history`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `projection_key` | string | 是 | 投影键名 |
| `artifact_id` | integer | 否 | 按 Artifact ID 过滤 |
| `projection_type` | string | 否 | 按投影类型过滤 |
| `limit` | integer | 否 | 最大记录数（默认 20） |

### `artifact_list`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `limit` | integer | 否 | 最大结果数 |
| `offset` | integer | 否 | 偏移量 |

### `artifact_get`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id_or_uid` | string | 是 | Artifact ID 或 UID（如 '1' 或 'art_ab12cd34ef56'） |

### `artifact_delete`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `artifact_id` | integer | 是 | Artifact ID |

### `get_provenance`

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `brain_slug` | string | 是 | Brain 页面 slug |

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

- **Artifact** — 上传的原始文件，含状态（active/deleted/purged）、来源类型（upload/sync/link/mcp）、上传意图（auto/document/attachment/memory/promote）
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
