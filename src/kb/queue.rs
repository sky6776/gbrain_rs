//! P2-2: KB 作业队列抽象层
//!
//! 当前实现仍以 SQLite `jobs` 表为主（`SqliteJobQueue` 包装）。
//! 引入 trait 是为后续切换到 NATS / Redis 等外部 MQ 做准备，让 worker
//! 只依赖接口而不绑定具体存储。
//!
//! P1/P2 阶段的最小实现：定义 trait + 提供 SQLite 默认实现的薄封装。
//! 大规模外部 MQ 接入留给后续迭代。

use crate::error::Result;
use crate::jobs::JobInput;
use std::path::PathBuf;

/// 队列中一条作业的抽象句柄。
///
/// `job_id` 在底层存储中唯一；`job_type` 用于路由 worker；
/// `payload` 为业务命令的序列化结果（JSON 字符串）。
#[derive(Debug, Clone)]
pub struct JobEnvelope {
    pub job_id: String,
    pub job_type: String,
    pub payload: String,
    pub attempts: i32,
}

/// 已被认领（claim）的作业句柄，需要 worker 在处理完成后 ack/nack。
#[derive(Debug, Clone)]
pub struct ClaimedJob {
    pub envelope: JobEnvelope,
    /// 释放锁的 token，ack/nack 时回传。
    pub claim_token: String,
}

/// KB 作业队列接口。
///
/// 实现方需要保证：
/// - `enqueue` 幂等（按 `dedup_key` 或 `job_type + payload hash` 去重）。
/// - `claim` 在并发 worker 下互斥（同一 job 同时只能被一个 worker 取到）。
/// - `ack` 成功后作业从可见队列移除；`nack(retry=true)` 重新入队，`nack(false)` 进 dead letter。
///
/// Trait 本身要求 `Send + Sync`，让 worker 可以只依赖队列接口并安全挂到
/// async runtime / 多 worker 调度层。SQLite 实现通过短连接访问数据库，不持有
/// `&Connection`，避免把 rusqlite 的非 Sync 连接泄漏到抽象层。
pub trait JobQueue: Send + Sync {
    fn enqueue(&self, job: JobInput) -> Result<String>;
    fn claim(&self, job_types: &[&str]) -> Result<Option<ClaimedJob>>;
    fn ack(&self, job_id: &str) -> Result<()>;
    fn nack(&self, job_id: &str, retry: bool, error: &str) -> Result<()>;
}

/// SQLite 默认实现的句柄（薄封装现有 `crate::jobs::JobQueue`）。
///
/// 真正的入队/认领逻辑仍由 `crate::jobs::JobQueue` 提供，此处把它包装成
/// `kb::queue::JobQueue` trait 的实例，便于后续替换为 NATS/Redis 实现。
#[derive(Debug, Clone)]
pub struct SqliteJobQueue {
    db_path: PathBuf,
}

impl SqliteJobQueue {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    fn with_queue<T>(
        &self,
        f: impl for<'a> FnOnce(&'a rusqlite::Connection, &crate::jobs::JobQueue<'a>) -> Result<T>,
    ) -> Result<T> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let queue = crate::jobs::JobQueue::new(&conn);
        f(&conn, &queue)
    }
}

impl JobQueue for SqliteJobQueue {
    fn enqueue(&self, job: JobInput) -> Result<String> {
        self.with_queue(|_, q| Ok(q.enqueue(job)?.to_string()))
    }

    fn claim(&self, job_types: &[&str]) -> Result<Option<ClaimedJob>> {
        // 现有 SQLite 队列按单一 job_type dequeue；这里逐类型尝试认领第一条
        self.with_queue(|_, q| {
            for jt in job_types {
                if let Ok(Some(claimed)) = q.dequeue_by_type(jt) {
                    return Ok(Some(ClaimedJob {
                        envelope: JobEnvelope {
                            job_id: claimed.id.to_string(),
                            job_type: claimed.job_type.clone(),
                            payload: claimed.payload.to_string(),
                            attempts: claimed.attempts,
                        },
                        claim_token: claimed.id.to_string(),
                    }));
                }
            }
            Ok(None)
        })
    }

    fn ack(&self, job_id: &str) -> Result<()> {
        let id: i64 = job_id.parse().unwrap_or(0);
        self.with_queue(|_, q| q.complete(id))
    }

    fn nack(&self, job_id: &str, retry: bool, error: &str) -> Result<()> {
        let id: i64 = job_id.parse().unwrap_or(0);
        self.with_queue(|conn, q| {
            // 现有 `fail` 已按 attempts < max_attempts 自动决定是否重试；
            // `retry=false` 时把 max_attempts 拉到当前 attempts，强制不重试。
            if !retry {
                // 通过直接 UPDATE 让 max_attempts = attempts，确保 fail 进入 'failed' 终态
                conn.execute(
                    "UPDATE jobs SET max_attempts = attempts WHERE id = ?1",
                    rusqlite::params![id],
                )?;
            }
            q.fail(id, error)
        })
    }
}
