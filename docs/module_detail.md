# gbrain TS vs Rust — 模块级详细对比

## 2026-05-04 更新摘要

本模块级对比的原始内容基于 2026-04-29。当前 Rust 版本已有以下变化：

- `BrainEngine` trait 为 59 个方法，新增 soft-delete restore/purge 与 stale chunk 查询接口。
- Engine 层已实现 `count_stale_chunks`、`list_stale_chunks`；CLI embed 与 Autopilot 会复用 stale chunk 路径。
- Operations 查询路径会生成 query embedding；`put_page` 会抽取 Markdown fenced code block 为 `fenced_code` chunk。
- Search 层已实现 source boost、hard exclude、include/exclude slug prefix，并提供 `chunk_embeddings` fallback vector scoring。
- Chunker 层仍没有完整 tree-sitter code chunker 和 code edge extractor，但已有轻量 fenced-code chunk 支持。
- Config 中仍没有完全等价的 TS `searchBoosts` 配置对象；Rust 当前使用内置 slug-prefix boost 规则。

[English version](./module_detail_en.md) | 中文

**日期**: 2026-05-04

---

## 1. Engine层

### TS: BrainEngine接口 (engine.ts, 1,721行)

```typescript
// 双引擎实现:
// - PGLiteEngine: 嵌入式WASM Postgres (浏览器/本地)
// - PostgresEngine: 远程Postgres (生产)

// 关键独有方法:
searchKeywordChunks(query, opts): Promise<CodeChunkResult[]>
countStaleChunks(): Promise<number>
listStaleChunks(limit): Promise<StaleChunkRow[]>
addCodeEdges(edges): Promise<number>
deleteCodeEdgesForChunks(chunkIds): Promise<number>
getCallersOf(slug, symbol): Promise<CodeEdgeResult[]>
getCalleesOf(slug, symbol): Promise<CodeEdgeResult[]>
getEdgesByChunk(chunkId): Promise<CodeEdgeResult[]>
withReservedConnection<T>(fn): Promise<T>  // advisory lock
```

### Rust: BrainEngine trait (engine.rs, 59方法)

```rust
// 单引擎实现: SqliteEngine (SQLite + FTS5 + sqlite-vec)
// NOT dyn-compatible — 使用具体类型

// 关键独有方法:
detect_dead_links(slug): Result<Vec<String>>
file_url_by_storage_path(storage_path): Result<Option<String>>
file_verify(file_id): Result<bool>
add_timeline_multi_batch(entries): Result<usize>
restore_page(slug): Result<bool>
purge_deleted_pages(older_than_hours): Result<Vec<String>>
count_stale_chunks(): Result<usize>
list_stale_chunks(limit): Result<Vec<StaleChunk>>
```

### 差异分析

| 方面 | TS | Rust |
|------|----|------|
| 引擎数量 | 2 (PGLite + Postgres) | 1 (SQLite) |
| dyn兼容 | ✓ (interface) | ✗ (trait) |
| 连接管理 | 连接池 + advisory lock | 单连接 |
| 代码边 | ✓ | ✗ |
| Stale chunks | ✓ | ✓ |
| 多源 | ✓ (source_id) | ✗ |

---

## 2. Operations层

### TS: Operations (operations.ts, 2,501行)

```typescript
// 关键独有方法:
extractAndEnrich(slug, content): Promise<EnrichResult>  // 批量+节流
reindexCodePage(slug): Promise<void>  // 代码页面重索引
reconcileLinks(slug): Promise<void>  // 批量重算链接
checkBacklinks(slug): Promise<BacklinkCheckResult>  // 查找缺失反向链接
publishPage(slug, opts): Promise<PublishResult>  // HTML导出+加密
```

### Rust: Operations (operations.rs, ~1,200行)

```rust
// 核心方法完整: put_page, get_page, delete_page, search, file_upload等
// 缺少: extractAndEnrich, reindexCodePage, reconcileLinks, checkBacklinks, publishPage
```

---

## 3. 搜索模块

### TS: search/ (7文件, ~2,300行)

```
search/
├── hybrid.ts          (721行) RRF融合 + source-boost + hard-exclude
├── sql-ranking.ts     (415行) Postgres SQL排名函数
├── source-boost.ts    (349行) Slug前缀加权
├── two-pass.ts        (325行) Cathedral II两遍检索
├── intent.ts          (276行) 查询意图分类
├── expansion.ts       (236行) 查询扩展
└── dedup.ts           (215行) 4层去重
```

### Rust: search/ (8文件, ~2,200行)

```
search/
├── hybrid.rs          (689行) RRF融合 + 时间衰减 + 余弦重评分
├── keyword.rs         (68行)  FTS5 BM25关键词搜索
├── vector.rs          (242行) sqlite-vec余弦相似度
├── intent.rs          (155行) 查询意图分类
├── expansion.rs       (236行) 查询扩展 + 注入防御
├── dedup.rs           (164行) 4层去重
├── fuzzy.rs           (317行) 字符三字符组Jaccard模糊搜索
└── eval.rs            (313行) 搜索质量评估框架
```

### 关键差异

| 方面 | TS | Rust |
|------|----|------|
| SQL排名 | sql-ranking.ts (415行, Postgres特定) | keyword.rs (68行, FTS5特定) |
| Source boost | ✓ (349行) | ✓（slug-prefix 加权） |
| Hard exclude | ✓ (SQL NOT LIKE) | ✓（include/exclude slug-prefix 过滤） |
| Two-pass retrieval | ✓ (325行) | ✗ |
| 模糊搜索 | pg_trgm (Postgres扩展) | 自实现 (317行) |
| 评估框架 | ✗ | ✓ (313行, P@k/R@k/MRR/nDCG@k) |
| 时间衰减 | ✗ | ✓ (hybrid.rs内) |
| 注入防御 | N/A (参数化查询) | ✓ (escape_fts_term) |

---

## 4. 分块模块

### TS: chunkers/ (5文件, ~1,890行)

```
chunkers/
├── recursive.ts       (211行) 递归文本分块
├── semantic.ts        (340行) 语义分块
├── llm.ts             (163行) LLM引导分块
├── code.ts            (1,050行) tree-sitter代码分块
└── edge-extractor.ts  (178行) 代码边提取
```

### Rust: chunker/ (3文件, ~1,361行 + operations.rs 轻量 fenced-code 抽取)

```
chunker/
├── mod.rs             (35行)  模块入口
├── recursive.rs       (366行) 递归文本分块
├── semantic.rs        (719行) 语义分块
└── llm.rs             (276行) LLM引导分块
```

### 关键差异

| 方面 | TS | Rust |
|------|----|------|
| 代码分块 | ✓ (tree-sitter, 1,050行) | 部分：Markdown fenced-code chunks |
| 边提取 | ✓ (178行) | ✗ |
| 限定名 | ✓ (109行) | ✗ |
| 语义分块 | 340行 | 719行 (更详细) |
| 递归分块 | 211行 | 366行 (更详细) |

---

## 5. 富化模块

### TS: enrichment/ (3文件, ~1,870行)

```
enrichment/
├── enrichment.ts      (1,059行) 富化管道
├── budget.ts          (478行)   API预算管理
└── completeness.ts    (333行)   完整性评分
```

### Rust: enrichment.rs (472行, 单文件)

```rust
// 包含: Tier分类, 实体检测, 自动富化, 标签/链接建议
// 分离到: completeness.rs (313行), budget.rs (422行)
```

### 关键差异

| 方面 | TS | Rust |
|------|----|------|
| extractAndEnrich | ✓ (批量+节流) | ✗ |
| 预算实现 | SELECT FOR UPDATE + TTL | TxGuard RAII + reserve/commit |
| 完整性评分 | 333行 | 313行 (基本一致) |
| 层级分类 | ✓ | ✓ (已修复Tier2可达性) |

---

## 6. 输出/验证模块

### TS: output/validators/ (4文件, ~590行)

```
output/validators/
├── back-link.ts       (150行) 反向链接验证
├── citation.ts        (180行) 引用格式验证
├── link.ts            (150行) 链接验证
└── triple-hr.ts       (110行) 三横线验证
```

### Rust: validators.rs (429行, 单文件)

```rust
// 包含: 反向链接验证, 链接验证, 来源引用验证, 链接对称性验证
// 缺少: citation.ts, triple-hr.ts
```

---

## 7. 存储模块

### TS: storage/ (3文件, ~1,560行)

```
storage/
├── file-storage.ts    (620行) 文件存储抽象
├── s3-storage.ts      (530行) S3兼容存储
└── supabase-storage.ts (410行) Supabase存储 + TUS上传
```

### Rust: file_storage.rs (331行, 单文件)

```rust
// 仅本地文件系统存储
// 缺少: S3, Supabase
// 独有: axum文件服务器 (可选feature)
```

---

## 8. Minions模块

### TS: minions/ (11文件, ~4,478行)

```
minions/
├── queue.ts           (1,281行) 作业队列
├── worker.ts          (513行)   Worker进程池
├── supervisor.ts      (630行)   进程管理+自动重启
├── subagent.ts        (710行)   LLM-in-loop工具调用
├── shell-handler.ts   (321行)   Shell命令执行
├── subagent-aggregator.ts (169行) 子代理聚合
├── plugin-loader.ts   (235行)   插件加载
├── rate-leases.ts     (152行)   速率租约
├── quiet-hours.ts     (94行)    静默时段
├── transcript.ts      (229行)   执行记录
└── audit/             (3文件)    审计处理器
```

### Rust: jobs.rs (422行, 单文件)

```rust
// 基本作业队列: submit, get, list, cancel, retry, complete, fail
// 缺少: Worker, Supervisor, Subagent, Shell, Plugin, RateLease, QuietHours, Transcript, Audit
```

---

## 9. MCP模块

### TS: mcp/ (4文件, ~686行)

```
mcp/
├── server.ts          (289行) HTTP+stdio双传输
├── dispatch.ts        (215行) 工具调度
├── tool-defs.ts       (132行) 工具定义
└── rate-limit.ts      (50行)  速率限制
```

### Rust: mcp/ (2文件, ~1,133行)

```
mcp/
├── mod.rs             (816行) stdio JSON-RPC服务器
└── tool_defs.rs       (317行) 工具定义
```

### 关键差异

| 方面 | TS | Rust |
|------|----|------|
| 传输方式 | HTTP + stdio | 仅stdio |
| 认证 | Bearer token | 无 |
| 速率限制 | IP + Token双层 | 无 |
| CORS | ✓ | 无 |
| 审计日志 | ✓ | 无 |
| 结构化错误 | OperationError(code/message/suggestion) | GBrainError枚举 |
| 参数验证 | 类型检查 | 基本验证 |
| 工具数量 | 37 | 32 |

---

## 10. Resolver模块

### TS: resolver/ (4文件, ~1,095行)

```
resolver/
├── interface.ts       (158行) Resolver接口定义
├── registry.ts        (151行) Resolver注册表
├── url-reachable.ts   (332行) URL可达性检测
└── handle-to-tweet.ts (428行) X/Twitter推文解析
```

### Rust: 完全缺失

Resolver模块在Rust中完全没有对应实现。这包括：
- 死链检测
- X/Twitter API集成
- 可扩展的resolver框架

---

## 11. Skillify/Skillpack模块

### TS: skillify/ (4文件, ~1,013行)

```
skillify/
├── skillify.ts        (310行) Skill脚手架/生成
├── skillify-check.ts  (360行) Skill验证
├── skillpack.ts       (440行) Skill包管理
└── skillpack-check.ts (228行) Skill包健康检查
```

### Rust: 完全缺失

Skill系统在Rust中没有对应实现。

---

## 12. Scaffold模块

### TS: scaffold.ts (236行)

```typescript
// 确定性引用构建器:
// - tweetCitation(url): 推文引用
// - emailCitation(subject, from, to, date): 邮件引用
// - meetingCitation(title, date, attendees): 会议引用
```

### Rust: scaffold.rs (154行)

```rust
// 基本一致的实现
// tweet_citation, email_citation, meeting_citation
// 已修复: 邮件主题中的markdown注入
```

---

## 13. Sync模块

### TS: sync.ts (~500行)

```typescript
// Git同步 + manifest追踪
// 支持多种Git URL格式
```

### Rust: sync.rs (839行)

```rust
// 更详细的实现
// 包含: validate_git_url (SSH注入防护), acknowledge_failures (原子写入)
// 已修复: 无冒号git@URL绕过
```

---

## 14. 配置模块

### TS: config.ts (~300行)

```typescript
interface BrainConfig {
  // 数据库
  databaseUrl?: string;
  pgliteDataDir?: string;

  // OpenAI
  openaiApiKey?: string;
  openaiBaseUrl?: string;
  embeddingModel?: string;
  embeddingDimensions?: number;

  // 搜索
  searchBoosts?: Record<string, number>;  // source-boost配置
  hardExclude?: string[];                  // 排除前缀

  // 存储
  storageBackend?: 'local' | 's3' | 'supabase';
  s3Config?: S3Config;
  supabaseConfig?: SupabaseConfig;

  // MCP
  mcpPort?: number;
  mcpAuthToken?: string;

  // Minions
  minionConcurrency?: number;
  minionQuietHours?: { start: string; end: string };

  // 其他
  autoLink?: boolean;
  autoTimeline?: boolean;
  postWriteLint?: boolean;
}
```

### Rust: config.rs (~200行)

```rust
pub struct Config {
    // 数据库
    pub db_path: String,
    pub gbrain_dir: String,

    // OpenAI
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub embedding_model: String,
    pub embedding_dimensions: usize,

    // 分块
    pub chunk_size: usize,
    pub chunk_overlap: usize,

    // 转录
    pub transcription_provider: String,
    pub transcription_groq_api_key: Option<String>,
    pub transcription_openai_api_key: Option<String>,

    // 其他
    pub auto_link: bool,
    pub auto_timeline: bool,
    pub post_write_lint: bool,
    pub search_debug: bool,

    // 日志
    pub log_level: String,
    pub log_to_file: bool,
    pub log_file_path: String,
    pub log_to_console: bool,
}
```

### 差异

| 配置项 | TS | Rust |
|--------|----|----|
| databaseUrl | ✓ | ✗ (用db_path) |
| storageBackend | ✓ (local/s3/supabase) | ✗ (仅local) |
| searchBoosts | ✓ | 部分：内置 slug-prefix boost |
| hardExclude | ✓ | ✗ |
| mcpPort | ✓ | ✗ |
| mcpAuthToken | ✓ | ✗ |
| minionConcurrency | ✓ | ✗ |
| minionQuietHours | ✓ | ✗ |
| chunk_size/overlap | ✗ | ✓ |
| transcription | ✗ | ✓ |
| log配置 | ✗ (用Bun内置) | ✓ |
| search_debug | ✗ | ✓ |

---

## 15. 错误处理对比

### TS: OperationError

```typescript
class OperationError extends Error {
  code: string;        // 'NOT_FOUND', 'INVALID_INPUT', 'SECURITY', etc.
  message: string;
  suggestion?: string;  // 修复建议
  docs?: string;        // 文档链接
}
```

### Rust: GBrainError

```rust
enum GBrainError {
    NotFound(String),
    InvalidInput(String),
    Security(String),
    Database(String),
    Io(std::io::Error),
    // ... 12个变体
}
```

### 差异

| 方面 | TS | Rust |
|------|----|------|
| 结构化错误码 | ✓ (code字段) | ✗ (枚举变体) |
| 修复建议 | ✓ (suggestion字段) | ✗ |
| 文档链接 | ✓ (docs字段) | ✗ |
| 错误链 | ✓ (cause) | ✓ (thiserror source) |
| MCP错误格式 | ✓ (JSON结构化) | ✗ (字符串) |

---

## 16. 测试对比

### TS测试

- 使用Bun内置test runner
- 测试内联在源文件中或独立test文件
- 集成测试需要PGLite实例
- 搜索质量评估 (eval命令)

### Rust测试

- 222个lib单元测试 (#[cfg(test)] mod tests)
- 4个dedup集成测试
- 17个engine集成测试
- 16个fuzzy集成测试
- 3个search集成测试
- 共262个测试，全部通过
- 使用:memory: SQLite，零配置

### 差异

| 方面 | TS | Rust |
|------|----|------|
| 测试框架 | Bun test | cargo test |
| 测试数量 | 未知 (内联) | 262 |
| 集成测试 | PGLite实例 | :memory: SQLite |
| 评估框架 | ✓ (eval命令) | ✓ (eval.rs) |
| 覆盖率 | 未知 | 未测量 |
