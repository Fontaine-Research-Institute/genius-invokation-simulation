//! **装备栏**（规则书·游戏区域 II(1)）：每个场上角色挂载一个装备区。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` 游戏区域 II(1) + 卡面 + `rules::ops::equip`）
//!
//! - II(1)：「『装备牌』打出给角色后，会附属在角色牌上。每个角色最多装备 1 张『武器』、
//!   1 张『圣遗物』、1 张『特技』，和任意张其他类型的装备。」
//! - 卡面：武器「『X』角色才能装备。角色最多装备 1 件『武器』」（54 张，如 311204 弓 / 311503 单手剑）；
//!   圣遗物「角色最多装备 1 件『圣遗物』」（63 张）；特技「角色最多装备 1 个『特技』」（22 张）；
//!   天赋「牌组中包含 <角色>，才能加入牌组」（155 张）—— 规则书把天赋算「其他类型装备」，不限张数。
//! - 槽位规则 / 互斥判定**复用** `rules::ops::equip`（`SlotRule` / `check_equip` / `SlotKind`），
//!   本层不重写「槽位上限 / 武器类型门」。
//!
//! ## 口径（D182 / D183 / D185 / D187）
//!
//! - 每个角色一个**装备栏实例**（`EquipmentZone`），可建多个（D182）；它自持 `owner`（角色实体号）、
//!   `entries`、统计；**不是全局一个装备区**（分工计划 §4.4.14）；
//! - 对外只给**只读指针**（`entries()` / `slot_entry()` / `talent_of()` / `owner()` / `stats()`）；
//! - 进 / 出区域的时机键由本区自己定义（D185），走 `timeline` 派发（只排序、不执行 D186）；
//! - **替换时旧装备弃置**（Lead 裁定 2/3，待复核）：同槽已有 -> 旧的那张离开装备栏；
//!   天赋同种（同卡 id）覆盖 -> 旧的那张离开装备栏。都发 `Discarded`。
//!
//! ## 待复核
//!
//! - 同槽已有时是「强制替换」还是「禁止打出」（卡文只有「最多装备 1 件」），按 Lead 裁定先用强制替换；
//! - 替换的派发顺序：`Discarded`（旧）-> `Equipped`（新）—— 规则书未规定。

use crate::base::CardId;
use crate::families::character::zones::stats::EquipmentZoneStats;
use crate::rules::ops::equip::{
    check_equip, CharacterEquip, EquipCard, EquipOutcome, EquipReject, SlotKind, WeaponType,
};
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::{EntityId, Nat};

/// 装备栏的**时机键**（D185：由本区自己定义）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EquipmentTiming {
    /// 装备**进入**装备栏（挂上角色）。
    Equipped,
    /// 装备**离开**装备栏（弃置：同槽替换 / 天赋同种覆盖 / 卸下）。
    Discarded,
}

/// 装备栏效果函数指针：`fn(&mut 装备栏, &Event, owner)`。
pub type EquipmentEffect = fn(&mut EquipmentZone, &Event, EntityId);

/// 装备栏的触发登记项。
pub type EquipmentTrigger = Trigger<EquipmentTiming, EquipmentEffect>;

/// 装备栏触发注册表（每个角色一个实例；同一张卡多实例登记进同一张表）。
pub type EquipmentTriggers = TriggerRegistry<EquipmentTiming, EquipmentEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type EquipmentPending = Queued<EquipmentEffect>;

/// 一件已装备的牌：装备实体 + 卡牌定义 id + 槽位种类。
///
/// `card` 用于「天赋同种覆盖」判定（同卡 id = 同种）；`slot` 由调用方按装备牌标签给出
/// （武器 / 圣遗物 / 特技 / 天赋，见 `rules::ops::equip`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EquipmentEntry {
    /// 装备实体号。
    pub equip: EntityId,
    /// 装备牌的定义 id。
    pub card: CardId,
    /// 槽位种类。
    pub slot: SlotKind,
}

/// **装备栏**：某个场上角色的装备列表。
///
/// 容器没有身份（D42 / D182）：它就是「该角色的装备区」这个变量；`owner` 让事件 / 查询知道
/// 「这是谁的装备栏」，不需要外部再维护「角色 -> 装备区」表。
#[derive(Clone, Debug)]
pub struct EquipmentZone {
    /// 这个装备栏属于哪个角色。
    owner: EntityId,
    /// 已装备的牌（顺序 = 挂上的先后）。
    entries: Vec<EquipmentEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: EquipmentZoneStats,
}

impl PartialEq for EquipmentZone {
    /// **统计不参与局面等价**（D187）。
    fn eq(&self, other: &EquipmentZone) -> bool {
        self.owner == other.owner && self.entries == other.entries
    }
}

impl Eq for EquipmentZone {}

/// 一次装备栏操作的结果：新状态 + 结果 + 待执行。
#[derive(Clone, Debug)]
pub struct EquipmentResolution {
    /// 操作后的装备栏。
    pub zone: EquipmentZone,
    /// 这次挂装备的结果（被拒绝时里面带原因）。
    pub outcome: EquipmentOutcome,
    /// 派发出来的待执行（本层不执行）。
    pub pending: Vec<EquipmentPending>,
}

/// 挂装备的结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EquipmentOutcome {
    /// 空槽：直接挂上。
    Attached,
    /// 同槽已有：旧装备弃置，新装备挂上（Lead 裁定 2）。
    Replaced {
        /// 被占用的槽位。
        slot: SlotKind,
        /// 被弃置的旧装备实体号。
        discarded: EntityId,
    },
    /// 天赋同种覆盖：旧的同种天赋弃置，新的挂上（Lead 裁定 3）。
    TalentOverwritten {
        /// 被弃置的旧天赋实体号。
        discarded: EntityId,
    },
    /// 卸下（不是替换）：这件装备被弃置。
    Discarded {
        /// 被弃置的装备实体号。
        equip: EntityId,
    },
    /// 不能装备（槽位 / 武器类型门拒绝）；状态不变。
    Rejected(EquipReject),
}

impl EquipmentZone {
    /// 建一个空装备栏（属于 `owner` 这个角色）。
    pub const fn new(owner: EntityId) -> EquipmentZone {
        EquipmentZone {
            owner,
            entries: Vec::new(),
            stats: EquipmentZoneStats::EMPTY,
        }
    }

    /// 这个装备栏属于哪个角色。
    pub fn owner(&self) -> EntityId {
        self.owner
    }

    /// 已装备的牌（**只读指针**，D183；顺序 = 挂上的先后）。
    pub fn entries(&self) -> &[EquipmentEntry] {
        &self.entries
    }

    /// 已装备张数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何装备。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 装备栏统计（只读指针，D183 / D187）。
    pub fn stats(&self) -> &EquipmentZoneStats {
        &self.stats
    }

    /// 某个槽位的第一件装备（只读指针；空槽 -> None）。
    pub fn slot_entry(&self, slot: SlotKind) -> Option<&EquipmentEntry> {
        self.entries.iter().find(|entry| entry.slot == slot)
    }

    /// 某个槽位的全部装备（只读指针；天赋槽可能多张）。
    pub fn entries_of_slot(&self, slot: SlotKind) -> Vec<&EquipmentEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.slot == slot)
            .collect()
    }

    /// 同种天赋（同卡 id）——天赋同种覆盖判定用。
    pub fn talent_of(&self, card: CardId) -> Option<&EquipmentEntry> {
        self.entries
            .iter()
            .find(|entry| entry.slot == SlotKind::Talent && entry.card == card)
    }

    /// 这个槽位是否已被占用（武器 / 圣遗物 / 特技各 1；天赋不算「占用」）。
    pub fn occupies(&self, slot: SlotKind) -> bool {
        slot != SlotKind::Talent && self.entries.iter().any(|entry| entry.slot == slot)
    }

    /// **挂装备**（规则书·游戏区域 II(1)）+ 派发「装备 / 弃置」。
    ///
    /// 判定复用 `rules::ops::equip::check_equip`：
    /// - 武器：角色武器类型必须匹配（`required_weapon`）；「其他武器」角色装不了武器；
    /// - 圣遗物 / 特技：每角色 1 件，同槽强制替换；
    /// - 天赋：不限张数，同卡 id 同种覆盖。
    ///
    /// 返回的结果里 `outcome` 是 `Rejected` 时**状态不变**、不派发。
    pub fn equip_with_triggers(
        &self,
        entry: EquipmentEntry,
        required_weapon: Option<WeaponType>,
        character: &CharacterEquip,
        triggers: &EquipmentTriggers,
    ) -> EquipmentResolution {
        let occupied: Vec<SlotKind> = self
            .entries
            .iter()
            .filter(|existing| existing.slot != SlotKind::Talent)
            .map(|existing| existing.slot)
            .collect();
        let same_kind_present = self.talent_of(entry.card).is_some();
        let outcome = check_equip(
            &EquipCard {
                slot: entry.slot,
                required_weapon,
            },
            character,
            &occupied,
            same_kind_present,
        );
        match outcome {
            EquipOutcome::Reject(reject) => EquipmentResolution {
                zone: self.clone(),
                outcome: EquipmentOutcome::Rejected(reject),
                pending: Vec::new(),
            },
            EquipOutcome::Attach => self.push_attached(entry, triggers),
            EquipOutcome::ReplaceExisting(slot) => {
                let old = self
                    .entries
                    .iter()
                    .find(|existing| existing.slot == slot)
                    .map(|existing| existing.equip);
                match old {
                    Some(discarded) => self.replace_with(entry, discarded, triggers),
                    None => self.push_attached(entry, triggers),
                }
            }
            EquipOutcome::OverwriteSameKind => match self.talent_of(entry.card) {
                Some(existing) => self.replace_with(entry, existing.equip, triggers),
                None => self.push_attached(entry, triggers),
            },
        }
    }

    /// 把 `entry` 挂上，旧的 `discarded` 先移出；派发 `Discarded` -> `Equipped`。
    fn replace_with(
        &self,
        entry: EquipmentEntry,
        discarded: EntityId,
        triggers: &EquipmentTriggers,
    ) -> EquipmentResolution {
        let mut zone = self.clone();
        zone.entries.retain(|existing| existing.equip != discarded);
        zone.stats.on_discarded(Nat::new(1));
        zone.entries.push(entry);
        zone.stats.on_equipped(Nat::new(1));
        let mut pending = collect(
            triggers,
            EquipmentTiming::Discarded,
            discarded_event(self.owner, discarded),
        );
        pending.extend(collect(
            triggers,
            EquipmentTiming::Equipped,
            equipped_event(self.owner, entry.equip),
        ));
        EquipmentResolution {
            zone,
            outcome: if entry.slot == SlotKind::Talent
                && self.talent_of(entry.card).map(|existing| existing.equip) == Some(discarded)
            {
                EquipmentOutcome::TalentOverwritten { discarded }
            } else {
                EquipmentOutcome::Replaced {
                    slot: entry.slot,
                    discarded,
                }
            },
            pending,
        }
    }

    /// 空槽直接挂上 + 派发 `Equipped`。
    fn push_attached(
        &self,
        entry: EquipmentEntry,
        triggers: &EquipmentTriggers,
    ) -> EquipmentResolution {
        let mut zone = self.clone();
        zone.entries.push(entry);
        zone.stats.on_equipped(Nat::new(1));
        let pending = collect(
            triggers,
            EquipmentTiming::Equipped,
            equipped_event(self.owner, entry.equip),
        );
        EquipmentResolution {
            zone,
            outcome: EquipmentOutcome::Attached,
            pending,
        }
    }

    /// **卸下**一件装备（弃置）+ 派发 `Discarded`。
    ///
    /// 失败（返回 None、不改状态）：这件装备不在本装备栏里。
    pub fn detach_with_triggers(
        &self,
        equip: EntityId,
        triggers: &EquipmentTriggers,
    ) -> Option<EquipmentResolution> {
        let position = self.entries.iter().position(|entry| entry.equip == equip)?;
        let mut zone = self.clone();
        zone.entries.remove(position);
        zone.stats.on_discarded(Nat::new(1));
        let pending = collect(
            triggers,
            EquipmentTiming::Discarded,
            discarded_event(self.owner, equip),
        );
        Some(EquipmentResolution {
            zone,
            outcome: EquipmentOutcome::Discarded { equip },
            pending,
        })
    }
}

/// 执行一条待执行列表（**调用方显式调用**；派发永不执行）。
pub fn execute_equipment(pending: &[EquipmentPending], zone: &mut EquipmentZone) {
    for item in pending {
        (item.effect)(zone, &item.event, item.owner);
    }
}

/// 收集某个时机 + 事件的待执行。
fn collect(
    triggers: &EquipmentTriggers,
    timing: EquipmentTiming,
    event: Event,
) -> Vec<EquipmentPending> {
    dispatch(triggers, timing, &event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 「装备挂上」事件：`who` = 装备实体，`target` = 角色。
pub fn equipped_event(owner: EntityId, equip: EntityId) -> Event {
    Event::new(Some(equip), Some(owner), Nat::ZERO, None, None)
}

/// 「装备弃置」事件：`who` = 装备实体，`target` = 角色。
pub fn discarded_event(owner: EntityId, equip: EntityId) -> Event {
    Event::new(Some(equip), Some(owner), Nat::ZERO, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ops::order::EffectSource;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    fn entry(equip: u32, card: u32, slot: SlotKind) -> EquipmentEntry {
        EquipmentEntry {
            equip: n(equip),
            card: CardId::new(card),
            slot,
        }
    }

    fn sword_user() -> CharacterEquip {
        CharacterEquip::with_weapon(WeaponType::Sword)
    }

    fn noop(_zone: &mut EquipmentZone, _event: &Event, _owner: EntityId) {}

    fn registry_for(timing: EquipmentTiming) -> EquipmentTriggers {
        let mut triggers = EquipmentTriggers::new();
        triggers.register(Trigger::new(
            timing,
            n(1),
            EffectSource::CharacterEquipmentOrStatus,
            n(0),
            noop,
        ));
        triggers
    }

    /// 同时登记「挂上 / 弃置」两个时机的注册表（替换时两条都会命中）。
    fn registry_both() -> EquipmentTriggers {
        let mut triggers = EquipmentTriggers::new();
        triggers.register(Trigger::new(
            EquipmentTiming::Equipped,
            n(1),
            EffectSource::CharacterEquipmentOrStatus,
            n(0),
            noop,
        ));
        triggers.register(Trigger::new(
            EquipmentTiming::Discarded,
            n(1),
            EffectSource::CharacterEquipmentOrStatus,
            n(0),
            noop,
        ));
        triggers
    }

    #[test]
    fn empty_zone_basics() {
        let zone = EquipmentZone::new(n(1));
        assert_eq!(zone.owner(), n(1));
        assert!(zone.is_empty());
        assert_eq!(zone.len(), Nat::ZERO);
        assert!(!zone.occupies(SlotKind::Weapon));
        assert!(zone.slot_entry(SlotKind::Weapon).is_none());
        assert!(zone.talent_of(CardId::new(217051)).is_none());
    }

    #[test]
    fn weapon_attaches_only_when_type_matches() {
        let triggers = EquipmentTriggers::new();
        let zone = EquipmentZone::new(n(1));
        // 弓牌给单手剑角色：拒绝，状态不变。
        let rejected = zone.equip_with_triggers(
            entry(10, 311204, SlotKind::Weapon),
            Some(WeaponType::Bow),
            &sword_user(),
            &triggers,
        );
        assert_eq!(
            rejected.outcome,
            EquipmentOutcome::Rejected(EquipReject::WeaponTypeMismatch)
        );
        assert!(rejected.pending.is_empty());
        assert_eq!(rejected.zone, zone);
        // 单手剑牌：挂上。
        let attached = zone.equip_with_triggers(
            entry(11, 311503, SlotKind::Weapon),
            Some(WeaponType::Sword),
            &sword_user(),
            &triggers,
        );
        assert_eq!(attached.outcome, EquipmentOutcome::Attached);
        assert_eq!(attached.zone.len(), n(1));
        assert!(attached.zone.occupies(SlotKind::Weapon));
        assert_eq!(attached.zone.slot_entry(SlotKind::Weapon).expect("有").equip, n(11));
    }

    #[test]
    fn other_weapon_character_cannot_equip_weapons_but_can_take_artifacts() {
        let triggers = EquipmentTriggers::new();
        let zone = EquipmentZone::new(n(1));
        let rejected = zone.equip_with_triggers(
            entry(10, 311402, SlotKind::Weapon),
            Some(WeaponType::Polearm),
            &CharacterEquip::OTHER_WEAPON,
            &triggers,
        );
        assert_eq!(
            rejected.outcome,
            EquipmentOutcome::Rejected(EquipReject::OtherWeaponCannotEquip)
        );
        // 圣遗物谁都能装。
        let artifact = zone.equip_with_triggers(
            entry(20, 312001, SlotKind::Artifact),
            None,
            &CharacterEquip::OTHER_WEAPON,
            &triggers,
        );
        assert_eq!(artifact.outcome, EquipmentOutcome::Attached);
    }

    #[test]
    fn occupied_slot_replaces_and_discards_old_equipment() {
        let triggers = registry_both();
        let zone = EquipmentZone::new(n(1));
        let first = zone
            .equip_with_triggers(
                entry(10, 311503, SlotKind::Weapon),
                Some(WeaponType::Sword),
                &sword_user(),
                &triggers,
            )
            .zone;
        let second = first.equip_with_triggers(
            entry(11, 311504, SlotKind::Weapon),
            Some(WeaponType::Sword),
            &sword_user(),
            &triggers,
        );
        assert_eq!(
            second.outcome,
            EquipmentOutcome::Replaced {
                slot: SlotKind::Weapon,
                discarded: n(10),
            }
        );
        assert_eq!(second.zone.len(), n(1));
        assert_eq!(second.zone.slot_entry(SlotKind::Weapon).expect("有").equip, n(11));
        // 派发顺序：旧装备弃置 -> 新装备挂上。
        assert_eq!(second.pending.len(), 2);
        assert_eq!(second.pending.first().expect("弃置").event.who, Some(n(10)));
        assert_eq!(second.pending.get(1).expect("挂上").event.who, Some(n(11)));
    }

    #[test]
    fn talents_coexist_unless_same_card() {
        let triggers = EquipmentTriggers::new();
        let zone = EquipmentZone::new(n(1));
        let one = zone
            .equip_with_triggers(
                entry(30, 217051, SlotKind::Talent),
                None,
                &sword_user(),
                &triggers,
            )
            .zone;
        let two = one
            .equip_with_triggers(
                entry(31, 217052, SlotKind::Talent),
                None,
                &sword_user(),
                &triggers,
            )
            .zone;
        assert_eq!(two.entries_of_slot(SlotKind::Talent).len(), 2);
        // 同种（同卡 id）覆盖：只留下新的那张。
        let overwritten = two.equip_with_triggers(
            entry(32, 217051, SlotKind::Talent),
            None,
            &sword_user(),
            &triggers,
        );
        assert_eq!(
            overwritten.outcome,
            EquipmentOutcome::TalentOverwritten { discarded: n(30) }
        );
        assert_eq!(overwritten.zone.entries_of_slot(SlotKind::Talent).len(), 2);
        assert!(overwritten.zone.talent_of(CardId::new(217051)).is_some());
        assert!(!overwritten
            .zone
            .entries()
            .iter()
            .any(|existing| existing.equip == n(30)));
    }

    #[test]
    fn detach_removes_and_dispatches_discarded() {
        let triggers = registry_for(EquipmentTiming::Discarded);
        let zone = EquipmentZone::new(n(1))
            .equip_with_triggers(
                entry(10, 311503, SlotKind::Weapon),
                Some(WeaponType::Sword),
                &sword_user(),
                &triggers,
            )
            .zone;
        let detached = zone.detach_with_triggers(n(10), &triggers).expect("在装备栏里");
        assert!(detached.zone.is_empty());
        assert_eq!(detached.pending.len(), 1);
        assert_eq!(detached.pending.first().expect("弃置").event.who, Some(n(10)));
        assert!(detached.zone.detach_with_triggers(n(10), &triggers).is_none());
    }

    #[test]
    fn stats_follow_feature_flag() {
        let triggers = EquipmentTriggers::new();
        let zone = EquipmentZone::new(n(1))
            .equip_with_triggers(
                entry(10, 311503, SlotKind::Weapon),
                Some(WeaponType::Sword),
                &sword_user(),
                &triggers,
            )
            .zone;
        if cfg!(feature = "stats") {
            assert_eq!(zone.stats().equipped().get(), n(1));
        } else {
            assert!(zone.stats().equipped().is_zero());
        }
    }
}
