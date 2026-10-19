//! 【行动牌】**区域承载**：支援区 / 手牌卡状态 / 「当前元素骰费用」字段
//! （覆盖报告 R3「区域/状态承载」缺口）。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` 游戏区域 III / VI + 分工计划 §4.4.15）
//!
//! - III. 支援区：支援牌打出后放支援区；最多 4 张；占满后打出新的支援牌**需要先选择一张弃置**；
//!   多个支援牌同时结算时按**支援区排列顺序**。
//! - VI. 手牌：只有手牌里的行动牌可以打出 / 元素调和；手牌上限 10。
//! - 分工计划 §4.4.15（Lead 已定）：支援的值 = `None | Flag(bool) | Count(Nat)` 三选一；
//!   `cost` 只是花费属性，「当前情况」属于被花的东西 —— 当前费用 = 基础费用 + 加费 / 减费。
//!
//! ## 三个块（各自**自持状态**、对外**只读指针**、可建多个、变量型：D182 / D183）
//!
//! | 块 | 文件 | 关键规则 |
//! |---|---|---|
//! | 支援区 | `support.rs` | 上限 4；满 4 时**选择弃置**（`enumerate_select` 分支）；值三选一 |
//! | 手牌卡状态 | `hand_state.rs` | 复用 `StateList`；手牌无序（每张牌一个列表，pos 固定 0） |
//! | 当前费用 | `cost.rs` | 基础费用 + 加费 / 减费；复用 `resources::dice::{CostComponent, apply_delta}` |
//!
//! ## 时机派发（D185 / D186）
//!
//! 支援区 / 手牌卡状态各自定义时机键并主动派发（`SupportTiming` / `HandCardStateTiming`），
//! 走 `timeline::dispatch`（按结算顺序排好、**不执行**）；「当前费用」是纯数据字段，没有时机。
//!
//! ## 统计（D187）
//!
//! 支援区 / 手牌卡状态的统计都下沉到 `stats/`；feature `stats` 关闭时零大小空操作，
//! 统计不参与局面等价；对外只给 `stats()` 只读指针（D183）。

pub mod cost;
pub mod hand_state;
pub mod record;
pub mod stats;
pub mod support;

pub use cost::CurrentCost;
pub use hand_state::{
    execute_hand_card_states, ExtraHandCardStatus, HandCardStateEffect, HandCardStateEntry,
    HandCardStatePending, HandCardStateResolution, HandCardStateTiming, HandCardStateTrigger,
    HandCardStateTriggers, HandCardStates, HandCostIncrease, HandEmpowerment, HandInvalidation,
    HandUntunable, HAND_COST_INCREASE, HAND_EMPOWERMENT, HAND_INVALIDATION, HAND_STATE_POS,
    HAND_UNTUNABLE,
};
pub use support::{
    execute_support, SupportAddBranch, SupportAddResult, SupportEffect, SupportEntry, SupportPending,
    SupportDuration, SupportResolution, SupportTickResolution, SupportTiming, SupportTrigger,
    SupportTriggers, SupportValue, SupportZone, SUPPORT_CAP,
};
