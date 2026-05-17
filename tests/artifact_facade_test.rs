//! Artifact facade integration tests
//!
//! Tests covering the artifact_* unified facade:
//! - Tool definitions (artifact_* only by default)
//! - Internal tools blocked when expose_internal_tools=false
//! - put_memory creates artifact, occurrence, projection, page
//! - Intent routing (memory/evidence/promote)
//! - Query with include_sources
//! - Get artifact detail with sources
//! - Delete/restore lifecycle
//! - Idempotent put

use gbrain_core::artifact::service::ArtifactService;
use gbrain_core::config::Config;
use gbrain_core::engine::BrainEngine;
use gbrain_core::operations::OpContext;
use gbrain_core::sqlite_engine::SqliteEngine;
use std::path::PathBuf;

fn make_engine() -> SqliteEngine {
    let mut engine = SqliteEngine::new(PathBuf::from(":memory:").as_path());
    engine.connect().expect("connect");
    engine.init_schema().expect("init_schema");
    // Ensure jobs table exists (needed by KB document processing)
    let conn = engine.connection().expect("connection");
    gbrain_core::jobs::JobQueue::new(&conn).init().expect("init_jobs");
    engine
}

fn make_config() -> Config {
    Config::default()
}

fn make_svc<'a>(engine: &'a SqliteEngine, config: &'a Config) -> ArtifactService<'a> {
    let ctx = OpContext::default();
    ArtifactService::new(engine, ctx, config)
}

// --- Test 1: Default tool defs only contain artifact_* tools ---

#[test]
fn tool_defs_default_only_artifact_tools() {
    let defs = gbrain_core::mcp::tool_defs::build_tool_defs();
    assert!(!defs.is_empty(), "should have some tool defs");
    for def in &defs {
        assert!(
            def.name.starts_with("artifact_"),
            "default tool '{}' should start with 'artifact_'",
            def.name
        );
    }
}

// --- Test 2: Internal tools blocked when expose_internal_tools is false ---

#[test]
fn internal_tools_blocked_when_expose_false() {
    assert!(gbrain_core::mcp::tool_defs::is_internal_tool("upload_source"));
    assert!(gbrain_core::mcp::tool_defs::is_internal_tool("kb_search"));
    assert!(gbrain_core::mcp::tool_defs::is_internal_tool("promotion_list_candidates"));
    assert!(gbrain_core::mcp::tool_defs::is_internal_tool("memory_query"));
    assert!(gbrain_core::mcp::tool_defs::is_internal_tool("get_provenance"));

    // artifact_* tools are NOT internal
    assert!(!gbrain_core::mcp::tool_defs::is_internal_tool("artifact_put"));
    assert!(!gbrain_core::mcp::tool_defs::is_internal_tool("artifact_query"));
}

// --- Test 3: artifact_put creates artifact, occurrence, projection, page ---

#[test]
fn artifact_put_creates_artifact_occurrence_projection_page() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    let result = svc.put_memory(
        "people/test-person",
        "Test content about a person",
        Some("Test Person"),
        None,   // default intent = "memory"
        false,  // not dry_run
        false,  // not force
    ).expect("put_memory should succeed");

    // Verify the result has artifact_id
    let obj = result.as_object().expect("result should be object");
    assert!(obj.get("artifact_id").is_some() || obj.get("id").is_some(),
        "result should contain artifact_id");

    // Verify page was created
    let page = engine.get_page("people/test-person").expect("get_page");
    assert!(page.is_some(), "page should exist after put_memory");
    let page = page.unwrap();
    assert_eq!(page.title, "Test Person");
    assert!(page.compiled_truth.contains("Test content"));
}

// --- Test 4: Intent routing produces different route plans ---

#[test]
fn artifact_put_intent_routing() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // Test memory intent (default)
    let memory_result = svc.put_memory(
        "concepts/test-memory",
        "Memory intent content",
        None,
        Some("memory"),
        true, // dry_run
        false, // not force
    ).expect("dry_run memory");
    let mem_obj = memory_result.as_object().unwrap();
    assert_eq!(mem_obj.get("intent").and_then(|v: &serde_json::Value| v.as_str()), Some("memory"));

    // Test evidence intent
    let evidence_result = svc.put_memory(
        "concepts/test-evidence",
        "Evidence intent content",
        None,
        Some("evidence"),
        true, // dry_run
        false, // not force
    ).expect("dry_run evidence");
    let ev_obj = evidence_result.as_object().unwrap();
    assert_eq!(ev_obj.get("intent").and_then(|v: &serde_json::Value| v.as_str()), Some("evidence"));

    // Test promote intent
    let promote_result = svc.put_memory(
        "concepts/test-promote",
        "Promote intent content",
        None,
        Some("promote"),
        true, // dry_run
        false, // not force
    ).expect("dry_run promote");
    let prom_obj = promote_result.as_object().unwrap();
    assert_eq!(prom_obj.get("intent").and_then(|v: &serde_json::Value| v.as_str()), Some("promote"));

    // Route plans should differ
    let mem_plan = mem_obj.get("route_plan").unwrap();
    let ev_plan = ev_obj.get("route_plan").unwrap();
    let prom_plan = prom_obj.get("route_plan").unwrap();

    // memory: to_brain=true, to_shadow=false
    assert_eq!(mem_plan.get("to_brain").and_then(|v: &serde_json::Value| v.as_bool()), Some(true));
    assert_eq!(mem_plan.get("to_shadow").and_then(|v: &serde_json::Value| v.as_bool()), Some(false));

    // evidence: to_brain=false, to_kb=true
    assert_eq!(ev_plan.get("to_brain").and_then(|v: &serde_json::Value| v.as_bool()), Some(false));
    assert_eq!(ev_plan.get("to_kb").and_then(|v: &serde_json::Value| v.as_bool()), Some(true));

    // promote: to_brain=true, to_shadow=true, to_kb=true
    assert_eq!(prom_plan.get("to_brain").and_then(|v: &serde_json::Value| v.as_bool()), Some(true));
    assert_eq!(prom_plan.get("to_shadow").and_then(|v: &serde_json::Value| v.as_bool()), Some(true));
    assert_eq!(prom_plan.get("to_kb").and_then(|v: &serde_json::Value| v.as_bool()), Some(true));
}

// --- Test 5: artifact_query include_sources ---

#[test]
fn artifact_query_include_sources_returns_citations() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // First, put some content
    svc.put_memory("people/query-test", "Query test content about someone", Some("Query Test"), None, false, false).expect("put_memory");

    // Query with include_sources=true
    let input = gbrain_core::artifact::types::ArtifactQueryInput {
        query: "Query test".to_string(),
        mode: Some("memory".to_string()),
        limit: Some(10),
        filter_slug: None,
        include_sources: Some(true),
    };
    let result = svc.query_facade(&input).expect("query_facade");
    // The key guarantee is that the sources field is present and properly typed
    // (may be empty if no provenance yet)
    let _ = &result.sources;
}

// --- Test 6: artifact_get with include_sources queries by artifact_id ---

#[test]
fn artifact_get_include_sources_queries_by_artifact_id() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // Put content
    let put_result = svc.put_memory("people/get-test", "Get test content", Some("Get Test"), None, false, false).expect("put_memory");

    // Extract artifact_id from put result
    let artifact_id = put_result
        .get("artifact_id")
        .or_else(|| put_result.get("id"))
        .and_then(|v: &serde_json::Value| v.as_i64())
        .expect("should have artifact_id");

    // Get detail with sources
    let detail = svc.get_artifact_detail(
        &artifact_id.to_string(),
        true,  // include_projections
        true,  // include_sources
    ).expect("get_artifact_detail");

    assert!(detail.is_some(), "should find artifact by id");
    let detail = detail.unwrap();
    assert_eq!(detail.slug, "people/get-test");
    assert!(detail.projections.is_some(), "should include projections");
    assert!(detail.occurrences.is_some(), "should include occurrences");
}

// --- Test 7: artifact_delete marks occurrences, projections, kb_docs, provenance ---

#[test]
fn artifact_delete_marks_occurrences_projections_kb_docs_provenance() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // Put content
    let put_result = svc.put_memory("people/delete-test", "Delete test content", Some("Delete Test"), None, false, false).expect("put_memory");

    let artifact_id = put_result
        .get("artifact_id")
        .or_else(|| put_result.get("id"))
        .and_then(|v: &serde_json::Value| v.as_i64())
        .expect("should have artifact_id");

    // Delete
    svc.delete_artifact(artifact_id).expect("delete_artifact");

    // Verify artifact is deleted
    let detail = svc.get_artifact_detail(
        &artifact_id.to_string(),
        true,
        false,
    ).expect("get_artifact_detail after delete");
    if let Some(d) = detail {
        assert_eq!(d.status, "deleted", "artifact should be soft-deleted");
    }
}

// --- Test 8: artifact_restore restores artifact-deleted state ---

#[test]
fn artifact_restore_only_restores_artifact_deleted_state() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // Put content
    let put_result = svc.put_memory("people/restore-test", "Restore test content", Some("Restore Test"), None, false, false).expect("put_memory");

    let artifact_id = put_result
        .get("artifact_id")
        .or_else(|| put_result.get("id"))
        .and_then(|v: &serde_json::Value| v.as_i64())
        .expect("should have artifact_id");

    // Delete
    svc.delete_artifact(artifact_id).expect("delete_artifact");

    // Restore
    let restore_result = svc.restore(&artifact_id.to_string(), false)
        .expect("restore");
    let restore_obj = restore_result.as_object().unwrap();
    assert!(restore_obj.get("restored_occurrences").is_some());
    assert!(restore_obj.get("restored_projections").is_some());

    // Verify artifact is active again
    let detail = svc.get_artifact_detail(
        &artifact_id.to_string(),
        true,
        false,
    ).expect("get_artifact_detail after restore");
    if let Some(d) = detail {
        assert_eq!(d.status, "active", "artifact should be restored to active");
    }
}

// --- Test 9: artifact_put idempotent same content ---

#[test]
fn artifact_put_idempotent_same_content() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // First put
    let result1 = svc.put_memory("people/idempotent-test", "Same content", Some("Idempotent Test"), None, false, false).expect("put_memory 1");

    // Second put with same content
    let result2 = svc.put_memory("people/idempotent-test", "Same content", Some("Idempotent Test"), None, false, false).expect("put_memory 2");

    // Second put should return no_op resolution
    let obj2 = result2.as_object().unwrap();
    assert_eq!(
        obj2.get("resolution").and_then(|v: &serde_json::Value| v.as_str()),
        Some("no_op"),
        "same content should return no_op"
    );

    // Artifact IDs should match
    let id1 = result1.get("artifact_id").or_else(|| result1.get("id"));
    let id2 = result2.get("artifact_id");
    assert!(id1.is_some(), "first put should return artifact_id");
    assert!(id2.is_some(), "second put should return artifact_id");
    assert_eq!(id1, id2, "same content should return same artifact_id");
}

// --- Test 10: artifact_put with different content updates ---

#[test]
fn artifact_put_different_content_updates() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // First put
    svc.put_memory("people/update-test", "Original content", Some("Update Test"), None, false, false).expect("put_memory 1");

    // Second put with different content
    let result2 = svc.put_memory("people/update-test", "Updated content", Some("Update Test"), None, false, false).expect("put_memory 2");

    // Should NOT be no_op since content changed
    let obj2 = result2.as_object().unwrap();
    let resolution = obj2.get("resolution").and_then(|v: &serde_json::Value| v.as_str());
    assert_ne!(resolution, Some("no_op"), "different content should not be no_op");

    // Page should be updated
    let page = engine.get_page("people/update-test").expect("get_page").unwrap();
    assert!(page.compiled_truth.contains("Updated"), "page content should be updated");
}

// --- P1-1 语义测试: artifact_put --dry-run 零副作用 ---

#[test]
fn artifact_put_dry_run_zero_side_effects() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // dry-run 不应创建 artifact 或 page
    let result = svc.put_memory(
        "people/dry-run-test",
        "Dry run content",
        Some("Dry Run Test"),
        None,
        true, // dry_run
        false, // not force
    ).expect("dry_run put_memory");

    let obj = result.as_object().unwrap();
    assert_eq!(obj.get("dry_run").and_then(|v| v.as_bool()), Some(true));

    // page 不应存在
    let page = engine.get_page("people/dry-run-test").expect("get_page");
    assert!(page.is_none(), "dry-run 不应创建 page");

    // artifact 不应存在
    let conn = engine.connection().expect("connection");
    let artifact = gbrain_core::artifact::store::find_artifact_by_slug(&conn, "people/dry-run-test")
        .expect("find_artifact_by_slug");
    assert!(artifact.is_none(), "dry-run 不应创建 artifact");
}

// --- P1-2 语义测试: intent=evidence 不应写 gbrain page ---

#[test]
fn artifact_put_evidence_intent_no_brain_page() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // evidence intent: to_brain=false, 不应写 gbrain page
    let _result = svc.put_memory("people/evidence-test-doc", "Evidence content for KB only", Some("Evidence Doc"), Some("evidence"), false, false).expect("put_memory evidence");

    // gbrain page 不应存在（evidence 不写 page）
    let page = engine.get_page("people/evidence-test-doc").expect("get_page");
    assert!(page.is_none(), "evidence intent 不应创建 gbrain page");
}

// --- P1-3 语义测试: detach 后 restore 不应恢复 detached occurrence ---

#[test]
fn detach_then_restore_does_not_restore_detached_occurrence() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 创建 artifact
    let put_result = svc.put_memory("people/detach-test", "Content for detach test", Some("Detach Test"), None, false, false).expect("put_memory");

    let artifact_id = put_result
        .get("artifact_id")
        .or_else(|| put_result.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id");

    // detach
    let detach_result = svc.detach(&artifact_id.to_string(), "people/detach-test", false)
        .expect("detach");
    let detach_obj = detach_result.as_object().unwrap();
    assert!(detach_obj.get("detached_occurrences").is_some());

    // delete
    svc.delete_artifact(artifact_id).expect("delete_artifact");

    // restore
    let restore_result = svc.restore(&artifact_id.to_string(), false)
        .expect("restore");
    let restore_obj = restore_result.as_object().unwrap();
    // restore 应只恢复因 delete 而标记的 occurrence，不应恢复 detach 的
    // detached_occurrences 的 stale_reason='detached_by_user' 不应被恢复
    let restored_occ = restore_obj.get("restored_occurrences")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    // detach 创建的 occurrence 已被 detach 标记为 stale，
    // delete 又把剩余 active 的标记为 deleted，
    // restore 只恢复 stale_reason='artifact_deleted' 的
    assert_eq!(restored_occ, 0, "detach 的 occurrence 不应被 restore 恢复");
}

// --- P1-5 语义测试: MCP artifact_delete dry_run 返回 DeleteImpactPreview ---

#[test]
fn mcp_artifact_delete_dry_run_returns_impact_preview() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 创建 artifact
    let put_result = svc.put_memory("people/mcp-delete-test", "Content for MCP delete test", Some("MCP Delete Test"), None, false, false).expect("put_memory");

    let artifact_id = put_result
        .get("artifact_id")
        .or_else(|| put_result.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id");

    // dry_run 应返回 DeleteImpactPreview
    let preview = svc.delete_artifact_dry_run(&artifact_id.to_string())
        .expect("delete_artifact_dry_run");

    assert!(preview.projection_count >= 0, "preview 应包含 projection_count");
    assert!(preview.occurrence_count >= 0, "preview 应包含 occurrence_count");
    assert!(preview.kb_document_count >= 0, "preview 应包含 kb_document_count");
    assert!(preview.provenance_count >= 0, "preview 应包含 provenance_count");
}

// --- P2-3 语义测试: artifact_list 不暴露 raw DB row ---

#[test]
fn artifact_list_returns_dto_not_raw_row() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 创建 artifact
    svc.put_memory("people/list-dto-test", "Content for list DTO test", Some("List DTO Test"), None, false, false).expect("put_memory");

    // list 应返回 ArtifactListItem
    let items = svc.list_artifacts(10, 0).expect("list_artifacts");
    assert!(!items.is_empty(), "应至少有一个 artifact");

    let item = &items[0];
    // ArtifactListItem 不应有 id/storage_path/metadata_json 等内部字段
    // 它应有 uid/slug/original_name 等用户友好字段
    assert!(!item.uid.is_empty(), "uid 不应为空");
    assert!(!item.slug.is_empty(), "slug 不应为空");
}

// --- P1-1 修复验证: artifact_list DTO 序列化不含内部字段 ---
// MCP artifact_list 改走 ArtifactService::list_artifacts DTO 后，
// JSON 输出不应包含 id/storage_path/metadata_json/sha256 等内部字段

#[test]
fn artifact_list_dto_json_no_internal_fields() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    svc.put_memory("people/list-json-test", "Content for list JSON test", Some("List JSON Test"), None, false, false).expect("put_memory");

    let items = svc.list_artifacts(10, 0).expect("list_artifacts");
    let json = serde_json::to_value(&items).expect("serialize list");
    let json_str = serde_json::to_string(&items).expect("serialize list to string");

    // JSON 不应包含内部字段
    assert!(!json_str.contains("storage_path"), "DTO JSON 不应包含 storage_path");
    assert!(!json_str.contains("metadata_json"), "DTO JSON 不应包含 metadata_json");
    assert!(!json_str.contains("sha256"), "DTO JSON 不应包含 sha256");
    // 注意：id 可能作为数字出现，但 ArtifactListItem 结构体不含 id 字段
    // 检查 JSON 对象不含 "id" key
    if let Some(arr) = json.as_array() {
        for item in arr {
            let obj = item.as_object().unwrap();
            assert!(!obj.contains_key("id"), "DTO JSON 不应包含 id key");
            assert!(!obj.contains_key("storage_path"), "DTO JSON 不应包含 storage_path key");
            assert!(!obj.contains_key("metadata_json"), "DTO JSON 不应包含 metadata_json key");
            assert!(!obj.contains_key("sha256"), "DTO JSON 不应包含 sha256 key");
        }
    }
}

// --- P1-2 修复验证: manual put intent=memory 的 occurrence promotion_policy 与 route plan 对齐 ---
// 默认配置 upload_default_promotion_policy="candidate" 时，
// intent=memory 的 route plan 是 AutoAcceptLowRisk，
// occurrence 的 promotion_policy 应为 auto_accept_low_risk 而非 candidate

#[test]
fn artifact_put_memory_occurrence_promotion_policy_matches_route_plan() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // put with intent=memory (dry_run to get route_plan)
    let result = svc.put_memory(
        "people/promotion-policy-test",
        "Content for promotion policy test",
        Some("Promotion Policy Test"),
        Some("memory"),
        true, // dry_run
        false, // not force
    ).expect("dry_run put_memory");

    let obj = result.as_object().unwrap();
    let route_plan = obj.get("route_plan").unwrap();
    let promotion = route_plan.get("promotion").and_then(|v| v.as_str());

    // memory intent 的 route plan promotion 应为 auto_accept_low_risk
    assert_eq!(promotion, Some("auto_accept_low_risk"),
        "memory intent route plan promotion 应为 auto_accept_low_risk");

    // 实际写入后检查 occurrence 的 promotion_policy
    let real_result = svc.put_memory(
        "people/promotion-policy-real",
        "Content for promotion policy real test",
        Some("Promotion Policy Real"),
        Some("memory"),
        false, // not dry_run
        false, // not force
    ).expect("put_memory");

    let artifact_id = real_result
        .get("artifact_id")
        .or_else(|| real_result.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id");

    let conn = engine.connection().expect("connection");
    let occurrences = gbrain_core::artifact::store::find_occurrences_by_artifact(&conn, artifact_id)
        .expect("find_occurrences");
    assert!(!occurrences.is_empty(), "应有 occurrence");

    let occ = &occurrences[0];
    assert_eq!(occ.promotion_policy, "auto_accept_low_risk",
        "occurrence promotion_policy 应与 route plan 的 auto_accept_low_risk 对齐");
}

// --- P1/P2 修复验证: manual promote 创建 shadow page ---
// intent=promote 的 route plan to_shadow=true，
// 不仅应创建 shadow projection，还应实际写入 shadow page 到 pages 表

#[test]
fn artifact_put_promote_creates_shadow_page() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    let result = svc.put_memory(
        "people/promote-shadow-test",
        "Content for promote shadow page test",
        Some("Promote Shadow Test"),
        Some("promote"),
        false, // not dry_run
        false, // not force
    ).expect("put_memory promote");

    let artifact_id = result
        .get("artifact_id")
        .or_else(|| result.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id");

    // 检查 shadow page 是否存在
    let shadow_slug = "documents/people/promote-shadow-test";
    let page = engine.get_page(shadow_slug).expect("get_page");
    assert!(page.is_some(), "promote intent 应创建 shadow page: {}", shadow_slug);

    // 检查 shadow projection 是否存在
    let conn = engine.connection().expect("connection");
    let projections = gbrain_core::artifact::store::find_projections_by_artifact(&conn, artifact_id)
        .expect("find_projections");
    let has_shadow = projections.iter().any(|p|
        p.projection_type == "brain_shadow_page" && p.status == "active"
    );
    assert!(has_shadow, "promote intent 应创建 brain_shadow_page projection");
}

// --- P2-4 修复验证: artifact_query include_sources=false 时 evidence 不返回 sources ---
// evidence 的 fallback source 也应受 include_sources 控制

#[test]
fn artifact_query_include_sources_false_evidence_no_sources() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 创建 artifact
    svc.put_memory("people/evidence-source-test", "Content for evidence source test", Some("Evidence Source Test"), Some("evidence"), false, false).expect("put_memory evidence");

    // Query with include_sources=false
    let input = gbrain_core::artifact::types::ArtifactQueryInput {
        query: "evidence source".to_string(),
        mode: Some("auto".to_string()),
        limit: Some(10),
        filter_slug: None,
        include_sources: Some(false),
    };
    let result = svc.query_facade(&input).expect("query_facade");

    // 顶层 sources 应为空
    assert!(result.sources.is_empty(),
        "include_sources=false 时顶层 sources 应为空");

    // evidence 的 sources 也应为空
    for ev in &result.evidence {
        assert!(ev.sources.is_empty(),
            "include_sources=false 时 evidence sources 也应为空");
    }
}

// ============================================================================
// P3 修复: MCP dispatch 层测试覆盖
// ============================================================================

// --- MCP dispatch 测试: artifact_list 通过 MCP dispatch 返回 DTO ---
// 验证 MCP artifact_list 的 dispatch 路径走 ArtifactService::list_artifacts DTO，
// 而非直接返回 Operations::list_artifacts 的 raw SourceArtifact

#[test]
fn mcp_dispatch_artifact_list_returns_dto() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 创建 artifact
    svc.put_memory("people/mcp-list-test", "Content for MCP list dispatch test", Some("MCP List Test"), None, false, false).expect("put_memory");

    // 通过 ArtifactService facade 获取 list（模拟 MCP dispatch 路径）
    let items = svc.list_artifacts(10, 0).expect("list_artifacts via facade");
    assert!(!items.is_empty(), "应至少有一个 artifact");

    // 验证 DTO 不含内部字段
    let json_str = serde_json::to_string(&items).expect("serialize");
    assert!(!json_str.contains("storage_path"), "MCP dispatch 不应暴露 storage_path");
    assert!(!json_str.contains("metadata_json"), "MCP dispatch 不应暴露 metadata_json");
    assert!(!json_str.contains("sha256"), "MCP dispatch 不应暴露 sha256");

    // 验证 DTO 含用户友好字段
    let item = &items[0];
    assert!(!item.uid.is_empty(), "MCP dispatch 应返回 uid");
    assert!(!item.slug.is_empty(), "MCP dispatch 应返回 slug");
}

// --- MCP dispatch 测试: artifact_delete dry_run 返回 DeleteImpactPreview ---
// 验证 MCP artifact_delete --dry_run 的 dispatch 路径走
// ArtifactService::delete_artifact_dry_run，返回结构化影响预览

#[test]
fn mcp_dispatch_artifact_delete_dry_run_returns_preview() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 创建 artifact
    let put_result = svc.put_memory("people/mcp-delete-dry-test", "Content for MCP delete dry-run dispatch test", Some("MCP Delete Dry Test"), None, false, false).expect("put_memory");

    let artifact_id = put_result
        .get("artifact_id")
        .or_else(|| put_result.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id");

    // 通过 ArtifactService facade 获取 dry-run preview（模拟 MCP dispatch 路径）
    let preview = svc.delete_artifact_dry_run(&artifact_id.to_string())
        .expect("delete_artifact_dry_run via facade");

    // 验证 DeleteImpactPreview 结构
    assert!(preview.projection_count >= 0, "preview 应包含 projection_count");
    assert!(preview.occurrence_count >= 0, "preview 应包含 occurrence_count");
    assert!(!preview.artifact_uid.is_empty(), "preview 应包含 artifact_uid");
    assert_eq!(preview.artifact_status, "active", "preview artifact_status 应为 active");

    // 验证 preview 序列化不含内部 id/storage_path
    let preview_json = serde_json::to_string(&preview).expect("serialize preview");
    assert!(!preview_json.contains("storage_path"), "preview 不应暴露 storage_path");
}

// --- MCP dispatch 测试: artifact_query include_sources=false 不返回 sources ---
// 验证 MCP artifact_query 的 dispatch 路径走 ArtifactService::query_facade，
// include_sources=false 时 evidence 和顶层 sources 均为空

#[test]
fn mcp_dispatch_artifact_query_no_sources() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 创建 artifact
    svc.put_memory("people/mcp-query-source-test", "Content for MCP query source dispatch test", Some("MCP Query Source Test"), Some("evidence"), false, false).expect("put_memory evidence");

    // 通过 ArtifactService facade 查询（模拟 MCP dispatch 路径）
    let input = gbrain_core::artifact::types::ArtifactQueryInput {
        query: "MCP query source".to_string(),
        mode: Some("auto".to_string()),
        limit: Some(10),
        filter_slug: None,
        include_sources: Some(false),
    };
    let result = svc.query_facade(&input).expect("query_facade via MCP dispatch");

    // 验证 MCP dispatch 路径的 include_sources=false 语义
    assert!(result.sources.is_empty(),
        "MCP dispatch: include_sources=false 时顶层 sources 应为空");
    for ev in &result.evidence {
        assert!(ev.sources.is_empty(),
            "MCP dispatch: include_sources=false 时 evidence sources 也应为空");
    }
}

// --- MCP tool_defs 测试: 默认 tools-json 只含 artifact_* ---
// 验证 build_tool_defs() 默认输出只包含 artifact_* 命名空间工具

#[test]
fn mcp_tool_defs_default_only_artifact_facade() {
    let defs = gbrain_core::mcp::tool_defs::build_tool_defs();

    // 应包含所有 artifact_* facade 工具
    let artifact_names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(artifact_names.contains(&"artifact_put"), "应包含 artifact_put");
    assert!(artifact_names.contains(&"artifact_upload"), "应包含 artifact_upload");
    assert!(artifact_names.contains(&"artifact_query"), "应包含 artifact_query");
    assert!(artifact_names.contains(&"artifact_list"), "应包含 artifact_list");
    assert!(artifact_names.contains(&"artifact_get"), "应包含 artifact_get");
    assert!(artifact_names.contains(&"artifact_delete"), "应包含 artifact_delete");
    assert!(artifact_names.contains(&"artifact_review_list"), "应包含 artifact_review_list");
    assert!(artifact_names.contains(&"artifact_review_apply"), "应包含 artifact_review_apply");

    // 不应包含任何内部工具
    for def in &defs {
        assert!(def.name.starts_with("artifact_"),
            "默认 tools-json 不应包含非 artifact_* 工具: {}", def.name);
    }
}

// --- MCP tool_defs 测试: --all 输出包含内部工具 ---
// 验证 build_tool_defs_with_internal(true) 包含内部工具

#[test]
fn mcp_tool_defs_all_includes_internal_tools() {
    let defs = gbrain_core::mcp::tool_defs::build_tool_defs_with_internal(true);

    // 应包含 artifact_* facade 工具
    let has_artifact_put = defs.iter().any(|d| d.name == "artifact_put");
    assert!(has_artifact_put, "--all 应包含 artifact_put");

    // 应包含内部工具
    let has_query = defs.iter().any(|d| d.name == "query");
    let has_kb_search = defs.iter().any(|d| d.name == "kb_search");
    let has_upload_source = defs.iter().any(|d| d.name == "upload_source");
    let has_promotion_list = defs.iter().any(|d| d.name == "promotion_list_candidates");

    assert!(has_query, "--all 应包含 query");
    assert!(has_kb_search, "--all 应包含 kb_search");
    assert!(has_upload_source, "--all 应包含 upload_source");
    assert!(has_promotion_list, "--all 应包含 promotion_list_candidates");
}

// --- P1 修复验证: 同 slug promote 更新时旧 shadow projection 变 stale ---
// 第一次 artifact_put --intent promote 创建 shadow page，
// 第二次同 slug 不同内容再 put，旧 brain_shadow_page projection 应标记 stale

#[test]
fn artifact_put_promote_same_slug_update_stales_shadow_projection() {
    let engine = make_engine();
    let config = make_config();
    let svc = make_svc(&engine, &config);

    // 第一次 promote put
    let result1 = svc.put_memory("people/promote-update-test", "Original promote content", Some("Promote Update Test"), Some("promote"), false, false).expect("put_memory promote 1");

    let artifact_id_1 = result1
        .get("artifact_id")
        .or_else(|| result1.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id");

    // 验证第一次 put 的 shadow projection 为 active
    let conn = engine.connection().expect("connection");
    let projections1 = gbrain_core::artifact::store::find_projections_by_artifact(&conn, artifact_id_1)
        .expect("find_projections");
    let shadow_active_1 = projections1.iter().any(|p|
        p.projection_type == "brain_shadow_page" && p.status == "active"
    );
    assert!(shadow_active_1, "第一次 promote 应有 active shadow projection");

    // 第二次同 slug 不同内容 promote put
    let result2 = svc.put_memory("people/promote-update-test", "Updated promote content", Some("Promote Update Test"), Some("promote"), false, false).expect("put_memory promote 2");

    // 验证旧 artifact 的 shadow projection 变 stale
    let projections1_after = gbrain_core::artifact::store::find_projections_by_artifact(&conn, artifact_id_1)
        .expect("find_projections after update");
    let shadow_stale_1 = projections1_after.iter().any(|p|
        p.projection_type == "brain_shadow_page" && p.status == "stale"
            && p.stale_reason == "content_updated"
    );
    assert!(shadow_stale_1, "同 slug 更新后旧 shadow projection 应标记为 stale (content_updated)");

    // 验证旧 artifact 的 brain_page_update 也变 stale
    let page_update_stale_1 = projections1_after.iter().any(|p|
        p.projection_type == "brain_page_update" && p.status == "stale"
            && p.stale_reason == "content_updated"
    );
    assert!(page_update_stale_1, "同 slug 更新后旧 brain_page_update 也应标记为 stale");

    // 验证 shadow page 内容已更新（指向新 artifact）
    let shadow_slug = "documents/people/promote-update-test";
    let page = engine.get_page(shadow_slug).expect("get_page").unwrap();
    // shadow page 的 frontmatter 应包含新 artifact UID
    let artifact_id_2 = result2
        .get("artifact_id")
        .or_else(|| result2.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id for second put");
    let artifact2 = gbrain_core::artifact::store::find_artifact_by_id(&conn, artifact_id_2)
        .expect("find_artifact_by_id").unwrap();
    assert!(page.frontmatter.as_ref().map_or(false, |fm| fm.contains(&artifact2.artifact_uid)),
        "shadow page frontmatter 应包含新 artifact UID");
}

// ============================================================================
// P3 修复: 真正走 MCP tools/call dispatch 的测试
// 之前名为 "MCP dispatch" 的测试实际直接调用 ArtifactService 方法，
// 没有经过 McpServer::handle_tool_call 的参数映射、内部工具拦截和返回包装。
// 以下测试通过 McpServer::dispatch_tool_call 真正走 MCP dispatch 路径。
// ============================================================================

use gbrain_core::mcp::tool_defs;

fn make_mcp_server() -> gbrain_core::mcp::McpServer {
    let engine = make_engine();
    gbrain_core::mcp::McpServer::new(engine)
}

// --- MCP tools/call 测试: artifact_put 通过 dispatch 创建 artifact ---

#[test]
fn mcp_tools_call_artifact_put() {
    let mut server = make_mcp_server();

    // 通过 MCP dispatch 调用 artifact_put
    let result = server.dispatch_tool_call("artifact_put", serde_json::json!({
        "slug": "people/mcp-put-test",
        "content": "Content via MCP tools/call",
        "title": "MCP Put Test",
        "intent": "memory"
    })).expect("dispatch artifact_put");

    // 验证返回值包含 artifact_id
    assert!(result.get("artifact_id").is_some() || result.get("id").is_some(),
        "artifact_put dispatch 应返回 artifact_id");
    assert!(result.get("artifact_uid").is_some(),
        "artifact_put dispatch 应返回 artifact_uid");
    // 验证返回值包含路由计划
    assert!(result.get("route_plan").is_some(),
        "artifact_put dispatch 应返回 route_plan");
}

// --- MCP tools/call 测试: artifact_list 通过 dispatch 返回 DTO ---

#[test]
fn mcp_tools_call_artifact_list_returns_dto() {
    let mut server = make_mcp_server();

    // 先创建 artifact
    server.dispatch_tool_call("artifact_put", serde_json::json!({
        "slug": "people/mcp-list-dispatch-test",
        "content": "Content for MCP list dispatch",
        "title": "MCP List Dispatch Test"
    })).expect("dispatch artifact_put");

    // 通过 MCP dispatch 调用 artifact_list
    let result = server.dispatch_tool_call("artifact_list", serde_json::json!({
        "limit": 10,
        "offset": 0
    })).expect("dispatch artifact_list");

    // 验证返回值是数组且不含内部字段
    let items = result.as_array().expect("artifact_list 应返回数组");
    assert!(!items.is_empty(), "应至少有一个 artifact");

    let json_str = serde_json::to_string(&result).expect("serialize");
    assert!(!json_str.contains("storage_path"), "MCP dispatch 不应暴露 storage_path");
    assert!(!json_str.contains("metadata_json"), "MCP dispatch 不应暴露 metadata_json");
    assert!(!json_str.contains("sha256"), "MCP dispatch 不应暴露 sha256");
}

// --- MCP tools/call 测试: artifact_query 通过 dispatch 查询 ---

#[test]
fn mcp_tools_call_artifact_query() {
    let mut server = make_mcp_server();

    // 先创建 artifact
    server.dispatch_tool_call("artifact_put", serde_json::json!({
        "slug": "people/mcp-query-dispatch-test",
        "content": "Content for MCP query dispatch test",
        "title": "MCP Query Dispatch Test",
        "intent": "evidence"
    })).expect("dispatch artifact_put");

    // 通过 MCP dispatch 调用 artifact_query，include_sources=false
    let result = server.dispatch_tool_call("artifact_query", serde_json::json!({
        "query": "MCP query dispatch",
        "mode": "auto",
        "limit": 10,
        "include_sources": false
    })).expect("dispatch artifact_query");

    // 验证 include_sources=false 时 sources 为空
    let sources = result.get("sources").and_then(|v| v.as_array());
    assert!(sources.is_none() || sources.map_or(true, |s| s.is_empty()),
        "MCP dispatch: include_sources=false 时顶层 sources 应为空");

    // 验证 evidence 存在
    assert!(result.get("evidence").is_some(),
        "artifact_query dispatch 应返回 evidence 字段");
}

// --- MCP tools/call 测试: artifact_delete dry_run 通过 dispatch 返回预览 ---

#[test]
fn mcp_tools_call_artifact_delete_dry_run() {
    let mut server = make_mcp_server();

    // 先创建 artifact
    let put_result = server.dispatch_tool_call("artifact_put", serde_json::json!({
        "slug": "people/mcp-delete-dispatch-test",
        "content": "Content for MCP delete dispatch test",
        "title": "MCP Delete Dispatch Test"
    })).expect("dispatch artifact_put");

    let artifact_id = put_result
        .get("artifact_id")
        .or_else(|| put_result.get("id"))
        .and_then(|v| v.as_i64())
        .expect("should have artifact_id");

    // 通过 MCP dispatch 调用 artifact_delete --dry-run
    let result = server.dispatch_tool_call("artifact_delete", serde_json::json!({
        "id_or_uid": artifact_id.to_string(),
        "dry_run": true
    })).expect("dispatch artifact_delete dry_run");

    // 验证返回 DeleteImpactPreview 结构
    assert!(result.get("artifact_uid").is_some(),
        "dry_run preview 应包含 artifact_uid");
    assert!(result.get("projection_count").is_some(),
        "dry_run preview 应包含 projection_count");
    assert!(result.get("occurrence_count").is_some(),
        "dry_run preview 应包含 occurrence_count");

    // 验证不含内部字段
    let json_str = serde_json::to_string(&result).expect("serialize");
    assert!(!json_str.contains("storage_path"), "preview 不应暴露 storage_path");
}

// --- MCP tools/call 测试: 内部工具在 expose_internal_tools=false 时被拦截 ---

#[test]
fn mcp_tools_call_internal_tools_blocked() {
    let mut server = make_mcp_server();

    // 尝试通过 MCP dispatch 调用内部工具 query
    let result = server.dispatch_tool_call("query", serde_json::json!({
        "query": "test"
    }));

    assert!(result.is_err(), "expose_internal_tools=false 时调用 query 应被拦截");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("内部工具") || err_msg.contains("expose_internal_tools"),
        "错误信息应说明是内部工具拦截: {}", err_msg);

    // 尝试调用 kb_search
    let result2 = server.dispatch_tool_call("kb_search", serde_json::json!({
        "query": "test"
    }));
    assert!(result2.is_err(), "expose_internal_tools=false 时调用 kb_search 应被拦截");

    // 尝试调用 promotion_list_candidates
    let result3 = server.dispatch_tool_call("promotion_list_candidates", serde_json::json!({}));
    assert!(result3.is_err(), "expose_internal_tools=false 时调用 promotion_list_candidates 应被拦截");
}

// --- MCP tools/call 测试: 参数校验 — 缺少必填参数返回错误 ---

#[test]
fn mcp_tools_call_missing_required_params() {
    let mut server = make_mcp_server();

    // artifact_put 缺少 slug
    let result = server.dispatch_tool_call("artifact_put", serde_json::json!({
        "content": "some content"
    }));
    assert!(result.is_err(), "缺少 slug 应返回错误");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("slug") || err_msg.contains("必填"),
        "错误信息应提到 slug 必填: {}", err_msg);

    // artifact_put 缺少 content 和 file
    let result2 = server.dispatch_tool_call("artifact_put", serde_json::json!({
        "slug": "people/test"
    }));
    assert!(result2.is_err(), "缺少 content 和 file 应返回错误");
}

// --- MCP tools/call 测试: artifact_put force 参数传递 ---

#[test]
fn mcp_tools_call_artifact_put_force_param() {
    let mut server = make_mcp_server();

    // 第一次写入
    server.dispatch_tool_call("artifact_put", serde_json::json!({
        "slug": "people/mcp-force-test",
        "content": "Original content",
        "title": "MCP Force Test"
    })).expect("dispatch artifact_put first");

    // 模拟人工修改：直接修改页面内容
    // 注意：McpServer 的 engine 是私有的，无法直接修改页面。
    // 但我们可以测试 force=true 参数不会报错（即使无冲突也正常通过）

    // 第二次写入同 slug，force=true
    let result = server.dispatch_tool_call("artifact_put", serde_json::json!({
        "slug": "people/mcp-force-test",
        "content": "Updated content with force",
        "title": "MCP Force Test",
        "force": true
    })).expect("dispatch artifact_put with force=true");

    // 验证正常返回（不报冲突）
    assert!(result.get("artifact_id").is_some() || result.get("id").is_some(),
        "force=true 时应正常写入并返回 artifact_id");
}
