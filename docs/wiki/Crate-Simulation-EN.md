# Crate-Simulation (Public Facade)

> [[Crate-Simulation|中文]] · English

*Scope: the `simulation` facade crate's responsibility, re-export shape, features, invariants, tests, and pending work.*

---

## 1. Responsibility

The single public entry point of this workspace (facade crate): re-exports only, no logic. External users depend on this crate alone and get a uniform path to engine / definitions / richtext (and optionally cards).

- Directory: `new/crates/simulation/`
- package / lib: `gi-genius-invokation-simulation` / `gi_genius_invokation_simulation`

## 2. Key Concepts and Types

| Concept | Location | Notes |
|---|---|---|
| Facade crate | `src/lib.rs` | the whole file is four `pub use` lines |
| Facade path | `gi_genius_invokation_simulation::{engine, definitions, richtext}` | the uniform cross-crate import path |
| Optional cards | `#[cfg(feature = "cards")] pub use gi_genius_invokation_cards as cards;` | not compiled and not in the dependency graph by default (D243) |
| Features | `Cargo.toml`: `default = []`, `cards = ["dep:gi-genius-invokation-cards"]` | enable explicitly when concrete cards are needed |

Dependencies: engine, definitions, and richtext are required; cards is optional (D243). Direction: simulation is on top; cards depends on definitions / engine and never on simulation.

## 3. Public Entry Points

| Usage | Notes |
|---|---|
| `simulation = { path = "crates/simulation" }` | downstream depends on the facade only |
| `... features = ["cards"]` | enable when concrete cards are needed |
| `use gi_genius_invokation_simulation::{engine, definitions, richtext};` | the uniform path for cross-crate types |

### 3.1 Real Source Excerpts (verbatim from the current repository)

**The whole facade crate `src/lib.rs`**

`new/crates/simulation/src/lib.rs:1-23`

```rust
//! `gi-genius-invokation-simulation`：本工作区的**对外唯一入口**（门面 crate）。
//!
//! 本 crate **只有 re-export，没有逻辑**：
//! - `engine`：机制原语（饱和数值 / 实体 / 区 / 随机与选择）；
//! - `definitions`：游戏定义层（基础通用数据 / 牌族 / 类型 / 标签 / 卡牌**资源与形状**）；
//!   **不含**具体卡牌（D240 / D241 / D243）；
//! - `richtext`：富文本解析（只做字符串处理，零游戏语义）；
//! - `cards`：具体卡牌（每张卡的数据 + 效果逻辑），**可选** feature `cards`（默认关闭，D243）。
//!
//! 依赖方向由 Cargo 强制：definitions 依赖 engine 与 richtext；cards 依赖 definitions 与 engine；
//! 具体卡牌**不在默认依赖图里**（用户口径「整个模拟 crate 默认不含具体 cards」），
//! 需要时显式打开 `--features cards`。
//! 外部使用者只需依赖本 crate，不必直接依赖内部 crate；跨 crate 的导入路径统一是
//! `gi_genius_invokation_simulation::{engine, definitions, richtext}`（打开 feature 后还有 `cards`）。

pub use gi_genius_invokation_engine as engine;
pub use gi_genius_invokation_definitions as definitions;
pub use gi_genius_invokation_richtext as richtext;

/// 具体卡牌（D240 / D241）：**可选**，默认不编译、不进入依赖图（D243）。
#[cfg(feature = "cards")]
pub use gi_genius_invokation_cards as cards;

```


## 4. Invariants and Prohibitions

> [!IMPORTANT]
> The facade has no logic: no functions, types, or constants; only `pub use`. It excludes concrete cards by default (D243).

- No reverse dependency: simulation depends on nothing beyond cards; cards never depends on simulation.
- Uniform external path: cross-crate types are always used via `simulation::{engine, definitions, richtext}`, never by depending on internal crates directly.
- Features do not change semantics: enabling `cards` only adds the `cards` path.

## 5. Basis (Rulebook and D Numbers)

| Topic | Basis |
|---|---|
| Repo-wide crate prefix `gi-genius-invokation-*` | D78 |
| Facade package name = `gi-genius-invokation-simulation`; short directory names under `crates/` | D79 |
| Concrete cards split into their own crate (cards) | D240 / D241 |
| Facade excludes concrete cards by default (cards optional) | D243 |
| This crate has no rulebook mapping | - |

## 6. Common Misuse

> [!WARNING]
> Do not add "convenience functions" or business logic to the facade; do not make `cards` a default feature.

- Depending on both `simulation` and internal crates and importing types through two paths (version drift).
- Forgetting `--features cards` for concrete cards: `simulation::cards` does not exist by default (a compile error is expected).
- Expecting default features to compile cards: they do not (intentional).

## 7. Tests

- This crate has no tests (the facade contract is re-exports only; `cargo test -p gi-genius-invokation-simulation -- --list` currently reports 0).
- Correctness is guaranteed by compilation:

```bash
cargo build --offline -p gi-genius-invokation-simulation
cargo build --offline -p gi-genius-invokation-simulation --features cards
```

## 8. Pending Work and Field Verification

| No. | Item |
|---|---|
| #24 / #25 | Card multi-versioning and load-time version selection (D248-D251) are not implemented; the entry points are expected to live in the loading / facade layer |
| no number | The `solver` crate does not exist yet (`new/Cargo.toml` comment); it will be a separate crate |
| no number | The documentation site (D239: GitHub Pages first) and this wiki relationship is up to maintainers; this crate has no doc code |

---

Last reviewed: 2026-10-19 (code: `new/crates/simulation/src/lib.rs:pub use`)
