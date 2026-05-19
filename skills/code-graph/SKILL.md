---
name: code-graph
version: 1.0.0
description: |
  Code knowledge graph operations. Find symbol definitions, trace references,
  explore call graphs, search code chunks, and reindex code pages.
triggers:
  - "find definition"
  - "code definition"
  - "who calls"
  - "callers of"
  - "callees of"
  - "code references"
  - "search code"
  - "code graph"
  - "reindex code"
tools:
  - artifact_query  # 统一查询接口
mutating: true
writes_pages: false
---

# Code Graph Skill

Navigate and query the code knowledge graph — a structured index of code symbols,
their definitions, references, and call relationships.

## Contract

This skill guarantees:
- Symbol lookups use `artifact_query` with code-aware parameters
- Call relationships are explored through the graph store via `artifact_query`
- Search results include language and symbol kind context
- All queries go through the unified `artifact_query` facade（统一对外接口）

## When to Use

Use this skill when:
- Finding where a function/type/constant is defined
- Tracing who calls a function (callers) or what a function calls (callees)
- Searching for code by keyword or symbol text
- Understanding call relationships between modules

Do NOT use this skill for:
- General brain page queries（直接使用 `artifact_query`，无需特定 mode）
- Document/evidence search（使用 `artifact_query` with mode=evidence）
- Entity lookups（使用 brain-ops lookup chain）

## Phases

代码图谱操作暂未通过 `artifact_query` 统一入口对外暴露——`artifact_query mode=graph` 尚未实现。
请使用内部 `code_def`/`code_refs`/`get_callers` 工具进行代码符号定义、引用和调用关系查询。
未来 `mode=graph` 实现后将统一到 artifact facade（设计文档 §8.2）。

### Phase 1: 符号定义查找

使用 `code_def` 查找符号定义：

1. 提供符号名（可含限定名），可选按语言过滤。
2. 结果包含定义所在的 chunk、文件路径和上下文。
3. 对于歧义符号（多文件同名），通过 `filter_slug` 缩小范围。

### Phase 2: 引用追踪

使用 `code_refs` 查找所有引用某符号的代码片段：

1. 提供符号名和可选的语言过滤。
2. 结果展示每个提及该符号的 chunk——import、调用、类型注解。
3. 结合定义查找和引用追踪构建完整的「定义 + 所有使用」图景。

### Phase 3: 调用图探索

使用 `get_callers` 进行有向调用图遍历：

- 查找调用者（"谁调用了这个函数？"）——入边
- 查找被调用者（"这个函数调用了什么？"）——出边
- 通过迭代结果追踪多跳调用链

### Phase 4: 代码片段搜索

使用内部代码搜索工具进行基于关键词的代码搜索：

1. 提供查询字符串（关键词、符号片段或概念）。
2. 可选按语言或符号类型过滤。
3. 结果返回带有语言上下文的匹配 chunk。

### Phase 5: 图谱边检查

使用 `get_callers` 查看特定代码片段的所有调用图边：

1. 提供前次搜索或定义结果中的 chunk 标识。
2. 返回该 chunk 的所有入边和出边。

## 与 Brain Query 的集成

代码图谱暂未集成到 `artifact_query`——`mode=graph` 尚未实现。
当前代码图谱操作需通过内部工具单独执行：

- `code_def` + `code_refs` — 定义查找 + 引用追踪
- `get_callers` — 调用关系遍历
- `artifact_query` with `filter_slug=<code_page>` — 限定到特定代码页面（通用查询）
- `artifact_query` with `include_sources=true` — 显示来源追溯（通用查询）

## Anti-Patterns

- **不要通过函数名猜测调用关系。** 使用 `code_refs`/`get_callers` 追踪实际边，而非名称模式。
- **不要跳过代码图谱直接文本搜索。** 内部代码工具对符号查找更精确。
- **不要在重构后不刷新数据。** 过期的代码边会产生错误的调用链。

## Tools Used

- `artifact_query` — 统一知识查询（含代码图谱检索能力）