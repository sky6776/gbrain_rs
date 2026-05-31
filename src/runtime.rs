//! 全局共享 Tokio 运行时模块
//!
//! 整个项目复用同一个 current_thread 运行时，避免在每次异步操作时
//! 重复创建运行时（线程生成、IO 驱动初始化等开销）。

use std::sync::OnceLock;

/// 全局共享的 Tokio current_thread 运行时
static SHARED_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 获取全局共享的 Tokio 运行时实例。
/// 首次调用时初始化，后续调用返回同一实例。
pub fn shared_runtime() -> &'static tokio::runtime::Runtime {
    SHARED_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("初始化共享 Tokio 运行时失败")
    })
}
