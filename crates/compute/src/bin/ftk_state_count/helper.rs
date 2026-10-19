//! `ftk_state_count` 的子模块：演示「同一个计算文件夹里可以放多个 rs」。
//!
//! main.rs 里写 `mod helper;` 就能用这里的函数；路径是
//! crates/compute/src/bin/ftk_state_count/helper.rs。
//!
//! 这里放的是占位「算」法：不真算，只按样本数折算一个可复现的计数。

/// 默认采样数（环境变量 FTK_STATE_COUNT_SAMPLES 未设置时使用）。
pub const DEFAULT_SAMPLES: u64 = 1_000_000;

/// 占位「状态计数」：每 4 次采样折算 1 个状态，仅用于演示输出格式。
///
/// 真实计算应在这里实现自己的算法；参数从环境变量 / 文件读，
/// 不要从命令行读（脚本不会传）。
pub fn count_states(samples: u64) -> u64 {
    samples / 4
}
