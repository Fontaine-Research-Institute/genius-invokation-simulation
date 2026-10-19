//! **一次「切换角色」端到端**（战斗行动）：支付 1 个任意元素骰 -> 角色区切换 ->
//! 出战状态转移。
//!
//! ## 依据（唯一权威：规则书 S1–S8 常规模式）
//!
//! - 规则书·进行行动 战斗行动【切换角色】：「支付1个任意类型的元素骰，将一个我方的
//!   『后台角色』切换为『出战角色』。」
//! - 规则书·进行行动 行动：「【战斗行动】完成一个战斗行动后，就会轮到对方行动。
//!   『使用技能』『切换角色』『宣布结束』是战斗行动。」
//!   English: "Using Skills, switching characters, and declaring Round end count as Combat Actions."
//! - 规则书·游戏区域 I(4)：「其他未出战的角色被称为『后台角色』。可以通过『切换角色』行动
//!   来将他们切换为『出战角色』。」II(3)：出战角色被切换为后台角色时，出战状态会转移到新的
//!   出战角色上。
//!
//! ## 本层做什么 / 不做什么
//!
//! - **做**：把「付费 + 区切换」串成一次行动。支付归骰池（D121 / D133），角色区切换与
//!   出战状态转移归 `state::flows::switch`；本层只**组合**，不重写任何区逻辑；
//! - **不做**：行动权推进（`rules::ops::stage` 给「战斗行动 -> 轮到对方」的判定，本层不推进
//!   回合 / 不切行动方，也**不校验现在轮到哪一方**）；不写「切换时 / 切换后」类卡牌效果
//!   （角色区自己派发时机，本层只把待执行原样交回，D186：派发不执行）；不选择切谁
//!   （由调用方 / 搜索层决定）。
//!
//! ## 顺序（= 本函数调用顺序）
//!
//! 1. **校验目标可切**：有出战角色、目标在角色区且不是当前出战角色；
//! 2. **支付 1 个任意骰**：`CostComponent::AnyColor(1)`（D168 / D169 的支付归骰池）；
//! 3. **角色区切换 + 出战状态转移**：`state::flows::switch::{switch_me, switch_opp}`；
//! 4. 把两区派发的待执行摊平成 [PendingSet]（角色区在前、出战状态区在后，对应调用顺序）。
//!
//! **校验先于支付**（与 `play_card` 的「打出条件先于付费」一致）：目标不可切时**不会扣骰**。
//! 规则书没有规定「目标非法」与「骰子不够」同时成立时报哪个；本层先问「能不能做」、
//! 再问「付不付得起」。
//!
//! ## 口径
//!
//! - **纯函数**：吃 &DefState、返回新 DefState + 待执行；失败返回 Err、**入参一个字节不改**
//!   （内部在克隆上做）；
//! - **敌我不编号**（D34）：[switch_character_me] / [switch_character_opp] 成对入口，
//!   共用一份实现（D35），不传 side 参数；
//! - **待执行原样交回**：调用方用 `flow::pending::execute_pending` 执行（角色区 / 出战状态区
//!   的 execute_* 由统一执行器分派）。
//!
//! ## 下落攻击标记（S8.10）
//!
//! 规则书·特殊词条 下落攻击：「角色被切换为『出战角色』后，本回合内的下一个战斗行动若为
//! 普通攻击，则被视为『下落攻击』。」
//!
//! - 切换成功后返回 [SwitchCharacterOutcome::plunging] = [PlungingMark::armed]（待消耗），
//!   并把它**写回切换方**的持有区 `PlayerDefs::plunging`（[SwitchCharacterOutcome::state] 已带）；
//! - 失败（`CannotSwitch` / `NotEnoughDice`）不改根状态：原标记保留；
//! - 规则书没写「切换时已有标记是否刷新」：`armed` 是布尔同一状态，置位与保留等价，
//!   本层不做额外区分。
//!
//! ## 待复核
//!
//! - **支付与「切换时」时机的先后**：规则书没写；本层先支付再派发区时机（支付本身不产生
//!   任何时机，因此不影响派发结果）；
//! - **出战状态转移的时机**：沿用区层（角色区切换之后）；区层文件自己标了待复核。

use crate::families::character::skill_flow::PlungingMark;
use crate::families::character::zones::combat_status::CombatStatusTriggers;
use crate::families::character::zones::field::CharacterZoneTriggers;
use crate::flow::pending::{FlowPending, PendingSet};
use crate::resources::dice::CostComponent;
use crate::state::flows::switch::{switch_me, switch_opp};
use crate::state::DefState;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 切换角色的骰子费用：**1 个任意类型的元素骰**（规则书原文）。
pub const SWITCH_COST_DICE: Nat = Nat::new(1);

/// **切换角色的外部依赖**（只借不拥有）。
///
/// 两个区注册表由调用方持有（每个牌手一个具名实例，D34）；本层只借用。
pub struct SwitchCharacterEnv<'a> {
    /// 角色区触发注册表（切出 / 切入 / 出战变化）。
    pub characters: &'a CharacterZoneTriggers,
    /// 出战状态区触发注册表（切换时转移）。
    pub combat_status: &'a CombatStatusTriggers,
}

/// **一次切换角色的结果**：新的一局 + 待执行（本层不执行，D186）+ 下落攻击标记新值。
#[derive(Clone, Debug)]
pub struct SwitchCharacterOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区 / 出战状态区派发出来的待执行（执行时用 `flow::pending::execute_pending`）。
    pub pending: PendingSet,
    /// 切换成功后的**下落攻击待消耗标记**（S8.10：切换为出战角色后的下一个战斗行动若为
    /// 普通攻击 -> 视为下落攻击）。
    ///
    /// - 持有区 = [crate::state::PlayerDefs::plunging]；本层已把它**写回切换方**的
    ///   [SwitchCharacterOutcome::state]，字段保留供观测 / 调用方对齐；
    /// - 失败时不会返回本结构 -> 根状态不变（原标记保留）；
    /// - 规则书没写「切换时已有标记是否刷新」：`armed` 是布尔同一状态（置位与保留等价）。
    pub plunging: PlungingMark,
}

/// 切换角色失败的原因（所有失败都保证入参不变、不扣骰）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwitchCharacterError {
    /// 目标不可切：没有出战角色 / 目标不在角色区 / 目标已经是出战角色。
    CannotSwitch,
    /// 骰子不够付 1 个任意骰。
    NotEnoughDice,
}

/// **切换角色**（我方，战斗行动）：支付 1 个任意骰 -> 后台角色 `to` 变出战角色。
pub fn switch_character_me(
    state: &DefState,
    env: &SwitchCharacterEnv<'_>,
    to: EntityId,
) -> Result<SwitchCharacterOutcome, SwitchCharacterError> {
    switch_character(state, env, to, true)
}

/// **切换角色**（敌方，战斗行动；与 [switch_character_me] 共用一份实现，D35）。
pub fn switch_character_opp(
    state: &DefState,
    env: &SwitchCharacterEnv<'_>,
    to: EntityId,
) -> Result<SwitchCharacterOutcome, SwitchCharacterError> {
    switch_character(state, env, to, false)
}

/// 共用实现：`switching_me = true` 表示我方切换。
fn switch_character(
    state: &DefState,
    env: &SwitchCharacterEnv<'_>,
    to: EntityId,
    switching_me: bool,
) -> Result<SwitchCharacterOutcome, SwitchCharacterError> {
    let user = if switching_me {
        state.me()
    } else {
        state.opp()
    };
    let zone = user.characters();
    // 1) 校验目标可切（只读；失败不扣骰）。
    if zone.active_entity().is_none() || !zone.contains(to) || zone.is_active(to) {
        return Err(SwitchCharacterError::CannotSwitch);
    }

    // 2) 支付 1 个任意骰；付不起 -> Err、状态不变。
    let cost = [CostComponent::AnyColor(SWITCH_COST_DICE)];
    let Some(paid_dice) = user.dice().pay(&cost, None) else {
        return Err(SwitchCharacterError::NotEnoughDice);
    };
    let paid_player = user.clone().with_dice(paid_dice);
    let paid_state = if switching_me {
        state.clone().with_me(paid_player)
    } else {
        state.clone().with_opp(paid_player)
    };

    // 3) 角色区切换 + 出战状态转移（区层；本层不改区内部实现）。
    let switched = if switching_me {
        switch_me(&paid_state, to, env.characters, env.combat_status)
    } else {
        switch_opp(&paid_state, to, env.characters, env.combat_status)
    };
    let Some(switched) = switched else {
        // 校验通过后区层还拒绝，只可能是两侧不一致（理论不可达）：按不可切处理。
        return Err(SwitchCharacterError::CannotSwitch);
    };

    // 4) 摊平成统一待执行：角色区在前、出战状态区在后（= 本函数的调用顺序）。
    let items: Vec<FlowPending> = switched
        .characters
        .into_iter()
        .map(FlowPending::CharacterZone)
        .chain(
            switched
                .combat_status
                .into_iter()
                .map(FlowPending::CombatStatus),
        )
        .collect();
    let mut pending = PendingSet::new();
    if switching_me {
        pending.me = items;
    } else {
        pending.opp = items;
    }
    // 5) 下落攻击标记：切换为出战角色后置位（S8.10），并**写回切换方**的持有区
    //    （`PlayerDefs::plunging`）；字段保留供观测 / 调用方对齐。
    let plunging = PlungingMark::new().armed();
    let state = switched.state;
    let switched_player = if switching_me {
        state.me().clone().with_plunging(plunging)
    } else {
        state.opp().clone().with_plunging(plunging)
    };
    let state = if switching_me {
        state.clone().with_me(switched_player)
    } else {
        state.clone().with_opp(switched_player)
    };
    Ok(SwitchCharacterOutcome {
        state,
        pending,
        plunging,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::zones::combat_status::{CombatStatusTiming, CombatStatusZone};
    use crate::families::character::zones::field::{CharacterZone, CharacterZoneTiming};
    use crate::resources::dice::{DicePool, DieFace};
    use crate::rules::ops::order::EffectSource;
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{characters, combat_status, entity, n, TEST_STATUS};
    use crate::timeline::{Event, Trigger};
    use gi_genius_invokation_engine::Nat;

    fn noop_character(_zone: &mut CharacterZone, _event: &Event, _owner: EntityId) {}

    fn noop_combat(_zone: &mut CombatStatusZone, _event: &Event, _owner: EntityId) {}

    /// 切换角色用的依赖（三个角色区时机 + 出战状态转移时机都登记，方便看派发条数）。
    struct Env {
        characters: CharacterZoneTriggers,
        combat_status: CombatStatusTriggers,
    }

    impl Env {
        fn new() -> Env {
            let mut characters = CharacterZoneTriggers::new();
            for timing in [
                CharacterZoneTiming::SwitchOut,
                CharacterZoneTiming::SwitchIn,
                CharacterZoneTiming::ActiveChanged,
            ] {
                characters.register(Trigger::new(
                    timing,
                    entity(1),
                    EffectSource::CharacterCard,
                    Nat::ZERO,
                    noop_character,
                ));
            }
            let mut combat_status = CombatStatusTriggers::new();
            combat_status.register(Trigger::new(
                CombatStatusTiming::Transferred,
                entity(1),
                EffectSource::CombatStatus,
                Nat::ZERO,
                noop_combat,
            ));
            Env {
                characters,
                combat_status,
            }
        }

        fn env(&self) -> SwitchCharacterEnv<'_> {
            SwitchCharacterEnv {
                characters: &self.characters,
                combat_status: &self.combat_status,
            }
        }
    }

    fn me_with_dice(pyro: u32) -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(characters(&[201, 202, 203]))
            .with_dice(DicePool::empty().add(DieFace::Pyro, n(pyro)))
    }

    #[test]
    fn switch_pays_one_die_then_changes_active() {
        let before = DefState::from_players(me_with_dice(2), PlayerDefs::new());
        let env = Env::new();
        let outcome = switch_character_me(&before, &env.env(), n(202)).expect("可以切");

        // 生效：出战角色换成 202，槽位顺序不变
        assert_eq!(
            outcome.state.me().characters().active_entity(),
            Some(n(202))
        );
        assert_eq!(
            outcome
                .state
                .me()
                .characters()
                .slots()
                .iter()
                .map(|slot| slot.character)
                .collect::<Vec<_>>(),
            vec![n(201), n(202), n(203)]
        );
        // 支付：2 -> 1
        assert_eq!(outcome.state.me().dice().total(), Nat::new(1));
        // 派发：3 条角色区时机（切出 / 切入 / 出战变化）；敌方没有任何待执行
        assert_eq!(outcome.pending.me.len(), 3);
        assert!(outcome.pending.opp.is_empty());
        // 待执行类型：全是角色区
        assert!(outcome
            .pending
            .me
            .iter()
            .all(|item| matches!(item, FlowPending::CharacterZone(_))));
        // S8.10：切换为出战角色后，下落攻击标记置位并写回切换方持有区（PlayerDefs::plunging）。
        assert!(outcome.plunging.is_pending());
        assert!(outcome.state.me().plunging().is_pending());
        // 纯函数：入参不变
        assert_eq!(before.me().characters().active_entity(), Some(n(201)));
        assert_eq!(before.me().dice().total(), Nat::new(2));
        assert!(before.opp().characters().is_empty());
    }

    #[test]
    fn switch_arms_the_plunging_mark_and_failure_keeps_the_old_one() {
        let before = DefState::from_players(me_with_dice(2), PlayerDefs::new());
        let env = Env::new();
        let outcome = switch_character_me(&before, &env.env(), n(202)).expect("可以切");
        // S8.10：切换为出战角色后，下一个战斗行动若为普通攻击 -> 下落攻击（先置位）。
        assert!(outcome.plunging.is_pending());
        assert!(outcome.state.me().plunging().is_pending());
        // 失败不产生新标记（Err 没有 outcome / 状态不变）-> 原标记保留。
        assert!(switch_character_me(&before, &env.env(), n(999)).is_err());
        assert!(switch_character_opp(&before, &env.env(), n(999)).is_err());
        assert!(!before.me().plunging().is_pending());
    }

    #[test]
    fn switch_moves_the_combat_status_and_returns_its_pending() {
        let before = DefState::from_players(
            me_with_dice(1)
                .with_combat_status(combat_status(n(201), &[(&TEST_STATUS, None, Some(n(2)))])),
            PlayerDefs::new(),
        );
        let env = Env::new();
        let outcome = switch_character_me(&before, &env.env(), n(202)).expect("可以切");
        // 出战状态转移到新出战角色（条目原样保留）
        assert_eq!(
            outcome.state.me().combat_status().attached_to(),
            Some(n(202))
        );
        assert_eq!(outcome.state.me().combat_status().len(), Nat::new(1));
        // 派发：3 条角色区 + 1 条出战状态转移
        assert_eq!(outcome.pending.me.len(), 4);
        assert!(matches!(
            outcome.pending.me.last(),
            Some(FlowPending::CombatStatus(_))
        ));
        assert_eq!(outcome.state.me().dice().total(), Nat::ZERO);
    }

    #[test]
    fn switch_without_enough_dice_changes_nothing() {
        let before = DefState::from_players(me_with_dice(0), PlayerDefs::new());
        let env = Env::new();
        let error = switch_character_me(&before, &env.env(), n(202)).expect_err("没骰子");
        assert_eq!(error, SwitchCharacterError::NotEnoughDice);
        assert_eq!(before.me().characters().active_entity(), Some(n(201)));
        assert_eq!(before.me().dice().total(), Nat::ZERO);
    }

    #[test]
    fn switch_rejects_unknown_same_or_no_active_without_paying() {
        let before = DefState::from_players(me_with_dice(2), PlayerDefs::new());
        let env = Env::new();
        // 不在角色区
        assert_eq!(
            switch_character_me(&before, &env.env(), n(999)).expect_err("不在区里"),
            SwitchCharacterError::CannotSwitch
        );
        // 已经是出战角色
        assert_eq!(
            switch_character_me(&before, &env.env(), n(201)).expect_err("已经出战"),
            SwitchCharacterError::CannotSwitch
        );
        // 没有出战角色
        let empty = DefState::new();
        assert_eq!(
            switch_character_me(&empty, &env.env(), n(201)).expect_err("没有出战角色"),
            SwitchCharacterError::CannotSwitch
        );
        // 失败都不扣骰（校验先于支付）
        assert_eq!(before.me().dice().total(), Nat::new(2));
    }

    #[test]
    fn opp_switch_only_touches_the_opponent_side() {
        let me = me_with_dice(1);
        let opp = PlayerDefs::new()
            .with_characters(characters(&[301, 302]))
            .with_dice(DicePool::empty().add(DieFace::Pyro, n(1)));
        let before = DefState::from_players(me.clone(), opp);
        let env = Env::new();
        let outcome = switch_character_opp(&before, &env.env(), n(302)).expect("可以切");
        assert_eq!(
            outcome.state.opp().characters().active_entity(),
            Some(n(302))
        );
        assert_eq!(outcome.state.opp().dice().total(), Nat::ZERO);
        // 我方一个字节都没动
        assert_eq!(outcome.state.me(), &me);
        assert_eq!(outcome.pending.opp.len(), 3);
        assert!(outcome.pending.me.is_empty());
        // 下落标记写回**切换方**（敌方）；我方持有区不受影响。
        assert!(outcome.plunging.is_pending());
        assert!(outcome.state.opp().plunging().is_pending());
        assert!(!outcome.state.me().plunging().is_pending());
    }
}
