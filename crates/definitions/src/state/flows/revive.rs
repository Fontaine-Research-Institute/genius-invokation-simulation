//! **复苏（复活）流程**：把「击倒区」里的角色放回角色区。
//!
//! ## 规则书依据（唯一权威：规则书 S1–S8 常规模式）
//!
//! - 规则书·特殊词条 料理：「另外，打出具有『复苏』效果的『料理』事件牌后，本回合内就不能
//!   打出其他具有『复苏』效果的『料理』事件牌了。」—— 只规定**次数限制**，没有写复活后
//!   生命值多少、装备 / 状态 / 充能怎么处理；
//! - 规则书·其他规则 击倒：「此时，角色身上所附属的装备和状态都会被弃置，充能会被清空。」
//!   → 这些在**击倒时已经没了**，复苏**不恢复**（本流程也不尝试恢复）；
//! - 生命值：规则书没写 → 由调用方按卡面（第 2 权威）从 `hp` 参数给，
//!   例如卡面「复苏1个角色，并治疗到1点生命值」就传 1；
//! - 「每回合最多复苏 1 个」由调用方先用 `rules::ops::food::can_revive(本回合已复苏数)` 判，
//!   本流程不持有计数（那属于料理 / 饱腹块，L1）。
//!
//! ## 本流程做什么（纯函数：吃 `&DefState`、返回新一局）
//!
//! 1. 在**击倒区**（`PlayerDefs::defeated`）找到这个角色（不在 → 失败、不改状态）；
//! 2. 生命值复位为 `hp`（`CharacterState::hp_set`，钳到上限；击倒时实际上限已回到定义值，D238）；
//! 3. 放回**角色区**（`CharacterZone::add_with_triggers`，派发「进入角色区」；角色区没有出战
//!    角色时按区既有语义自动成为出战角色）；
//! 4. 从击倒区移除。
//!
//! ## 规则书没写清 / TODO（不自行规定）
//!
//! - **复苏后是否要成为出战角色 / 是否派发「出战角色变化」**：规则书只写「出战角色被击倒后
//!   选择一个后台角色成为新出战角色」，没写复苏；现复用角色区既有语义（没有出战角色时自动成为
//!   出战），**TODO 待实测**；
//! - **复苏是否派发「进入角色区」时机**：规则书没写；现按区既有语义派发 `Entered`，**TODO**；
//! - **复苏是否恢复生命上限成长**：击倒时已把「生命上限」成长清掉（D238），复苏不恢复，**TODO**；
//! - **复苏是否解除饱腹 / 本回合复苏计数**：属料理 / 饱腹块（L1），不在本流程。

use crate::families::character::zones::field::{CharacterZonePending, CharacterZoneTriggers};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 一次复苏的结果：新的一局 + 角色区派发的待执行。
#[derive(Clone, Debug)]
pub struct ReviveOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 `execute_character_zone`）。
    pub characters: Vec<CharacterZonePending>,
}

/// **我方角色复苏**：把击倒区里的角色放回我方角色区（规则书·特殊词条 料理「复苏」）。
///
/// - `hp`：复苏后设置的生命值（卡面给；规则书没写具体值）；
/// - 失败（返回 `None`、不改状态）：该角色不在我方击倒区 / 角色区已满。
pub fn revive_me(
    state: &DefState,
    character: EntityId,
    hp: Nat,
    characters: &CharacterZoneTriggers,
) -> Option<ReviveOutcome> {
    let mut next = state.clone();
    let pending = revive_side(next.me_mut(), character, hp, characters)?;
    Some(ReviveOutcome {
        state: next,
        characters: pending,
    })
}

/// **敌方角色复苏**（同 [`revive_me`]，作用在 `opp` 上；D35 共用实现）。
pub fn revive_opp(
    state: &DefState,
    character: EntityId,
    hp: Nat,
    characters: &CharacterZoneTriggers,
) -> Option<ReviveOutcome> {
    let mut next = state.clone();
    let pending = revive_side(next.opp_mut(), character, hp, characters)?;
    Some(ReviveOutcome {
        state: next,
        characters: pending,
    })
}

/// 一侧的复苏：击倒区取角色 -> 生命复位 -> 放回角色区 -> 从击倒区移除（D35）。
#[inline]
fn revive_side(
    player: &mut PlayerDefs,
    character: EntityId,
    hp: Nat,
    characters: &CharacterZoneTriggers,
) -> Option<Vec<CharacterZonePending>> {
    // 1. 击倒区里必须有这个角色（先克隆出来；失败时状态一个字节都不动）。
    let defeated = player.defeated_character(character)?.clone();
    // 2. 生命值复位（装备 / 状态 / 充能在击倒时已弃置 / 清空，复苏不恢复）。
    let revived = defeated.state.hp_set(hp);
    // 3. 放回角色区（派发 Entered；角色区没有出战角色时按区既有语义自动成为出战）。
    let added = player
        .characters()
        .add_with_triggers(character, revived, characters)?;
    // 4. 写回角色区 + 从击倒区移除。
    {
        let parts = player.parts_mut();
        *parts.characters = added.zone;
        parts.defeated.retain(|slot| slot.character != character);
    }
    Some(added.pending)
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::growth::GrowthList;
    use crate::families::character::zones::equipment::EquipmentTriggers;
    use crate::rules::ops::equip::SlotKind;
    use crate::state::flows::defeat::{defeat_me, defeat_opp, DefeatHpReset};
    use crate::state::terminal::{terminal, Outcome};
    use crate::state::testing::{characters, equipment, n};

    fn started_players(me: PlayerDefs, opp: PlayerDefs) -> DefState {
        DefState::from_players(me, opp).with_round(n(3))
    }

    fn both_sides_have_one_character() -> DefState {
        started_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new().with_characters(characters(&[401])),
        )
    }

    /// 把我方唯一角色击倒：角色区空、击倒区 1 人、终局判负。
    fn defeat_me_only_character(state: &DefState) -> DefState {
        let growth = GrowthList::new();
        let hp = DefeatHpReset::new(n(10), &growth);
        defeat_me(
            state,
            n(201),
            &hp,
            &CharacterZoneTriggers::new(),
            &EquipmentTriggers::new(),
        )
        .expect("角色在我方角色区")
        .state
    }

    #[test]
    fn revive_returns_the_character_and_terminal_stops_judging_loss() {
        let before = both_sides_have_one_character();
        let defeated = defeat_me_only_character(&before);
        // 击倒后：角色区为空 -> 判负（即使击倒区还记着这个角色）
        assert_eq!(defeated.me().characters().len(), Nat::ZERO);
        assert_eq!(defeated.me().defeated().len(), 1);
        assert_eq!(terminal(&defeated), Outcome::OppWins);

        // 复苏：回到角色区、HP 复位、击倒区清空
        let revived =
            revive_me(&defeated, n(201), n(1), &CharacterZoneTriggers::new()).expect("在击倒区");
        assert_eq!(revived.state.me().characters().len(), Nat::new(1));
        assert!(revived.state.me().defeated().is_empty());
        let slot = revived
            .state
            .me()
            .characters()
            .slot_of(n(201))
            .expect("回到角色区");
        assert_eq!(slot.state.hp(), n(1));
        // 角色区原本没有出战角色 -> 按区既有语义自动成为出战（TODO：规则书没写）
        assert_eq!(revived.state.me().characters().active_entity(), Some(n(201)));
        // 角色区非空 -> 终局不再判负
        assert_eq!(terminal(&revived.state), Outcome::Ongoing);
        // 纯函数：入参不变（击倒后的局面还在）
        assert_eq!(defeated.me().characters().len(), Nat::ZERO);
        assert_eq!(defeated.me().defeated().len(), 1);
    }

    #[test]
    fn revive_rejects_a_character_that_is_not_in_the_defeat_zone() {
        let state = both_sides_have_one_character();
        // 在角色区（没被击倒）-> 不能复苏
        assert!(revive_me(&state, n(201), n(1), &CharacterZoneTriggers::new()).is_none());
        // 击倒区没有这个未知角色
        assert!(revive_me(&state, n(999), n(1), &CharacterZoneTriggers::new()).is_none());
        // 失败不改状态
        assert_eq!(state.me().characters().len(), Nat::new(1));
        assert!(state.me().defeated().is_empty());
    }

    #[test]
    fn revive_does_not_restore_discarded_equipment_or_cleared_state() {
        // 我方 201 带一件圣遗物；被击倒时装备被弃置、状态 / 充能清空（规则书·其他规则 击倒）。
        let me = PlayerDefs::new()
            .with_characters(characters(&[201]))
            .with_equipment_zone(equipment(n(201), &[(11, SlotKind::Artifact, 312001)]));
        let before = started_players(me, PlayerDefs::new().with_characters(characters(&[401])));
        let defeated = defeat_me_only_character(&before);
        assert!(defeated.me().equipment_of(n(201)).is_none());

        let revived =
            revive_me(&defeated, n(201), n(2), &CharacterZoneTriggers::new()).expect("在击倒区");
        let slot = revived
            .state
            .me()
            .characters()
            .slot_of(n(201))
            .expect("回到角色区");
        // 击倒时弃置 / 清空的内容复苏不恢复（规则书没有「复活恢复装备 / 状态」）
        assert!(slot.state.states.is_empty());
        assert!(slot.state.attachment.is_empty());
        assert_eq!(slot.state.energy(), Nat::ZERO);
        assert!(revived.state.me().equipment_of(n(201)).is_none());
        // 生命值 = 参数；生命上限 = 定义值（击倒时已重置，D238）
        assert_eq!(slot.state.hp(), n(2));
        assert_eq!(slot.state.max_hp(), n(10));
    }

    #[test]
    fn revive_opp_does_not_touch_me() {
        let me = PlayerDefs::new().with_characters(characters(&[201]));
        let opp = PlayerDefs::new().with_characters(characters(&[401]));
        let before = started_players(me, opp);
        let growth = GrowthList::new();
        let hp = DefeatHpReset::new(n(10), &growth);
        let defeated = defeat_opp(
            &before,
            n(401),
            &hp,
            &CharacterZoneTriggers::new(),
            &EquipmentTriggers::new(),
        )
        .expect("角色在敌方角色区")
        .state;
        let me_snapshot = defeated.me().clone();

        let revived =
            revive_opp(&defeated, n(401), n(3), &CharacterZoneTriggers::new()).expect("在击倒区");
        assert_eq!(revived.state.opp().characters().len(), Nat::new(1));
        assert_eq!(
            revived
                .state
                .opp()
                .characters()
                .slot_of(n(401))
                .expect("回到角色区")
                .state
                .hp(),
            n(3)
        );
        // 我方一个字节都没动
        assert_eq!(revived.state.me(), &me_snapshot);
    }
}
