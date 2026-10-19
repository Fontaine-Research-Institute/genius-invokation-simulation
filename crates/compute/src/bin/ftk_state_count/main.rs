//! 示例计算：`ftk_state_count`（D231 - D236 的样板）。
//!
//! 只演示「一个计算 = 一个文件夹 + 可放多个 rs + 多组用例」的写法，**不做真实计算**，
//! 只往 stdout 打一行 JSON。真实计算把这里换成自己的算法即可。
//!
//! 参数约定：run-compute.py **不接受计算参数**；一个计算要跑哪些参数组合，
//! 写在同目录 compute.json 的 cases 里，每条用例的 cmd 由脚本拼在
//! `cargo run ... --` 之后传进来。这里演示读 `--samples <数>`；
//! 环境变量 FTK_STATE_COUNT_SAMPLES 只在没有 `--samples` 时兜底（便于手工调试）。
//!
//! 输出约定：结果写 stdout（一行 JSON，方便 CI 直接当 artifact）；
//! 进度 / 日志写 stderr，别污染 stdout。
//!
//! 答案约定（D236）：正确答案只放文件，配置里用 expected_file 指向它；
//! run-compute.py 会解析 stdout 并与答案文件的 JSON 深度比较。

mod helper;

fn main() {
    let samples = samples_arg().unwrap_or_else(default_samples);
    let states = helper::count_states(samples);
    println!("{{\"compute\":\"ftk_state_count\",\"samples\":{samples},\"states\":{states}}}");
}

/// 解析 `--samples <数>`；参数来自 compute.json 用例的 cmd（脚本不解释语义）。
fn samples_arg() -> Option<u64> {
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--samples" {
            return args.next().and_then(|value| value.trim().parse::<u64>().ok());
        }
    }
    None
}

/// 没有命令行参数时的兜底：读环境变量，再兜到默认值。
fn default_samples() -> u64 {
    std::env::var("FTK_STATE_COUNT_SAMPLES")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(helper::DEFAULT_SAMPLES)
}
