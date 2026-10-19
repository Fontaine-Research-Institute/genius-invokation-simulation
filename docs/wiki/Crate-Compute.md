# Crate-Compute (算力工具链) 

> 中文 · [[Crate-Compute-EN|English]]

*本页范围: `compute` crate 的职责, 计算组织与配置, 脚本与 CI 契约, 不变量, 测试与待接入项.*

---

## 1. 职责

按"计算名"组织仓库里的 Rust 计算程序: 一个计算 = 一个 bin 文件夹; 脚本只按名字定位, 编译, 运行, 不向程序传计算参数 (参数组合预先写在仓库配置里) ; 计算程序本身不承载游戏逻辑, 只做驱动 + 参数装配 + 输出格式.

- 目录: `new/crates/compute/`
- package: `gi-genius-invokation-compute` (无 lib, 只有 `src/bin/<计算名>/`) 

## 2. 关键概念与类型

| 概念 | 位置 | 说明 |
|---|---|---|
| crate | `new/crates/compute/Cargo.toml` | workspace 成员; 默认 `[dependencies]` 为空; 某个计算需要 engine / definitions / simulation 时自己加 (影响全部计算)  |
| 一个计算 = 一个文件夹 | `src/bin/<计算名>/` | 文件夹名 = 计算名 = Cargo bin target 名; Cargo 自动识别 `src/bin/<名字>/main.rs`, 不用写 `[[bin]]` |
| 入口 | `src/bin/<计算名>/main.rs` | 入口文件名固定; 同目录其它 `.rs` 用 `mod xxx;` 当子模块 |
| 用例配置 | `src/bin/<计算名>/compute.json` (可选)  | `cases[]` (`name` / `cmd` / `expected_file` / `timeout_minutes`) + 全局 `timeout_minutes` |
| 正确答案 | `src/bin/<计算名>/expected/*.json` | 配置里只给相对路径 (D236) ; 内容是程序 stdout 应有的 JSON |
| 驱动脚本 | `new/tools/run-compute.py` | 只收一个计算名 (或 `--list` / `--configs` / `--all`) ; 用例串行, 比对, 汇总 |
| 说明文档 | `new/tools/COMPUTE.md` | 怎么加计算, 配置格式, 本地跑, CI 用法 |
| CI | `new/.github/workflows/compute.yml` | 手动触发; `discover` 扫配置 → `compute` matrix: 一个计算一个 job / runner 并行 |
| 示例计算 | `src/bin/ftk_state_count/` | `main.rs` + `helper.rs` + `compute.json` + `expected/{default,big}.json`; 占位, 不真算 |

**示例计算行为**: 读 `--samples <数>` (参数来自 `compute.json` 的 `cases[].cmd`) , 缺省读环境变量 `FTK_STATE_COUNT_SAMPLES`, 再兜到 `DEFAULT_SAMPLES`; 输出一行 JSON, `states = samples / 4` 为占位算法.

## 3. 公开入口

| 命令 | 作用 |
|---|---|
| `python tools/run-compute.py <计算名>` | 按 `cases` 顺序串行跑所有用例, 逐条比对答案并打印汇总 |
| `python tools/run-compute.py --list` | 列出全部计算名与配置 / 答案统计 |
| `python tools/run-compute.py --configs` | 只列出带 `compute.json` 的计算名 (CI discover 用)  |
| `python tools/run-compute.py --all` | 依次跑所有带配置的计算, 最后打绿 / 红汇总 |

脚本每条用例实际执行: `cargo run --offline --release -p gi-genius-invokation-compute --bin <计算名> -- <用例的 cmd...>`.

**程序契约**: stdout 只放结果 (有答案时必须是合法 JSON 值) ; 进度 / 日志写 stderr; 同一次运行内用例串行.

**退出码**

| 退出码 | 含义 |
|---|---|
| 0 | 全部用例通过 (含只运行, 无正确答案)  |
| 1 | 计算名非法 / 不存在 / 配置非法; `--all` 有红项 |
| 3 | stdout 不是合法 JSON, 或与答案文件不一致 |
| 124 | 超过用例 / 全局 `timeout_minutes` |
| 其它 | 第一条失败的编译 / 运行退出码 (127 = 找不到 cargo)  |

**CI**: `workflow_dispatch` 输入 `name` (可选, 留空 = 所有带配置的计算) 与 `timeout-minutes` (默认 30) ; `discover` 扫 `--configs` 输出 JSON 数组, `compute` matrix 一个计算一个 runner, `fail-fast: false`; artifact 为 `compute-output-<计算名>` (stdout / stderr / exit-code 三个文件) .新增计算不需要改 workflow.

### 3.1 真实源码
**示例计算入口 `main.rs`**

`new/crates/compute/src/bin/ftk_state_count/main.rs:1-43`

```rust
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

```

**子模块 `helper.rs`**

`new/crates/compute/src/bin/ftk_state_count/helper.rs:1-18`

```rust
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

```

**用例配置 `compute.json`**

`new/crates/compute/src/bin/ftk_state_count/compute.json:1-20`

```json
{
  "cases": [
    {
      "name": "默认",
      "cmd": [],
      "expected_file": "expected/default.json"
    },
    {
      "name": "大样本",
      "cmd": ["--samples", "4000000"],
      "expected_file": "expected/big.json"
    },
    {
      "name": "只跑（无正确答案）",
      "cmd": ["--samples", "1234"]
    }
  ],
  "timeout_minutes": 10
}

```


## 4. 不变量与禁止

> [!IMPORTANT]
> 计算名 = 文件夹名 (小写字母 / 数字 / `-` / `_`, 不能含路径分隔符或 `..`) ; 入口固定 `main.rs`; 脚本不接受计算参数, 参数组合写在 `compute.json` 的 `cases[].cmd` 里 (D231) .

- 正确答案只用文件: 配置只允许 `expected_file` (相对该计算目录, `/` 分隔) , 不支持内联 `expected` (D236) ; 答案文件缺失 → 警告 + 按"无答案"处理.
- stdout 只放结果; stderr 放日志, 否则比对失败, artifact 被污染.
- 同一计算内用例串行; 不同计算并行 (matrix 各一个 runner) .
- 离线: 脚本固定 `cargo run --offline`; 依赖必须在 `Cargo.lock` 里且本机能离线解析.
- 不承载游戏逻辑: 算法应留在 engine / definitions 复用.
- 不需要改 `Cargo.toml` 的 `[[bin]]`, 也不需要改 workflow.

## 5. 依据 (规则书与 D 编号) 

| 主题 | 依据 |
|---|---|
| 按计算名找 Rust 程序, 编译运行, 不传计算参数 | D231 |
| 计算名 = 文件夹 (可含多个 `.rs`)  | D232 |
| 每个计算目录可放配置: 正确答案 + 是否测试 (实现里"是否测试"由有无 `expected_file` 表达)  | D233 (D235 / D236 细化)  |
| CI: 每个计算一个独立 job + runner, 并行跑 | D234 |
| 多套执行命令 + 各自正确答案; 同计算用例顺序执行 | D235 |
| 正确答案只用文件路径引用, 不内联内容 | D236 |
| 本 crate 无规则书映射 (算力工具)  | — |

## 6. 常见误用

> [!WARNING]
> 不要给 `run-compute.py` 传计算参数, 不要内联 `expected`, 不要把结果 / 日志写错流.

- 用 `--bin my-compute` 但文件夹叫 `my_compute`: 名字精确匹配.
- 同一计算内并发跑用例: 违反"同计算串行".
- 在 compute 里复制游戏逻辑 / 硬编码卡牌数据.
- 为一个计算往 `[dependencies]` 塞重依赖 (影响全部计算) .
- 新增计算后去改 workflow: discover 自动发现, 不用改.

## 7. 测试

- 本 crate 自身没有 Rust 单元测试 (`cargo test -p gi-genius-invokation-compute -- --list` 当前 0 条) ; 它的测试是计算用例 + 答案文件比对: 

| 验证 | 说明 |
|---|---|
| `python tools/run-compute.py --list` | 应列出 `ftk_state_count` 与用例 / 答案统计 |
| `python tools/run-compute.py ftk_state_count` | 3 条用例: 默认 (有答案) / 大样本 (有答案) / 只跑 (无答案)  |
| 答案文件 | `expected/default.json` (`samples 1000000 / states 250000`) , `expected/big.json` (`samples 4000000 / states 1000000`)  |
| CI | `new/.github/workflows/compute.yml` (手动触发)  |

## 8. 待接入与待实测

| 编号 | 事项 |
|---|---|
| 无编号 | 示例计算 `ftk_state_count` 是占位 (`samples / 4`) , 不做真实计算; 真实 FTK / 枚举算法待 engine / definitions 实现后接入 |
| 无编号 | 真实计算结果入 golden 前需先定算法与可复现口径 |
| 无编号 | CI 只能手动触发 (`workflow_dispatch`) , 不是 push 自动跑 |
| 无编号 | 本 crate 无游戏语义, 不涉及 `留空.md` 的规则待实测项 |

---

最后核对: 2026-10-19 (代码: `new/tools/run-compute.py:main`) 
