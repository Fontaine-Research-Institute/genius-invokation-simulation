//! 状态层测试夹具（`cfg(test)`；不进发布构建）。
//!
//! 只放**搭局面**的小工具：数值 / 实体号、测试用状态身份、造角色区 / 牌库 / 手牌 / 装备栏 / 出战状态 /
//! 召唤物区。**不测任何规则**（规则测试在各流程的 `tests` 里）。

use crate::base::CardId;
use crate::families::action_card::status::{StateForm, StateIdentity, StateText};
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::pile::PileState;
use crate::families::action_card::zones::CardInstance;
use crate::families::character::character::CharacterState;
use crate::families::character::zones::combat_status::CombatStatusZone;
use crate::families::character::zones::equipment::{
    EquipmentEntry, EquipmentTriggers, EquipmentZone,
};
use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
use crate::families::summon::zones::{SummonEntry, SummonZone};
use crate::generated::Localized;
use crate::rules::ops::equip::{CharacterEquip, SlotKind};
use gi_genius_invokation_engine::{EntityId, Nat};

/// `Nat` 简写。
pub(crate) fn n(value: u32) -> Nat {
    Nat::new(value)
}

/// 实体号简写。
pub(crate) fn entity(value: u32) -> EntityId {
    Nat::new(value)
}

/// 测试用卡牌 A（**不是真实卡牌数据**）。
pub(crate) const CARD_A: CardId = CardId::new(300001);

/// 布尔类测试状态身份（`StateIdentity` 的实现样本；**不是真实状态**）。
#[derive(Debug)]
pub(crate) struct TestStatus;

impl StateIdentity for TestStatus {
    fn text(&self) -> &'static StateText {
        static TEXT: StateText = StateText {
            name: Localized::EMPTY,
            description: Localized::EMPTY,
        };
        &TEXT
    }

    fn form(&self) -> StateForm {
        StateForm::Boolean
    }
}

/// `TestStatus` 的静态实例（状态身份 = 它的地址）。
pub(crate) static TEST_STATUS: TestStatus = TestStatus;

/// 一个生命 10 / 生命上限 10 / 充能 0 / 充能上限 2 的角色状态。
pub(crate) fn character() -> CharacterState {
    character_with(Nat::ZERO)
}

/// 指定当前充能的角色状态。
pub(crate) fn character_with(energy: Nat) -> CharacterState {
    CharacterState::new(n(10), n(10), energy, n(2))
}

/// 一个角色槽位（默认充能 0）。
pub(crate) fn slot(value: u32) -> CharacterSlot {
    CharacterSlot::new(entity(value), character())
}

/// 指定充能的角色槽位。
pub(crate) fn slot_with(value: u32, energy: Nat) -> CharacterSlot {
    CharacterSlot::new(entity(value), character_with(energy))
}

/// 用一组角色实体号建角色区（第一张成为默认出战角色，见 `try_from_slots`）。
pub(crate) fn characters(values: &[u32]) -> CharacterZone {
    let slots: Vec<CharacterSlot> = values.iter().map(|value| slot(*value)).collect();
    CharacterZone::try_from_slots(slots).expect("测试角色区合法（不超过 3 个、实体不重复）")
}

/// 用一组实体号建固定段牌库（每张都是 `CARD_A`）。
pub(crate) fn pile(values: &[u32]) -> PileState {
    PileState::from_fixed(
        values
            .iter()
            .map(|value| (entity(*value), CARD_A))
            .collect(),
    )
}

/// 用一组实体号建手牌（每张都是 `CARD_A`）。
pub(crate) fn hand(values: &[u32]) -> HandState {
    let mut hand = HandState::new();
    for value in values {
        let (next, _) = hand.add(CardInstance::new(entity(*value), CARD_A));
        hand = next;
    }
    hand
}

/// 建一个角色的装备栏（`(装备实体号, 槽位, 卡牌 id)`；空注册表，不派发）。
pub(crate) fn equipment(owner: EntityId, items: &[(u32, SlotKind, u32)]) -> EquipmentZone {
    let mut zone = EquipmentZone::new(owner);
    let triggers = EquipmentTriggers::new();
    for (equip, slot, card) in items {
        let resolution = zone.equip_with_triggers(
            EquipmentEntry {
                equip: entity(*equip),
                card: CardId::new(*card),
                slot: *slot,
            },
            None,
            &CharacterEquip::OTHER_WEAPON,
            &triggers,
        );
        zone = resolution.zone;
    }
    zone
}

/// 建一个挂了若干出战状态的出战状态区（空注册表，不派发）。
pub(crate) fn combat_status(
    active: EntityId,
    statuses: &[(&'static dyn StateIdentity, Option<Nat>, Option<Nat>)],
) -> CombatStatusZone {
    let triggers = crate::families::character::zones::combat_status::CombatStatusTriggers::new();
    let mut zone = CombatStatusZone::new();
    for (status, layers, duration) in statuses {
        zone = zone
            .attach_with_triggers(active, *status, *layers, *duration, &triggers)
            .zone;
    }
    zone
}

/// 建一个召唤物区（`(召唤物实体号, 可用次数)`）。
pub(crate) fn summons(items: &[(u32, u32)]) -> SummonZone {
    let triggers = crate::families::summon::zones::SummonTriggers::new();
    let mut zone = SummonZone::new();
    for (summon, uses) in items {
        zone = zone
            .summon_with_triggers(
                SummonEntry::new(entity(*summon), CARD_A).with_uses(Some(n(*uses))),
                &triggers,
            )
            .zone;
    }
    zone
}
