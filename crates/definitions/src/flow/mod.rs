//! **端到端接线层**：把各块串成「一局对局实际怎么走」（D184 的最后一步：接线，不写具体卡）。
//!
//! 本目录只做**串联与分派**，不重写任何规则：
//!
//! | 文件 | 串什么 | 上游块 |
//! |---|---|---|
//! | skill_use.rs | 一次「使用技能」端到端：能否使用 -> 派发「使用技能时」-> 支付（骰子 + 充能）-> 执行「使用技能时」-> 效果体（默认 keywords 一步；旧效果体计划兼容）-> 步末 flush -> 派发「使用技能后」（插入式结算）；含下落攻击标记（S8.10：读根状态、战斗行动结束清除并写回）与分支终局判定 | families/character/skill_flow/**、keywords/**、families/character/damage.rs、damage_chain/**、reactions/** |
//! | play_card.rs | 一次「打出行动牌」端到端：打出条件 -> 派发并执行「打出前」-> 付费 -> 按 ActionCardType 落位（支援区 / 装备栏 / 事件离场）-> 事件牌效果体（keywords 一步）-> 步末 flush -> 派发「打出后」；分支带终局判定；**打牌时机判定**（S3.1 / S8.8）；**支援牌持续回合写入**（Z6/Z14）；**料理/饱腹**（L1）与**复苏料理**（L2，组合 flow::revive） | families/action_card/**、keywords/**、families/character/zones/equipment.rs |
//! | switch_character.rs | 一次「切换角色」端到端（战斗行动）：支付 1 个任意骰 -> 角色区切换 -> 出战状态转移 -> 下落攻击标记置位并写回切换方（S8.10） | state/flows/switch.rs、resources/dice/** |
//! | tuning.rs | 一次「元素调和」端到端（快速行动）：弃 1 张手牌 -> 转 1 个骰为出战角色元素类型（未指定来源骰的内置算法留空，D117） | families/action_card/zones/hand、resources/dice/** |
//! | game_end.rs | **终局判定接线**：只读 `DefState` 的 `settlement_outcome` 薄入口（S1.4 胜利条件 / S7.7 回合上限） | state/terminal.rs |
//! | elemental_transmutation.rs | **元素幻变（K67）单独钩子**：投掷阶段 2+2 / 第 1 轮「首次生成」4 骰 -> 2+2 的可调用入口（投掷流程的接入在 turn.rs 的 [RollTransmutation] 显式开关；时序待实测，留空 #15/#64） | rules/ops/opening.rs、resources/dice/** |
//! | record.rs | flow 的记录内容类型（D225 / D227：使用技能 / 打出牌 / 一步 / 步末 flush / 步边界插入） | records/** |
//! | recorded.rs | 记录接线（D223 / D224：成功结算后把事件写进调用方给的 Recorder）；打牌有 `_with_timing` 变体（S8.8 战斗行动标签） | records/** |
//! | turn.rs | 回合骨架：回合开始（计数清零 + 回合上限判定）-> 投掷阶段（8 骰 + 1 次重投）-> 行动阶段（调用方驱动；含宣布回合结束 F4 + 先手归属）-> 结束阶段（各区块 tick（含支援区持续回合 Z6/S8.6）-> **先执行结束阶段触发的效果**（S2.4 第 1 步，先手顺序）-> 双方各抓 2 张（第 2 步，分支 + 折叠））-> 回合结束广播；宣布结束（双方都宣布后）/ 进新回合时把先手牌手写进 `DefState.first_player`；**元素幻变投掷开关**（L9：[roll_me_with_transmutation]/[roll_opp_with_transmutation]，有对应卡时 8 颗里固定 2+2） | state/flows/**、resources/dice/** |
//! | pending.rs | **统一执行器**：把各区的待执行按类型分派到对应的 execute_*（只分派，不写效果） | 各区 execute_* |
//!
//! ## 口径（与各块一致，逐条）
//!
//! - **纯函数优先**：入口吃 &DefState、返回新 DefState（+ 待执行）；失败返回 Err，
//!   **入参一个字节都不改**（内部全部在克隆上做，Err 时克隆被丢弃）。唯一例外是
//!   pending::execute_pending / execute_frames 这类**执行器**：它们必须调用各区的
//!   execute_*（&mut 区），所以对外仍以「吃 &DefState、返回新 DefState」的纯函数形状出现；
//! - **派发只排序、不执行**（D186）：timeline::dispatch 产出「函数指针 + 事件」的有序列表；
//!   **本层是允许调用执行器的「调用方」**，执行仍只走各区的 execute_*（pending.rs 统一分派）；
//! - **插入式结算**（D196）：skill_use 把「使用技能后」用 timeline::Resolution::insert
//!   **插到调用方当前结算位置**，子结算做完回到插入点继续（不是 LIFO 触发栈）；
//! - **惰性不确定 + 观测展开 + 折叠回**（D190 / D191）：投掷 / 重投 / 反应效果这类观测点返回
//!   Vec<(DefState, Fraction)> 分支（精确概率、零浮点、不采样）；局面完全相同的分支用
//!   [fold_states] 合并、概率精确相加（折叠回）；
//! - **区自持、对外只读指针**（D182 / D183）：本层不改任何区的内部实现，只在区之间搬新值
//!   （各区的 with_* 构造器 / 只读指针）；
//! - **敌我不编号**（D29 / D34）：需要「对某一方做」的入口一律**成对**出现
//!   （use_skill_me / use_skill_opp、play_card_me / play_card_opp、roll_me / roll_opp…），
//!   共用实现只写一份；不传 Side 参数、不用 [PlayerDefs; 2] + 下标；
//! - **数值全是 Nat**（饱和）、**概率全是 Fraction**（分子 / 分母 ExactNat128）；
//!   本目录零浮点、零 HashMap、零裸 + - * /（§8.3）。
//!
//! ## 依据（唯一权威）
//!
//! - 规则书·回合流程（投掷 / 行动 / 结束阶段）、进行行动（使用技能 / 切换角色 / 打出手牌 /
//!   元素调和 / 宣布结束）、游戏区域（手牌 / 支援区 / 装备栏）、元素反应·效果、
//!   其他规则（结算顺序 / 击倒）；
//! - 结算顺序.md：§一 同一时机内（当前牌手 -> 来源类别 -> 进场先后 -> 登记顺序）、
//!   §二 插入式结算（不是栈）、§三 常见插入点（造成伤害 / 扣血后 / 元素伤害附着 /
//!   生命值降到 0 / 结束阶段）；
//! - 工作区口径：D29 / D34（敌我成对入口）、D121 / D133（支付归被花的东西）、
//!   D182 / D183（区自持 + 只读指针）、D185 / D186（时机键归家族、登记函数指针、派发不执行）、
//!   D190 / D191（观测展开 + 折叠回）、D194 / D196（结算帧 + 插入）。
//!
//! ## 待接入（汇总；各文件里还有更细的条目）
//!
//! 1. **击倒清理**（D255）：flow 在 hooks.rs 留了 [DefeatCleanup] **回调注入点**；
//!    use_skill_* / play_card_* 在**每支结算后、判终局前**调用它（订单「结算 -> 清理 -> 判终局」）。
//!    清理本体 [crate::state::flows::defeat] 需要按角色注入 `DefeatHpReset{定义 max_hp, GrowthList}`
//!    （角色定义 / 成长不在 definitions 层），由上层（cards）实现回调注入；不注入时
//!    [NO_DEFEAT_CLEANUP] = 不清理（旧行为）。
//! 2. **抓牌落地**（F10 / D188）：turn 的结束阶段抓牌接 [DrawHooks] 注入点：
//!    `base_cost` 回调 -> 新抓手牌建 CurrentCost；`DrawBranch::drawn_marks` -> 手牌卡状态表
//!    （物化时不派发状态触发，触发派发待接入）。
//!
//! （其它仍待接入的细项——费用修正链 / `SkillDef::effect` 占位 / Queued 等价比较——
//! 见各文件自己的「待接入 / 待复核」小节。）

pub mod elemental_transmutation;
pub mod game_end;
pub mod hooks;
pub mod pending;
pub mod play_card;
pub mod record;
pub mod recorded;
pub mod revive;
pub mod skill_use;
pub mod switch_character;
pub mod technique;
pub mod tuning;
pub mod turn;

pub use elemental_transmutation::{
    transmute_opening_me, transmute_opening_opp, transmute_roll_phase_me, transmute_roll_phase_opp,
};
pub use game_end::{is_terminal, settlement_outcome, Outcome};
pub use hooks::{
    apply_defeat_cleanup, apply_draw_branch, DefeatCleanup, DrawHooks, NO_DEFEAT_CLEANUP,
};
pub use pending::{execute_frames, execute_pending, execute_pending_mut, FlowPending, PendingSet};
pub use play_card::{
    play_card_me, play_card_me_arcane, play_card_me_arcane_with_timing, play_card_me_food,
    play_card_me_food_with_timing, play_card_me_with_timing, play_card_opp, play_card_opp_arcane,
    play_card_opp_arcane_with_timing, play_card_opp_food, play_card_opp_food_with_timing,
    play_card_opp_with_timing, ArcanePlayOutcome, CardPlacement, FoodPlayOutcome, FoodReviveEnv,
    KeywordEnv, PlayBranch, PlayCardEnv, PlayCardError, PlayCardOutcome, PlayCardRequest,
    PlayEffect, PlayPending, PlayTiming, PlayTrigger, PlayTriggerBook, PlayTriggers,
};
pub use record::{FlowRecord, SkillUseRecord};
pub use recorded::{
    play_card_me_recorded, play_card_me_recorded_with_timing, play_card_opp_recorded,
    play_card_opp_recorded_with_timing, use_skill_me_recorded, use_skill_opp_recorded,
};
pub use revive::{revive_me, revive_opp, ReviveEnv, ReviveError, ReviveOutcome};
pub use skill_use::{
    use_skill_me, use_skill_me_with_plunging, use_skill_opp, use_skill_opp_with_plunging,
    SkillBranch, SkillDamageSummary, SkillEffectPlan, SkillFlowEnv, SkillOutcome, SkillTriggerBook,
    SkillUseError, SkillUseRequest,
};
pub use switch_character::{
    switch_character_me, switch_character_opp, SwitchCharacterEnv, SwitchCharacterError,
    SwitchCharacterOutcome, SWITCH_COST_DICE,
};
pub use technique::{
    use_technique_me, use_technique_opp, TechniqueEnv, TechniqueError, TechniqueOutcome,
    TechniqueRequest,
};
pub use tuning::{
    tuning_me, tuning_opp, TuningError, TuningOutcome, TuningRequest, TUNING_DICE_COUNT,
    TUNING_HAND_DISCARD,
};
pub use turn::{
    action_phase_end, action_phase_start, begin_next_round, declare_round_end_me,
    declare_round_end_opp, end_phase_by_first_player, end_phase_me_first, end_phase_opp_first,
    reroll_me, reroll_opp, roll_me, roll_opp, roll_phase, round_begin, round_end,
    DeclareRoundEndError, DeclareRoundEndOutcome, EndPhaseError, EndPhaseStep, RoundBeginOutcome,
    RoundEndState, TurnEffect, TurnEndBranch, TurnEndOutcome, TurnEnv, TurnPending, TurnTiming,
    TurnTrigger, TurnTriggers, ROLL_COUNT,
};

use crate::state::DefState;
use gi_genius_invokation_engine::prob::Fraction;

/// **折叠回**（D191）：把局面完全相同的分支合并，概率**精确相加**。
///
/// 观测点展开出的分支是临时的（D191：展开 / 回收成对）：一旦不再需要区分「怎么走到这个局面」，
/// 就把等价分支折叠回一支。比较口径 = DefState 的 PartialEq（各区自己的等价语义，
/// 统计不参与，D187）；顺序保持首次出现顺序（可复现）。
///
/// 注意：只比较**局面**，不比较待执行（Queued 没有 PartialEq）；需要连待执行一起折叠时，
/// 由调用方先执行完待执行再调本函数（skill_use / play_card 内部就是这么用的）。
pub fn fold_states(branches: Vec<(DefState, Fraction)>) -> Vec<(DefState, Fraction)> {
    let mut folded: Vec<(DefState, Fraction)> = Vec::new();
    for (state, probability) in branches {
        match folded.iter_mut().find(|(existing, _)| *existing == state) {
            Some(entry) => entry.1 = entry.1.exact_add(probability),
            None => folded.push((state, probability)),
        }
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::dice::{DicePool, DieFace};
    use crate::state::PlayerDefs;
    use gi_genius_invokation_engine::nat::ExactNat128;
    use gi_genius_invokation_engine::Nat;

    /// 精确分数 1 / denominator。
    fn fraction(denominator: u128) -> Fraction {
        Fraction::new(ExactNat128::new(1), ExactNat128::new(denominator))
    }

    /// 只改我方骰池的一局（用来造「不同 / 相同局面」）。
    fn state_with_pyro(count: u32) -> DefState {
        DefState::from_players(
            PlayerDefs::new().with_dice(DicePool::empty().add(DieFace::Pyro, Nat::new(count))),
            PlayerDefs::new(),
        )
    }

    #[test]
    fn fold_states_merges_equal_states_and_keeps_first_seen_order() {
        // 三支：A、B、A -> 折叠成 A（1/4 + 3/4 = 1）、B（1/2）
        let a = state_with_pyro(1);
        let b = state_with_pyro(2);
        let half = fraction(2);
        let three_quarters =
            fraction(4).exact_mul(Fraction::new(ExactNat128::new(3), ExactNat128::new(1)));
        let folded = fold_states(vec![
            (a.clone(), fraction(4)),
            (b.clone(), half),
            (a.clone(), three_quarters),
        ]);
        assert_eq!(folded.len(), 2);
        assert_eq!(folded[0].0, a);
        assert_eq!(folded[1].0, b);
        assert_eq!(folded[0].1, Fraction::ONE);
        assert_eq!(folded[1].1, half);
    }

    #[test]
    fn fold_states_with_empty_input_is_empty_and_single_branch_unchanged() {
        assert!(fold_states(Vec::new()).is_empty());
        let single = state_with_pyro(3);
        let folded = fold_states(vec![(single.clone(), Fraction::ONE)]);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].0, single);
        assert!(folded[0].1.is_one());
    }
}
