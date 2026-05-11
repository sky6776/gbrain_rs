//! KB 作业 worker — 后台轮询并处理 KB 文档作业
//!
//! 轮询 `claim_next_kb_job`，调用 `process_document_async`，
//! 然后标记作业完成或失败。可通过 CLI 子命令或 MCP 内置后台线程运行。
//! 支持多 worker 并发处理 (P5-023)。

use crate::config::Config;
use crate::embedding::Embedder;
use crate::engine::BrainEngine;
use crate::error::{GBrainError, Result};
use crate::kb::engine::KbEngine;
use crate::kb::jobs::{claim_next_kb_job, complete_kb_job, fail_kb_job};
use crate::kb::pipeline::process_document_async;
use crate::kb::raptor::RaptorConfig;
use crate::sqlite_engine::SqliteEngine;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

/// 运行一次 KB 作业处理循环：认领一个待处理作业并执行。
/// 返回是否处理了一个作业。
pub fn run_kb_worker_once(engine: &SqliteEngine, config: &Config) -> Result<bool> {
    let conn = engine.connection()?;

    // 认领下一个待处理作业
    let claimed = claim_next_kb_job(conn)?;
    let Some((job_db_id, payload)) = claimed else {
        debug!("KB worker: 无待处理作业");
        return Ok(false);
    };

    info!(
        job_db_id,
        document_id = payload.document_id,
        "KB worker: 认领作业"
    );

    // 创建 embedder（如果已配置 API key）
    let embedder: Option<Arc<Embedder>> = config.openai_api_key.as_deref().map(|api_key| {
        Arc::new(Embedder::new(
            api_key,
            config.openai_base_url.as_deref(),
            Some(&config.embedding_model),
            Some(config.embedding_dimensions),
        ))
    });

    // 构建 RaptorConfig（使用默认值）
    let raptor_config = RaptorConfig::default();

    // 创建 tokio 运行时执行异步管道
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| GBrainError::InvalidInput(format!("创建异步运行时失败: {}", e)))?;

    // 执行文档处理管道
    let result = rt.block_on(process_document_async(
        conn,
        &payload,
        embedder,
        Some(&raptor_config),
        config.kb_raptor_secret_ref.as_deref(),
        config.kb_raptor_base_url.as_deref(),
        if config.kb_raptor_model.is_empty() { None } else { Some(config.kb_raptor_model.as_str()) },
        None,
    ));

    match result {
        Ok(process_result) => {
            info!(
                job_db_id,
                document_id = payload.document_id,
                word_total = process_result.word_total,
                split_total = process_result.split_total,
                "KB worker: 文档处理完成"
            );
            complete_kb_job(conn, job_db_id)?;
            Ok(true)
        }
        Err(e) => {
            warn!(
                job_db_id,
                document_id = payload.document_id,
                error = %e,
                "KB worker: 文档处理失败"
            );
            // 更新文档状态为失败
            let kb = KbEngine::new(conn);
            let _ = kb.update_document_status(
                payload.document_id,
                Some(crate::kb::types::STATUS_FAILED),
                None,
                Some(&e.to_string()),
                None,
                None,
                None,
            );
            fail_kb_job(conn, job_db_id, &e.to_string())?;
            Ok(true)
        }
    }
}

/// 运行一次 re-embed 作业处理循环：认领一个 kb_reembed 或 kb_reembed_node 作业并处理。
/// 返回是否处理了一个作业。
pub fn run_reembed_worker_once(engine: &SqliteEngine, config: &Config) -> Result<bool> {
    let conn = engine.connection()?;
    let claimed = crate::kb::jobs::claim_next_reembed_job(conn)?;
    let Some((job_db_id, job_type, payload)) = claimed else {
        return Ok(false);
    };

    let embedder: Option<Arc<Embedder>> = config.openai_api_key.as_deref().map(|api_key| {
        Arc::new(Embedder::new(
            api_key,
            config.openai_base_url.as_deref(),
            Some(&config.embedding_model),
            Some(config.embedding_dimensions),
        ))
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all().build()
        .map_err(|e| GBrainError::InvalidInput(format!("创建异步运行时失败: {}", e)))?;

    let result = match job_type.as_str() {
        "kb_reembed_node" => {
            // 单节点修复：读取 node_id，嵌入内容，写入 kb_node_embeddings
            let node_id: i64 = payload.get("node_id").and_then(|v| v.as_i64()).unwrap_or(0);
            if node_id == 0 {
                fail_kb_job(conn, job_db_id, "missing node_id in kb_reembed_node payload")?;
                return Ok(true);
            }
            reembed_single_node(conn, &embedder, &rt, node_id, None)
        }
        "kb_reembed" => {
            // 文档级重嵌入：读取所有无 embedding 的节点，逐个嵌入
            let doc_id: i64 = payload.get("document_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let target_index_id: i64 = payload.get("target_embedding_index_id")
                .and_then(|v| v.as_i64()).unwrap_or(0);
            reembed_document_nodes(conn, &embedder, &rt, doc_id, target_index_id)
        }
        _ => Err(GBrainError::InvalidInput(format!("未知 re-embed 作业类型: {}", job_type))),
    };

    match result {
        Ok(count) => {
            info!(job_db_id, node_count = count, "re-embed worker: 处理完成");
            complete_kb_job(conn, job_db_id)?;
            Ok(true)
        }
        Err(e) => {
            warn!(job_db_id, error = %e, "re-embed worker: 处理失败");
            fail_kb_job(conn, job_db_id, &e.to_string())?;
            Ok(true)
        }
    }
}

/// 对单个节点执行 re-embed，返回嵌入向量维度数
fn reembed_single_node(
    conn: &Connection,
    embedder: &Option<Arc<Embedder>>,
    rt: &tokio::runtime::Runtime,
    node_id: i64,
    target_index_id: Option<i64>,
) -> Result<usize> {
    let (content, embedding_text): (String, String) = conn.query_row(
        "SELECT content, embedding_text FROM kb_document_nodes WHERE id=?1",
        rusqlite::params![node_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).map_err(|e| GBrainError::Database(format!("节点 {} 不存在: {}", node_id, e)))?;

    let text_to_embed = if embedding_text.is_empty() { &content } else { &embedding_text };
    let embedder = embedder.as_ref()
        .ok_or_else(|| GBrainError::InvalidInput("未配置 embedding API key".into()))?;
    let vectors = rt.block_on(embedder.embed_batch(&[text_to_embed]))?;
    let vec = vectors.into_iter().next()
        .ok_or_else(|| GBrainError::InvalidInput("embedding 返回空结果".into()))?;
    let dims = vec.len();
    let blob = vec.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();

    if let Some(idx_id) = target_index_id {
        conn.execute(
            "INSERT OR REPLACE INTO kb_node_embeddings (node_id, embedding, dimensions, model, embedding_index_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![node_id, blob, dims as i32, "text-embedding-3-large", idx_id],
        )?;
    } else {
        conn.execute(
            "INSERT OR REPLACE INTO kb_node_embeddings (node_id, embedding, dimensions, model) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![node_id, blob, dims as i32, "text-embedding-3-large"],
        )?;
    }
    Ok(dims)
}

/// 对文档中所有缺失 embedding 的节点执行批量 re-embed
fn reembed_document_nodes(
    conn: &Connection,
    embedder: &Option<Arc<Embedder>>,
    rt: &tokio::runtime::Runtime,
    document_id: i64,
    target_index_id: i64,
) -> Result<usize> {
    // 查找文档中无 embedding 的节点
    let node_ids: Vec<(i64, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT n.id, n.content, n.embedding_text FROM kb_document_nodes n \
             WHERE n.document_id = ?1 \
             AND n.id NOT IN (SELECT node_id FROM kb_node_embeddings \
                              WHERE (?2 = 0 OR embedding_index_id = ?2))"
        )?;
        let rows = stmt.query_map(rusqlite::params![document_id, target_index_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        rows.filter_map(|r: rusqlite::Result<_>| r.ok()).collect()
    };

    if node_ids.is_empty() {
        return Ok(0);
    }

    let embedder = embedder.as_ref()
        .ok_or_else(|| GBrainError::InvalidInput("未配置 embedding API key".into()))?;

    // 批量嵌入（每批最多 20 个节点）
    for chunk in node_ids.chunks(20) {
        let texts: Vec<&str> = chunk.iter().map(|(_, c, et)| {
            if et.is_empty() { c.as_str() } else { et.as_str() }
        }).collect();
        let vectors = rt.block_on(embedder.embed_batch(&texts))?;
        for ((node_id, _, _), vec) in chunk.iter().zip(vectors.iter()) {
            let blob = vec.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
            let dims = vec.len() as i32;
            if target_index_id != 0 {
                conn.execute(
                    "INSERT OR REPLACE INTO kb_node_embeddings (node_id, embedding, dimensions, model, embedding_index_id) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![node_id, blob, dims, "text-embedding-3-large", target_index_id],
                )?;
            } else {
                conn.execute(
                    "INSERT OR REPLACE INTO kb_node_embeddings (node_id, embedding, dimensions, model) \
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![node_id, blob, dims, "text-embedding-3-large"],
                )?;
            }
        }
    }
    Ok(node_ids.len())
}

/// 以守护进程模式运行 KB worker：持续轮询并处理所有类型的 KB 作业。
/// 包括 kb_process_document（文档解析/切分/嵌入）、kb_reembed（文档级重嵌入）、
/// kb_reembed_node（单节点修复）。
///
/// `interval_secs` 为无作业时的轮询间隔。
pub fn run_kb_worker_loop(engine: &SqliteEngine, config: &Config, interval_secs: u64) -> ! {
    info!(interval_secs, "KB worker: 启动守护进程模式");
    loop {
        // 先处理文档处理作业，再处理 re-embed 作业
        let had_work = match run_kb_worker_once(engine, config) {
            Ok(true) => true,
            Ok(false) => match run_reembed_worker_once(engine, config) {
                Ok(true) => true,
                Ok(false) => false,
                Err(e) => {
                    warn!(error = %e, "re-embed worker: 处理循环出错");
                    false
                }
            },
            Err(e) => {
                warn!(error = %e, "KB worker: 处理循环出错");
                false
            }
        };
        if !had_work {
            // 无待处理作业，等待后重试
            std::thread::sleep(std::time::Duration::from_secs(interval_secs));
        }
    }
}

/// 在独立线程中启动 KB worker 守护进程。
pub fn spawn_kb_worker_thread(db_path: PathBuf, config: Config, interval_secs: u64) {
    // 防御性检查：kb_enabled=false 时不启动 worker
    if !config.kb_enabled {
        info!("KB subsystem is disabled, worker thread not spawned");
        return;
    }
    std::thread::Builder::new()
        .name("kb-worker".to_string())
        .spawn(move || {
            // 在 worker 线程中创建独立的数据库连接（使用与主线程相同的 Config）
            let mut engine = SqliteEngine::with_config(&db_path, config.clone());
            if let Err(e) = engine.connect() {
                warn!(error = %e, "KB worker: 数据库连接失败");
                return;
            }
            if let Err(e) = engine.init_schema() {
                warn!(error = %e, "KB worker: 初始化 schema 失败");
                return;
            }
            info!("KB worker: 后台线程已启动");
            run_kb_worker_loop(&engine, &config, interval_secs);
        })
        .expect("无法创建 KB worker 线程");
}

// ---------------------------------------------------------------------------
// P5-023: 多 worker 并发处理
// ---------------------------------------------------------------------------

/// 启动多个并发 KB worker 线程。
///
/// 每个 worker 拥有独立的数据库连接。`claim_next_kb_job` 的原子认领
/// 保证同一作业不会被两个 worker 同时处理。
///
/// `worker_count` 为并发 worker 数量（建议不超过 CPU 核心数）。
/// `shutdown` 信号用于优雅停止所有 worker。
pub fn spawn_kb_worker_pool(
    db_path: PathBuf,
    config: Config,
    interval_secs: u64,
    worker_count: usize,
    shutdown: Arc<AtomicBool>,
) {
    if !config.kb_enabled {
        info!("KB subsystem is disabled, worker pool not spawned");
        return;
    }
    if worker_count == 0 {
        info!("KB worker pool: worker_count=0, nothing to spawn");
        return;
    }
    info!(worker_count, "KB worker pool: 启动 {} 个 worker", worker_count);

    for i in 0..worker_count {
        let db_path = db_path.clone();
        let config = config.clone();
        let shutdown = shutdown.clone();
        let thread_name = format!("kb-worker-{}", i);

        std::thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                let mut engine = SqliteEngine::with_config(&db_path, config.clone());
                if let Err(e) = engine.connect() {
                    warn!(worker = i, error = %e, "KB worker: 数据库连接失败");
                    return;
                }
                if let Err(e) = engine.init_schema() {
                    warn!(worker = i, error = %e, "KB worker: 初始化 schema 失败");
                    return;
                }
                info!(worker = i, "KB worker: 后台线程已启动");

                loop {
                    if shutdown.load(Ordering::Relaxed) {
                        info!(worker = i, "KB worker: 收到停止信号，退出");
                        break;
                    }
                    // 先处理文档处理作业，再处理 re-embed 作业
                    let had_work = match run_kb_worker_once(&engine, &config) {
                        Ok(true) => true,
                        Ok(false) => match run_reembed_worker_once(&engine, &config) {
                            Ok(true) => true,
                            Ok(false) => false,
                            Err(e) => {
                                warn!(worker = i, error = %e, "re-embed worker: 出错");
                                false
                            }
                        },
                        Err(e) => {
                            warn!(worker = i, error = %e, "KB worker: 处理循环出错");
                            std::thread::sleep(std::time::Duration::from_secs(interval_secs));
                            continue;
                        }
                    };
                    if had_work {
                        continue;
                    }
                    // 无作业时短暂等待，但提前检查 shutdown
                    for _ in 0..interval_secs {
                        if shutdown.load(Ordering::Relaxed) {
                            info!(worker = i, "KB worker: 收到停止信号，退出");
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
            })
            .expect(&format!("无法创建 {} 线程", thread_name));
    }
}
