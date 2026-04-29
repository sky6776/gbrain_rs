//! Engine CRUD integration tests against in-memory SQLite

use gbrain_core::engine::BrainEngine;
use gbrain_core::sqlite_engine::SqliteEngine;
use gbrain_core::types::*;
use std::path::PathBuf;

fn make_engine() -> SqliteEngine {
    let mut engine = SqliteEngine::new(PathBuf::from(":memory:").as_path());
    engine.connect().expect("connect");
    engine.init_schema().expect("init_schema");
    engine
}

fn page_input(title: &str, content: &str, page_type: PageType) -> PageInput {
    PageInput {
        title: title.to_string(),
        compiled_truth: content.to_string(),
        page_type,
        timeline: None,
        frontmatter: None,
        content_hash: None,
    }
}

#[test]
fn test_put_and_get_page() {
    let engine = make_engine();
    let page = engine
        .put_page(
            "people/alice",
            page_input("Alice", "An engineer", PageType::Person),
        )
        .expect("put");
    assert_eq!(page.slug, "people/alice");
    assert_eq!(page.title, "Alice");
    assert_eq!(page.page_type, PageType::Person);

    let got = engine.get_page("people/alice").expect("get").expect("some");
    assert_eq!(got.title, "Alice");
    assert_eq!(got.compiled_truth, "An engineer");
}

#[test]
fn test_update_page() {
    let engine = make_engine();
    engine
        .put_page(
            "people/alice",
            page_input("Alice", "Original", PageType::Person),
        )
        .expect("put1");
    engine
        .put_page(
            "people/alice",
            page_input("Alice Updated", "Updated content", PageType::Person),
        )
        .expect("put2");

    let got = engine.get_page("people/alice").expect("get").expect("some");
    assert_eq!(got.title, "Alice Updated");
    assert_eq!(got.compiled_truth, "Updated content");
}

#[test]
fn test_delete_page() {
    let engine = make_engine();
    engine
        .put_page(
            "people/alice",
            page_input("Alice", "Content", PageType::Person),
        )
        .expect("put");
    engine.delete_page("people/alice").expect("delete");
    let got = engine.get_page("people/alice").expect("get");
    assert!(got.is_none());
}

#[test]
fn test_list_pages() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put1");
    engine
        .put_page("companies/acme", page_input("Acme", "B", PageType::Company))
        .expect("put2");

    let filters = PageFilters {
        page_type: None,
        tag: None,
        limit: Some(50),
        offset: None,
        updated_after: None,
    };
    let pages = engine.list_pages(filters).expect("list");
    assert_eq!(pages.len(), 2);
}

#[test]
fn test_list_pages_by_type() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put1");
    engine
        .put_page("companies/acme", page_input("Acme", "B", PageType::Company))
        .expect("put2");

    let filters = PageFilters {
        page_type: Some(PageType::Person),
        tag: None,
        limit: Some(50),
        offset: None,
        updated_after: None,
    };
    let pages = engine.list_pages(filters).expect("list");
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].slug, "people/alice");
}

#[test]
fn test_tags() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put");

    engine
        .add_tag("people/alice", "engineer")
        .expect("add_tag1");
    engine.add_tag("people/alice", "rust").expect("add_tag2");

    let tags = engine.get_tags("people/alice").expect("get_tags");
    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&"engineer".to_string()));
    assert!(tags.contains(&"rust".to_string()));

    engine
        .remove_tag("people/alice", "engineer")
        .expect("remove_tag");
    let tags = engine.get_tags("people/alice").expect("get_tags2");
    assert_eq!(tags.len(), 1);
}

#[test]
fn test_links() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put1");
    engine
        .put_page("companies/acme", page_input("Acme", "B", PageType::Company))
        .expect("put2");

    engine
        .add_link(
            "people/alice",
            "companies/acme",
            None,
            Some("works_at"),
            None,
            None,
            None,
        )
        .expect("add_link");

    let links = engine.get_links("people/alice").expect("get_links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].to_slug, "companies/acme");
    assert_eq!(links[0].link_type, "works_at");

    let backlinks = engine
        .get_backlinks("companies/acme")
        .expect("get_backlinks");
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].from_slug, "people/alice");
}

#[test]
fn test_chunks() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put");

    let chunks = vec![
        ChunkInput {
            chunk_index: 0,
            chunk_text: "First chunk".to_string(),
            source: ChunkSource::CompiledTruth,
            token_count: 10,
        },
        ChunkInput {
            chunk_index: 1,
            chunk_text: "Second chunk".to_string(),
            source: ChunkSource::Timeline,
            token_count: 10,
        },
    ];
    engine
        .upsert_chunks("people/alice", &chunks)
        .expect("upsert");

    let got = engine.get_chunks("people/alice").expect("get_chunks");
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].chunk_text, "First chunk");
    assert_eq!(got[1].chunk_text, "Second chunk");
}

#[test]
fn test_timeline() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put");

    let entry = TimelineInput {
        date: "2024-01-15".to_string(),
        source: None,
        summary: "Joined Acme Corp".to_string(),
        detail: None,
    };
    engine
        .add_timeline_entry("people/alice", entry, false)
        .expect("add_timeline");

    let entries = engine
        .get_timeline("people/alice", None)
        .expect("get_timeline");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].summary, "Joined Acme Corp");
}

#[test]
fn test_stats() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put");
    engine.add_tag("people/alice", "engineer").expect("tag");

    let stats = engine.get_stats().expect("stats");
    assert_eq!(stats.page_count, 1);
    assert_eq!(stats.tag_count, 1);
}

#[test]
fn test_health() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put");

    let health = engine.get_health().expect("health");
    assert_eq!(health.page_count, 1);
    assert!(health.brain_score > 0.0);
}

#[test]
fn test_resolve_slugs() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put");
    engine
        .put_page("people/bob", page_input("Bob", "B", PageType::Person))
        .expect("put2");

    let slugs = engine.resolve_slugs("people").expect("resolve");
    assert_eq!(slugs.len(), 2);
}

#[test]
fn test_config() {
    let engine = make_engine();
    engine.set_config("test_key", "test_value").expect("set");
    let val = engine.get_config("test_key").expect("get");
    assert_eq!(val, Some("test_value".to_string()));
}

#[test]
fn test_raw_data() {
    let engine = make_engine();
    engine
        .put_page("people/alice", page_input("Alice", "A", PageType::Person))
        .expect("put");
    engine
        .put_raw_data("people/alice", "raw_key", serde_json::json!({"foo": "bar"}))
        .expect("put");
    let data = engine.get_raw_data("people/alice", "raw_key").expect("get");
    assert!(data.is_some());
}

#[test]
fn test_disconnect() {
    let mut engine = make_engine();
    engine.disconnect().expect("disconnect");
}
