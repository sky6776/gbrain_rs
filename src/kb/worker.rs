//! KB 作业 worker — 后台轮询并处理 KB 文档作业
//!
//! 轮询 `claim_next_kb_job`，调用 `process_document_async`，
//! 然后标记作业完成或失败。可通过 CLI 子命令或 MCP 内置后台线程运行。

use crate::config::Config;
use crate::embedding::Embedder;
use crate::engine::BrainEngine;
use crate::error::{GBrainError, Result};
use crate::kb::engine::KbEngine;
use crate::kb::jobs::{claim_next_kb_job, complete_kb_job, fail_kb_job};
use crate::kb::pipeline::process_document_async;
use crate::kb::raptor::RaptorConfig;
use crate::sqlite_engine::SqliteEngine;
use std::path::PathBuf;
use std::sync::Arc;
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

/// 以守护进程模式运行 KB worker：持续轮询并处理作业。
/// `interval_secs` 为无作业时的轮询间隔。
pub fn run_kb_worker_loop(engine: &SqliteEngine, config: &Config, interval_secs: u64) -> ! {
    info!(interval_secs, "KB worker: 启动守护进程模式");
    loop {
        match run_kb_worker_once(engine, config) {
            Ok(true) => {
                // 处理了一个作业，立即检查下一个
                continue;
            }
            Ok(false) => {
                // 无待处理作业，等待后重试
                std::thread::sleep(std::time::Duration::from_secs(interval_secs));
            }
            Err(e) => {
                warn!(error = %e, "KB worker: 处理循环出错，等待后重试");
                std::thread::sleep(std::time::Duration::from_secs(interval_secs));
            }
        }
    }
}

/// 在独立线程中启动 KB worker 守护进程。
/// 返回数据库路径，调用方需自行创建 SqliteEngine。
pub fn spawn_kb_worker_thread(db_path: PathBuf, config: Config, interval_secs: u64) {
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
