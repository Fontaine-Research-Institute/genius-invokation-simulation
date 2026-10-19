# Crate-Compute (Compute Toolchain)

> [[Crate-Compute|中文]] · English

*Scope: the `compute` crate's responsibility, compute organization and configuration, script and CI contracts, invariants, tests, and pending work.*

---

## 1. Responsibility

Organizes Rust compute programs by "compute name": one computation = one bin folder. The script locates, compiles, and runs a program by name only and never passes compute parameters (parameter sets are pre-written in repository configuration). The compute programs themselves carry no game logic; they only do driving, parameter assembly, and output formatting.

- Directory: `new/crates/compute/`
- package: `gi-genius-invokation-compute` (no lib; only `src/bin/<compute-name>/`)

## 2. Key Concepts and Types

| Concept | Location | Notes |
|---|---|---|
| Crate | `new/crates/compute/Cargo.toml` | workspace member; `[dependencies]` empty by default; a computation adds engine / definitions / simulation itself (affects all computations) |
| One computation = one folder | `src/bin/<compute-name>/` | folder name = compute name = Cargo bin target name; Cargo auto-detects `src/bin/<name>/main.rs`, no `[[bin]]` needed |
| Entry | `src/bin/<compute-name>/main.rs` | fixed entry file name; other `.rs` files are submodules via `mod xxx;` |
| Case config | `src/bin/<compute-name>/compute.json` (optional) | `cases[]` (`name` / `cmd` / `expected_file` / `timeout_minutes`) plus a global `timeout_minutes` |
| Expected answers | `src/bin/<compute-name>/expected/*.json` | configuration stores a relative path only (D236); the file contains the JSON the program prints |
| Driver script | `new/tools/run-compute.py` | takes one compute name (or `--list` / `--configs` / `--all`); runs cases serially, compares, summarizes |
| Documentation | `new/tools/COMPUTE.md` | how to add a computation, config format, local runs, CI usage |
| CI | `new/.github/workflows/compute.yml` | manually triggered; `discover` scans configs, then a `compute` matrix: one job/runner per computation |
| Sample compute | `src/bin/ftk_state_count/` | `main.rs` + `helper.rs` + `compute.json` + `expected/{default,big}.json`; a placeholder that does not really compute |

**Sample behavior**: reads `--samples <n>` (parameters come from `compute.json`'s `cases[].cmd`), falls back to the `FTK_STATE_COUNT_SAMPLES` environment variable, then to `DEFAULT_SAMPLES`; prints one JSON line where `states = samples / 4` is a placeholder algorithm.

## 3. Public Entry Points

| Command | Purpose |
|---|---|
| `python tools/run-compute.py <compute-name>` | runs all cases of that computation serially, compares answers, prints a summary |
| `python tools/run-compute.py --list` | lists all compute names with config/answer statistics |
| `python tools/run-compute.py --configs` | lists only computations that have `compute.json` (used by CI discover) |
| `python tools/run-compute.py --all` | runs all configured computations and prints a green/red summary |

For each case the script actually runs: `cargo run --offline --release -p gi-genius-invokation-compute --bin <compute-name> -- <case cmd...>`.

**Program contract**: stdout carries the result only (a valid JSON value when an expected answer exists); progress/logs go to stderr; cases within one run are serial.

**Exit codes**

| Code | Meaning |
|---|---|
| 0 | all cases passed (including run-only cases without an expected answer) |
| 1 | illegal / missing compute name, illegal config; `--all` with red items |
| 3 | stdout is not valid JSON, or does not match the expected file |
| 124 | exceeded case / global `timeout_minutes` |
| other | the first failing compile/run exit code (127 = cargo not found) |

**CI**: `workflow_dispatch` inputs `name` (optional; empty = all configured computations) and `timeout-minutes` (default 30). `discover` scans with `--configs` and emits a JSON array; `compute` is a matrix with one runner per computation and `fail-fast: false`; artifacts are `compute-output-<compute-name>` (stdout / stderr / exit-code files). Adding a computation does not require workflow changes.

### 3.1 Real Source Excerpts (verbatim from the current repository)

**Sample compute entry `main.rs`**

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

**Submodule `helper.rs`**

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

**Case configuration `compute.json`**

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


## 4. Invariants and Prohibitions

> [!IMPORTANT]
> Compute name = folder name (lowercase letters / digits / `-` / `_`; no path separators or `..`); the entry is fixed as `main.rs`; the script accepts no compute parameters — parameter sets live in `compute.json`'s `cases[].cmd` (D231).

- Expected answers are files only: config allows `expected_file` only (relative to the compute directory, `/` separated); inline `expected` is unsupported (D236). A missing answer file produces a warning and is treated as "no answer".
- stdout carries results only; stderr carries logs, otherwise comparison fails and artifacts are polluted.
- Cases within one computation are serial; different computations run in parallel (one matrix runner each).
- Offline: the script always uses `cargo run --offline`; dependencies must be in `Cargo.lock` and resolvable offline.
- No game logic here: algorithms belong in engine / definitions for reuse.
- No need to edit `Cargo.toml` `[[bin]]` or the workflow.

## 5. Basis (Rulebook and D Numbers)

| Topic | Basis |
|---|---|
| Locate a Rust program by compute name, compile and run it, pass no compute parameters | D231 |
| Compute name = folder (may contain multiple `.rs` files) | D232 |
| Each compute directory may hold a config declaring the correct answer and whether to test (implemented as presence/absence of `expected_file`) | D233 (refined by D235 / D236) |
| CI: one independent job + runner per computation, in parallel | D234 |
| Multiple execution commands with their own expected answers; cases within a computation run in order | D235 |
| Expected answers referenced by file path only, never inlined | D236 |
| This crate has no rulebook mapping (a compute tool) | - |

## 6. Common Misuse

> [!WARNING]
> Do not pass compute parameters to `run-compute.py`, do not inline `expected`, and do not write results or logs to the wrong stream.

- Using `--bin my-compute` while the folder is `my_compute`: names are exact.
- Running cases of one computation concurrently: violates "serial within a computation".
- Copying game logic or hard-coding card data inside a computation.
- Adding heavy dependencies to `[dependencies]` for one computation (affects all computations).
- Editing the workflow after adding a computation: discover finds it automatically.

## 7. Tests

- This crate has no Rust unit tests (`cargo test -p gi-genius-invokation-compute -- --list` currently reports 0); its tests are compute cases plus expected-answer comparison:

| Verification | Notes |
|---|---|
| `python tools/run-compute.py --list` | should list `ftk_state_count` with case/answer statistics |
| `python tools/run-compute.py ftk_state_count` | three cases: default (with answer) / large sample (with answer) / run-only (no answer) |
| Expected files | `expected/default.json` (`samples 1000000 / states 250000`), `expected/big.json` (`samples 4000000 / states 1000000`) |
| CI | `new/.github/workflows/compute.yml` (manual trigger) |

## 8. Pending Work and Field Verification

| No. | Item |
|---|---|
| no number | The sample computation `ftk_state_count` is a placeholder (`samples / 4`); real FTK / enumeration algorithms come after engine / definitions support |
| no number | Before real compute results go into golden files, the algorithm and reproducibility rules must be settled |
| no number | CI is manual-trigger only (`workflow_dispatch`), not run on push |
| no number | This crate carries no game semantics and has no rulebook field-verification items |

---

Last reviewed: 2026-10-19 (code: `new/tools/run-compute.py:main`)
