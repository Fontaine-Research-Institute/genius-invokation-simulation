//! **复苏（L2）端到端**（规则书·特殊词条 料理 + 权威关键字；S8.1(4)）。
//!
//! ## 依据（权威顺序 RULES §三.3）
//!
//! - 规则书·特殊词条 **料理**（第一权威）：「另外，打出具有「复苏」效果的「料理」事件牌后，
//!   本回合内就不能打出其他具有「复苏」效果的「料理」事件牌了。」——只规定**每回合次数**，
//!   没有写复活后生命值 / 装备 / 状态怎么处理；
//! - 规则书·其他规则 **击倒**：「此时，角色身上所附属的装备和状态都会被弃置，充能会被清空。」
//!   -> 这些在击倒时已经清掉，复苏**不恢复**（由 state::flows::revive 负责，本层只串联）；
//! - 卡面（第二权威）：复苏后的生命值由卡面写（例如「治疗到 1 点生命值」），由调用方给 hp；
//! - 原语（只复用、不重写）：
//!   - crate::rules::ops::food::can_revive：本回合是否还能通过料理复苏；
//!   - crate::state::flows::revive::revive_me / revive_opp：击倒区 -> 生命复位 ->
//!     放回角色区 -> 从击倒区移除（纯函数，返回角色区待执行）。
//!
//! ## 本层做什么
//!
//! 把「次数判定 -> 状态流程 -> 记次数 -> 返回新区 + 角色区待执行」串成一个纯函数入口：
//! 吃 &DefState、返回新 DefState（+ 待执行 + 更新后的本回合复苏计数）；失败返回 Err、
//! **入参一个字节都不改**。
//!
//! ## 每回合次数由谁持有
//!
//! state 层没有「本回合已复苏数」字段（不在本任务写范围）；按 D182 / 既有 flow 口径，
//! 由**调用方持有**并把当前值放进 ReviveEnv.revived_this_round，成功后从
//! ReviveOutcome.revived_this_round 取回新值（+1）。回合结束时调用方把计数清零
//! （规则书·特殊词条 料理：本回合内限制）。
//!
//! ## 规则书没写清 / TODO（不自行编排）
//!
//! - **复苏后是否成为出战角色 / 是否派发「出战角色变化」**：规则书没写；复用角色区既有语义
//!   （角色区没有出战角色时自动成为出战），见 state::flows::revive 的 TODO；
//! - **复苏是否派发「进入角色区」**：规则书没写；现按区既有语义派发 Entered；
//! - **复苏是否恢复生命上限成长 / 装备 / 状态**：击倒时已清，复苏不恢复（TODO 待实测）；
//! - **复苏计数属于「每位牌手」还是全局**：规则书「本回合内就不能打出其他…」按牌手执行，
//!   本层按每位牌手各自的计数建模（调用方按方传）；
//! - **「具有复苏效果的料理事件牌」本身**（是不是料理 / 有没有复苏）由卡牌定义判定，
//!   本层不认具体卡牌：只收 CardId 做记录 / 观测。

use crate::base::CardId;
use crate::families::character::zones::field::{CharacterZonePending, CharacterZoneTriggers};
use crate::rules::ops::food::can_revive;
use crate::state::flows::revive::{
    revive_me as revive_character_me, revive_opp as revive_character_opp,
};
use crate::state::DefState;
use gi_genius_invokation_engine::{nadd, EntityId, Nat};

/// 一次复苏的**外部依赖**（只借不拥有）。
pub struct ReviveEnv<'a> {
    /// 角色区触发注册表（复苏会派发「进入角色区」时机）。
    pub characters: &'a CharacterZoneTriggers,
    /// **本回合**已经通过料理复苏的角色数（调用方持有；每回合上限见 rules::ops::food）。
    pub revived_this_round: Nat,
}

impl<'a> ReviveEnv<'a> {
    /// 建复苏依赖。
    pub const fn new(
        characters: &'a CharacterZoneTriggers,
        revived_this_round: Nat,
    ) -> ReviveEnv<'a> {
        ReviveEnv {
            characters,
            revived_this_round,
        }
    }
}

/// 复苏失败的原因（所有失败都保证入参不变、不消耗次数）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReviveError {
    /// 本回合的复苏次数已经用完（规则书·特殊词条 料理；每回合 1 次）。
    NoReviveLeft,
    /// 这个角色不在击倒区（没被击倒 / 已经复苏过）。
    NotRevivable,
    /// 角色区已满（3 个槽位）：放不回去。
    CharacterZoneFull,
}

/// 一次复苏的结果（纯数据）。
#[derive(Clone, Debug)]
pub struct ReviveOutcome {
    /// 复苏后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 execute_character_zone）。
    pub characters: Vec<CharacterZonePending>,
    /// 这次复苏用的「复苏料理」卡 id（记录 / 观测用；数值来自卡面，由调用方给 hp）。
    pub card: CardId,
    /// **更新后**的本回合复苏计数（= 传入值 + 1；调用方写回自己的计数）。
    pub revived_this_round: Nat,
}

/// **我方角色复苏**（L2）：次数判定 -> 击倒区取人 -> 生命复位 -> 放回角色区 -> 记次数。
///
/// - card：具有复苏效果的料理事件牌 id（只做记录 / 观测；本层不解释卡牌语义）；
/// - character：要复苏的角色实体（必须在我方击倒区）；
/// - hp：复苏后生命值（卡面给；规则书没写具体值）；
/// - 失败 -> Err、入参一个字节都不改、**不消耗**本回合复苏次数。
pub fn revive_me(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
) -> Result<ReviveOutcome, ReviveError> {
    revive_side(state, env, card, character, hp, true)
}

/// **敌方角色复苏**（与 revive_me 共用一份实现，D35）。
pub fn revive_opp(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
) -> Result<ReviveOutcome, ReviveError> {
    revive_side(state, env, card, character, hp, false)
}

/// 共用实现：on_me = true 表示复苏我方角色。
fn revive_side(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
    on_me: bool,
) -> Result<ReviveOutcome, ReviveError> {
    // 1) 次数：每回合最多通过料理复苏 1 个角色（规则书·特殊词条 料理）。
    if !can_revive(env.revived_this_round) {
        return Err(ReviveError::NoReviveLeft);
    }
    // 2) 角色必须真的在击倒区（失败不消耗次数）。
    let player = if on_me { state.me() } else { state.opp() };
    if player.defeated_character(character).is_none() {
        return Err(ReviveError::NotRevivable);
    }
    // 3) 角色区已满则放不回去（先判，避免 state 流程返回 None 后丢失原因）。
    if player.characters().is_full() {
        return Err(ReviveError::CharacterZoneFull);
    }
    // 4) 状态流程（击倒区 -> 生命复位 -> 放回角色区 -> 从击倒区移除）。
    let outcome = if on_me {
        revive_character_me(state, character, hp, env.characters)
    } else {
        revive_character_opp(state, character, hp, env.characters)
    };
    let Some(outcome) = outcome else {
        return Err(ReviveError::NotRevivable);
    };
    Ok(ReviveOutcome {
        state: outcome.state,
        characters: outcome.characters,
        card,
        revived_this_round: nadd!(env.revived_this_round, Nat::new(1)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::character::CharacterState;
    use crate::families::character::zones::field::{
        CharacterSlot, CharacterZone, CharacterZoneTiming,
    };
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{characters, n};
    use crate::state::PlayerDefs;
    use crate::timeline::{Event, Trigger};

    const REVIVE_FOOD: CardId = CardId::new(333009);

    /// 我方：角色区空、击倒区里有 201；敌方：1 个角色。
    fn state_with_me_defeated() -> DefState {
        let defeated = CharacterSlot::new(
            n(201),
            CharacterState::new(Nat::new(10), Nat::new(10), Nat::ZERO, Nat::new(2)),
        );
        DefState::from_players(
            PlayerDefs::new().with_defeated(vec![defeated]),
            PlayerDefs::new().with_characters(characters(&[401])),
        )
    }

    fn env<'a>(characters: &'a CharacterZoneTriggers) -> ReviveEnv<'a> {
        ReviveEnv::new(characters, Nat::ZERO)
    }

    #[test]
    fn revive_puts_the_character_back_and_counts_once() {
        let before = state_with_me_defeated();
        // 登记「进入角色区」触发，验证角色区 pending 被派发出来。
        fn noop(_zone: &mut CharacterZone, _event: &Event, _owner: EntityId) {}
        let mut triggers = CharacterZoneTriggers::new();
        triggers.register(Trigger::new(
            CharacterZoneTiming::Entered,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop,
        ));
        let outcome =
            revive_me(&before, &env(&triggers), REVIVE_FOOD, n(201), n(1)).expect("可复苏");
        // 角色回到角色区、击倒区清空、HP = 参数。
        assert_eq!(outcome.state.me().characters().len(), Nat::new(1));
        assert!(outcome.state.me().defeated().is_empty());
        let slot = outcome
            .state
            .me()
            .characters()
            .slot_of(n(201))
            .expect("回到角色区");
        assert_eq!(slot.state.hp(), n(1));
        // 角色区派发了「进入角色区」待执行。
        assert_eq!(outcome.characters.len(), 1);
        // 次数 +1、卡 id 原样带回。
        assert_eq!(outcome.revived_this_round, n(1));
        assert_eq!(outcome.card, REVIVE_FOOD);
        // 纯函数：入参不变。
        assert_eq!(before.me().characters().len(), Nat::ZERO);
        assert_eq!(before.me().defeated().len(), 1);
        // 敌方一个字节都没动。
        assert_eq!(outcome.state.opp(), before.opp());
    }

    #[test]
    fn revive_is_rejected_when_the_round_count_is_used_up() {
        let before = state_with_me_defeated();
        let triggers = CharacterZoneTriggers::new();
        let used = ReviveEnv::new(&triggers, Nat::new(1));
        let error = revive_me(&before, &used, REVIVE_FOOD, n(201), n(1)).expect_err("次数用完");
        assert_eq!(error, ReviveError::NoReviveLeft);
        // 入参不变。
        assert_eq!(before.me().characters().len(), Nat::ZERO);
        assert_eq!(before.me().defeated().len(), 1);
    }

    #[test]
    fn revive_rejects_characters_that_are_not_defeated() {
        let triggers = CharacterZoneTriggers::new();
        // 活着的角色不能复苏。
        let alive = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new(),
        );
        assert_eq!(
            revive_me(&alive, &env(&triggers), REVIVE_FOOD, n(201), n(1)).expect_err("不在击倒区"),
            ReviveError::NotRevivable
        );
        // 未知角色也不能。
        let before = state_with_me_defeated();
        assert_eq!(
            revive_me(&before, &env(&triggers), REVIVE_FOOD, n(999), n(1)).expect_err("未知角色"),
            ReviveError::NotRevivable
        );
    }

    #[test]
    fn revive_rejects_a_full_character_zone() {
        let defeated = CharacterSlot::new(
            n(204),
            CharacterState::new(Nat::new(10), Nat::new(10), Nat::ZERO, Nat::new(2)),
        );
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201, 202, 203]))
                .with_defeated(vec![defeated]),
            PlayerDefs::new(),
        );
        let triggers = CharacterZoneTriggers::new();
        assert_eq!(
            revive_me(&before, &env(&triggers), REVIVE_FOOD, n(204), n(1)).expect_err("区满"),
            ReviveError::CharacterZoneFull
        );
        // 失败不消耗次数、不改状态。
        assert_eq!(before.me().characters().len(), Nat::new(3));
        assert_eq!(before.me().defeated().len(), 1);
    }

    #[test]
    fn revive_opp_does_not_touch_me() {
        let defeated = CharacterSlot::new(
            n(401),
            CharacterState::new(Nat::new(10), Nat::new(10), Nat::ZERO, Nat::new(2)),
        );
        let me = PlayerDefs::new().with_characters(characters(&[201]));
        let opp = PlayerDefs::new().with_defeated(vec![defeated]);
        let before = DefState::from_players(me, opp);
        let me_snapshot = before.me().clone();
        let triggers = CharacterZoneTriggers::new();
        let outcome =
            revive_opp(&before, &env(&triggers), REVIVE_FOOD, n(401), n(2)).expect("可复苏");
        assert_eq!(outcome.state.opp().characters().len(), Nat::new(1));
        assert_eq!(
            outcome
                .state
                .opp()
                .characters()
                .slot_of(n(401))
                .expect("回到角色区")
                .state
                .hp(),
            n(2)
        );
        // 我方一个字节都没动。
        assert_eq!(outcome.state.me(), &me_snapshot);
        assert_eq!(outcome.revived_this_round, n(1));
    }
}
