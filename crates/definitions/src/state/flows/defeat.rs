//! **击倒清理流程**（D193：击倒后的清理归**角色块**，不在触发层做）。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json`）
//!
//! - 规则书·其他规则 击倒：「当角色的生命值被降为 0，则在技能或效果结算完成后，角色就会被『击倒』。
//!   此时，角色身上所附属的装备和状态都会被弃置，充能会被清空。」
//!   English: "…all Equipment and Statuses attached to it will be discarded, and its Energy will be cleared."
//! - 规则书·游戏区域 I(5)：「如果『出战角色』被击倒，牌手就需要选择一个『后台角色』成为新的『出战角色』。」
//!   → 补位是**另一个动作**，走 `flows::switch::choose_active_after_defeat_me/opp`；
//! - D193（Lead 拍板）：「击倒后弃置都属于角色」—— 清理放角色块，触发层只负责「中断 / 恢复」。
//! - **D238**（用户口径）：「最大生命值上限（实际可变那个），角色被击倒后变回定义值」——
//!   生命上限有**定义值**（`CharacterDef::max_hp`，不可变）与**实际值**（定义值 + 成长增益，可变）；
//!   击倒时把**实际值**重置回**定义值** = 清掉全部「生命上限」成长条目
//!   （`GrowthList::reset_max_hp`），其它属性成长保留。
//! - 规则书·特殊词条 料理：「『饱腹』状态会在回合结束或角色被击倒时解除。」→ 击倒清理时对
//!   该角色显式调 `families::character::satiated::clear_on_defeat`（回合结束的解除在
//!   `state::flows::end_phase`）；
//! - D194 / D195：击倒会**中断**当前结算；免于击倒 + 治疗后从插入点继续。
//!   那属于 `timeline` 的帧机制（`Resolution::insert` / `discard`），**不在本流程**
//!   —— 本流程只做「确实被击倒之后」的状态清理。
//!
//! ## 步骤（顺序即语义，见 D193 与规则书·其他规则 击倒）
//!
//! 1. **移出角色区**：`CharacterZone::remove_with_triggers`（派发 `Left`；出战角色离场时
//!    出战指针置空，等补位）；
//! 2. **弃置装备**：把这个角色的装备栏逐个 `detach_with_triggers`（派发 `Discarded`），
//!    然后把整个装备栏随角色一起移除（角色不在场，装备栏不存在）；
//! 3. **弃置状态 + 清空充能**：这个角色的运行期状态（`CharacterState`）随槽位一起离开角色区，
//!    本流程给出**清理后**的那份状态：状态列表清空、元素附着清空、充能归零；
//!    「饱腹」按时机显式解除（`clear_on_defeat`，规则书·特殊词条 料理）——
//!    放进 [`DefeatOutcome::cleared`]，同时把清掉多少充能记在
//!    [`DefeatOutcome::cleared_energy`]（供统计 / 断言 / 上层记录）；
//! 4. **实际最大生命值上限回到定义值**（D238）：先把这个角色的「生命上限」成长条目清掉
//!    （`GrowthList::reset_max_hp`，增加 / 减少都清），再按 `actual_max_hp(定义值)` 重算实际上限；
//!    重置后的成长列表放进 [`DefeatOutcome::reset_growth`]，由持有成长的那一层写回。
//!    当前生命按既有不变式处理（`hp <= max_hp`；击倒时通常已是 0，这里只钳不补）；
//! 5. **记入击倒区**（内部记录）：把「角色身份 + 清理后的运行期状态」追加到
//!    [`PlayerDefs::defeated`]，供复苏（`state::flows::revive`）把角色放回角色区。
//!    **规则书没有可见的「击倒区」**（见「待复核」），本记录不参与结算、不派发时机。
//!
//! ## 待复核
//!
//! - **出战状态**（`CombatStatusZone`）**不在**本流程处置：英文口径是「Team Combat Status」
//!   （阵营出战状态），本层按「它属于阵营、跟着出战角色走」处理 —— 角色被击倒时**不弃置**，
//!   等补位时由 `choose_active_after_defeat_*` 转移给新出战角色。规则书 II(3) 只说切换时转移，
//!   没说击倒时弃置，**待实测**；
//! - 弃置装备的派发顺序 = 装备栏里的挂载先后（规则书未规定同角色多装备的弃置顺序）；
//! - 角色牌本身离开角色区后去哪里（击倒区 / 弃牌堆）：规则书只说「角色就会被击倒」，本层**不建弃牌堆**；
//!   为了支持「复苏」，本层在 `PlayerDefs` 里留一个**内部记录** `defeated`（身份 + 清理后状态），
//!   它**不是**规则书里的可见区域、不参与结算 / 不派发时机；规则书 / 实测若给出正式「击倒区」规则，按它改（TODO）；
//! - **只重置生命上限**：D238 只提「最大生命值上限」，所以充能上限的成长（`MaxEnergy`）**不清**；
//!   规则书 / 用户口径未说要一起清，**待复核**；
//! - **待接入：成长列表的来源**。成长条目目前由装备 / 天赋 / 场地各自持有（`growth.rs` 的
//!   「由持有它的东西表达」），还没有「逐角色成长」被聚合进状态层；所以本流程要求调用方把
//!   该角色当前的 `GrowthList` 传进来（[DefeatHpReset]）。等角色区 / 状态层持有成长后，
//!   这里改成从状态里读，入口签名不变即可。
//!
//! ## 全灭判定不在这里（归 `state::terminal`）
//!
//! 规则书·其他规则 击倒只规定「击倒后清理」；「击倒对方牌手的所有角色牌即获胜」（胜利条件）
//! 与「第15回合开始时仍未分胜负则双方都输」（对局回合上限）是**结算完成后的终局判定**，
//! 归纯函数 [`crate::state::terminal::terminal`]，本流程只负责把角色移出角色区。
//!
//! 顺序（`结算顺序.md` §三「生命值降到 0」）：中断 -> 免于击倒判定 -> 治疗后从 cursor
//! 继续；确实被击倒 -> 本流程清理 -> **结算完成后**才判终局。因此：
//! - **免于击倒窗口**里角色仍留在角色区，终局判定不会把他算成「已全灭」（不判负）；
//! - HP 归 0 但本流程还没跑时同样不判负（规则书·其他规则 击倒：结算完成后才被击倒）。

use crate::families::action_card::status::StateList;
use crate::families::character::attachment::Attachment;
use crate::families::character::character::CharacterState;
use crate::families::character::growth::GrowthList;
use crate::families::character::satiated::clear_on_defeat;
use crate::families::character::zones::equipment::{EquipmentPending, EquipmentTriggers};
use crate::families::character::zones::field::{
    CharacterSlot, CharacterZonePending, CharacterZoneTriggers,
};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 击倒时把「实际最大生命值上限」重置回定义值所需的资料（D238）。
///
/// 生命上限有两个值（见 `families/character/growth.rs`）：
/// - **定义值**：`CharacterDef::max_hp`，不可变 —— [`DefeatHpReset::definition`]；
/// - **实际值**：定义值 + 成长增益，可变 —— 就是 `CharacterState::max_hp`。
///
/// 重置 = 清掉该角色的「生命上限」成长条目（[`GrowthList::reset_max_hp`]），
/// 再按 `actual_max_hp(定义值)` 重算实际值。**定义值本身不写进成长列表**（growth.rs 口径）。
///
/// 待接入：逐角色的成长目前还没有聚合进状态层（成长条目由装备 / 天赋 / 场地各自持有），
/// 所以这里由调用方把该角色当前的 `GrowthList` 传进来。
#[derive(Clone, Copy, Debug)]
pub struct DefeatHpReset<'a> {
    /// 卡牌定义的生命上限（`CharacterDef::max_hp`；D238「定义值」）。
    pub definition: Nat,
    /// 这个角色的成长条目（D238「实际值」= 定义值 + 生命上限成长）。
    pub growth: &'a GrowthList,
}

impl<'a> DefeatHpReset<'a> {
    /// 用「定义值 + 该角色成长」建重置资料。
    pub const fn new(definition: Nat, growth: &'a GrowthList) -> DefeatHpReset<'a> {
        DefeatHpReset { definition, growth }
    }
}

/// 一次击倒清理的结果：新的一局 + 派发 + 清理后的角色状态。
pub struct DefeatOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 `execute_character_zone`）。
    pub characters: Vec<CharacterZonePending>,
    /// 装备栏派发的待执行（执行时用 `execute_equipment`）。
    pub equipment: Vec<EquipmentPending>,
    /// 清理前的充能（= 被清空的量）。
    pub cleared_energy: Nat,
    /// 清理后的角色运行期状态（已离场：状态列表空、元素附着空、充能 0、
    /// **实际最大生命值上限已回到定义值**，D238）。
    pub cleared: CharacterState,
    /// 重置后的成长条目（**生命上限条目已清空**、其它属性成长保留，D238）：
    /// 由持有成长的那一层写回（本层不持有成长，见 [`DefeatHpReset`]）。
    pub reset_growth: GrowthList,
}

/// **我方角色被击倒后的清理**（规则书·其他规则 击倒）。
///
/// `hp`：这个角色的**定义值 + 成长**（D238：击倒后实际上限回到定义值）。
///
/// 失败（返回 None、不改状态）：这个角色不在我方角色区。
pub fn defeat_me(
    state: &DefState,
    character: EntityId,
    hp: &DefeatHpReset,
    characters: &CharacterZoneTriggers,
    equipment: &EquipmentTriggers,
) -> Option<DefeatOutcome> {
    let mut next = state.clone();
    let cleanup = defeat_side(next.me_mut(), character, hp, characters, equipment)?;
    Some(DefeatOutcome {
        state: next,
        characters: cleanup.characters,
        equipment: cleanup.equipment,
        cleared_energy: cleanup.cleared_energy,
        cleared: cleanup.cleared,
        reset_growth: cleanup.reset_growth,
    })
}

/// **敌方角色被击倒后的清理**（同 [`defeat_me`]，作用在 `opp` 上）。
pub fn defeat_opp(
    state: &DefState,
    character: EntityId,
    hp: &DefeatHpReset,
    characters: &CharacterZoneTriggers,
    equipment: &EquipmentTriggers,
) -> Option<DefeatOutcome> {
    let mut next = state.clone();
    let cleanup = defeat_side(next.opp_mut(), character, hp, characters, equipment)?;
    Some(DefeatOutcome {
        state: next,
        characters: cleanup.characters,
        equipment: cleanup.equipment,
        cleared_energy: cleanup.cleared_energy,
        cleared: cleanup.cleared,
        reset_growth: cleanup.reset_growth,
    })
}

// ======================================================================
// 共用实现（D35）
// ======================================================================

/// 一侧击倒清理的结果（内部用；对外是 [`DefeatOutcome`]）。
struct SideCleanup {
    /// 角色区派发的待执行。
    characters: Vec<CharacterZonePending>,
    /// 装备栏派发的待执行。
    equipment: Vec<EquipmentPending>,
    /// 清理前的充能。
    cleared_energy: Nat,
    /// 清理后的角色运行期状态。
    cleared: CharacterState,
    /// 重置后的成长条目（生命上限条目已清空）。
    reset_growth: GrowthList,
}

/// 击倒清理：移出角色区 -> 弃置装备 -> 状态 / 充能归零 -> 实际上限回到定义值（D238）。
#[inline]
fn defeat_side(
    player: &mut PlayerDefs,
    character: EntityId,
    hp: &DefeatHpReset,
    characters: &CharacterZoneTriggers,
    equipment: &EquipmentTriggers,
) -> Option<SideCleanup> {
    // 1. 先读槽位（移出之后就没了）：充能量、清理后的状态、重置后的成长都在这里定下来。
    let slot = player.characters().slot_of(character)?.clone();
    let cleared_energy = slot.state.energy();
    let (cleared, reset_growth) = clean_defeated(&slot.state, hp);

    // 2. 移出角色区（派发 Left；出战角色离场 -> 出战指针置空）。
    let removed = player
        .characters()
        .remove_with_triggers(character, characters)?;

    // 3. 弃置装备（逐件派发 Discarded），然后把整个装备栏随角色一起移除。
    let mut equipment_pending: Vec<EquipmentPending> = Vec::new();
    if let Some(zone) = player.equipment_of(character) {
        let equips: Vec<EntityId> = zone.entries().iter().map(|entry| entry.equip).collect();
        let mut current = zone.clone();
        for equip in equips {
            if let Some(detached) = current.detach_with_triggers(equip, equipment) {
                current = detached.zone;
                equipment_pending.extend(detached.pending);
            }
        }
    }

    {
        let parts = player.parts_mut();
        *parts.characters = removed.zone;
        parts.equipment.retain(|zone| zone.owner() != character);
        // 5. 记入击倒区（内部记录）：身份 + 清理后的状态，供复苏放回角色区。
        parts
            .defeated
            .push(CharacterSlot::new(character, cleared.clone()));
    }
    Some(SideCleanup {
        characters: removed.pending,
        equipment: equipment_pending,
        cleared_energy,
        cleared,
        reset_growth,
    })
}

/// 击倒后的角色运行期状态 + 重置后的成长：
///
/// - 状态列表清空、元素附着清空、充能归零（规则书·其他规则 击倒）；
/// - **实际最大生命值上限回到定义值**（D238）：`GrowthList::reset_max_hp()` 清掉全部「生命上限」
///   成长条目（增加 / 减少都清），再用 `actual_max_hp(定义值)` 重算；其它属性成长保留；
/// - 当前生命按既有不变式处理：`hp <= max_hp`（击倒时通常已是 0，这里只钳不补）。
fn clean_defeated(state: &CharacterState, hp: &DefeatHpReset) -> (CharacterState, GrowthList) {
    let reset_growth = hp.growth.reset_max_hp();
    let mut next = state.clone();
    // 规则书·特殊词条 料理第 3 条：角色被击倒时解除「饱腹」。
    // 击倒本来也会清空状态列表（规则书·其他规则 击倒），这里仍**显式**走按时机命名的解除，
    // 让「饱腹解除」有自己的接线点，而不是依赖「清空列表」的顺带结果。
    next = clear_on_defeat(&next);
    next.states = StateList::new();
    next.attachment = Attachment::none();
    next.max_hp = reset_growth.actual_max_hp(hp.definition);
    if next.hp > next.max_hp {
        next.hp = next.max_hp;
    }
    (next.energy_set(Nat::ZERO), reset_growth)
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
    use crate::families::character::growth::{Growth, GrowthAttribute};
    use crate::families::character::zones::equipment::{EquipmentTiming, EquipmentZone};
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone, CharacterZoneTiming};
    use crate::rules::ops::equip::SlotKind;
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{
        equipment as equipment_fixture, entity, n, slot, slot_with, TEST_STATUS,
    };
    use crate::timeline::{Event, Trigger};

    fn noop_character(_zone: &mut CharacterZone, _event: &Event, _owner: EntityId) {}

    fn noop_equipment(_zone: &mut EquipmentZone, _event: &Event, _owner: EntityId) {}

    fn character_triggers() -> CharacterZoneTriggers {
        let mut triggers = CharacterZoneTriggers::new();
        triggers.register(Trigger::new(
            CharacterZoneTiming::Left,
            entity(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_character,
        ));
        triggers
    }

    fn equipment_triggers() -> EquipmentTriggers {
        let mut triggers = EquipmentTriggers::new();
        triggers.register(Trigger::new(
            EquipmentTiming::Discarded,
            entity(1),
            EffectSource::CharacterEquipmentOrStatus,
            Nat::ZERO,
            noop_equipment,
        ));
        triggers
    }

    /// 没有成长的角色（定义值 = 上限 = 10，与 `testing::character()` 一致）。
    fn no_growth() -> GrowthList {
        GrowthList::new()
    }

    fn state_with_equipment() -> DefState {
        let characters_zone = CharacterZone::try_from_slots(vec![
            slot_with(201, n(1)),
            slot(202),
        ])
        .expect("合法角色区");
        let player = crate::state::player::PlayerDefs::new()
            .with_characters(characters_zone)
            .with_equipment_zone(equipment_fixture(
                n(201),
                &[(11, SlotKind::Artifact, 312001), (12, SlotKind::Talent, 313001)],
            ))
            .with_equipment_zone(equipment_fixture(n(202), &[(21, SlotKind::Artifact, 312002)]));
        DefState::from_players(player, crate::state::player::PlayerDefs::new())
    }

    #[test]
    fn defeat_removes_character_discards_equipment_and_clears_energy() {
        let before = state_with_equipment();
        let growth = no_growth();
        let hp = DefeatHpReset::new(n(10), &growth);
        let outcome =
            defeat_me(&before, n(201), &hp, &character_triggers(), &equipment_triggers())
                .expect("在区里");
        let after = &outcome.state;

        // 移出角色区（出战角色离场 -> 出战指针置空，等补位）
        assert_eq!(after.me().characters().len(), Nat::new(1));
        assert_eq!(after.me().characters().active_entity(), None);
        assert!(after.me().characters().slot_of(n(201)).is_none());
        // 派发：Left 1 条 + Discarded 2 条（这一角色两件装备）
        assert_eq!(outcome.characters.len(), 1);
        assert_eq!(outcome.equipment.len(), 2);
        // 装备栏随角色移除；别人的装备栏不动
        assert!(after.me().equipment_of(n(201)).is_none());
        assert_eq!(
            after.me().equipment_of(n(202)).expect("202 还在").len(),
            Nat::new(1)
        );
        // 记入击倒区（内部记录，供复苏）：角色身份 + 清理后状态
        assert_eq!(after.me().defeated().len(), 1);
        assert_eq!(
            after
                .me()
                .defeated_character(n(201))
                .expect("在击倒区")
                .state
                .max_hp(),
            n(10)
        );
        // 清空充能 + 弃置状态 / 附着
        assert_eq!(outcome.cleared_energy, Nat::new(1));
        assert_eq!(outcome.cleared.energy(), Nat::ZERO);
        assert!(outcome.cleared.states.is_empty());
        assert!(outcome.cleared.attachment.is_empty());
        // 纯函数：入参不变
        assert_eq!(before.me().characters().len(), Nat::new(2));
    }

    #[test]
    fn defeat_rejects_character_not_in_zone() {
        let before = state_with_equipment();
        let growth = no_growth();
        let hp = DefeatHpReset::new(n(10), &growth);
        assert!(
            defeat_me(&before, n(999), &hp, &character_triggers(), &equipment_triggers()).is_none()
        );
    }

    #[test]
    fn defeat_opp_does_not_touch_me() {
        let opp_zone = CharacterZone::try_from_slots(vec![CharacterSlot::new(n(301), crate::state::testing::character())])
            .expect("合法角色区");
        let opp = crate::state::player::PlayerDefs::new()
            .with_characters(opp_zone)
            .with_equipment_zone(equipment_fixture(
                n(301),
                &[(31, SlotKind::Artifact, 312003)],
            ));
        let me = crate::state::player::PlayerDefs::new();
        let before = DefState::from_players(me, opp);
        let me_snapshot = before.me().clone();
        let growth = no_growth();
        let hp = DefeatHpReset::new(n(10), &growth);
        let outcome =
            defeat_opp(&before, n(301), &hp, &character_triggers(), &equipment_triggers())
                .expect("在区里");
        assert_eq!(outcome.state.opp().characters().len(), Nat::ZERO);
        assert_eq!(outcome.equipment.len(), 1);
        // 我方一个字节都没动
        assert_eq!(outcome.state.me(), &me_snapshot);
    }

    #[test]
    fn defeat_resets_actual_max_hp_to_definition_and_keeps_other_growth() {
        // 成长：生命上限 +5、充能上限 -1、攻击 +2（D238：击倒只清生命上限那一条）。
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(1)))
            .with(Growth::increase(GrowthAttribute::Attack, n(2)));
        let definition = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        // 被击倒的角色：当前生命 0（上限已被成长提到 15）。
        let grown = growth.apply_to_character(&definition).hp_sub(n(10));
        assert_eq!(grown.max_hp(), n(15));
        assert_eq!(grown.hp(), Nat::ZERO);
        assert_eq!(grown.max_energy(), n(1));

        let me_zone = CharacterZone::try_from_slots(vec![
            CharacterSlot::new(n(201), grown),
            slot(202),
        ])
        .expect("合法角色区");
        let opp_zone = CharacterZone::try_from_slots(vec![CharacterSlot::new(
            n(301),
            growth.apply_to_character(&definition).hp_sub(n(10)),
        )])
        .expect("合法角色区");
        let before = DefState::from_players(
            crate::state::player::PlayerDefs::new().with_characters(me_zone),
            crate::state::player::PlayerDefs::new().with_characters(opp_zone),
        );

        let hp = DefeatHpReset::new(n(10), &growth);
        let outcome =
            defeat_me(&before, n(201), &hp, &character_triggers(), &equipment_triggers())
                .expect("在区里");

        // D238：实际最大生命值上限回到定义值
        assert_eq!(outcome.cleared.max_hp(), n(10));
        // 当前生命按既有不变式处理（击倒时已是 0）
        assert_eq!(outcome.cleared.hp(), Nat::ZERO);
        // 重置后的成长：生命上限条目清空（增 / 减两侧都为 0），其它属性保留
        assert_eq!(
            outcome.reset_growth.totals(GrowthAttribute::MaxHp),
            crate::families::character::growth::GrowthTotals::ZERO
        );
        assert_eq!(outcome.reset_growth.totals(GrowthAttribute::Attack).gain, n(2));
        assert_eq!(
            outcome.reset_growth.totals(GrowthAttribute::MaxEnergy).loss,
            n(1)
        );
        // 充能上限的成长不清（D238 只提生命上限，待复核）
        assert_eq!(outcome.cleared.max_energy(), n(1));
        // 对方不受影响：敌方角色的实际上限仍是 15
        let opp_character = outcome.state.opp().characters().active_entity().expect("有角色");
        let opp_slot = outcome
            .state
            .opp()
            .characters()
            .slot_of(opp_character)
            .expect("在区里");
        assert_eq!(opp_slot.state.max_hp(), n(15));
        // 纯函数：入参不变
        assert_eq!(before.me().characters().len(), Nat::new(2));
    }

    #[test]
    fn defeat_clamps_current_hp_to_the_reset_max_hp() {
        // 成长后上限 15、当前生命 12；定义值 10 -> 击倒重置后上限 10、生命钳到 10。
        let growth = GrowthList::new().with(Growth::increase(GrowthAttribute::MaxHp, n(5)));
        let definition = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let grown = growth.apply_to_character(&definition).hp_add(n(2));
        assert_eq!(grown.max_hp(), n(15));
        assert_eq!(grown.hp(), n(12));

        let zone = CharacterZone::try_from_slots(vec![CharacterSlot::new(n(201), grown)])
            .expect("合法角色区");
        let before = DefState::from_players(
            crate::state::player::PlayerDefs::new().with_characters(zone),
            crate::state::player::PlayerDefs::new(),
        );
        let hp = DefeatHpReset::new(n(10), &growth);
        let outcome =
            defeat_me(&before, n(201), &hp, &character_triggers(), &equipment_triggers())
                .expect("在区里");
        assert_eq!(outcome.cleared.max_hp(), n(10));
        assert_eq!(outcome.cleared.hp(), n(10));
    }

    #[test]
    fn defeat_clears_satiated_of_the_defeated_character_and_keeps_others() {
        // 201：饱腹 + 一个无关角色状态；202：带一个无关角色状态。
        let mut satiated = CharacterState::new(n(10), n(10), Nat::ZERO, n(2)).with_satiated();
        satiated.states.add(CHARACTER_STATE_POS, &TEST_STATUS, None);
        let mut other = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        other.states.add(CHARACTER_STATE_POS, &TEST_STATUS, None);
        let zone = CharacterZone::try_from_slots(vec![
            CharacterSlot::new(n(201), satiated),
            CharacterSlot::new(n(202), other),
        ])
        .expect("合法角色区");
        let before = DefState::from_players(
            crate::state::player::PlayerDefs::new().with_characters(zone),
            crate::state::player::PlayerDefs::new(),
        );
        let growth = no_growth();
        let hp = DefeatHpReset::new(n(10), &growth);
        let outcome =
            defeat_me(&before, n(201), &hp, &character_triggers(), &equipment_triggers())
                .expect("在区里");

        // 规则书·特殊词条 料理第 3 条：角色被击倒时解除「饱腹」。
        assert!(!outcome.cleared.satiated());
        // 击倒区记录的是解除后的状态（复苏回来也不会带着饱腹）
        assert!(!outcome
            .state
            .me()
            .defeated_character(n(201))
            .expect("在击倒区")
            .state
            .satiated());
        // 另一个角色不受影响：它的无关角色状态还在
        let survivor = outcome.state.me().characters().slot_of(n(202)).expect("还在");
        assert!(survivor
            .state
            .states
            .contains(CHARACTER_STATE_POS, &TEST_STATUS));
        // 纯函数：入参 201 仍然饱腹
        assert!(before
            .me()
            .characters()
            .slot_of(n(201))
            .expect("在区里")
            .state
            .satiated());
    }
}
