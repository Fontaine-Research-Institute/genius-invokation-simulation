# Crate-Simulation（对外门面）

> 中文 · [[Crate-Simulation-EN|English]]

*本页范围：`simulation` 门面 crate 的职责、re-export 形状、feature、不变量、测试与待接入项。*

---

## 1. 职责

本工作区的对外唯一入口（门面 crate）：只有 re-export，没有逻辑。外部使用者只依赖它，就能按统一路径拿到 engine / definitions / richtext（以及可选的 cards）。

- 目录：`new/crates/simulation/`
- package / lib：`gi-genius-invokation-simulation` / `gi_genius_invokation_simulation`

## 2. 关键概念与类型

| 概念 | 位置 | 说明 |
|---|---|---|
| 门面 crate | `src/lib.rs` | 全文只有 4 条 `pub use` |
| 门面路径 | `gi_genius_invokation_simulation::{engine, definitions, richtext}` | 跨 crate 导入的统一路径 |
| 可选卡牌 | `#[cfg(feature = "cards")] pub use gi_genius_invokation_cards as cards;` | 默认不编译、不进入依赖图（D243） |
| feature | `Cargo.toml`：`default = []`、`cards = ["dep:gi-genius-invokation-cards"]` | 需要具体卡牌时显式打开 |

依赖：engine、definitions、richtext 三个必需；cards 为可选依赖（D243）。依赖方向：simulation 在最上层；cards 依赖 definitions / engine，不依赖 simulation。

## 3. 公开入口

| 使用方式 | 说明 |
|---|---|
| `simulation = { path = "crates/simulation" }` | 下游默认只依赖门面 |
| `... features = ["cards"]` | 需要具体卡牌时显式打开 |
| `use gi_genius_invokation_simulation::{engine, definitions, richtext};` | 跨 crate 类型的统一路径 |

### 3.1 真实源码（逐字摘自当前仓库）

**门面 crate 全文 `src/lib.rs`**

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


## 4. 不变量与禁止

> [!IMPORTANT]
> 门面没有逻辑：不放函数、类型、常量；只允许 `pub use`。默认不含具体卡牌（D243）。

- 不反向依赖：simulation 不依赖 cards 之外的业务实现；cards 不依赖 simulation。
- 外部路径统一：跨 crate 类型一律经 `simulation::{engine, definitions, richtext}` 使用，不直接依赖内部 crate。
- feature 不改语义：打开 `cards` 只增加 `cards` 路径，不改变已有三个 re-export。

## 5. 依据（规则书与 D 编号）

| 主题 | 依据 |
|---|---|
| 全仓 crate 统一前缀 `gi-genius-invokation-*` | D78 |
| 门面包名 = `gi-genius-invokation-simulation`；`crates/` 目录用短名 | D79 |
| 具体卡牌拆到独立 crate（cards） | D240 / D241 |
| 模拟门面默认不含具体卡牌（cards 可选） | D243 |
| 本 crate 无规则书映射（门面不承载语义） | — |

## 6. 常见误用

> [!WARNING]
> 不要往门面里加「方便函数」或业务逻辑；不要把 `cards` 设成默认 feature。

- 下游同时依赖 `simulation` 和内部 crate、从两条路径导入类型（容易版本漂移）。
- 忘了对具体卡牌打开 `--features cards`：默认 `simulation::cards` 不存在（编译期报错是预期）。
- 以为默认 feature 会编译 cards：不会（有意）。

## 7. 测试

- 本 crate 没有测试（门面契约 = 只有 re-export；`cargo test -p gi-genius-invokation-simulation -- --list` 当前 0 条）。
- 正确性由编译保证：

```bash
cargo build --offline -p gi-genius-invokation-simulation
cargo build --offline -p gi-genius-invokation-simulation --features cards
```

## 8. 待接入与待实测

| 编号 | 事项 |
|---|---|
| #24 / #25 | 卡牌多版本与加载期选版本（D248–D251）未实现；将来「默认最新 / 加载后可单独指定」的入口预期落在加载 / 门面层 |
| 无编号 | `solver` crate 尚未建（`new/Cargo.toml` 注释）；将来作为独立 crate |
| 无编号 | 文档站（D239：GitHub Pages 为主）与本 Wiki 的关系待维护者定；本 crate 无文档代码 |

---

最后核对：2026-10-19（代码：`new/crates/simulation/src/lib.rs:pub use`）
