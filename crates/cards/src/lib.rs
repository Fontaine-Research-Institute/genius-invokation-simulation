//! gi-genius-invokation-cards：具体卡牌层（D240 / D241 / D242 / D243）。
//!
//! **分工（D241）**：
//! - `gi-genius-invokation-definitions`：卡牌**资源与形状** —— 卡牌 id `CardId`、牌族
//!   `CardFamily`、牌类型、标签、费用组件 `CostComponent`、以及各类定义载体
//!   （`CharacterDef` / `ActionCardDef` / `SummonDef` / `StatusDef` / `EnemyDef` / `SkillDef` …）；
//! - 本 crate：**具体某张卡是什么** —— 每张卡一份数据与效果逻辑，按 D163 一卡一目录。
//!
//! **目录结构完全平移**（用户口径「结构还这么放」）：crate 根下仍是
//! `src/cards/<牌族>/<卡>/mod.rs`，与在 definitions 里时一模一样；
//! 源码嵌入的 path 口径也仍以 `src/cards/...` 开头。
//!
//! **依赖方向单向**：cards -> definitions（definitions 不依赖本 crate）；
//! 上层门面 `gi-genius-invokation-simulation` 默认**不**依赖本 crate（D243，可选 feature `cards`）。
//!
//! **源码嵌入（D242）**：宏 `card_source!` / `module_source!`、`SourceFile` / `SourceOwner`、
//! use 依赖解析器都是**通用定义**，留在 `gi_genius_invokation_definitions::source`；
//! 本 crate 只通过 [source] 维护**卡牌源码表** `SOURCE_FILES`（每张卡一行）。

// §8.3：上层 crate 禁止手写裸 + - * /（与 definitions 同口径）。
#![deny(clippy::arithmetic_side_effects)]

pub mod cards;
pub mod source;
