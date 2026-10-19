//! **回合流程骨架**：回合开始（计数清零 + 回合上限判定）-> 投掷阶段（8 骰 + 1 次重投）->
//! 行动阶段（由调用方驱动；宣布结束 F4）-> 结束阶段（各区块 tick + 双方各抓 2 张，F10）->
//! 回合结束广播。
//!
//! ## 依据（唯一权威）
//!
//! - 规则书·回合流程：「每个回合，都会按照『投掷阶段』『行动阶段』『结束阶段』顺序进行」；
//! - 投掷阶段：「每回合的投掷阶段中，会投掷 8 个『元素骰』。投掷完成后，还有 1 次机会
//!   选择任意数量元素骰进行『重投』」；
//! - 行动阶段：「从『先手牌手』开始，双方『轮流进行行动』」；双方都宣布结束后进入结束阶段；
//! - 结束阶段：「1、从『先手牌手』开始，处理在『结束阶段』触发的卡牌效果。2、每位牌手从
//!   自己的牌堆中抓 2 张牌。3、开始新的回合！」；
//! - 特殊词条 持续回合 / 可用次数：「每个结束阶段，都会使『持续回合』-1」「每次触发消耗 1 次
//!   『可用次数』」-> 结束阶段 tick 复用 state::flows::end_phase（出战状态 + 召唤物）；
//! - 回合开始（计数清零）：state::flows::round_begin::begin_round（技能每回合使用次数清零）。
//!
//! ## 本层做什么 / 不做什么
//!
//! - 做：把上面几段串成可调用的纯函数；投掷 / 重投走骰子块（D190：观测点展开成
//!   「骰池分支 + 精确概率」，**不采样**）；阶段广播用本层定义的时机键 + timeline::dispatch
//!   （D185：阶段时机键由需要广播的块自己定义）；
//! - 不做：行动阶段的决策 / 行动权（由调用方驱动，规则书·进行行动）、击倒清理由上层调
//!   `state::flows::defeat`（本层只读 `DefState` 判终局，见「终局判定接线」）；
//! - 元素幻变（L9）：[roll_me_with_transmutation] / [roll_opp_with_transmutation] 用**显式开关**
//!   接进投掷（有对应卡时 8 颗里固定 2+2；无卡时与 [roll_me]/[roll_opp] 完全一致）；
//!   「首次生成」= 第 1 轮（[RollTransmutation::opening]），时序待实测（留空 #15 / #64）；
//!   单独钩子仍在 [crate::flow::elemental_transmutation]（非投掷入口用）。
//!
//! ## 观测展开 / 折叠回（D190 / D191）
//!
//! 投掷与重投是**观测点**：本层返回分支列表（每支 = 骰池 + 精确概率），不做采样；
//! 调用方拿到分支后可以选择「先展开哪一侧、观察到什么再决定重投哪些骰」。两侧分别返回
//! （roll_me / roll_opp），避免 8 颗骰的联合分支爆炸；需要联合时由调用方按需组合。
//! 折叠回见 flow::fold_states：投掷分支各支局面不同、通常不需要折叠；**结束阶段抓牌**
//! 的两侧分支会交叉组合后折叠（见下节 F10）。
//!
//! ## 结束阶段抓 2 张（F10；S2.4 第 2 步）
//!
//! 规则书·回合流程 结束阶段：「2、每位牌手从自己的牌堆中抓 2 张牌。」
//!
//! - [TurnEndOutcome] 返回**分支列表**（[TurnEndBranch]），不是单局面：牌库块的抓牌原语
//!   `PileState::draw_many` 把随机段展开成「分支 + 精确概率」（D190 不采样）；
//! - 双方**分别**展开再**交叉组合**（概率 `exact_mul`），最后用 [fold_states] 折叠相同局面
//!   （D191），概率精确相加；
//! - 牌库没有牌 -> 这一侧抓不到牌（规则书·游戏区域 V「牌堆中已经没有卡牌时，牌手不会获得
//!   手牌」），按「原样一支、概率 1」参与组合，结束阶段照常进行；
//! - 手牌上限 10 / 溢出自动弃置由牌库块处理（规则书·游戏区域 VI）。
//!
//! **执行顺序（S2.4）**：本层是派发的调用方，先按「先手方 -> 另一方；同侧按 S7.5 来源类别 ->
//! 进场先后」**执行**结束阶段触发的效果（第 1 步），之后才做双方各抓 2 张（第 2 步）——
//! 所以 [TurnEndOutcome] 的每个分支都已按规则书顺序落地；[TurnEndOutcome::end_phase] /
//! [TurnEndOutcome::pending] 保留供观测，**调用方不要再执行**。
//!
//! TODO（待接入）：
//! 1. **新抓手牌的费用表**：需要按基础费用建 `CurrentCost`（缺 base_cost 回调，
//!    与 play_card 的费用表同步是同一项，D184 第 3 步）；
//! 2. **抽到牌携带的待展开随机状态**（`DrawBranch::drawn_marks`，D188）：本层暂不物化，
//!    等状态层给出「抽牌后状态落地」入口再接。
//!
//! ## 结束阶段支援牌持续回合（Z6；S8.6）
//!
//! 规则书·特殊词条 持续回合：「每个结束阶段，都会使『持续回合』-1。『持续回合』耗尽后，
//! 会立刻弃置这张牌。」
//!
//! - 放在 S2.4 第 1 步「效果阶段」内、抓牌之前：双方支援区各 tick 一次
//!   （`SupportZone::tick_end_phase_with_triggers`，区层实现；本层只调用，不重写数值）；
//! - tick 后的支援区状态已写进返回分支（持续回合 -1 / 耗尽弃置）；
//! - 派发出来的「弃置时」待执行按 D186 交给调用方（[TurnEndOutcome::support_me] /
//!   [TurnEndOutcome::support_opp]，本层不执行效果）；执行顺序：先手方列表先（S7.5 第 1 条）。
//!
//! TODO / 待实测（规则书没写清，不自行规定）：
//! 1. 规则书 S8.6 只有通用条款，**没说支援牌持续回合在哪个时机递减**；区层按「结束阶段」
//!    实现（与卡牌 / 状态持续回合的通用条文一致），待实测；
//! 2. 支援效果严格来说应在抓牌前执行，但按 D186 交给调用方后会在抓牌之后执行 ——
//!    与 S2.4「1 效果 -> 2 抓牌」的严格顺序有偏差，待复核。
//!
//! ## 终局判定接线（Task 2；方案 A：只读 DefState）
//!
//! 规则书·胜利条件（S1.4）+ 对局回合上限（S7.7）；判定本体在 `state::terminal::terminal`。
//! 本层按 Lead 拍板的方案 A 接线：
//!
//! - [round_begin] / [begin_next_round] -> [RoundBeginOutcome::outcome]：**回合上限**（第 15 回合
//!   开始仍未分胜负 -> 双方都输）；
//! - [TurnEndBranch::outcome]：结束阶段每支局面的判定（供调用方在结束阶段待执行 + defeat 清理后
//!   用 [crate::flow::game_end::settlement_outcome] 复判）；
//! - **[crate::flow::game_end::settlement_outcome]** 薄入口：**必须在「每步结算 + 调用方完成
//!   `state::flows::defeat` 清理」之后调用**（HP 归 0 但清理未跑 -> 角色仍在角色区 ->
//!   如实判 `Outcome::Ongoing`）；
//! - 击倒清理由上层调 `state::flows::defeat`（需要按角色注入 `DefeatHpReset`，角色定义 /
//!   成长不在 definitions 层）—— 本层不改 `state/**`，也不替上层做清理。
//!
//! ## 宣布回合结束与先手归属（F4）
//!
//! 规则书·进行行动 战斗行动【宣布结束】：「放弃本回合的行动，『宣布回合结束』。先『宣布回合结束』
//! 的牌手会成为『先手牌手』，并在下回合的行动阶段优先行动。这之后，未『宣布回合结束』的牌手
//! 可以继续行动，直到『宣布回合结束』。双方都『宣布回合结束』后，行动阶段结束，进入本回合的
//! 『结束阶段』。」
//!
//! - [RoundEndState] 承载**宣布记账**（双方是否已宣布 + 本回合先宣布者）；
//!   **本回合先手牌手**写进根状态 `DefState::first_player`（见下两条），未裁定就是
//!   `None`（规则书没规定第一回合先手归谁，不编默认值）；
//! - [declare_round_end_me] / [declare_round_end_opp]：置位、记先宣布者；**双方都宣布后**
//!   派发 [TurnTiming::ActionPhaseEnd]、置 `action_phase_over`，并把本回合先手牌手
//!   （[RoundEndState::first_player]）写进 `DefState`；
//! - [end_phase_by_first_player]：读 [RoundEndState::first_player] 决定结束阶段先手方，
//!   并把该值写进返回分支的 `DefState`（调用方拿到即一致）；
//!   [TurnEndOutcome::ordered_end_phase] 把四块待执行按「先手方：出战状态 -> 召唤物；
//!   再另一方同样」摊平成有序序列（S2.4 第 1 步）；
//! - [begin_next_round]：回合开始（回合数 +1、计数清零、终局判定）+ 把本回合先宣布者接成
//!   下回合先手（S3.6）并写进 `DefState::first_player`。
//!
//! ### TODO（规则书没写清，不自行规定；同步列 留空.md）
//!
//! 1. **第一回合的先手怎么定**：规则书·游戏准备只说双方选出战角色，没写先手裁定。
//!    本层用 [RoundEndState::with_first_player] / `DefState::with_first_player` 交给外部
//!    （开局 / 加载流程）；没给时 [end_phase_by_first_player] 返回
//!    [EndPhaseError::FirstPlayerUnknown]；
//! 2. **结束阶段用的「先手牌手」指谁**：S3.6 说先宣布者「会成为先手牌手，并在下回合的行动阶段
//!    优先行动」，S2.4 又说结束阶段「从先手牌手开始」；两处都没写清结束阶段用的是**本回合先手**
//!    还是**刚宣布的先手**。本层把两者分开存：[RoundEndState::first_player]（本回合先手，
//!    结束阶段用它）/ [RoundEndState::first_declarer]（下回合先手）。若实测为「先宣布者立即
//!    接管结束阶段顺序」，把 [end_phase_by_first_player] 改读 [RoundEndState::first_declarer]
//!    即可；
//! 3. **先宣布者之后能否继续行动**：规则书说「放弃本回合的行动」-> 已宣布的一方不能再行动；
//!    本层只给 [RoundEndState::has_ended] 查询，行动权拦截归调用方的行动循环；
//!    本入口也**不校验「现在轮到哪一方」**（当前牌手 / 行动权不在本层状态里）；
//! 4. （F10 的 TODO 见上面「结束阶段抓 2 张」一节；F4 本身没有未决条目。）
//!
//! ## 待接入 / 待复核
//!
//! - **投掷阶段的阶段广播**：roll_phase 只派发，不改变掷骰结果（掷骰结果由 roll_me / roll_opp
//!   的观测展开给）；
//! - **行动阶段开始 / 结束**：只做广播（行动权 / 行动计数归调用方）；
//! - **结束阶段抓牌后的手牌费用表 / 待展开随机状态**：见 F10 一节的 TODO；
//! - **重投选择**：本层接收「要重投哪些骰面各几颗」，由调用方在观测后给（换牌 / 重投的策略在工作区
//!   搜索层，D25 / D26 不做选择策略）；给不存在的骰面 -> 空分支（视为非法选择）。

use crate::families::action_card::areas::support::{SupportPending, SupportTriggers};
use crate::families::character::zones::combat_status::{
    execute_combat_status, CombatStatusPending, CombatStatusTriggers,
};
use crate::families::summon::zones::{execute_summon, SummonPending, SummonTriggers};
use crate::flow::fold_states;
use crate::flow::game_end::{settlement_outcome, Outcome};
use crate::flow::hooks::{apply_draw_branch, DrawHooks};
use crate::flow::pending::FlowPending;
use crate::keywords::target::Perspective;
use crate::resources::dice::{DicePool, DieFace};
use crate::rules::ops::opening::{is_opening_round, ElementalTransmutation};
use crate::rules::ops::order::{EffectSource, SOURCE_ORDER};
use crate::rules::ops::stage::END_PHASE_DRAW;
use crate::state::flows::end_phase::{self, EndPhasePending};
use crate::state::flows::round_begin::begin_round;
use crate::state::{DefState, PlayerDefs};
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, nsub, EntityId, Nat};

/// 投掷阶段的骰子数：8（规则书·回合流程 投掷阶段）。
pub const ROLL_COUNT: Nat = Nat::new(8);

/// 回合里**需要广播的阶段时机键**（D185：由需要广播的块自己定义）。
///
/// 这是**阶段广播**（「现在到哪个阶段了」），不是具体卡牌时机；具体卡的「结束阶段」
/// 触发登记在各区自己的注册表里、由各区在 tick 时派发（本层只做流程骨架）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TurnTiming {
    /// 回合开始（计数清零之后）。
    RoundBegin,
    /// 投掷阶段开始。
    RollPhase,
    /// 行动阶段开始。
    ActionPhaseStart,
    /// 行动阶段结束（双方都宣布回合结束后）。
    ActionPhaseEnd,
    /// 结束阶段（各区块 tick 之后）。
    EndPhase,
    /// 回合结束（进入下一个回合之前）。
    RoundEnd,
}

/// 回合同步的阶段效果函数指针：fn(&mut 对局, &Event, owner)。
pub type TurnEffect = fn(&mut DefState, &Event, EntityId);

/// 回合同步的阶段触发登记项。
pub type TurnTrigger = Trigger<TurnTiming, TurnEffect>;

/// 回合同步的阶段触发注册表（每个牌手一个具名实例，D34；本层只接收打出方 / 当前方的那一张）。
pub type TurnTriggers = TriggerRegistry<TurnTiming, TurnEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type TurnPending = Queued<TurnEffect>;

/// 回合流程的**外部依赖**（只借不拥有）。
pub struct TurnEnv<'a> {
    /// 阶段广播注册表。
    pub triggers: &'a TurnTriggers,
    /// 结束阶段 tick 用的出战状态区触发注册表。
    pub combat_status: &'a CombatStatusTriggers,
    /// 结束阶段 tick 用的召唤物区触发注册表。
    pub summons: &'a SummonTriggers,
    /// 结束阶段 tick 用的支援区触发注册表（Z6：支援牌持续回合）。
    pub supports: &'a SupportTriggers,
    /// 抓牌路径的**注入点**（F10 手牌费用表 + D188 drawn_marks 物化）。
    ///
    /// 不注入时传 DrawHooks::none()：新抓手牌的费用表不同步；drawn_marks 仍会物化进
    /// 手牌卡状态表（D188）。调用方需要费用表时注入 base_cost 回调（卡牌定义在上层）。
    pub draw: DrawHooks<'a>,
}

/// 结束阶段的结果：**分支列表**（tick 已应用、结束阶段效果已执行、双方各抓 2 张）+
/// 各区 tick / 阶段广播的待执行（保留供观测）。
///
/// **顺序（S2.4）**：第 1 步「从先手牌手开始，处理在结束阶段触发的卡牌效果」由本层作为调用方
/// **先执行**（先手方 -> 另一方；同一侧按 S7.5 来源类别 -> 进场先后）；第 2 步「每位牌手抓 2 张」
/// 在效果全部落地**之后**再做。因此返回的每个分支都已按规则书顺序结算完。
///
/// ## 为什么是分支列表（D190 / D191；F10）
///
/// 规则书·回合流程 结束阶段第 2 步：「每位牌手从自己的牌堆中抓 2 张牌。」抓牌可能遇到
/// 随机段（牌库块把它展开成「抓牌分支 + 精确概率」，不采样）；本层两侧分别展开再**组合**
/// （概率精确相乘），最后用 [fold_states] 折叠相同局面（D191）：
///
/// - 牌库没有牌 -> 这一侧**抓不到牌**（规则书·游戏区域 V：牌库空了不会获得手牌），
///   该侧按「原样一支、概率 1」处理，结束阶段照常进行；
/// - 牌库分支天然多支 -> 交叉组合成多支；局面完全相同的分支折叠、概率精确相加；
/// - 待执行（`end_phase` / `pending`）与分支无关：派发只排序、不执行（D186），每支相同。
///
/// 手牌上限 10 / 溢出自动弃置已在牌库块内处理（规则书·游戏区域 VI）。
pub struct TurnEndOutcome {
    /// 各分支（tick 与抓 2 张都已应用；顺序 = 首次出现顺序）。
    pub branches: Vec<TurnEndBranch>,
    /// 出战状态 / 召唤物 tick 派发出来的待执行（**已由本层按 S2.4/S7.5 顺序执行**；
    /// 保留供观测 / 记录，调用方不要再执行）。
    pub end_phase: EndPhasePending,
    /// 阶段广播（EndPhase）派发出来的待执行（**已由本层执行**；保留供观测，调用方不要再执行）。
    pub pending: Vec<TurnPending>,
    /// 我方支援区 tick（持续回合 -1 / 耗尽弃置）派发出来的待执行（**未执行**；D186）。
    ///
    /// 顺序 = 支援区排列顺序（规则书·游戏区域 III）；先手 / 后手的执行先后由调用方按
    /// 本次结束阶段的先手牌手决定：**先手方的 support 列表先执行**（S7.5 第 1 条）。
    pub support_me: Vec<SupportPending>,
    /// 敌方支援区 tick 派发出来的待执行（**未执行**；与 [TurnEndOutcome::support_me] 同口径）。
    pub support_opp: Vec<SupportPending>,
}

/// 结束阶段的一支分支：tick + 抓牌后的完整局面 + 精确概率 + 终局判定。
#[derive(Clone, Debug)]
pub struct TurnEndBranch {
    /// 这一支的完整局面（tick 与抓 2 张都已应用）。
    pub state: DefState,
    /// 这一支的精确概率。
    pub probability: Fraction,
    /// 这一支的终局判定（只读；调用时点见 [settlement_outcome]）。
    pub outcome: Outcome,
}

// ======================================================================
// 回合开始
// ======================================================================

/// **回合开始**的结果：新的一局 + 阶段广播待执行 + 终局判定（回合上限）。
#[derive(Clone, Debug)]
pub struct RoundBeginOutcome {
    /// 回合开始后的局面（回合数 +1、双方每回合计数清零）。
    ///
    /// [begin_next_round] 还会把新回合的先手牌手写进 [DefState::first_player]；直接调用
    /// [round_begin] 时沿用入参局面里的值（第一回合由开局流程外部裁定）。
    pub state: DefState,
    /// [TurnTiming::RoundBegin] 派发出来的待执行。
    pub pending: Vec<TurnPending>,
    /// 回合开始后的终局判定（S7.7：第 15 回合开始仍未分胜负 -> 双方都输）。
    pub outcome: Outcome,
}

/// **回合开始**：双方每回合计数清零 + 回合数 +1（state::flows::round_begin）+ 派发 RoundBegin
/// + 终局判定（回合上限，S7.7）。
///
/// 依据：state::flows::round_begin::begin_round（技能使用次数清零、回合数推进）；
/// 规则书·其他规则 对局回合上限（第 15 回合开始即终局）。
pub fn round_begin(state: &DefState, env: &TurnEnv<'_>) -> RoundBeginOutcome {
    let next = begin_round(state);
    let pending = broadcast(env, TurnTiming::RoundBegin);
    let outcome = settlement_outcome(&next);
    RoundBeginOutcome {
        state: next,
        pending,
        outcome,
    }
}

// ======================================================================
// 投掷阶段（8 骰 + 1 次重投；观测展开，D190）
// ======================================================================

/// **投掷阶段开始**：只派发阶段广播（掷骰结果由下面的 roll_me / roll_opp 给）。
pub fn roll_phase(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::RollPhase))
}

/// **元素幻变（K67）对投掷阶段的接入开关**（卡面 331004–331012「投掷阶段：总是投出 2 个 X
/// 和 2 个 Y」）。
///
/// **显式开关 / 条件**：调用方只在「对应的元素幻变卡在场 + 队伍元素集合恰等于 {X,Y}（K23）」时
/// 传 `Some`；`None` = 没有对应卡 / 条件不满足 -> 走**普通投掷**，现有流程完全不变。
#[derive(Clone, Copy, Debug)]
pub struct RollTransmutation {
    /// 卡牌绑定的两种元素（具体卡由卡牌层给；rules 层只做通用判定）。
    pub rule: ElementalTransmutation,
    /// `true` = 第 1 轮「首次生成」语义（卡面顺序：抓 -> 弃 -> 上场后 4 骰转 2+2）；
    /// `false` = 每轮投掷阶段的「总是投出 2+2」。
    ///
    /// 两种语义的**骰子运算相同**（`apply_roll_phase` 即 `convert_opening_dice`），
    /// 但触发 / 时序不同，待实测（留空 #15 / #64）。
    pub opening: bool,
}

/// 元素幻变固定骰：两种绑定元素各 2 颗（卡面原文）。
pub const TRANSMUTATION_FIXED_EACH: Nat = Nat::new(2);

/// 元素幻变把 **4 颗**骰转换成 2+2（`convert_opening_dice` 的固定 4）。
pub const TRANSMUTATION_CONVERTED_DICE: Nat = Nat::new(4);

/// **我方投掷 8 颗骰（带元素幻变开关；L9）**。
///
/// `Some` 时按「8 颗里固定 2 个 X + 2 个 Y」生成：等价于「先投 8 颗、再把其中 4 颗转成 2+2」
/// 的分布（骰子独立同分布；折叠组不能直接 `convert`，所以实现取等价构造 = 4 颗随机 + 2+2 固定）。
/// `None` -> 与 [roll_me] 完全一致。
///
/// TODO（规则书 / 卡面没写清，待实测）：
/// - 「投掷 2+2」的时序（重投之前 / 之后）与「首次生成」的时序，见留空 #15 / #64；
/// - 若实测「2+2 是额外追加在 8 颗之外」或「替换的不是 4 颗」，需调整本函数。
pub fn roll_me_with_transmutation(
    state: &DefState,
    transmutation: Option<RollTransmutation>,
) -> Vec<(DefState, Fraction)> {
    roll_side_with_transmutation(state, true, transmutation)
}

/// **敌方投掷 8 颗骰（带元素幻变开关）**（与 [roll_me_with_transmutation] 共用实现，D35）。
pub fn roll_opp_with_transmutation(
    state: &DefState,
    transmutation: Option<RollTransmutation>,
) -> Vec<(DefState, Fraction)> {
    roll_side_with_transmutation(state, false, transmutation)
}

/// 一侧的投掷（带元素幻变开关）；`None` / 「首次生成」但不在第 1 轮 -> 普通投掷。
fn roll_side_with_transmutation(
    state: &DefState,
    rolling_me: bool,
    transmutation: Option<RollTransmutation>,
) -> Vec<(DefState, Fraction)> {
    let Some(transmutation) = transmutation else {
        return roll_side(state, rolling_me);
    };
    // 「首次生成」只在第 1 轮生效（is_opening_round；时序本身待实测，见 RollTransmutation）。
    if transmutation.opening && !is_opening_round(state.round()) {
        return roll_side(state, rolling_me);
    }
    let first = DieFace::from_element(transmutation.rule.elements[0]);
    let second = DieFace::from_element(transmutation.rule.elements[1]);
    let random_count = nsub!(ROLL_COUNT, TRANSMUTATION_CONVERTED_DICE);
    DicePool::empty()
        .generate_random(random_count, false, false)
        .into_iter()
        .map(|(pool, probability)| {
            let pool = pool
                .add(first, TRANSMUTATION_FIXED_EACH)
                .add(second, TRANSMUTATION_FIXED_EACH);
            (with_dice(state, pool, rolling_me), probability)
        })
        .collect()
}

/// **我方投掷 8 颗骰**（投掷阶段；D190：返回骰池分支 + 精确概率，不采样）。
///
/// 骰池先清空再投（规则书：每回合的投掷阶段会投掷 8 个元素骰；元素骰不跨回合保留）。
/// 返回空 Vec 只可能出现在骰池已满（理论不可达：先清空了）。
/// **不带元素幻变**；有对应卡时用 [roll_me_with_transmutation]（L9）。
pub fn roll_me(state: &DefState) -> Vec<(DefState, Fraction)> {
    roll_side(state, true)
}

/// **敌方投掷 8 颗骰**（与 roll_me 共用一份实现，D35；不带元素幻变）。
///
/// 带元素幻变开关时用 [roll_opp_with_transmutation]。
pub fn roll_opp(state: &DefState) -> Vec<(DefState, Fraction)> {
    roll_side(state, false)
}

/// 一侧的投掷：从空骰池生成 8 颗（不限定基础色 = 可以出万能，D140）。
fn roll_side(state: &DefState, rolling_me: bool) -> Vec<(DefState, Fraction)> {
    DicePool::empty()
        .generate_random(ROLL_COUNT, false, false)
        .into_iter()
        .map(|(pool, probability)| (with_dice(state, pool, rolling_me), probability))
        .collect()
}

/// **我方重投**：把 rerolled 里列出的骰面各重投几颗（投掷阶段 1 次机会）。
///
/// - rerolled = (骰面, 颗数) 列表，必须都在当前骰池里（否则返回空分支：非法选择）；
/// - 重投的颗数按同样数量重新随机（不限定基础色）；
/// - 保留的骰子不动（观测已落地为具体骰面，D190：观测后展开成确定值）。
pub fn reroll_me(state: &DefState, rerolled: &[(DieFace, Nat)]) -> Vec<(DefState, Fraction)> {
    reroll_side(state, rerolled, true)
}

/// **敌方重投**（与 reroll_me 共用一份实现）。
pub fn reroll_opp(state: &DefState, rerolled: &[(DieFace, Nat)]) -> Vec<(DefState, Fraction)> {
    reroll_side(state, rerolled, false)
}

/// 一侧的重投：先扣掉要重投的骰子，再按同数量重新生成（观测展开）。
fn reroll_side(
    state: &DefState,
    rerolled: &[(DieFace, Nat)],
    rerolling_me: bool,
) -> Vec<(DefState, Fraction)> {
    let current = if rerolling_me {
        state.me().dice()
    } else {
        state.opp().dice()
    };
    let mut kept = current.clone();
    let mut count = Nat::ZERO;
    for (face, amount) in rerolled {
        let Some(next) = kept.remove(*face, *amount) else {
            return Vec::new();
        };
        kept = next;
        count = nadd!(count, *amount);
    }
    if count == Nat::ZERO {
        return vec![(state.clone(), Fraction::ONE)];
    }
    kept.generate_random(count, false, false)
        .into_iter()
        .map(|(pool, probability)| (with_dice(state, pool, rerolling_me), probability))
        .collect()
}

/// 把骰池写回某一侧（纯函数）。
fn with_dice(state: &DefState, dice: DicePool, on_me: bool) -> DefState {
    if on_me {
        state.clone().with_me(state.me().clone().with_dice(dice))
    } else {
        state.clone().with_opp(state.opp().clone().with_dice(dice))
    }
}

// ======================================================================
// 行动阶段（由调用方驱动；本层只广播）
// ======================================================================

/// **行动阶段开始**：派发广播（行动权 / 行动决策由调用方驱动）。
pub fn action_phase_start(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::ActionPhaseStart))
}

/// **行动阶段结束**（双方都宣布回合结束之后）：派发广播。
pub fn action_phase_end(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::ActionPhaseEnd))
}

// ======================================================================
// 宣布回合结束 / 先手归属（F4）
// ======================================================================

/// 行动阶段里「宣布回合结束」的记账：本回合先手 + 双方是否已宣布 + 下回合先手。
///
/// ## 规则书依据
///
/// - 进行行动 战斗行动【宣布结束】：「放弃本回合的行动，『宣布回合结束』。先『宣布回合结束』
///   的牌手会成为『先手牌手』，并在下回合的行动阶段优先行动。这之后，未『宣布回合结束』的
///   牌手可以继续行动，直到『宣布回合结束』。双方都『宣布回合结束』后，行动阶段结束，
///   进入本回合的『结束阶段』。」
/// - 回合流程 行动阶段：「从『先手牌手』开始，双方『轮流进行行动』。」
/// - 回合流程 结束阶段：「1、从『先手牌手』开始，处理在『结束阶段』触发的卡牌效果。」
///
/// ## 为什么由 flow 层承载（TODO）
///
/// 覆盖报告 §4 F4 的落地方案是给 `DefState` 加 `first_player: Perspective` +
/// 每方 `ended_this_round: bool`；但本次任务写范围不含 `state/**`（别的成员在改），
/// 所以先由 flow 层的记账类型承载 —— 形状与「DefState 字段」等价，搬进 `DefState` 时
/// 只需把入口里多出来的 `round` 参数删掉。
///
/// 另见模块文档「宣布回合结束与先手归属」里的两条 TODO（第一回合先手怎么定 /
/// 结束阶段用哪个「先手牌手」）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RoundEndState {
    /// 本回合的先手牌手（决定本回合行动阶段与结束阶段的处理顺序）。
    ///
    /// 第一回合由外部裁定（TODO：规则书没写先手裁定）；之后由上一回合的先宣布者接任
    /// （[RoundEndState::start_next_round]）。**双方都宣布后**会写进根状态
    /// `DefState::first_player`（[declare_round_end_me] / [declare_round_end_opp]），
    /// 新回合开始时由 [begin_next_round] 再写一次。
    first_player: Option<Perspective>,
    /// 本回合**先**宣布回合结束的牌手（规则书 S3.6：成为下回合的先手牌手）。
    first_declarer: Option<Perspective>,
    /// 我方是否已宣布回合结束。
    me_ended: bool,
    /// 敌方是否已宣布回合结束。
    opp_ended: bool,
}

impl RoundEndState {
    /// 还没有人宣布、也还没有先手牌手（第一回合开局；先手待外部裁定）。
    pub const fn new() -> RoundEndState {
        RoundEndState {
            first_player: None,
            first_declarer: None,
            me_ended: false,
            opp_ended: false,
        }
    }

    /// 带上本回合先手牌手（第一回合由外部裁定；之后一般由上一回合的
    /// [RoundEndState::start_next_round] 给）。
    pub const fn with_first_player(first: Perspective) -> RoundEndState {
        RoundEndState {
            first_player: Some(first),
            first_declarer: None,
            me_ended: false,
            opp_ended: false,
        }
    }

    /// 本回合先手牌手（结束阶段 / 行动阶段的处理顺序起始方）；未知 -> None。
    pub const fn first_player(&self) -> Option<Perspective> {
        self.first_player
    }

    /// 本回合先宣布回合结束的牌手（= 下回合先手牌手，S3.6）；还没人宣布 -> None。
    pub const fn first_declarer(&self) -> Option<Perspective> {
        self.first_declarer
    }

    /// **下回合的行动阶段优先行动方**（S3.6：先宣布者成为先手牌手）；还没人宣布 -> None。
    pub const fn next_first_player(&self) -> Option<Perspective> {
        self.first_declarer
    }

    /// 我方是否已宣布回合结束。
    pub const fn me_ended(&self) -> bool {
        self.me_ended
    }

    /// 敌方是否已宣布回合结束。
    pub const fn opp_ended(&self) -> bool {
        self.opp_ended
    }

    /// 双方是否都已宣布回合结束（= 可以进入结束阶段，S3.6 / S2.4）。
    pub const fn both_ended(&self) -> bool {
        self.me_ended && self.opp_ended
    }

    /// 某一方是否已宣布回合结束。
    pub fn has_ended(&self, side: Perspective) -> bool {
        match side {
            Perspective::Me => self.me_ended,
            Perspective::Opp => self.opp_ended,
        }
    }

    /// **开始下一回合**：清掉本回合的宣布标记，把先宣布者接成下回合先手。
    ///
    /// - 有人先宣布 -> 下回合先手 = 先宣布者（S3.6「并在下回合的行动阶段优先行动」）；
    /// - 没人宣布（异常 / 空局面）-> 保留本回合先手，不让先手丢失。
    pub fn start_next_round(&self) -> RoundEndState {
        RoundEndState {
            first_player: self.first_declarer.or(self.first_player),
            first_declarer: None,
            me_ended: false,
            opp_ended: false,
        }
    }
}

/// 一次「宣布回合结束」的结果。
#[derive(Clone, Debug)]
pub struct DeclareRoundEndOutcome {
    /// 结算后的一局：宣布结束**不改任何区**；双方都宣布后会把本回合先手牌手写进
    /// `DefState::first_player`（= `round.first_player()`；None = 未裁定）。
    pub state: DefState,
    /// 记账后的回合结束状态。
    pub round: RoundEndState,
    /// 双方是否都已宣布（= 行动阶段结束、可以进入结束阶段）。
    pub action_phase_over: bool,
    /// 双方都宣布时派发的 [TurnTiming::ActionPhaseEnd]（只有这一种情况非空）。
    pub pending: Vec<TurnPending>,
}

/// 宣布回合结束失败的原因。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeclareRoundEndError {
    /// 这一方本回合已经宣布过了。
    AlreadyEnded,
}

/// **我方宣布回合结束**（战斗行动，S3.6）：置位；先宣布者记为下回合先手。
///
/// 双方都宣布后派发 [TurnTiming::ActionPhaseEnd]、置 `action_phase_over`，并把
/// 本回合先手牌手写进 `DefState::first_player`。
pub fn declare_round_end_me(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> Result<DeclareRoundEndOutcome, DeclareRoundEndError> {
    declare_round_end(state, round, env, Perspective::Me)
}

/// **敌方宣布回合结束**（与 [declare_round_end_me] 共用一份实现，D35）。
pub fn declare_round_end_opp(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> Result<DeclareRoundEndOutcome, DeclareRoundEndError> {
    declare_round_end(state, round, env, Perspective::Opp)
}

/// 共用实现：`declaring` 是宣布方。
fn declare_round_end(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
    declaring: Perspective,
) -> Result<DeclareRoundEndOutcome, DeclareRoundEndError> {
    if round.has_ended(declaring) {
        return Err(DeclareRoundEndError::AlreadyEnded);
    }
    let mut next = *round;
    match declaring {
        Perspective::Me => next.me_ended = true,
        Perspective::Opp => next.opp_ended = true,
    }
    // 先宣布者成为「先手牌手」（S3.6；本层把它当**下回合**先手，见模块文档 TODO）。
    if next.first_declarer.is_none() {
        next.first_declarer = Some(declaring);
    }
    let action_phase_over = next.both_ended();
    let pending = if action_phase_over {
        broadcast(env, TurnTiming::ActionPhaseEnd)
    } else {
        Vec::new()
    };
    // 双方都宣布后「定下先手」：把本回合先手牌手写进根状态（None = 还没裁定，不编默认值）。
    let state = if action_phase_over {
        state.clone().with_first_player(next.first_player())
    } else {
        state.clone()
    };
    Ok(DeclareRoundEndOutcome {
        state,
        round: next,
        action_phase_over,
        pending,
    })
}

/// **回合开始（下一回合）**：回合数 +1 / 计数清零 + 终局判定（[round_begin]）+ 把本回合
/// 先宣布者接成下回合先手（[RoundEndState::start_next_round]）。
pub fn begin_next_round(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> (RoundBeginOutcome, RoundEndState) {
    let next_round = round.start_next_round();
    let RoundBeginOutcome {
        state: begun,
        pending,
        outcome,
    } = round_begin(state, env);
    // S3.6：先宣布者成为下回合先手；写进根状态（None = 未裁定，不编默认值）。
    let begun = begun.with_first_player(next_round.first_player());
    (
        RoundBeginOutcome {
            state: begun,
            pending,
            outcome,
        },
        next_round,
    )
}

// ======================================================================
// 结束阶段 / 回合结束
// ======================================================================

/// **结束阶段（我方先手）**：出战状态 tick -> 召唤物 tick（state::flows::end_phase）+
/// 派发 EndPhase 广播 -> **执行结束阶段触发的效果（S2.4 第 1 步）** -> 双方各抓 2 张
/// （S2.4 第 2 步；分支 + 折叠）。
pub fn end_phase_me_first(state: &DefState, env: &TurnEnv<'_>) -> TurnEndOutcome {
    turn_end(
        end_phase::end_phase_me_first(state, env.combat_status, env.summons),
        env,
        Perspective::Me,
    )
}

/// **结束阶段（敌方先手）**（与 end_phase_me_first 共用一份实现）。
pub fn end_phase_opp_first(state: &DefState, env: &TurnEnv<'_>) -> TurnEndOutcome {
    turn_end(
        end_phase::end_phase_opp_first(state, env.combat_status, env.summons),
        env,
        Perspective::Opp,
    )
}

/// 结束阶段失败的原因。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EndPhaseError {
    /// 还没有双方都宣布回合结束（S3.6：双方都宣布后进入结束阶段）。
    ActionPhaseNotOver,
    /// 不知道本回合先手牌手（第一回合先手由外部裁定，规则书没写；需先
    /// [RoundEndState::with_first_player]）。
    FirstPlayerUnknown,
}

/// **结束阶段**（按 [RoundEndState] 记录的先手牌手选先手方）。
///
/// 规则书·回合流程 结束阶段「1、从『先手牌手』开始……」：先手方先 tick，再另一方。
/// 与 [end_phase_me_first] / [end_phase_opp_first] 的关系：本入口只是**按记账选**其中一个，
/// 不再让调用方硬选；待执行的有序展开见 [TurnEndOutcome::ordered_end_phase]。
pub fn end_phase_by_first_player(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> Result<TurnEndOutcome, EndPhaseError> {
    if !round.both_ended() {
        return Err(EndPhaseError::ActionPhaseNotOver);
    }
    let Some(first) = round.first_player() else {
        return Err(EndPhaseError::FirstPlayerUnknown);
    };
    // 结束阶段分支里的 DefState 也带上先手牌手（调用方拿到即一致）。
    let state = state.clone().with_first_player(round.first_player());
    Ok(match first {
        Perspective::Me => end_phase_me_first(&state, env),
        Perspective::Opp => end_phase_opp_first(&state, env),
    })
}

/// 结束阶段里**按先手顺序**展开的一条待执行（哪一方 + 待执行本身）。
#[derive(Clone, Debug)]
pub struct EndPhaseStep {
    /// 这一条属于哪一方（视角方绑定，不是编号，D34）。
    pub side: Perspective,
    /// 待执行本身（出战状态 / 召唤物，统一成 [FlowPending]）。
    pub pending: FlowPending,
}

impl TurnEndOutcome {
    /// 恰好一支时唯一的那一支（牌库确定 / 双方都抓不到牌）；多支时返回 None（改用 branches）。
    pub fn single(&self) -> Option<&TurnEndBranch> {
        if self.branches.len() == 1 {
            self.branches.first()
        } else {
            None
        }
    }

    /// **按先手顺序展开结束阶段待执行**（规则书 S2.4：从先手牌手开始）。
    ///
    /// 顺序：先手方出战状态 -> 先手方召唤物 -> 另一方出战状态 -> 另一方召唤物
    /// （来源类别顺序见 `rules::ops::order`）。广播待执行（`self.pending`）不在其中：
    /// 它们是阶段广播，按派发顺序执行即可。
    pub fn ordered_end_phase(&self, first: Perspective) -> Vec<EndPhaseStep> {
        let mut steps = Vec::new();
        for side in [first, first.other()] {
            match side {
                Perspective::Me => push_end_phase_side(
                    &mut steps,
                    side,
                    &self.end_phase.me_combat_status,
                    &self.end_phase.me_summons,
                ),
                Perspective::Opp => push_end_phase_side(
                    &mut steps,
                    side,
                    &self.end_phase.opp_combat_status,
                    &self.end_phase.opp_summons,
                ),
            }
        }
        steps
    }
}

/// 把某一方的结束阶段待执行按「出战状态 -> 召唤物」追加进有序序列。
fn push_end_phase_side(
    steps: &mut Vec<EndPhaseStep>,
    side: Perspective,
    combat_status: &[CombatStatusPending],
    summons: &[SummonPending],
) {
    for pending in combat_status {
        steps.push(EndPhaseStep {
            side,
            pending: FlowPending::CombatStatus(*pending),
        });
    }
    for pending in summons {
        steps.push(EndPhaseStep {
            side,
            pending: FlowPending::Summon(*pending),
        });
    }
}

/// 结束阶段共用实现：各区 tick 结果 -> **执行结束阶段触发的效果（S2.4 第 1 步）** ->
/// 双方各抓 2 张（S2.4 第 2 步，分支展开 + 折叠）+ 阶段广播。
fn turn_end(
    outcome: end_phase::EndPhaseOutcome,
    env: &TurnEnv<'_>,
    first: Perspective,
) -> TurnEndOutcome {
    let broadcast = broadcast(env, TurnTiming::EndPhase);
    let mut state = outcome.state;
    // S2.4 第 1 步：先手方先、按 S7.5 来源类别 -> 进场先后执行结束阶段效果。
    // 本层是派发的调用方（D186：派发只排序，执行归调用方），所以顺序由这里保证。
    execute_end_phase_effects(&mut state, &broadcast, &outcome.pending, first);
    // Z6（结束阶段内的效果阶段）：支援区持续回合 -1 / 耗尽弃置；派发出来的待执行按 D186
    // 交给调用方（本层不执行效果），但 tick 后的区状态已写进分支局面。
    let support_me = tick_support_side(&mut state, Perspective::Me, env.supports);
    let support_opp = tick_support_side(&mut state, Perspective::Opp, env.supports);
    // S2.4 第 2 步：效果全部落地**之后**双方各抓 2 张（分支 + 折叠）。
    let branches = end_phase_draw_branches(&state, env.draw);
    TurnEndOutcome {
        branches,
        end_phase: outcome.pending,
        pending: broadcast,
        support_me,
        support_opp,
    }
}

/// **一侧的支援区结束阶段 tick**（Z6：规则书·特殊词条「持续回合」）：
/// 带持续回合的支援牌 -1、耗尽立刻弃置；返回派发出来的待执行（本层不执行效果，D186）。
fn tick_support_side(
    state: &mut DefState,
    side: Perspective,
    triggers: &SupportTriggers,
) -> Vec<SupportPending> {
    let mut player = player_of(state, side);
    let ticked = player.supports().tick_end_phase_with_triggers(triggers);
    player = player.with_supports(ticked.zone);
    set_player(state, side, player);
    ticked.pending
}

/// 结束阶段里**待执行的触发效果**（S2.4 第 1 步；本层是调用方，按规则顺序直接执行）。
enum EndPhaseEffect {
    /// 阶段广播（TurnTriggers；来源类别由注册方自报）。
    Broadcast(TurnPending),
    /// 出战状态区的 tick 待执行。
    CombatStatus {
        side: Perspective,
        pending: CombatStatusPending,
    },
    /// 召唤物区的 tick 待执行。
    Summon {
        side: Perspective,
        pending: SummonPending,
    },
}

impl EndPhaseEffect {
    /// 就「先手」而言这一条属于哪一侧。
    ///
    /// TODO(阶段广播的两侧注册表)：TurnTriggers 现在只有一张表（T4 遗留），本层按「调用方给的
    /// 是当前 / 先手方那张表」处理；对方侧广播待接入（届时这里改成真正的两条）。
    fn side(&self, first: Perspective) -> Perspective {
        match self {
            EndPhaseEffect::Broadcast(_) => first,
            EndPhaseEffect::CombatStatus { side, .. } | EndPhaseEffect::Summon { side, .. } => {
                *side
            }
        }
    }

    /// 来源类别（S7.5 第 2 条：角色牌 -> 角色装备和状态 -> 阵营出战状态 -> 召唤物 -> 支援牌）。
    fn source_kind(&self) -> EffectSource {
        match self {
            EndPhaseEffect::Broadcast(pending) => pending.source_kind,
            EndPhaseEffect::CombatStatus { pending, .. } => pending.source_kind,
            EndPhaseEffect::Summon { pending, .. } => pending.source_kind,
        }
    }

    /// 进场先后（S7.5 第 3 条）。
    fn entry(&self) -> Nat {
        match self {
            EndPhaseEffect::Broadcast(pending) => pending.entry,
            EndPhaseEffect::CombatStatus { pending, .. } => pending.entry,
            EndPhaseEffect::Summon { pending, .. } => pending.entry,
        }
    }
}

/// 来源类别的先后序号（复用 rules::ops::order 的唯一顺序表）。
fn source_rank(source: EffectSource) -> usize {
    SOURCE_ORDER
        .iter()
        .position(|candidate| *candidate == source)
        .unwrap_or(SOURCE_ORDER.len())
}

/// **执行结束阶段触发的效果**（S2.4 第 1 步）。
///
/// 顺序 = S7.5：先手方全部先 -> 另一方；同一侧内先按来源类别（第 2 条）、再按进场先后（第 3 条）。
/// 只调用各区提供的 execute_*（D186：本层是派发的调用方，不重写效果）。
fn execute_end_phase_effects(
    state: &mut DefState,
    broadcast: &[TurnPending],
    end_phase: &EndPhasePending,
    first: Perspective,
) {
    let mut items: Vec<EndPhaseEffect> = Vec::new();
    for pending in broadcast {
        items.push(EndPhaseEffect::Broadcast(*pending));
    }
    for pending in &end_phase.me_combat_status {
        items.push(EndPhaseEffect::CombatStatus {
            side: Perspective::Me,
            pending: *pending,
        });
    }
    for pending in &end_phase.me_summons {
        items.push(EndPhaseEffect::Summon {
            side: Perspective::Me,
            pending: *pending,
        });
    }
    for pending in &end_phase.opp_combat_status {
        items.push(EndPhaseEffect::CombatStatus {
            side: Perspective::Opp,
            pending: *pending,
        });
    }
    for pending in &end_phase.opp_summons {
        items.push(EndPhaseEffect::Summon {
            side: Perspective::Opp,
            pending: *pending,
        });
    }

    items.sort_by(|left, right| {
        let side_rank = |side: Perspective| usize::from(side != first);
        side_rank(left.side(first))
            .cmp(&side_rank(right.side(first)))
            .then_with(|| source_rank(left.source_kind()).cmp(&source_rank(right.source_kind())))
            .then_with(|| left.entry().cmp(&right.entry()))
    });

    for item in items {
        match item {
            EndPhaseEffect::Broadcast(pending) => {
                (pending.effect)(state, &pending.event, pending.owner);
            }
            EndPhaseEffect::CombatStatus { side, pending } => {
                let mut player = player_of(state, side);
                let mut zone = player.combat_status().clone();
                execute_combat_status(std::slice::from_ref(&pending), &mut zone);
                player = player.with_combat_status(zone);
                set_player(state, side, player);
            }
            EndPhaseEffect::Summon { side, pending } => {
                let mut player = player_of(state, side);
                let mut zone = player.summons().clone();
                execute_summon(std::slice::from_ref(&pending), &mut zone);
                player = player.with_summons(zone);
                set_player(state, side, player);
            }
        }
    }
}

/// 取某一方的只读快照（执行时先 clone，再整体写回；不借用区的可变引用）。
fn player_of(state: &DefState, side: Perspective) -> PlayerDefs {
    match side {
        Perspective::Me => state.me().clone(),
        Perspective::Opp => state.opp().clone(),
    }
}

/// 把执行后的一方整体写回。
fn set_player(state: &mut DefState, side: Perspective, player: PlayerDefs) {
    let next = match side {
        Perspective::Me => state.clone().with_me(player),
        Perspective::Opp => state.clone().with_opp(player),
    };
    *state = next;
}

/// **结束阶段第 2 步**（S2.4）：「每位牌手从自己的牌堆中抓 2 张牌。」
///
/// 返回分支列表（D190 观测展开 + D191 折叠回）：
/// - 双方**分别**展开自己的抓牌分支，再做**交叉组合**（概率精确相乘）；
/// - 局面完全相同的分支用 [fold_states] 折叠、概率精确相加（顺序 = 首次出现顺序）；
/// - 抓不到牌的一侧按「原样一支、概率 1」参与组合（空牌库 / 牌库不足，S4.6）。
///
/// **调用时点**：本函数由 [turn_end] 在**执行完结束阶段效果（S2.4 第 1 步）之后**调用 ——
/// 抓牌必须晚于效果处理（S2.4：1 效果 -> 2 抓牌）。每一支都带 [settlement_outcome] 的终局判定
/// （只反映已落地局面；defeat 清理后再用 [crate::flow::game_end::settlement_outcome] 复判）。
fn end_phase_draw_branches(state: &DefState, draw_hooks: DrawHooks<'_>) -> Vec<TurnEndBranch> {
    let me_options = draw_side_options(state.me(), draw_hooks);
    let opp_options = draw_side_options(state.opp(), draw_hooks);
    let mut branches: Vec<(DefState, Fraction)> = Vec::new();
    for (me, me_probability) in &me_options {
        for (opp, opp_probability) in &opp_options {
            branches.push((
                state.clone().with_me(me.clone()).with_opp(opp.clone()),
                me_probability.exact_mul(*opp_probability),
            ));
        }
    }
    fold_states(branches)
        .into_iter()
        .map(|(state, probability)| TurnEndBranch {
            outcome: settlement_outcome(&state),
            state,
            probability,
        })
        .collect()
}

/// 一侧的「结束阶段抓 2 张」：返回 (抓牌后的一方, 精确概率) 分支。
///
/// 抓不到牌（空牌库 / 牌库里没有牌）-> 原样一支、概率 1：规则书·游戏区域 V
/// 「牌堆中已经没有卡牌时，牌手不会获得手牌」，结束阶段照常进行，不丢分支。
fn draw_side_options(
    player: &PlayerDefs,
    draw_hooks: DrawHooks<'_>,
) -> Vec<(PlayerDefs, Fraction)> {
    let drawn = player.pile().draw_many(player.hand(), END_PHASE_DRAW);
    if drawn.is_empty() {
        return vec![(player.clone(), Fraction::ONE)];
    }
    drawn
        .into_iter()
        .map(|(branch, probability)| {
            // F10：新抓手牌按注入的基础费用重建 CurrentCost；
            // D188：抽到牌携带的待展开随机状态物化进手牌卡状态表。
            (apply_draw_branch(player, &branch, draw_hooks), probability)
        })
        .collect()
}

/// **回合结束**：派发 RoundEnd（进入下一个回合之前的收尾；抓 2 张牌是牌库块的事）。
pub fn round_end(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::RoundEnd))
}

// ======================================================================
// 共用：阶段广播
// ======================================================================

/// 派发一个阶段时机（只排序、不执行，D186）。
fn broadcast(env: &TurnEnv<'_>, timing: TurnTiming) -> Vec<TurnPending> {
    let event = Event::EMPTY;
    dispatch(env.triggers, timing, &event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 阶段效果函数指针的来源类别（本层只提供常量，避免调用方自己猜）。
///
/// 阶段广播不是「卡牌效果」，来源类别按「规则书第 2 条里最接近的一类」取：
/// 回合 / 阶段类卡牌效果多来自场上卡（角色牌 / 出战状态 / 召唤物 / 支援牌），
/// 由登记方自己选；本常量给出「默认 = 角色牌」（待复核）。
pub const DEFAULT_BROADCAST_SOURCE: EffectSource = EffectSource::CharacterCard;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::base::{Element, SkillId};
    use crate::families::action_card::areas::support::{
        SupportEntry, SupportTiming, SupportValue, SupportZone,
    };
    use crate::families::action_card::status::Mark;
    use crate::families::action_card::zones::pile::PileState;
    use crate::families::character::skill_flow::SkillUses;
    use crate::families::character::zones::combat_status::{
        CombatStatusTiming, CombatStatusTriggers, CombatStatusZone,
    };
    use crate::families::summon::zones::{SummonTiming, SummonTriggers, SummonZone};
    use crate::flow::game_end::{settlement_outcome, Outcome};
    use crate::flow::hooks::DrawHooks;
    use crate::keywords::target::Perspective;
    use crate::resources::dice::{CostComponent, DicePool, DieFace};
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{
        characters, combat_status, entity, hand, n, pile, summons, CARD_A, TEST_STATUS,
    };
    use crate::state::DefState;
    use crate::timeline::{Event, Trigger};
    use gi_genius_invokation_engine::prob::Fraction;
    use gi_genius_invokation_engine::rng::Rng;
    use gi_genius_invokation_engine::Nat;

    const NORMAL: SkillId = SkillId::new(12161);

    fn noop_turn(_state: &mut DefState, _event: &Event, _owner: EntityId) {}
    fn noop_combat(_zone: &mut CombatStatusZone, _event: &Event, _owner: EntityId) {}
    fn noop_summon(_zone: &mut SummonZone, _event: &Event, _owner: EntityId) {}
    fn noop_support(_zone: &mut SupportZone, _event: &Event, _owner: EntityId) {}

    /// 空依赖（没有 tick 触发登记）。
    struct EmptyEnv {
        triggers: TurnTriggers,
        combat_status: CombatStatusTriggers,
        summons: SummonTriggers,
        supports: SupportTriggers,
        draw: DrawHooks<'static>,
    }

    impl EmptyEnv {
        fn new() -> EmptyEnv {
            EmptyEnv {
                triggers: TurnTriggers::new(),
                combat_status: CombatStatusTriggers::new(),
                summons: SummonTriggers::new(),
                supports: SupportTriggers::new(),
                draw: DrawHooks::none(),
            }
        }

        fn env(&self) -> TurnEnv<'_> {
            TurnEnv {
                triggers: &self.triggers,
                combat_status: &self.combat_status,
                summons: &self.summons,
                supports: &self.supports,
                draw: self.draw,
            }
        }

        fn env_with_draw<'a>(&'a self, draw: DrawHooks<'a>) -> TurnEnv<'a> {
            TurnEnv {
                triggers: &self.triggers,
                combat_status: &self.combat_status,
                summons: &self.summons,
                supports: &self.supports,
                draw,
            }
        }
    }

    fn limited_player() -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(characters(&[201]))
            .with_skill_uses(SkillUses::new().with_limit(NORMAL, n(1)).recorded(NORMAL))
    }

    #[test]
    fn round_begin_resets_skill_uses_and_dispatches_the_broadcast() {
        let before = DefState::from_players(limited_player(), limited_player());
        let mut empty = EmptyEnv::new();
        empty.triggers.register(Trigger::new(
            TurnTiming::RoundBegin,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_turn,
        ));
        let outcome = round_begin(&before, &empty.env());
        assert_eq!(outcome.pending.len(), 1);
        let after = &outcome.state;
        // 计数清零、上限保留
        assert_eq!(after.me().skill_uses().used_count(NORMAL), Nat::ZERO);
        assert_eq!(after.me().skill_uses().limit(NORMAL), Some(Nat::new(1)));
        assert_eq!(after.opp().skill_uses().used_count(NORMAL), Nat::ZERO);
        // 回合数推进；未到 15 -> Ongoing（S7.7）
        assert_eq!(after.round(), Nat::new(1));
        assert_eq!(outcome.outcome, Outcome::Ongoing);
        // 派发不执行：状态里其它区没被效果改（noop 也看不出来，这里钉住计数）
    }

    #[test]
    fn roll_phase_broadcasts_without_changing_dice() {
        let before = DefState::from_players(
            PlayerDefs::new().with_dice(crate::resources::dice::DicePool::empty()),
            PlayerDefs::new(),
        );
        let mut empty = EmptyEnv::new();
        empty.triggers.register(Trigger::new(
            TurnTiming::RollPhase,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_turn,
        ));
        let (after, pending) = roll_phase(&before, &empty.env());
        assert_eq!(pending.len(), 1);
        assert_eq!(after, before);
    }

    #[test]
    fn roll_me_returns_eight_dice_branches_with_probability_one() {
        let before = DefState::from_players(PlayerDefs::new(), PlayerDefs::new());
        let branches = roll_me(&before);
        assert!(!branches.is_empty());
        for (state, probability) in &branches {
            assert_eq!(state.me().dice().total(), ROLL_COUNT);
            assert!(!probability.is_zero());
        }
        let total = branches
            .iter()
            .fold(Fraction::ZERO, |sum, (_, probability)| {
                sum.exact_add(*probability)
            });
        assert!(total.is_one(), "投掷分支概率和必须为 1");
        // 敌方不受影响
        assert_eq!(branches[0].0.opp(), before.opp());
        // 纯函数：入参不变
        assert_eq!(before.me().dice().total(), Nat::ZERO);
    }

    #[test]
    fn roll_opp_only_touches_the_opponent_side() {
        let before = DefState::from_players(PlayerDefs::new(), PlayerDefs::new());
        let branches = roll_opp(&before);
        assert!(!branches.is_empty());
        let (state, _) = branches.first().expect("一支");
        assert_eq!(state.opp().dice().total(), ROLL_COUNT);
        assert_eq!(state.me(), before.me());
    }

    #[test]
    fn reroll_keeps_dice_count_and_replaces_the_selected_ones() {
        // 8 万能骰，重投 2 颗 -> 仍是 8 颗；36 种重投结果（8 面取 2 的多重集）。
        let before = DefState::from_players(
            PlayerDefs::new().with_dice(
                crate::resources::dice::DicePool::empty().add(DieFace::Omni, Nat::new(8)),
            ),
            PlayerDefs::new(),
        );
        let branches = reroll_me(&before, &[(DieFace::Omni, Nat::new(2))]);
        assert_eq!(branches.len(), 36);
        let total = branches
            .iter()
            .fold(Fraction::ZERO, |sum, (_, probability)| {
                sum.exact_add(*probability)
            });
        assert!(total.is_one());
        for (state, _) in &branches {
            assert_eq!(state.me().dice().total(), ROLL_COUNT);
        }
        // 纯函数
        assert_eq!(before.me().dice().total(), Nat::new(8));
    }

    #[test]
    fn reroll_of_nonexistent_face_is_an_illegal_choice() {
        let before = DefState::from_players(
            PlayerDefs::new().with_dice(
                crate::resources::dice::DicePool::empty().add(DieFace::Pyro, Nat::new(2)),
            ),
            PlayerDefs::new(),
        );
        assert!(reroll_me(&before, &[(DieFace::Omni, Nat::new(1))]).is_empty());
        // 重投 0 颗 = 无操作一支
        let none = reroll_me(&before, &[]);
        assert_eq!(none.len(), 1);
        assert!(none[0].1.is_one());
        assert_eq!(none[0].0, before);
    }

    #[test]
    fn action_phase_broadcasts_are_pure() {
        let before = DefState::from_players(limited_player(), PlayerDefs::new());
        let mut empty = EmptyEnv::new();
        for timing in [
            TurnTiming::ActionPhaseStart,
            TurnTiming::ActionPhaseEnd,
            TurnTiming::RoundEnd,
        ] {
            empty.triggers.register(Trigger::new(
                timing,
                n(1),
                EffectSource::CharacterCard,
                Nat::ZERO,
                noop_turn,
            ));
        }
        let (state_start, pending_start) = action_phase_start(&before, &empty.env());
        assert_eq!(pending_start.len(), 1);
        assert_eq!(state_start, before);
        let (state_end, pending_end) = action_phase_end(&before, &empty.env());
        assert_eq!(pending_end.len(), 1);
        assert_eq!(state_end, before);
        let (state_round, pending_round) = round_end(&before, &empty.env());
        assert_eq!(pending_round.len(), 1);
        assert_eq!(state_round, before);
    }

    #[test]
    fn end_phase_ticks_combat_status_and_summons() {
        // 我方：持续回合 2 -> 1；召唤物可用次数 2 -> 1。
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_combat_status(combat_status(n(201), &[(&TEST_STATUS, None, Some(n(2)))]))
                .with_summons(summons(&[(301, 2)])),
            PlayerDefs::new(),
        );
        // tick 触发：出战状态 / 召唤物各登记一个时机（只为让 pending 非空）。
        let mut combat_triggers = CombatStatusTriggers::new();
        for timing in [
            CombatStatusTiming::DurationTicked,
            CombatStatusTiming::Depleted,
        ] {
            combat_triggers.register(Trigger::new(
                timing,
                n(1),
                EffectSource::CombatStatus,
                Nat::ZERO,
                noop_combat,
            ));
        }
        let mut summon_triggers = SummonTriggers::new();
        for timing in [SummonTiming::Triggered, SummonTiming::Depleted] {
            summon_triggers.register(Trigger::new(
                timing,
                n(1),
                EffectSource::Summon,
                Nat::ZERO,
                noop_summon,
            ));
        }
        let mut triggers = TurnTriggers::new();
        triggers.register(Trigger::new(
            TurnTiming::EndPhase,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_turn,
        ));
        let supports = SupportTriggers::new();
        let env = TurnEnv {
            triggers: &triggers,
            combat_status: &combat_triggers,
            summons: &summon_triggers,
            supports: &supports,
            draw: DrawHooks::none(),
        };
        let outcome = end_phase_me_first(&before, &env);
        let branch = outcome.single().expect("固定牌库 / 空牌库 -> 一支");
        assert_eq!(
            branch.state.me().combat_status().duration(&TEST_STATUS),
            Some(Nat::new(1))
        );
        assert_eq!(
            branch
                .state
                .me()
                .summons()
                .entry_of(n(301))
                .expect("还在")
                .uses,
            Some(Nat::new(1))
        );
        assert_eq!(outcome.end_phase.me_combat_status.len(), 1);
        assert_eq!(outcome.end_phase.me_summons.len(), 1);
        assert_eq!(outcome.pending.len(), 1);
        // 纯函数
        assert_eq!(
            before.me().combat_status().duration(&TEST_STATUS),
            Some(Nat::new(2))
        );
        let _ = entity(1);
    }

    #[test]
    fn end_phase_opp_first_uses_the_same_implementation() {
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_combat_status(combat_status(n(201), &[(&TEST_STATUS, None, Some(n(1)))])),
            PlayerDefs::new()
                .with_characters(characters(&[401]))
                .with_combat_status(combat_status(n(401), &[(&TEST_STATUS, None, Some(n(1)))])),
        );
        let env = EmptyEnv::new();
        let outcome = end_phase_opp_first(&before, &env.env());
        let branch = outcome.single().expect("空牌库 -> 一支");
        // 双方持续回合都 1 -> 0，都耗尽弃置
        assert!(branch.state.me().combat_status().is_empty());
        assert!(branch.state.opp().combat_status().is_empty());
    }

    fn support_zone(entries: &[(u32, Option<u32>)]) -> SupportZone {
        let triggers = SupportTriggers::new();
        let mut zone = SupportZone::new();
        for (entity, duration) in entries {
            let entry = SupportEntry::new(CardId::new(300_001), n(*entity), SupportValue::None)
                .with_duration(duration.map(n));
            let (result, _) = zone
                .add_with_triggers(entry, &triggers)
                .pop()
                .expect("未满 -> 一支");
            zone = result.zone;
        }
        zone
    }

    #[test]
    fn end_phase_ticks_support_durations_and_returns_pending_to_callers() {
        // 我方：持续回合 2 -> 1（保留）；无持续回合的不动。
        // 敌方：持续回合 1 -> 耗尽弃置（派发 Discarded 待执行）。
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_supports(support_zone(&[(301, Some(2)), (302, None)])),
            PlayerDefs::new()
                .with_characters(characters(&[401]))
                .with_supports(support_zone(&[(401, Some(1))])),
        );
        let mut empty = EmptyEnv::new();
        empty.supports.register(Trigger::new(
            SupportTiming::Discarded,
            n(1),
            EffectSource::Support,
            Nat::ZERO,
            noop_support,
        ));
        let outcome = end_phase_me_first(&before, &empty.env());
        let branch = outcome.single().expect("一支");
        // tick 结果已写进分支局面。
        assert_eq!(
            branch.state.me().supports().duration_of(n(301)),
            Some(Some(Nat::new(1)))
        );
        assert_eq!(branch.state.me().supports().duration_of(n(302)), Some(None));
        assert!(!branch.state.opp().supports().contains(n(401)));
        // D186：派发出来的待执行交给调用方（本层未执行「弃置时」效果）；先手方 = 我方。
        assert!(outcome.support_me.is_empty());
        assert_eq!(outcome.support_opp.len(), 1);
        // 纯函数：入参不变。
        assert_eq!(
            before.opp().supports().duration_of(n(401)),
            Some(Some(Nat::new(1)))
        );
    }

    // ------------------------------------------------------------------
    // 结束阶段抓 2 张（F10）+ 终局判定接线
    // ------------------------------------------------------------------

    #[test]
    fn end_phase_draws_two_cards_for_each_side() {
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_pile(pile(&[101, 102, 103])),
            PlayerDefs::new()
                .with_characters(characters(&[401]))
                .with_pile(pile(&[201, 202, 203])),
        );
        let outcome = end_phase_me_first(&before, &end_phase_env().env());
        let branch = outcome.single().expect("固定牌库 -> 恰好一支");
        assert!(branch.probability.is_one());
        // 双方各抓 2 张（S2.4 第 2 步）；固定段按顺序从顶抓。
        assert_eq!(branch.state.me().hand().len(), Nat::new(2));
        assert_eq!(branch.state.me().pile().len(), Nat::new(1));
        assert_eq!(branch.state.opp().hand().len(), Nat::new(2));
        assert_eq!(branch.state.opp().pile().len(), Nat::new(1));
        assert!(branch.state.me().hand().contains(n(101)));
        assert!(branch.state.me().hand().contains(n(102)));
        // 纯函数：入参不变
        assert!(before.me().hand().is_empty());
        assert_eq!(before.me().pile().len(), Nat::new(3));
        // 未到回合上限 / 未全灭
        assert_eq!(branch.outcome, Outcome::Ongoing);
    }

    #[test]
    fn end_phase_draw_with_empty_piles_keeps_the_branch() {
        let before = DefState::from_players(PlayerDefs::new(), PlayerDefs::new());
        let outcome = end_phase_me_first(&before, &end_phase_env().env());
        assert_eq!(outcome.branches.len(), 1);
        let branch = outcome.single().expect("一支");
        assert!(branch.probability.is_one());
        // S4.6：牌库没有牌 -> 不会获得手牌；结束阶段照常进行，不丢分支。
        assert_eq!(branch.state, before);
    }

    #[test]
    fn end_phase_random_pile_folds_to_one_branch_when_draw_order_does_not_matter() {
        // 随机段 2 张、手牌空（free 10 >= 抽 2）：两种抽取顺序后继局面相同 -> 牌库块自己
        // 折叠成一支；本层组合 / 折叠后仍是一支、概率 1。
        let random = PileState::from_random(vec![(n(301), CARD_A), (n(302), CARD_A)]);
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_pile(random),
            PlayerDefs::new(),
        );
        let outcome = end_phase_me_first(&before, &end_phase_env().env());
        assert_eq!(outcome.branches.len(), 1);
        let branch = outcome.single().expect("一支");
        assert!(branch.probability.is_one());
        assert_eq!(branch.state.me().hand().len(), Nat::new(2));
        assert!(branch.state.me().pile().is_empty());
    }

    #[test]
    fn end_phase_draw_branches_combine_with_exact_probabilities() {
        // 手牌 9 张（free = 1）从随机段抓 2 张：只有 1 张能进手牌、另一张溢出弃置；
        // 「哪张进手牌」产生 2 支有序分支（D99 安全兜底），概率各 1/2。
        let full = hand(&[101, 102, 103, 104, 105, 106, 107, 108, 109]);
        let random = PileState::from_random(vec![(n(301), CARD_A), (n(302), CARD_A)]);
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_hand(full)
                .with_pile(random),
            PlayerDefs::new(),
        );
        let outcome = end_phase_me_first(&before, &end_phase_env().env());
        assert_eq!(outcome.branches.len(), 2);
        let mut total = Fraction::ZERO;
        for branch in &outcome.branches {
            // 双方各抓 2；我方 9 + 1 = 10（另一张溢出弃置）、牌库空；敌方空库不抓。
            assert_eq!(branch.state.me().hand().len(), Nat::new(10));
            assert!(branch.state.me().pile().is_empty());
            assert!(branch.state.opp().hand().is_empty());
            total = total.exact_add(branch.probability);
        }
        assert!(total.is_one());
    }

    #[test]
    fn end_phase_branch_carries_the_terminal_outcome() {
        // 模拟「击倒清理已完成」的局面：对方角色区已空、回合数 1 -> 结束阶段分支应带 MeWins。
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new(),
        )
        .with_round(n(1));
        let outcome = end_phase_me_first(&before, &end_phase_env().env());
        let branch = outcome.single().expect("一支");
        assert_eq!(branch.outcome, Outcome::MeWins);
        // 直接入口不改先手（before 未裁定）。
        assert_eq!(branch.state.first_player(), None);
    }

    #[test]
    fn round_begin_reports_the_round_limit() {
        // S7.7：第 15 回合开始仍未分胜负 -> 双方都输（RoundLimit）。
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new().with_characters(characters(&[401])),
        )
        .with_round(n(14));
        let outcome = round_begin(&before, &EmptyEnv::new().env());
        assert_eq!(outcome.state.round(), Nat::new(15));
        assert_eq!(outcome.outcome, Outcome::RoundLimit);
        assert!(settlement_outcome(&outcome.state).is_terminal());
    }

    // ------------------------------------------------------------------
    // 宣布回合结束 / 先手归属（F4）
    // ------------------------------------------------------------------

    /// 双方都宣布一次（`first` = 先宣布方）；本回合先手固定为我方。
    fn round_with_both_declared(before: &DefState, first: Perspective) -> DeclareRoundEndOutcome {
        let round = RoundEndState::with_first_player(Perspective::Me);
        let env = EmptyEnv::new();
        match first {
            Perspective::Me => {
                let declared =
                    declare_round_end_me(before, &round, &env.env()).expect("我方第一次宣布");
                declare_round_end_opp(&declared.state, &declared.round, &env.env())
                    .expect("敌方第一次宣布")
            }
            Perspective::Opp => {
                let declared =
                    declare_round_end_opp(before, &round, &env.env()).expect("敌方第一次宣布");
                declare_round_end_me(&declared.state, &declared.round, &env.env())
                    .expect("我方第一次宣布")
            }
        }
    }

    #[test]
    fn declare_round_end_records_first_declarer_and_opens_the_end_phase() {
        let before = DefState::from_players(limited_player(), limited_player());
        let mut env = EmptyEnv::new();
        env.triggers.register(Trigger::new(
            TurnTiming::ActionPhaseEnd,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_turn,
        ));
        let round = RoundEndState::with_first_player(Perspective::Me);

        // 我方先宣布：还没结束、没有广播；先宣布者 = 我方；本回合先手不变
        let first = declare_round_end_me(&before, &round, &env.env()).expect("第一次宣布");
        assert!(first.round.me_ended());
        assert!(!first.round.opp_ended());
        assert!(!first.round.both_ended());
        assert!(!first.action_phase_over);
        assert!(first.pending.is_empty());
        assert_eq!(first.round.first_declarer(), Some(Perspective::Me));
        assert_eq!(first.round.next_first_player(), Some(Perspective::Me));
        assert_eq!(first.round.first_player(), Some(Perspective::Me));
        assert_eq!(first.state, before);
        // 单方宣布还不「定先手」-> 不改根状态（before 的 first_player = None）。
        assert_eq!(first.state.first_player(), None);

        // 同一方再宣布 -> 已经宣布过
        assert_eq!(
            declare_round_end_me(&first.state, &first.round, &env.env()).expect_err("重复宣布"),
            DeclareRoundEndError::AlreadyEnded
        );

        // 敌方也宣布 -> 行动阶段结束、派发 ActionPhaseEnd；先宣布者仍是我方
        let second =
            declare_round_end_opp(&first.state, &first.round, &env.env()).expect("敌方第一次宣布");
        assert!(second.round.both_ended());
        assert!(second.action_phase_over);
        assert_eq!(second.pending.len(), 1);
        assert_eq!(second.round.first_declarer(), Some(Perspective::Me));
        assert_eq!(
            second.round.start_next_round().first_player(),
            Some(Perspective::Me)
        );
        // 双方都宣布后定下先手：本回合先手写进根状态（= round.first_player()）。
        assert_eq!(second.state.first_player(), Some(Perspective::Me));
    }

    #[test]
    fn declare_round_end_keeps_none_when_the_first_player_is_undecided() {
        let before = DefState::from_players(limited_player(), limited_player());
        let env = EmptyEnv::new();
        // 未裁定（RoundEndState::new）：双方都宣布后仍 None —— 规则书没规定第一回合先手，
        // 不编默认值；先宣布者只记在 RoundEndState（下回合由 begin_next_round 写入）。
        let round = RoundEndState::new();
        let a = declare_round_end_me(&before, &round, &env.env()).expect("我方宣布");
        assert_eq!(a.state.first_player(), None);
        let b = declare_round_end_opp(&a.state, &a.round, &env.env()).expect("敌方宣布");
        assert_eq!(b.state.first_player(), None);
        assert_eq!(b.round.first_player(), None);
        assert_eq!(b.round.first_declarer(), Some(Perspective::Me));
    }

    #[test]
    fn round_begin_preserves_the_first_player_recorded_in_def_state() {
        let base = DefState::from_players(limited_player(), limited_player())
            .with_first_player(Some(Perspective::Opp));
        let outcome = round_begin(&base, &EmptyEnv::new().env());
        assert_eq!(outcome.state.first_player(), Some(Perspective::Opp));
        // 未裁定也保持 None（不编默认值）。
        let unset = DefState::from_players(limited_player(), limited_player());
        let outcome = round_begin(&unset, &EmptyEnv::new().env());
        assert_eq!(outcome.state.first_player(), None);
    }

    #[test]
    fn first_declarer_becomes_the_next_round_first_player() {
        let before = DefState::from_players(limited_player(), limited_player());
        // 本回合先手是我方；敌方先宣布 -> 下回合先手换成敌方（S3.6）
        let declared = round_with_both_declared(&before, Perspective::Opp);
        assert_eq!(declared.round.first_player(), Some(Perspective::Me));
        assert_eq!(declared.round.first_declarer(), Some(Perspective::Opp));
        let next = declared.round.start_next_round();
        assert_eq!(next.first_player(), Some(Perspective::Opp));
        assert!(next.first_declarer().is_none());
        assert!(!next.me_ended() && !next.opp_ended());

        // 回合开始流程把先手接过去，同时回合数 +1、计数清零
        let mut empty = EmptyEnv::new();
        empty.triggers.register(Trigger::new(
            TurnTiming::RoundBegin,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_turn,
        ));
        let (outcome, round_next) = begin_next_round(&before, &declared.round, &empty.env());
        assert_eq!(outcome.state.round(), Nat::new(1));
        assert_eq!(
            outcome.state.me().skill_uses().used_count(NORMAL),
            Nat::ZERO
        );
        assert_eq!(round_next.first_player(), Some(Perspective::Opp));
        // S3.6：先宣布者成为下回合先手 -> 写进根状态。
        assert_eq!(outcome.state.first_player(), Some(Perspective::Opp));
        assert_eq!(outcome.pending.len(), 1);
        assert_eq!(outcome.outcome, Outcome::Ongoing);
        // 双方宣布时写的是「本回合先手」（我方）；新回合才换成先宣布者（敌方）。
        assert_eq!(declared.state.first_player(), Some(Perspective::Me));
    }

    /// 双方都有出战状态（持续回合 1 -> 耗尽）与召唤物（可用次数 2 -> 1）的局面。
    fn two_sided_end_phase_state() -> DefState {
        DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_combat_status(combat_status(n(201), &[(&TEST_STATUS, None, Some(n(1)))]))
                .with_summons(summons(&[(301, 2)])),
            PlayerDefs::new()
                .with_characters(characters(&[401]))
                .with_combat_status(combat_status(n(401), &[(&TEST_STATUS, None, Some(n(1)))]))
                .with_summons(summons(&[(501, 2)])),
        )
    }

    /// 结束阶段用的依赖（阶段广播 + 出战状态 tick + 召唤物 tick）。
    fn end_phase_env() -> EmptyEnv {
        let mut empty = EmptyEnv::new();
        empty.triggers.register(Trigger::new(
            TurnTiming::EndPhase,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_turn,
        ));
        for timing in [
            CombatStatusTiming::DurationTicked,
            CombatStatusTiming::Depleted,
        ] {
            empty.combat_status.register(Trigger::new(
                timing,
                n(1),
                EffectSource::CombatStatus,
                Nat::ZERO,
                noop_combat,
            ));
        }
        for timing in [SummonTiming::Triggered, SummonTiming::Depleted] {
            empty.summons.register(Trigger::new(
                timing,
                n(1),
                EffectSource::Summon,
                Nat::ZERO,
                noop_summon,
            ));
        }
        empty
    }

    #[test]
    fn end_phase_requires_both_declared_and_a_known_first_player() {
        let before = DefState::new();
        let empty = end_phase_env();
        let round = RoundEndState::with_first_player(Perspective::Me);
        assert_eq!(
            end_phase_by_first_player(&before, &round, &empty.env()).err(),
            Some(EndPhaseError::ActionPhaseNotOver)
        );

        // 双方都宣布但没给本回合先手 -> 不知道先手
        let unknown = RoundEndState::new();
        let a = declare_round_end_me(&before, &unknown, &empty.env()).expect("我方宣布");
        let b = declare_round_end_opp(&a.state, &a.round, &empty.env()).expect("敌方宣布");
        assert_eq!(
            end_phase_by_first_player(&before, &b.round, &empty.env()).err(),
            Some(EndPhaseError::FirstPlayerUnknown)
        );

        // 有先手 + 双方都宣布 -> 可以进入结束阶段
        let declared = round_with_both_declared(&before, Perspective::Me);
        assert!(end_phase_by_first_player(&before, &declared.round, &empty.env()).is_ok());
    }

    #[test]
    fn ordered_end_phase_follows_the_first_player() {
        let before = two_sided_end_phase_state();
        let empty = end_phase_env();

        // 先手我方：我方出战状态 -> 我方召唤物 -> 敌方出战状态 -> 敌方召唤物
        let me_round = round_with_both_declared(&before, Perspective::Me).round;
        let me_first =
            end_phase_by_first_player(&before, &me_round, &empty.env()).expect("可进结束阶段");
        let steps = me_first.ordered_end_phase(Perspective::Me);
        assert_eq!(steps.len(), 4);
        assert_eq!(
            steps.iter().map(|step| step.side).collect::<Vec<_>>(),
            vec![
                Perspective::Me,
                Perspective::Me,
                Perspective::Opp,
                Perspective::Opp
            ]
        );
        assert!(matches!(&steps[0].pending, FlowPending::CombatStatus(_)));
        assert!(matches!(&steps[1].pending, FlowPending::Summon(_)));
        assert!(matches!(&steps[2].pending, FlowPending::CombatStatus(_)));
        assert!(matches!(&steps[3].pending, FlowPending::Summon(_)));

        // 先手敌方：敌方出战状态 -> 敌方召唤物 -> 我方出战状态 -> 我方召唤物
        let opp_round = RoundEndState::with_first_player(Perspective::Opp);
        let a = declare_round_end_me(&before, &opp_round, &empty.env()).expect("我方宣布");
        let b = declare_round_end_opp(&a.state, &a.round, &empty.env()).expect("敌方宣布");
        let opp_first =
            end_phase_by_first_player(&before, &b.round, &empty.env()).expect("可进结束阶段");
        let steps = opp_first.ordered_end_phase(Perspective::Opp);
        assert_eq!(
            steps.iter().map(|step| step.side).collect::<Vec<_>>(),
            vec![
                Perspective::Opp,
                Perspective::Opp,
                Perspective::Me,
                Perspective::Me
            ]
        );
        // 先手只影响派发顺序，两类 tick 的最终局面一样（空牌库 -> 各一支）；
        // 但分支里的 DefState.first_player 跟随本次结束阶段先手（调用方拿到即一致）。
        let me_branch = me_first.branches.first().expect("一支");
        let opp_branch = opp_first.branches.first().expect("一支");
        assert_eq!(me_branch.state.me(), opp_branch.state.me());
        assert_eq!(me_branch.state.opp(), opp_branch.state.opp());
        assert_eq!(me_branch.state.round(), opp_branch.state.round());
        assert_eq!(me_branch.state.first_player(), Some(Perspective::Me));
        assert_eq!(opp_branch.state.first_player(), Some(Perspective::Opp));
    }

    #[test]
    fn round_end_state_is_pure_and_tracks_announcements() {
        let fresh = RoundEndState::new();
        assert_eq!(fresh.first_player(), None);
        assert_eq!(fresh.next_first_player(), None);
        assert!(!fresh.me_ended() && !fresh.opp_ended());
        assert!(!fresh.has_ended(Perspective::Me));

        // 宣布不改变原有的记账实例（纯函数）
        let tracked = RoundEndState::with_first_player(Perspective::Opp);
        let env = EmptyEnv::new();
        let _ = declare_round_end_me(&DefState::new(), &tracked, &env.env()).expect("宣布");
        assert!(!tracked.me_ended());
        assert_eq!(tracked.first_declarer(), None);

        // 开始下一回合：清标记，保留 / 接替先手
        let kept = tracked.start_next_round();
        assert_eq!(kept.first_player(), Some(Perspective::Opp));
        assert!(!kept.me_ended() && !kept.opp_ended());
    }
    // ------------------------------------------------------------------
    // 结束阶段顺序（S2.4：先效果、后抓牌）
    // ------------------------------------------------------------------

    /// 结束阶段效果：把「当前手牌张数」记进骰池（用于观测该效果看到的局面）。
    fn record_hand_len_as_dice(state: &mut DefState, _event: &Event, _owner: EntityId) {
        let hand = state.me().hand().len();
        let dice = DicePool::empty().add(DieFace::Pyro, hand);
        let player = state.me().clone().with_dice(dice);
        *state = state.clone().with_me(player);
    }

    #[test]
    fn end_phase_effects_execute_before_the_draw() {
        // 初始手牌 2、牌库 3。结束阶段效果把「看到的手牌数」记成骰子数：
        // - 正确顺序（S2.4：先效果 -> 后抓 2 张）=> 骰子 = 2（效果看到抓牌前的手牌）；
        // - 反过来的顺序（先抓牌 -> 后效果）=> 骰子 = 4。断言 2 即证明顺序正确。
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_hand(hand(&[101, 102]))
                .with_pile(pile(&[103, 104, 105])),
            PlayerDefs::new(),
        );
        let mut env = EmptyEnv::new();
        env.triggers.register(Trigger::new(
            TurnTiming::EndPhase,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            record_hand_len_as_dice,
        ));
        let outcome = end_phase_me_first(&before, &env.env());
        let branch = outcome.single().expect("固定牌库 -> 一支");
        assert_eq!(branch.state.me().dice().total(), Nat::new(2));
        // 第 2 步抓牌已在其后落地：手牌 2 + 2 = 4、牌库 3 - 2 = 1。
        assert_eq!(branch.state.me().hand().len(), Nat::new(4));
        assert_eq!(branch.state.me().pile().len(), Nat::new(1));
        // 效果与抓牌都已在本层落地；保留的待执行只供观测，调用方不要再执行。
        assert_eq!(outcome.pending.len(), 1);
    }
    // ------------------------------------------------------------------
    // 元素幻变（L9）：有卡时 4 骰 -> 2+2，无卡时流程不变
    // ------------------------------------------------------------------

    fn superconduct_rule() -> ElementalTransmutation {
        // 331004 元素幻变：超导祝佑 = 冰 + 雷（顺序不影响规则）。
        ElementalTransmutation::from_elements([Element::Cryo, Element::Electro])
    }

    #[test]
    fn roll_with_transmutation_guarantees_two_plus_two_among_eight_dice() {
        let before = DefState::from_players(limited_player(), limited_player()).with_round(n(2));
        let branches = roll_me_with_transmutation(
            &before,
            Some(RollTransmutation {
                rule: superconduct_rule(),
                opening: false,
            }),
        );
        assert!(!branches.is_empty());
        let mut total = Fraction::ZERO;
        for (state, probability) in &branches {
            total = total.exact_add(*probability);
            let dice = state.me().dice();
            // 总数仍是 8（规则书·投掷阶段）。
            assert_eq!(dice.total(), ROLL_COUNT);
            let snapshot = dice.all_dice(&mut Rng::from_seed(7));
            // 卡面：「总是投出 2 个冰元素骰和 2 个雷元素骰」——每支都至少各 2 颗
            // （其余 4 颗随机；同色会合并，所以用 >=）。
            let known_count = |element: Element| {
                snapshot
                    .known
                    .iter()
                    .find(|(candidate, _)| *candidate == element)
                    .map(|(_, count)| *count)
                    .unwrap_or(Nat::ZERO)
            };
            assert!(known_count(Element::Cryo) >= TRANSMUTATION_FIXED_EACH);
            assert!(known_count(Element::Electro) >= TRANSMUTATION_FIXED_EACH);
        }
        assert!(total.is_one());
        // 纯函数：入参骰池不变。
        assert_eq!(before.me().dice().total(), Nat::ZERO);
    }

    #[test]
    fn roll_without_a_transmutation_card_keeps_the_old_flow() {
        let before = DefState::from_players(limited_player(), limited_player());
        let plain = roll_me(&before);
        // None = 没有对应卡 -> 与 roll_me 完全一致。
        let unchanged = roll_me_with_transmutation(&before, None);
        assert_eq!(plain.len(), unchanged.len());
        for ((state, probability), (plain_state, plain_probability)) in
            plain.iter().zip(unchanged.iter())
        {
            assert_eq!(state, plain_state);
            assert_eq!(probability, plain_probability);
        }
        // 「首次生成」开关在非第 1 轮退化为普通投掷（时序保护；待实测）。
        let opening_outside = roll_me_with_transmutation(
            &before,
            Some(RollTransmutation {
                rule: superconduct_rule(),
                opening: true,
            }),
        );
        assert_eq!(opening_outside.len(), plain.len());
        for ((state, _), (plain_state, _)) in opening_outside.iter().zip(plain.iter()) {
            assert_eq!(state, plain_state);
        }
    }

    #[test]
    fn opening_transmutation_applies_in_round_one() {
        let before = DefState::from_players(limited_player(), limited_player()).with_round(n(1));
        let branches = roll_me_with_transmutation(
            &before,
            Some(RollTransmutation {
                rule: superconduct_rule(),
                opening: true,
            }),
        );
        assert!(!branches.is_empty());
        let mut total = Fraction::ZERO;
        for (state, probability) in &branches {
            total = total.exact_add(*probability);
            let snapshot = state.me().dice().all_dice(&mut Rng::from_seed(11));
            let known_count = |element: Element| {
                snapshot
                    .known
                    .iter()
                    .find(|(candidate, _)| *candidate == element)
                    .map(|(_, count)| *count)
                    .unwrap_or(Nat::ZERO)
            };
            assert!(known_count(Element::Cryo) >= TRANSMUTATION_FIXED_EACH);
            assert!(known_count(Element::Electro) >= TRANSMUTATION_FIXED_EACH);
            assert_eq!(state.me().dice().total(), ROLL_COUNT);
        }
        assert!(total.is_one());
    }

    #[test]
    fn end_phase_draw_syncs_costs_and_materializes_drawn_marks() {
        // F10：新抓手牌按注入的基础费用建 CurrentCost；
        // D188：抽到牌携带的待展开随机状态物化进手牌卡状态表。
        fn base_two(_card: CardId) -> Vec<CostComponent> {
            vec![CostComponent::AnyColor(n(2))]
        }
        let marked = pile(&[101, 102])
            .add_pending_mark(
                Mark {
                    state: &TEST_STATUS,
                    layers: None,
                },
                &[Nat::ZERO, Nat::new(1)],
            )
            .expect("位置合法");
        let before = DefState::from_players(
            PlayerDefs::new().with_pile(marked).with_hand(hand(&[])),
            PlayerDefs::new(),
        )
        .with_round(n(1));
        let empty = EmptyEnv::new();
        let outcome = end_phase_me_first(
            &before,
            &empty.env_with_draw(DrawHooks::with_base_cost(&base_two)),
        );
        let branch = outcome.single().expect("固定牌库 -> 一支");
        assert_eq!(branch.state.me().hand().len(), Nat::new(2));
        for instance in branch.state.me().hand().entries() {
            let cost = branch
                .state
                .me()
                .costs()
                .cost_of(instance.entity)
                .expect("新抓手牌有基础费用");
            assert_eq!(cost.total(), Nat::new(2));
            assert!(branch
                .state
                .me()
                .hand_states()
                .contains_state(instance.entity, &TEST_STATUS));
        }
    }
}
