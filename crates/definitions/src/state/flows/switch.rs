//! **切换角色流程**：角色区切换 -> 出战状态转移（骰子支付由上层）。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json`）
//!
//! - 规则书·进行行动 战斗行动【切换角色】：「支付 1 个任意类型的元素骰，将一个我方的
//!   『后台角色』切换为『出战角色』。」
//!   → **骰子支付不在本层**（D121 / D133：支付归骰池；本层只做区域与时机）；
//! - 规则书·游戏区域 I(4)：「其他未出战的角色被称为『后台角色』。可以通过『切换角色』行动来将他们
//!   切换为『出战角色』。」I(5)：「如果『出战角色』被击倒，牌手就需要选择一个『后台角色』成为新的
//!   『出战角色』。」
//! - 规则书·游戏区域 II(3)：「出战角色被切换为后台角色时，『出战状态』会转移到新的出战角色上。」
//!
//! ## 步骤（顺序即调用顺序）
//!
//! 1. **角色区切换**：`CharacterZone::switch_active_with_triggers`（派发
//!    `SwitchOut` -> `SwitchIn` -> `ActiveChanged`，只排序不执行 D186）；
//! 2. **出战状态转移**：`CombatStatusZone::transfer_with_triggers`（只改挂载的出战角色，
//!    条目原样保留、派发 `Transferred`）；待派出战状态区为空时**不派发**（没有状态可转移）；
//! 3. 骰子支付：**不在这里**（调用方先付 1 个任意骰再调本流程）。
//!
//! ## 被击倒后的强制补位（规则书·游戏区域 I(5)）
//!
//! 补位没有「切出」这个动作（原出战角色已经离场），所以走
//! `CharacterZone::choose_active_after_defeat_with_triggers`（只派发 `SwitchIn` +
//! `ActiveChanged`），出战状态同样转移。入口：[`choose_active_after_defeat_me`] /
//! [`choose_active_after_defeat_opp`]。
//!
//! ## 待复核
//!
//! - **补位算不算一次「切换角色」行动**：规则书只说「选择一个后台角色成为新的出战角色」，
//!   没说会不会触发「切换时」类效果（区域层已按「只派发 SwitchIn + ActiveChanged」实现，本层沿用）；
//! - **出战状态转移的先后**：本层按「先改角色区、再发出战状态转移」；区域层在转移时派发
//!   `Transferred`。两者与「切换角色时」类效果的相对次序，规则书未规定，待实测。

use crate::families::character::zones::combat_status::{
    CombatStatusPending, CombatStatusTriggers,
};
use crate::families::character::zones::field::{CharacterZonePending, CharacterZoneTriggers};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::EntityId;

/// 一次切换 / 补位的结果：新的一局 + 派发出来的待执行（本层不执行，D186）。
pub struct SwitchOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 `execute_character_zone`）。
    pub characters: Vec<CharacterZonePending>,
    /// 出战状态区派发的待执行（执行时用 `execute_combat_status`）。
    pub combat_status: Vec<CombatStatusPending>,
}

/// **我方切换角色**（把后台角色 `to` 切为出战角色）。
///
/// 失败（返回 None、不改状态）：没有出战角色 / `to` 不在我方角色区 / `to` 已经是出战角色。
/// **不付骰子**（规则书要求支付 1 个任意元素骰，由调用方在调本流程前从骰池扣）。
pub fn switch_me(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        switch_side(next.me_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}

/// **敌方切换角色**（同 [`switch_me`]，作用在 `opp` 上）。
pub fn switch_opp(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        switch_side(next.opp_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}

/// **我方被击倒后的强制补位**（规则书·游戏区域 I(5)）：选一个我方后台角色成为新出战角色。
///
/// 失败（返回 None、不改状态）：`to` 不在我方角色区 / `to` 已经是出战角色。
pub fn choose_active_after_defeat_me(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        choose_after_defeat_side(next.me_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}

/// **敌方被击倒后的强制补位**（同 [`choose_active_after_defeat_me`]）。
pub fn choose_active_after_defeat_opp(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        choose_after_defeat_side(next.opp_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}

// ======================================================================
// 共用实现（D35：一份实现，两个入口各自传「那一侧的数据引用」）
// ======================================================================

/// 切换角色：角色区切换 -> 出战状态转移。
#[inline]
fn switch_side(
    player: &mut PlayerDefs,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<(Vec<CharacterZonePending>, Vec<CombatStatusPending>)> {
    let switched = player.characters().switch_active_with_triggers(to, characters)?;
    {
        let parts = player.parts_mut();
        *parts.characters = switched.zone;
    }
    let combat_pending = transfer_combat_status(player, to, combat_status);
    Some((switched.pending, combat_pending))
}

/// 被击倒后的强制补位：角色区补位 -> 出战状态转移。
#[inline]
fn choose_after_defeat_side(
    player: &mut PlayerDefs,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<(Vec<CharacterZonePending>, Vec<CombatStatusPending>)> {
    let chosen = player
        .characters()
        .choose_active_after_defeat_with_triggers(to, characters)?;
    {
        let parts = player.parts_mut();
        *parts.characters = chosen.zone;
    }
    let combat_pending = transfer_combat_status(player, to, combat_status);
    Some((chosen.pending, combat_pending))
}

/// 出战状态转移给新出战角色（规则书·游戏区域 II(3)）；空区不派发。
#[inline]
fn transfer_combat_status(
    player: &mut PlayerDefs,
    new_active: EntityId,
    triggers: &CombatStatusTriggers,
) -> Vec<CombatStatusPending> {
    if player.combat_status().is_empty() {
        return Vec::new();
    }
    let Some(transferred) = player
        .combat_status()
        .transfer_with_triggers(new_active, triggers)
    else {
        return Vec::new();
    };
    let pending = transferred.pending;
    {
        let parts = player.parts_mut();
        *parts.combat_status = transferred.zone;
    }
    pending
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::zones::combat_status::CombatStatusZone;
    use crate::families::character::zones::field::CharacterZone;
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{characters, combat_status, entity, n, TEST_STATUS};
    use crate::timeline::{Event, Trigger};
    use gi_genius_invokation_engine::Nat;

    fn noop_character(_zone: &mut CharacterZone, _event: &Event, _owner: EntityId) {}

    fn noop_combat(_zone: &mut CombatStatusZone, _event: &Event, _owner: EntityId) {}

    /// 登记「全部角色区时机」的注册表（用来观察派发了几条）。
    fn character_triggers() -> CharacterZoneTriggers {
        let mut triggers = CharacterZoneTriggers::new();
        for timing in [
            crate::families::character::zones::field::CharacterZoneTiming::SwitchOut,
            crate::families::character::zones::field::CharacterZoneTiming::SwitchIn,
            crate::families::character::zones::field::CharacterZoneTiming::ActiveChanged,
        ] {
            triggers.register(Trigger::new(
                timing,
                entity(1),
                EffectSource::CharacterCard,
                Nat::ZERO,
                noop_character,
            ));
        }
        triggers
    }

    /// 登记 Transferred 的注册表。
    fn combat_triggers() -> CombatStatusTriggers {
        let mut triggers = CombatStatusTriggers::new();
        triggers.register(Trigger::new(
            crate::families::character::zones::combat_status::CombatStatusTiming::Transferred,
            entity(1),
            EffectSource::CombatStatus,
            Nat::ZERO,
            noop_combat,
        ));
        triggers
    }

    fn state_with_status() -> DefState {
        let zone = characters(&[201, 202, 203]);
        let player = crate::state::player::PlayerDefs::new()
            .with_characters(zone)
            .with_combat_status(combat_status(n(201), &[(&TEST_STATUS, None, Some(n(2)))]));
        DefState::from_players(player, crate::state::player::PlayerDefs::new())
    }

    #[test]
    fn switch_changes_active_transfers_status_and_keeps_slot_order() {
        let before = state_with_status();
        let outcome = switch_me(&before, n(202), &character_triggers(), &combat_triggers())
            .expect("可以切");
        let after = &outcome.state;
        assert_eq!(after.me().characters().active_entity(), Some(n(202)));
        // 槽位顺序不变（切换不重排）
        assert_eq!(
            after
                .me()
                .characters()
                .slots()
                .iter()
                .map(|slot| slot.character)
                .collect::<Vec<_>>(),
            vec![n(201), n(202), n(203)]
        );
        // 出战状态转移到新出战角色（条目不动）
        assert_eq!(after.me().combat_status().attached_to(), Some(n(202)));
        assert_eq!(after.me().combat_status().len(), Nat::new(1));
        // 派发：角色区 3 条（切出 / 切入 / 出战变化）+ 出战状态 1 条（转移）
        assert_eq!(outcome.characters.len(), 3);
        assert_eq!(outcome.combat_status.len(), 1);
        // 纯函数：入参不变
        assert_eq!(before.me().characters().active_entity(), Some(n(201)));
        assert_eq!(before.me().combat_status().attached_to(), Some(n(201)));
    }

    #[test]
    fn switch_without_combat_status_dispatches_nothing() {
        let zone = characters(&[201, 202]);
        let player = crate::state::player::PlayerDefs::new().with_characters(zone);
        let before = DefState::from_players(player, crate::state::player::PlayerDefs::new());
        let outcome = switch_me(&before, n(202), &character_triggers(), &combat_triggers())
            .expect("可以切");
        assert!(outcome.combat_status.is_empty());
        assert_eq!(outcome.state.me().combat_status().attached_to(), None);
    }

    #[test]
    fn switch_rejects_unknown_or_same_or_no_active() {
        let before = state_with_status();
        // 不在角色区
        assert!(switch_me(&before, n(999), &character_triggers(), &combat_triggers()).is_none());
        // 已经是出战角色
        assert!(switch_me(&before, n(201), &character_triggers(), &combat_triggers()).is_none());
        // 没有出战角色
        let empty = DefState::new();
        assert!(switch_me(&empty, n(201), &character_triggers(), &combat_triggers()).is_none());
    }

    #[test]
    fn opp_switch_does_not_touch_me() {
        let me = state_with_status();
        let opp = crate::state::player::PlayerDefs::new()
            .with_characters(characters(&[301, 302]))
            .with_combat_status(combat_status(n(301), &[(&TEST_STATUS, None, Some(n(1)))]));
        let before = DefState::from_players(me.me().clone(), opp);
        let me_snapshot = before.me().clone();
        let outcome = switch_opp(&before, n(302), &character_triggers(), &combat_triggers())
            .expect("可以切");
        assert_eq!(outcome.state.opp().characters().active_entity(), Some(n(302)));
        assert_eq!(outcome.state.opp().combat_status().attached_to(), Some(n(302)));
        // 我方一个字节都没动
        assert_eq!(outcome.state.me(), &me_snapshot);
    }

    #[test]
    fn choose_active_after_defeat_dispatches_in_and_changed_only() {
        // 先让原出战角色离场（模拟击倒后的角色区），再补位。
        let zone = characters(&[201, 202, 203]);
        let removed = zone
            .remove_with_triggers(n(201), &character_triggers())
            .expect("在区里");
        let player = crate::state::player::PlayerDefs::new()
            .with_characters(removed.zone)
            .with_combat_status(combat_status(n(201), &[(&TEST_STATUS, None, Some(n(1)))]));
        let before = DefState::from_players(player, crate::state::player::PlayerDefs::new());
        let outcome = choose_active_after_defeat_me(
            &before,
            n(202),
            &character_triggers(),
            &combat_triggers(),
        )
        .expect("可以补位");
        assert_eq!(outcome.state.me().characters().active_entity(), Some(n(202)));
        // 补位只派发 切入 + 出战变化（没有切出方）= 2 条
        assert_eq!(outcome.characters.len(), 2);
        // 出战状态同样转移
        assert_eq!(
            outcome.state.me().combat_status().attached_to(),
            Some(n(202))
        );
    }
}