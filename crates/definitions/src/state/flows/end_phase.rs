//! **结束阶段流程**：出战状态 tick -> 召唤物 tick（持续回合 / 可用次数扣减）。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json`）
//!
//! - 规则书·回合流程 结束阶段：「1、从『先手牌手』开始，处理在『结束阶段』触发的卡牌效果。
//!   2、每位牌手从自己的牌堆中抓 2 张牌。3、开始新的回合！」
//! - 规则书·特殊词条 持续回合：「每个结束阶段，都会使『持续回合』-1。『持续回合』耗尽后，
//!   会立刻弃置这张牌。」English: "Each End Phase, 1 Round will be removed from such a card's Duration."
//! - 规则书·特殊词条 可用次数：「每次触发这张牌的效果，都会消耗 1 次『可用次数』。『可用次数』耗尽后，
//!   会立刻弃置这张牌。」
//! - 规则书·特殊词条 料理：「『饱腹』状态会在回合结束或角色被击倒时解除。」
//!   → 结束阶段对**双方角色**调用 `families::character::satiated::clear_at_round_end`
//!   （角色被击倒时的解除在 `state::flows::defeat`）；
//! - 结算顺序（`结算顺序.md` §一）：当前牌手优先 -> 来源类别（角色牌 / 角色装备和状态 /
//!   阵营出战状态 / 召唤物 / 支援牌）-> 进场先后。
//!   → 结束阶段本层按「先手方：出战状态 -> 召唤物；再另一方同样」的顺序调（**只排区级先后**，
//!   区内部顺序由各区的 `dispatch` 保证）。
//!
//! ## 扣减全部复用 `rules/ops/lifetime.rs`（本层不重写数值）
//!
//! - 持续回合 -1 / 耗尽判定：`duration_after_end_phase` / `is_depleted`
//!   （由 `CombatStatusZone::tick_end_phase_with_triggers` 内部调用）；
//! - 可用次数 -1 / 耗尽判定：`uses_after_trigger` / `is_depleted`
//!   （由 `SummonZone::tick_with_triggers` 内部调用）。
//!
//! 本流程只负责**什么时候调它们**（结束阶段）与**按什么顺序**调（先手方先）。
//!
//! ## 先手是谁
//!
//! 规则书写「从『先手牌手』开始」。本层**不引入 side 枚举**（D34），改成两个入口：
//! [`end_phase_me_first`] / [`end_phase_opp_first`]（D29：敌我用多个函数表达）。
//!
//! ## 待接入
//!
//! - **支援牌的持续回合**：`SupportEntry` 没有 `duration` 字段（只读文件），本流程跳过；
//! - **结束阶段触发的角色牌 / 装备 / 角色状态效果**：它们由各块的触发注册表在效果执行时派发
//!   （D185），本流程只做「会自己按结束阶段递减」的那两类承载（出战状态 / 召唤物）；
//! - **结束阶段抓 2 张牌**（规则书回合流程第 2 步）归牌库块，本流程不做。
//!
//! ## 待实测（规则书没写清）
//!
//! - 「饱腹」在结束阶段的**哪一步**解除：规则书只写「回合结束」，没写与「持续回合 -1」/
//!   结束阶段触发效果的先后 -> 现放在本流程每侧 tick 之后（返回前）；待实测。

use crate::families::character::character::CharacterState;
use crate::families::character::satiated::clear_at_round_end;
use crate::families::character::zones::combat_status::{CombatStatusPending, CombatStatusTriggers};
use crate::families::summon::zones::{SummonPending, SummonTriggers};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::EntityId;

/// 一次结束阶段派发出来的全部待执行（本层不执行，D186）。
///
/// 执行顺序（= 本层调用顺序）：先手方出战状态 -> 先手方召唤物 -> 另一方出战状态 -> 另一方召唤物。
#[derive(Clone, Debug, Default)]
pub struct EndPhasePending {
    /// 我方出战状态区派发的待执行。
    pub me_combat_status: Vec<CombatStatusPending>,
    /// 我方召唤物区派发的待执行。
    pub me_summons: Vec<SummonPending>,
    /// 敌方出战状态区派发的待执行。
    pub opp_combat_status: Vec<CombatStatusPending>,
    /// 敌方召唤物区派发的待执行。
    pub opp_summons: Vec<SummonPending>,
}

/// 一次结束阶段的结果：新的一局 + 待执行。
pub struct EndPhaseOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 派发出来的待执行。
    pub pending: EndPhasePending,
}

/// **结束阶段**：我方先手（我方出战状态 -> 我方召唤物 -> 敌方出战状态 -> 敌方召唤物）。
pub fn end_phase_me_first(
    state: &DefState,
    combat_status: &CombatStatusTriggers,
    summons: &SummonTriggers,
) -> EndPhaseOutcome {
    let mut next = state.clone();
    let (me_combat_status, me_summons) = tick_side(next.me_mut(), combat_status, summons);
    let (opp_combat_status, opp_summons) = tick_side(next.opp_mut(), combat_status, summons);
    EndPhaseOutcome {
        state: next,
        pending: EndPhasePending {
            me_combat_status,
            me_summons,
            opp_combat_status,
            opp_summons,
        },
    }
}

/// **结束阶段**：敌方先手（敌方出战状态 -> 敌方召唤物 -> 我方出战状态 -> 我方召唤物）。
pub fn end_phase_opp_first(
    state: &DefState,
    combat_status: &CombatStatusTriggers,
    summons: &SummonTriggers,
) -> EndPhaseOutcome {
    let mut next = state.clone();
    let (opp_combat_status, opp_summons) = tick_side(next.opp_mut(), combat_status, summons);
    let (me_combat_status, me_summons) = tick_side(next.me_mut(), combat_status, summons);
    EndPhaseOutcome {
        state: next,
        pending: EndPhasePending {
            me_combat_status,
            me_summons,
            opp_combat_status,
            opp_summons,
        },
    }
}

// ======================================================================
// 共用实现（D35：一份实现，两个入口换先手顺序）
// ======================================================================

/// 一侧的结束阶段：出战状态 tick -> 召唤物 tick（顺序 = 结算顺序第 2 条）。
#[inline]
fn tick_side(
    player: &mut PlayerDefs,
    combat_status: &CombatStatusTriggers,
    summons: &SummonTriggers,
) -> (Vec<CombatStatusPending>, Vec<SummonPending>) {
    // 1. 阵营出战状态：持续回合 -1，耗尽的立刻弃置（复用 rules::ops::lifetime）。
    let ticked = player.combat_status().tick_end_phase_with_triggers(combat_status);
    {
        let parts = player.parts_mut();
        *parts.combat_status = ticked.zone;
    }

    // 2. 召唤物：可用次数 -1，耗尽弃置（复用 rules::ops::lifetime）。
    //    先取实体号列表再逐个 tick：tick 可能把召唤物移出（耗尽），不能边借边改。
    let entities: Vec<EntityId> = player
        .summons()
        .entries()
        .iter()
        .map(|entry| entry.summon)
        .collect();
    let mut zone = player.summons().clone();
    let mut summon_pending: Vec<SummonPending> = Vec::new();
    for summon in entities {
        if let Some(ticked_summon) = zone.tick_with_triggers(summon, summons) {
            zone = ticked_summon.zone;
            summon_pending.extend(ticked_summon.pending);
        }
    }
    {
        let parts = player.parts_mut();
        *parts.summons = zone;
    }

    // 3. 角色状态：解除「饱腹」（规则书·特殊词条 料理第 3 条「回合结束…解除」）。
    //    只按时机清饱腹；其它角色状态原样保留（本流程不做别的角色状态递减）。
    let cleared: Vec<(EntityId, CharacterState)> = player
        .characters()
        .slots()
        .iter()
        .map(|slot| (slot.character, clear_at_round_end(&slot.state)))
        .collect();
    let mut characters = player.characters().clone();
    for (character, state) in cleared {
        if let Some(next) = characters.with_state(character, state) {
            characters = next;
        }
    }
    {
        let parts = player.parts_mut();
        *parts.characters = characters;
    }

    (ticked.pending, summon_pending)
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
    use crate::families::character::zones::combat_status::{
        CombatStatusTiming, CombatStatusZone,
    };
    use crate::families::character::zones::field::CharacterSlot;
    use crate::families::summon::zones::{SummonTiming, SummonZone};
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{
        character, characters, combat_status, entity, n, summons, TEST_STATUS,
    };
    use crate::state::player::PlayerDefs;
    use crate::timeline::{Event, Trigger};
    use gi_genius_invokation_engine::Nat;

    fn noop_combat(_zone: &mut CombatStatusZone, _event: &Event, _owner: EntityId) {}

    fn noop_summon(_zone: &mut SummonZone, _event: &Event, _owner: EntityId) {}

    fn combat_triggers() -> CombatStatusTriggers {
        let mut triggers = CombatStatusTriggers::new();
        for timing in [CombatStatusTiming::DurationTicked, CombatStatusTiming::Depleted] {
            triggers.register(Trigger::new(
                timing,
                entity(1),
                EffectSource::CombatStatus,
                Nat::ZERO,
                noop_combat,
            ));
        }
        triggers
    }

    fn summon_triggers() -> SummonTriggers {
        let mut triggers = SummonTriggers::new();
        for timing in [SummonTiming::Triggered, SummonTiming::Depleted] {
            triggers.register(Trigger::new(
                timing,
                entity(1),
                EffectSource::Summon,
                Nat::ZERO,
                noop_summon,
            ));
        }
        triggers
    }

    fn side(active: u32, status_duration: u32, summon: u32, uses: u32) -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(characters(&[active]))
            .with_combat_status(combat_status(
                n(active),
                &[(&TEST_STATUS, None, Some(n(status_duration)))],
            ))
            .with_summons(summons(&[(summon, uses)]))
    }

    #[test]
    fn end_phase_ticks_durations_and_usages_and_discards_when_depleted() {
        // 我方：持续回合 2 -> 1、可用次数 2 -> 1（都还在）
        // 敌方：持续回合 1 -> 0、可用次数 1 -> 0（都耗尽弃置）
        let before = DefState::from_players(side(201, 2, 301, 2), side(401, 1, 501, 1));
        let outcome = end_phase_me_first(&before, &combat_triggers(), &summon_triggers());
        let after = &outcome.state;

        // 我方：都还在，数值各 -1
        assert_eq!(
            after.me().combat_status().duration(&TEST_STATUS),
            Some(Nat::new(1))
        );
        assert_eq!(
            after.me().summons().entry_of(n(301)).expect("还在").uses,
            Some(Nat::new(1))
        );
        // 派发：我方 1 条 DurationTicked + 1 条 Triggered
        assert_eq!(outcome.pending.me_combat_status.len(), 1);
        assert_eq!(outcome.pending.me_summons.len(), 1);

        // 敌方：耗尽 -> 弃置
        assert!(after.opp().combat_status().is_empty());
        assert!(after.opp().summons().is_empty());
        assert_eq!(outcome.pending.opp_combat_status.len(), 1);
        assert_eq!(outcome.pending.opp_summons.len(), 1);

        // 纯函数：入参不变
        assert_eq!(
            before.me().combat_status().duration(&TEST_STATUS),
            Some(Nat::new(2))
        );
        assert_eq!(before.opp().summons().len(), Nat::new(1));
    }

    #[test]
    fn end_phase_without_statuses_or_summons_dispatches_nothing() {
        let before = DefState::new();
        let outcome = end_phase_me_first(&before, &combat_triggers(), &summon_triggers());
        assert!(outcome.pending.me_combat_status.is_empty());
        assert!(outcome.pending.me_summons.is_empty());
        assert!(outcome.pending.opp_combat_status.is_empty());
        assert!(outcome.pending.opp_summons.is_empty());
        assert_eq!(&outcome.state, &before);
    }

    #[test]
    fn first_player_order_changes_only_the_dispatch_order_not_the_state() {
        // 两类递减都是「自己减自己的」，先手顺序不影响最终局面；本测试钉住这一点，
        // 顺序语义留在 pending 的四个字段里（调用方按先手顺序执行）。
        let before = DefState::from_players(side(201, 2, 301, 2), side(401, 2, 501, 2));
        let me_first = end_phase_me_first(&before, &combat_triggers(), &summon_triggers());
        let opp_first = end_phase_opp_first(&before, &combat_triggers(), &summon_triggers());
        assert_eq!(me_first.state, opp_first.state);
        // 双方都还在场（持续回合 2 -> 1、可用次数 2 -> 1）
        assert_eq!(
            me_first.state.me().combat_status().duration(&TEST_STATUS),
            Some(Nat::new(1))
        );
        assert_eq!(
            me_first.state.opp().summons().entry_of(n(501)).expect("还在").uses,
            Some(Nat::new(1))
        );
    }

    /// 一个「饱腹 + 一个无关角色状态」的角色槽位（验证只清饱腹、不动别的角色状态）。
    fn satiated_slot(value: u32) -> CharacterSlot {
        let mut state = character().with_satiated();
        state.states.add(CHARACTER_STATE_POS, &TEST_STATUS, None);
        CharacterSlot::new(n(value), state)
    }

    fn side_with_satiated(value: u32) -> PlayerDefs {
        let zone = crate::families::character::zones::field::CharacterZone::try_from_slots(vec![
            satiated_slot(value),
        ])
        .expect("合法角色区");
        PlayerDefs::new().with_characters(zone)
    }

    #[test]
    fn end_phase_clears_satiated_on_both_sides_and_keeps_other_character_states() {
        // 规则书·特殊词条 料理第 3 条：「饱腹」状态会在回合结束时解除。
        let before = DefState::from_players(side_with_satiated(201), side_with_satiated(401));
        let outcome = end_phase_me_first(&before, &combat_triggers(), &summon_triggers());

        let after_slots: Vec<&CharacterSlot> = outcome
            .state
            .me()
            .characters()
            .slots()
            .iter()
            .chain(outcome.state.opp().characters().slots().iter())
            .collect();
        assert_eq!(after_slots.len(), 2);
        for slot in after_slots {
            // 双方都解除饱腹
            assert!(!slot.state.satiated());
            // 无关角色状态不受影响（只清饱腹，不清状态列表）
            assert!(slot.state.states.contains(CHARACTER_STATE_POS, &TEST_STATUS));
        }
        // 纯函数：入参仍然饱腹
        assert!(before.me().characters().slots().first().expect("有").state.satiated());
        assert!(before.opp().characters().slots().first().expect("有").state.satiated());
    }
}