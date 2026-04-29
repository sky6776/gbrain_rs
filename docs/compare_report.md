# gbrain TypeScript vs gbrain-rs Rust 对比报告

[English version](./compare_report_en.md) | 中文

**日期**: 2026-04-29
**TS版本**: gbrain v0.22.8 (Bun runtime)
**Rust版本**: gbrain-rs (native, SQLite + sqlite-vec + FTS5)

---

## 1. 代码规模对比

| 指标 | TypeScript (gbrain) | Rust (gbrain-rs) | 比率 |
|------|---------------------|-------------------|------|
| 核心源代码行数 | ~30,890 (src/core/) | ~19,922 (src/) | 1.55x |
| CLI命令行数 | ~16,690 (src/commands/) | ~1,050 (src/bin/gbrain.rs) | 15.9x |
| MCP服务器行数 | ~686 (src/mcp/) | ~1,133 (src/mcp/) | 0.61x |
| 测试代码行数 | (内联测试) | ~783 (tests/) | — |
| **总源代码** | **~50,500** | **~20,705** | **2.44x** |
| 源文件数 | ~97 (core/) + 49 (commands/) | ~44 | 3.3x |

Rust版本代码量约为TS版本的41%，主要原因是：
- Rust缺少49个CLI命令文件（TS有完整的命令系统）
- Rust缺少Minions子系统（4,478行）
- Rust缺少Resolver子系统（1,095行）
- Rust缺少Skillify/Skillpack系统（1,013行）
- Rust缺少代码索引/代码分块系统（tree-sitter相关，~1,500行）

---

## 2. 数据库引擎对比

| 特性 | TypeScript | Rust |
|------|-----------|------|
| **主数据库** | PostgreSQL / PGLite (WASM) | SQLite |
| **全文搜索** | tsvector + ts_rank | FTS5 + BM25 |
| **向量搜索** | pgvector (余弦距离) | sqlite-vec (余弦相似度) |
| **模糊搜索** | pg_trgm (三字符组) | 自实现字符三字符组Jaccard |
| **连接池** | postgres.js 内置 | 单连接（SQLite不需要池） |
| **事务支持** | Postgres事务 + advisory lock | BEGIN IMMEDIATE + COMMIT/ROLLBACK |
| **Schema版本** | V1-V29 | V1-V8 |
| **多源支持** | source_id复合键 (v0.18+) | 无 |
| **PGLite** | 嵌入式WASM Postgres | 无（直接用SQLite） |
| **PgBouncer** | 自动检测兼容 | 不适用 |

**关键差异**:
- TS使用Postgres生态（pgvector, pg_trgm, tsvector），Rust使用SQLite生态（sqlite-vec, FTS5）
- TS支持多源brain（source_id复合键），Rust不支持
- TS的Schema迁移到V29，Rust仅到V8
- TS有PGLite（浏览器内Postgres）和远程Postgres双引擎，Rust只有SQLite

---

## 3. BrainEngine接口对比

### 方法数量

| 类别 | TS方法数 | Rust方法数 | 差异 |
|------|---------|-----------|------|
| 生命周期 | 4 | 4 | 一致 |
| 页面CRUD | 6 | 6 | 一致 |
| 搜索 | 4 | 3 | TS多searchKeywordChunks |
| 分块 | 6 | 5 | TS多countStaleChunks, listStaleChunks |
| 链接 | 9 | 9 | 一致 |
| 标签 | 3 | 3 | 一致 |
| 时间线 | 3 | 4 | Rust多add_timeline_multi_batch |
| 原始数据 | 2 | 2 | 一致 |
| 版本 | 3 | 3 | 一致 |
| 统计/健康 | 2 | 2 | 一致 |
| 孤儿/死链 | 1 | 2 | Rust多detect_dead_links |
| 配置 | 2 | 2 | 一致 |
| 文件 | 4 | 5 | Rust多file_url_by_storage_path, file_verify |
| 代码边 | 5 | 0 | **TS独有** |
| 其他 | 5 | 5 | 大致一致 |
| **总计** | **~59** | **~55** | |

### TS独有方法

| 方法 | 说明 |
|------|------|
| `searchKeywordChunks()` | 搜索代码分块（v0.19+） |
| `countStaleChunks()` | 统计过时分块数 |
| `listStaleChunks()` | 列出过时分块 |
| `findOrphanPages()` | 查找孤儿页面（Rust有detect_orphans） |
| `addCodeEdges()` | 添加代码调用边（v0.20 Cathedral II） |
| `deleteCodeEdgesForChunks()` | 删除代码边 |
| `getCallersOf()` | 查询谁调用了某符号 |
| `getCalleesOf()` | 查询某符号调用了谁 |
| `getEdgesByChunk()` | 查询分块的代码边 |
| `withReservedConnection()` | 专用连接（advisory lock） |

---

## 4. 类型系统对比

### PageType变体

| PageType | TS | Rust | 状态 |
|----------|----|----|------|
| person | ✓ | ✓ | 一致 |
| company | ✓ | ✓ | 一致 |
| deal | ✓ | ✓ | 一致 |
| yc | ✓ | ✓ | 一致 |
| civic | ✓ | ✓ | 一致 |
| project | ✓ | ✓ | 一致 |
| concept | ✓ | ✓ | 一致 |
| source | ✓ | ✓ | 一致 |
| media | ✓ | ✓ | 一致 |
| writing | ✓ | ✓ | 一致 |
| analysis | ✓ | ✓ | 一致 |
| guide | ✓ | ✓ | 一致 |
| hardware | ✓ | ✓ | 一致 |
| architecture | ✓ | ✓ | 一致 |
| meeting | ✓ | ✓ | 一致 |
| note | ✓ | ✓ | 一致 |
| **email** | ✓ | ✗ | **Rust缺失** |
| **slack** | ✓ | ✗ | **Rust缺失** |
| **calendar-event** | ✓ | ✗ | **Rust缺失** |
| **code** | ✓ | ✗ | **Rust缺失** |

Rust缺少4个PageType变体：email, slack, calendar-event, code。这些是v0.18+新增的工作流类型。

### Chunk差异

| 特性 | TS | Rust |
|------|----|------|
| chunk_source | compiled_truth / timeline / **fenced_code** | compiled_truth / timeline |
| 代码元数据 | ✓ (language, symbol_name, symbol_type, start_line, end_line, parent_symbol_path, doc_comment, symbol_name_qualified) | ✗ |
| embedding字段 | ✓ (Float32Array) | ✗ (不存储在Chunk结构中) |
| model字段 | ✓ | ✗ |
| embedded_at | ✓ | ✗ |
| StaleChunkRow | ✓ | ✗ |

### SearchOpts差异

| 特性 | TS | Rust |
|------|----|------|
| exclude_slug_prefixes | ✓ | ✗ |
| include_slug_prefixes | ✓ | ✗ |
| language | ✓ (v0.20) | ✗ |
| symbolKind | ✓ (v0.20) | ✗ |
| nearSymbol | ✓ (v0.20) | ✗ |
| walkDepth | ✓ (v0.20) | ✗ |
| sourceId | ✓ (v0.18) | ✗ |
| expanded_queries | ✗ | ✓ |
| expanded_embeddings | ✗ | ✓ |
| dedup_opts | ✗ | ✓ |

### 其他类型差异

| 类型 | TS | Rust | 差异 |
|------|----|----|------|
| PageKind | ✓ (markdown/code) | ✗ | Rust无PageKind |
| CodeEdgeInput | ✓ | ✗ | Cathedral II代码边 |
| CodeEdgeResult | ✓ | ✗ | Cathedral II代码边 |
| Link.direction | ✗ | ✓ | Rust有LinkDirection |
| LinkBatchInput.direction | ✗ | ✓ | Rust有方向字段 |
| PutPageResult | ✓ | ✓ | 一致 |
| ListPageEntry | ✓ | ✓ | 一致 |

---

## 5. 搜索管道对比

### 管道步骤

| 步骤 | TS | Rust | 差异 |
|------|----|----|------|
| 1. 关键词搜索 | tsvector + ts_rank + **source-boost** + **hard-exclude** | FTS5 BM25加权 | TS有source-boost和hard-exclude |
| 2. 向量搜索 | pgvector cosine distance | sqlite-vec cosine similarity | 不同引擎，相同算法 |
| 3. 回退拓宽 | ✓ | ✓ | 一致 |
| 4. RRF融合 | ✓ (k=60) | ✓ (k=60) | 一致 |
| 5. compiled_truth提升 | ✓ (2.0x) | ✓ | 权重可能不同 |
| 6. 余弦重评分 | ✓ (0.7*rrf + 0.3*cosine) | ✓ | 一致 |
| 7. 反向链接提升 | ✓ (1 + 0.05*ln(1+count)) | ✓ | 一致 |
| 8. 时间衰减 | ✗ (在hybrid.ts中未显式实现) | ✓ | **Rust独有** |
| 9. 意图类型提升 | ✓ | ✓ | 一致 |
| 10. 两遍检索 | ✓ (v0.20 Cathedral II) | ✗ | **TS独有** |
| 11. 4层去重 | ✓ | ✓ | 一致 |
| 12. source-boost | ✓ (按slug前缀加权) | ✗ | **TS独有** |
| 13. hard-exclude | ✓ (按slug前缀排除) | ✗ | **TS独有** |

### TS独有搜索特性

| 特性 | 文件 | 说明 |
|------|------|------|
| source-boost | search/source-boost.ts | 按slug前缀的搜索结果加权（如 originals/:1.5） |
| hard-exclude | search/sql-ranking.ts | SQL注入hard-exclude NOT(LIKE)子句 |
| two-pass retrieval | search/two-pass.ts | Cathedral II代码结构检索（BFS遍历code_edges） |
| searchKeywordChunks | engine.ts | 代码分块搜索 |

### Rust独有搜索特性

| 特性 | 文件 | 说明 |
|------|------|------|
| 时间衰减提升 | search/hybrid.rs | 1/(1 + days/half_life) 时间衰减 |
| expanded_queries/embeddings | types.rs SearchOpts | 预计算扩展查询和嵌入 |
| dedup_opts | types.rs SearchOpts | 可自定义去重选项 |

---

## 6. MCP工具对比

### 工具清单

| 工具 | TS | Rust | 差异 |
|------|----|----|------|
| get_page | ✓ | ✓ | 一致 |
| put_page | ✓ | ✓ | 一致 |
| delete_page | ✓ | ✓ | 一致 |
| list_pages | ✓ | ✓ | 一致 |
| search | ✓ | ✓ | 一致 |
| query | ✓ | ✓ | 一致 |
| add_tag | ✓ | ✓ | 一致 |
| remove_tag | ✓ | ✓ | 一致 |
| get_tags | ✓ | ✓ | 一致 |
| add_link | ✓ | ✓ | 一致 |
| remove_link | ✓ | ✓ | 一致 |
| get_links | ✓ | ✓ | 一致 |
| get_backlinks | ✓ | ✓ | 一致 |
| traverse_graph | ✓ | ✓ | 一致 |
| add_timeline_entry | ✓ | ✓ | 一致 |
| get_timeline | ✓ | ✓ | 一致 |
| get_stats | ✓ | ✓ | 一致 |
| get_health | ✓ | ✓ | 一致 |
| get_versions | ✓ | ✓ | 一致 |
| revert_version | ✓ | ✓ | 一致 |
| sync_brain | ✓ | ✓ | 一致 |
| put_raw_data | ✓ | ✓ | 一致 |
| get_raw_data | ✓ | ✓ | 一致 |
| resolve_slugs | ✓ | ✓ | 一致 |
| get_chunks | ✓ | ✓ | 一致 |
| log_ingest | ✓ | ✓ | 一致 |
| get_ingest_log | ✓ | ✓ | 一致 |
| file_list | ✓ | ✓ | 一致 |
| file_upload | ✓ | ✓ | 一致 |
| file_url | ✓ | ✓ | 一致 |
| submit_job | ✓ | ✓ | 一致 |
| get_job | ✓ | ✓ | 一致 |
| list_jobs | ✓ | ✓ | 一致 |
| cancel_job | ✓ | ✓ | 一致 |
| retry_job | ✓ | ✓ | 一致 |
| get_job_progress | ✓ | ✓ | 一致 |
| pause_job | ✓ | ✗ | **Rust缺失** |
| resume_job | ✓ | ✗ | **Rust缺失** |
| replay_job | ✓ | ✗ | **Rust缺失** |
| send_job_message | ✓ | ✗ | **Rust缺失** |
| find_orphans | ✓ | ✓ | 一致 |

### TS独有MCP特性

| 特性 | 说明 |
|------|------|
| HTTP传输 | Bearer token认证 + IP/Token双层速率限制 + CORS + 请求体大小限制 |
| 速率限制 | 令牌桶算法，LRU有界Map，防桶重置攻击 |
| 结构化错误 | OperationError带code/message/suggestion/docs字段 |
| 参数验证 | 类型检查（string/number/boolean/object/array） |

Rust的MCP仅支持stdio传输，无HTTP传输、无速率限制、无结构化错误码。

---

## 7. 分块器对比

| 分块器 | TS | Rust | 差异 |
|--------|----|----|------|
| Recursive | ✓ (211行) | ✓ (366行) | Rust实现更详细 |
| Semantic | ✓ (340行) | ✓ (719行) | Rust实现更详细 |
| LLM-guided | ✓ (163行) | ✓ (276行) | 一致 |
| **Code (tree-sitter)** | ✓ (1,050行) | ✗ | **Rust缺失** |
| **Edge extractor** | ✓ (178行) | ✗ | **Rust缺失** |
| **Qualified names** | ✓ (109行) | ✗ | **Rust缺失** |

Rust缺少整个代码分块子系统（tree-sitter AST解析、代码边提取、限定名构建）。

---

## 8. 富化管道对比

| 特性 | TS | Rust | 差异 |
|------|----|------|------|
| 实体检测 | ✓ (正则+公司后缀) | ✓ (正则+公司后缀) | 一致 |
| 层级分类 | ✓ (Tier1/2/3) | ✓ (Tier1/2/3) | 一致 |
| 完整性评分 | ✓ (7个rubric, 按实体类型) | ✓ (7个rubric, 按实体类型) | 一致 |
| 预算管理 | ✓ (SELECT FOR UPDATE, TTL, 午夜翻转) | ✓ (TxGuard RAII, reserve/commit/rollback) | 不同实现，相同语义 |
| 自动富化 | ✓ (stub创建+反向链接) | ✓ (stub创建+反向链接) | 一致 |
| extractAndEnrich | ✓ (批量+节流) | ✗ | **Rust缺失** |

---

## 9. 验证器对比

| 验证器 | TS | Rust | 差异 |
|--------|----|----|------|
| 反向链接验证 | ✓ (back-link.ts) | ✓ (validators.rs) | 一致 |
| 引用验证 | ✓ (citation.ts, 180行) | ✗ | **Rust缺失** |
| 链接验证 | ✓ (link.ts, 150行) | ✓ (validators.rs) | 一致 |
| 三横线验证 | ✓ (triple-hr.ts) | ✗ | **Rust缺失** |
| BrainWriter | ✓ (writer.ts, 330行) | ✓ (writer.rs, 429行) | 一致 |
| Scaffold | ✓ (scaffold.ts, 236行) | ✓ (scaffold.rs, 154行) | 一致 |
| SlugRegistry | ✓ (slug-registry.ts) | ✓ (resolver.rs) | 一致 |
| Post-write lint | ✓ (post-write.ts) | ✓ (config flag) | 一致 |

---

## 10. 存储后端对比

| 后端 | TS | Rust | 差异 |
|------|----|------|------|
| 本地文件系统 | ✓ | ✓ | 一致 |
| **S3兼容** | ✓ (AWS S3, R2, MinIO) | ✗ | **Rust缺失** |
| **Supabase Storage** | ✓ (含TUS可恢复上传) | ✗ | **Rust缺失** |
| 文件服务器 | ✗ | ✓ (axum, 可选feature) | **Rust独有** |

---

## 11. Minions子系统对比

| 特性 | TS | Rust | 差异 |
|------|----|------|------|
| **作业队列** | ✓ (MinionQueue, 1,281行) | ✓ (jobs.rs, 422行) | TS更复杂 |
| **Worker** | ✓ (MinionWorker, 513行) | ✗ | **Rust缺失** |
| **Supervisor** | ✓ (进程管理, 630行) | ✗ | **Rust缺失** |
| **Subagent** | ✓ (LLM-in-loop, 710行) | ✗ | **Rust缺失** |
| **Shell handler** | ✓ (321行) | ✗ | **Rust缺失** |
| **Subagent aggregator** | ✓ (169行) | ✗ | **Rust缺失** |
| **Plugin loader** | ✓ (235行) | ✗ | **Rust缺失** |
| **Rate leases** | ✓ (152行) | ✗ | **Rust缺失** |
| **Quiet hours** | ✓ (94行) | ✗ | **Rust缺失** |
| **Transcript** | ✓ (229行) | ✗ | **Rust缺失** |
| **Audit handlers** | ✓ (3个) | ✗ | **Rust缺失** |

Rust有基本的作业队列（submit/get/list/cancel/retry），但缺少整个Minions运行时：Worker进程池、Supervisor进程管理、Subagent LLM循环、Shell执行、插件系统等。

---

## 12. Resolver子系统对比

| 特性 | TS | Rust | 差异 |
|------|----|------|------|
| **Resolver接口** | ✓ (interface.ts, 158行) | ✗ | **Rust缺失** |
| **Resolver注册表** | ✓ (registry.ts, 151行) | ✗ | **Rust缺失** |
| **URL可达性检测** | ✓ (url-reachable.ts, 332行) | ✗ | **Rust缺失** |
| **X API推文解析** | ✓ (handle-to-tweet.ts, 428行) | ✗ | **Rust缺失** |

整个Resolver子系统在Rust中缺失。这包括死链检测、X/Twitter API集成等。

---

## 13. CLI命令对比

### Rust已实现的命令

| 命令 | TS | Rust |
|------|----|----|
| get | ✓ | ✓ |
| put | ✓ | ✓ |
| delete | ✓ | ✓ |
| list | ✓ | ✓ |
| search | ✓ | ✓ |
| query/ask | ✓ | ✓ |
| tag/untag | ✓ | ✓ |
| link/unlink | ✓ | ✓ |
| backlinks | ✓ | ✓ |
| timeline | ✓ | ✓ |
| stats | ✓ | ✓ |
| health | ✓ | ✓ |
| history | ✓ | ✓ |
| revert | ✓ | ✓ |
| lint | ✓ | ✓ |
| extract | ✓ | ✓ |
| embed | ✓ | ✓ |
| sync | ✓ | ✓ |
| serve | ✓ | ✓ |
| import | ✓ | ✓ |
| files | ✓ | ✓ |
| jobs | ✓ | ✓ |
| autopilot | ✓ | ✓ |

### TS有但Rust缺失的命令

| 命令 | 行数 | 说明 |
|------|------|------|
| init | 382 | 创建brain（PGLite/Supabase/URL） |
| doctor | 1,050 | 健康检查（resolver, skills, pgvector, RLS, embeddings） |
| upgrade | 259 | 自更新 |
| check-update | 179 | 检查新版本 |
| integrations | 1,005 | 管理集成配方 |
| auth | 262 | HTTP MCP token管理 |
| apply-migrations | 420 | Schema迁移编排 |
| config | 50 | 显示/获取/设置brain配置 |
| migrate | 305 | 引擎间迁移 |
| features | 305 | 扫描使用+推荐未用功能 |
| export | 56 | 导出brain到markdown |
| agent | 333 | Subagent运行时 |
| agent-logs | 185 | Agent日志查看 |
| dream | 209 | 一次性夜间维护 |
| code-def | 136 | 查找符号定义 |
| code-refs | 133 | 查找符号引用 |
| code-callers | 74 | 查找调用者 |
| code-callees | 80 | 查找被调用者 |
| reindex-code | 324 | 代码页面重索引 |
| reconcile-links | 177 | 批量重算链接 |
| eval | 344 | 搜索质量评估 |
| routing-eval | 209 | 路由质量评估 |
| skillify | 310 | Skill脚手架/生成 |
| skillify-check | 360 | Skill验证 |
| skillpack | 440 | Skill包管理 |
| skillpack-check | 228 | Skill包健康检查 |
| publish | 378 | 可分享HTML导出（含AES-256加密） |
| report | 82 | 保存时间戳报告 |
| sources | 372 | 多源brain管理 |
| resolvers | 195 | Resolver配置 |
| frontmatter | 299 | Frontmatter操作 |
| frontmatter-install-hook | 216 | Git hook安装 |
| check-backlinks | 274 | 查找/修复缺失反向链接 |
| orphans | 241 | 查找孤儿页面 |
| integrity | 762 | Brain完整性检查 |
| check-resolvable | 315 | 验证skill tree可达性 |
| repair-jsonb | 169 | 修复损坏的JSONB列 |
| graph-query | 118 | 图查询 |

---

## 14. 安全边界对比

| 特性 | TS | Rust | 差异 |
|------|----|------|------|
| remote标志 | ✓ (OpContext.remote) | ✓ (OpContext.remote) | 一致 |
| 路径遍历防护 | ✓ | ✓ | 一致 |
| 符号链接拒绝 | ✓ | ✓ | 一致 |
| Slug验证 | ✓ | ✓ | 一致 |
| 文件名验证 | ✓ | ✓ | 一致 |
| **HTTP Bearer token** | ✓ | ✗ | **Rust缺失** |
| **IP速率限制** | ✓ | ✗ | **Rust缺失** |
| **Token速率限制** | ✓ | ✗ | **Rust缺失** |
| **CORS** | ✓ (默认拒绝) | ✗ | **Rust缺失** |
| **请求体大小限制** | ✓ (1MB) | ✓ (1MB) | 一致 |
| **审计日志** | ✓ (mcp_request_log) | ✗ | **Rust缺失** |
| **SSH注入防护** | ✓ | ✓ | 一致 |
| **FTS5注入防护** | N/A (用tsvector) | ✓ | Rust独有 |
| **内容大小限制** | ✓ | ✓ | 一致 |

---

## 15. 依赖对比

### TS核心依赖

| 依赖 | 用途 | Rust对应 |
|------|------|---------|
| @electric-sql/pglite | 嵌入式Postgres | rusqlite (SQLite) |
| postgres | Postgres客户端 | 不适用 |
| pgvector | 向量搜索 | sqlite-vec |
| openai | OpenAI API | reqwest (手动实现) |
| @anthropic-ai/sdk | Anthropic API | 不适用 |
| @aws-sdk/client-s3 | S3存储 | 不适用 |
| gray-matter | Frontmatter解析 | serde_yaml |
| marked | Markdown渲染 | 不适用 |
| @dqbd/tiktoken | Token计数 | 不适用 |
| web-tree-sitter | 代码解析 | 不适用 |
| @modelcontextprotocol/sdk | MCP SDK | 手动JSON-RPC实现 |

### Rust核心依赖

| 依赖 | 用途 | TS对应 |
|------|------|--------|
| rusqlite (bundled) | SQLite + FTS5 + sqlite-vec | pglite/postgres |
| clap (derive) | CLI参数解析 | 手动arg解析 |
| reqwest (rustls-tls) | HTTP客户端 | fetch/openai sdk |
| serde + serde_json + serde_yaml | 序列化 | JSON.parse/YAML.parse |
| thiserror v2 | 错误推导 | 自定义Error类 |
| tokio | 异步运行时 | Bun (原生async) |
| sha2 + infer | 哈希+MIME检测 | crypto/mime |
| chrono | 时间戳 | Date |
| regex | 正则 | RegExp |
| unicode-normalization | 文本规范化 | 不适用 |
| tracing | 结构化日志 | console |
| Optional: axum + tower-http | 文件服务器 | 不适用 |

---

## 16. 架构差异总结

### 共同的三层架构

两个版本都遵循相同的三层设计：

```
Engine层 (BrainEngine接口/trait)
    ↓
Operations层 (业务逻辑)
    ↓
Interface层 (CLI + MCP)
```

### 关键架构差异

| 方面 | TS | Rust |
|------|----|----|
| **运行时** | Bun (JS/TS) | 原生编译 |
| **异步模型** | 全async/await | Engine同步, CLI/MCP边界spawn_blocking |
| **数据库** | Postgres双引擎(PGLite+远程) | 单引擎SQLite |
| **类型系统** | TypeScript接口 | Rust trait (NOT dyn-compatible) |
| **错误处理** | OperationError类+结构化错误码 | GBrainError枚举+thiserror |
| **配置** | config.json + env | env + config.json |
| **事务** | Postgres事务+advisory lock | BEGIN IMMEDIATE+COMMIT/ROLLBACK |
| **部署** | Bun编译单binary | cargo build单binary |

---

## 17. 功能完整性评分

| 模块 | 完整性 | 说明 |
|------|--------|------|
| 核心引擎 | 85% | 缺少代码边、多源、stale chunks |
| 搜索管道 | 80% | 缺少source-boost、hard-exclude、two-pass |
| MCP工具 | 90% | 缺少pause/resume/replay/send_message |
| 分块器 | 60% | 缺少code chunker、edge extractor、qualified names |
| 富化管道 | 90% | 缺少批量extractAndEnrich |
| 验证器 | 70% | 缺少citation、triple-hr验证器 |
| 存储后端 | 40% | 缺少S3、Supabase；独有axum文件服务器 |
| Minions/作业 | 30% | 基本队列有，整个运行时缺失 |
| Resolver | 0% | 完全缺失 |
| CLI命令 | 40% | 核心命令有，25+高级命令缺失 |
| 安全 | 75% | 缺少HTTP认证、速率限制、CORS、审计日志 |
| **总体** | **~65%** | 核心功能完整，高级特性大量缺失 |

---

## 18. Rust独有优势

| 优势 | 说明 |
|------|------|
| 原生性能 | 无GC暂停，无WASM开销，编译优化 |
| 内存安全 | 编译时保证，无null/undefined运行时错误 |
| 零配置 | SQLite无需Postgres服务器 |
| 小binary | 单文件部署，无Bun/node运行时依赖 |
| 时间衰减搜索 | Rust独有的搜索结果时间衰减提升 |
| axum文件服务器 | 可选的内嵌HTTP文件服务器 |
| FTS5 BM25加权 | 更精细的全文搜索权重控制 |
| 编译时类型检查 | 更强的类型安全保证 |

---

## 19. TS独有优势

| 优势 | 说明 |
|------|------|
| 多引擎支持 | PGLite(本地) + Postgres(远程) 双引擎 |
| 代码索引 | tree-sitter代码解析、符号定义/引用/调用者/被调用者 |
| Cathedral II | 两遍结构检索、代码边图遍历 |
| Subagent运行时 | LLM-in-loop工具调用、token追踪 |
| Resolver系统 | 死链检测、X API集成、可扩展resolver框架 |
| Skill系统 | Skill生成、验证、打包、安装 |
| HTTP MCP | Bearer认证、双层速率限制、CORS、审计日志 |
| S3/Supabase存储 | 云存储后端、TUS可恢复上传 |
| 多源brain | source_id复合键、跨源去重 |
| 发布/导出 | HTML导出、AES-256加密、skill打包 |
| 集成系统 | 集成配方管理 |
| 进程管理 | Supervisor进程监控、自动重启 |

---

## 20. 建议优先补齐的Rust功能

按影响排序：

| 优先级 | 功能 | 预估工作量 | 理由 |
|--------|------|-----------|------|
| P0 | HTTP MCP传输 + Bearer认证 | 2-3天 | 安全性关键，远程访问必需 |
| P0 | 速率限制 | 1天 | 防DoS/滥用 |
| P1 | citation验证器 | 0.5天 | 数据质量保证 |
| P1 | PageType扩展 (email/slack/calendar-event/code) | 0.5天 | 类型完整性 |
| P1 | source-boost + hard-exclude搜索 | 1天 | 搜索质量提升 |
| P2 | 多源brain (source_id) | 2天 | 多仓库场景 |
| P2 | S3存储后端 | 1-2天 | 云部署需求 |
| P2 | Subagent运行时 | 3-5天 | 自动化工作流 |
| P3 | Code chunker (tree-sitter) | 5-7天 | 代码索引需求 |
| P3 | Resolver系统 | 2-3天 | 死链检测、外部API |
| P3 | Skill系统 | 3-5天 | Skill生态 |
| P4 | Supervisor进程管理 | 2天 | 生产稳定性 |
| P4 | 审计日志 | 1天 | 可观测性 |
