//! **状态层**：一局对局的完整状态（D34 / D42 / D182 / D183）。
//!
//! ## 形态（D34 / D42）
//!
//! 一局对局的根状态是 [`DefState`]：敌我两个**具名字段** `me`（我方）与 `opp`（敌方），
//! 外加一个回合数 `round`（0 = 还没开始第一回合；终局判定要用）与先手牌手
//! `first_player`（本回合行动 / 结束阶段的起始方；引擎 `to_move` 读它）。
//! 详见 `def_state.rs` / `terminal.rs`。
//! 敌我**不用编号、不用 `[T; 2]` + 下标、不用带判别值的 `Side` 枚举**（D34）：
//! 就是同一套 `PlayerDefs` 结构的**两个实例**，代码要用哪一方就写哪个字段（D42：区 = 状态里的变量）。
//!
//! ## 文件
//!
//! | 文件 | 内容 |
//! |---|---|
//! | `player.rs` | [`PlayerDefs`]（一方的全部区，含下落攻击标记与击倒区内部记录）+ [`HandCosts`]（每张手牌的当前费用） |
//! | `def_state.rs` | [`DefState`]（`me` / `opp` 两个具名实例 + 回合数 + 先手牌手 + 只读指针入口） |
//! | `terminal.rs` | 终局判定（全灭 / 双方同时全灭 / 第 15 回合上限）+ 引擎 `GameRules` 适配（`terminal` / `to_move`） |
//! | `flows/` | **串联流程**：开局 / 切换角色 / 击倒清理 / 复苏 / 下落标记清除 / 结束阶段 / 回合开始 |
//!
//! ## 口径（D182 / D183：区自持状态、对外只读指针）
//!
//! - 每个区都是**自己持有自己的状态**的普通值类型（各区在 `families/*/zones`、
//!   `families/*/areas`、`resources/dice` 里，本层**不重写**任何区域逻辑，只做组合）；
//! - `PlayerDefs` / `DefState` 的字段**全部私有**，对外只有 `&self` 的只读指针
//!   （`PlayerDefs::hand` -> `&HandState`、`PlayerDefs::dice` -> `&DicePool` …）；
//! - 可变入口只有 `pub(crate)` 的 `PlayerDefs::parts_mut`（`state::flows` 这一层用），
//!   本 crate 之外拿不到 `&mut`；
//! - 本层**不做数值运算、不解释卡牌语义**：数值 / 判定都在各区与 `rules::ops` 里（D133）。
//!
//! ## 构造 / 克隆 / 相等（本层写清的三件事）
//!
//! - **构造**：[`DefState::new`] / [`PlayerDefs::new`] 建两个完全空的实例（所有区空、骰池空）；
//!   正式建局时由上层把各区的初始值（卡组、角色牌、初始骰池…）填进来（见 `flows/opening.rs`）；
//! - **克隆**：`Clone` 是**深拷贝**（整体局面快照）。这与「各区都是纯函数、返回新状态」一致：
//!   流程吃 `&DefState`、克隆一份改完返回，入参一个字节都不动（见 `flows/mod.rs`「为什么是纯函数」）；
//! - **相等**：`PartialEq` **不比较统计**（D187：统计不改变游戏语义）。各容器自己的 `PartialEq`
//!   已经忽略 `stats`，所以本层直接按字段比较即可；**不实现 `Eq`**，因为骰池里的折叠组带
//!   精确概率质量（`ExactNat128`，`DicePool` 本身只有 `PartialEq`）。
//!
//! ## 待接入（本层刻意不做）
//!
//! - **骰子支付**：切换角色 / 打出手牌的费用支付归骰池（D121 / D133），本层只串「区域与时机」；
//! - **换人 / 换牌的策略**：本层只提供「给定选择后世界怎么变」，选谁 / 换哪几张由上层搜索决定；
//! - **支援牌的持续回合**：`SupportEntry` 目前没有 `duration` 字段（`areas/support.rs` 只读），
//!   结束阶段流程先只减出战状态与召唤物，支援的持续回合待接入；
//! - **手牌卡状态的跨区搬运**：见 `areas/hand_state.rs` 的待复核，本层不搬。

pub mod def_state;
pub mod flows;
pub mod player;
pub mod terminal;

/// 测试夹具（`cfg(test)`；只在单测里编进 crate）。
#[cfg(test)]
pub(crate) mod testing;

pub use def_state::DefState;
pub use player::{HandCosts, PlayerDefs};
pub use terminal::{Outcome, ROUND_LIMIT};
