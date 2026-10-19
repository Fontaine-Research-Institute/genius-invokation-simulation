//! 装备槽与互斥 / 替换（types=装备 + tags=武器 / 圣遗物 / 天赋 / 特技）。
//!
//! **权威依据（规则书·游戏区域 II 角色附属）**：
//! 「『装备牌』打出给角色后，会附属在角色牌上。每个角色最多装备 1 张『武器』、1 张『圣遗物』、
//! 1 张『特技』，和任意张其他类型的装备。」
//! English: "Each character can attach at most 1 Weapon, 1 Artifact, 1 Technique,
//! and any number of other types of Equipment."
//!
//! 补充依据（卡面，规则书未写）：
//! - 武器：「『X』角色才能装备。角色最多装备 1 件『武器』」（54 张，如 311204 弓 / 311503 单手剑 /
//!   311104 法器 / 311306 双手剑 / 311402 长柄武器）；
//! - 圣遗物：「角色最多装备 1 件『圣遗物』」（63 张，如 312001 冒险家头带）；
//! - 特技：「角色最多装备 1 个『特技』」（22 张，如 313010 膨膨兽）+ 规则书·特殊词条 特技；
//! - 天赋：「牌组中包含 <角色>，才能加入牌组」（155 张，如 217051 在地为化）；
//!   规则书把天赋算作「其他类型装备」→ 不限张数。
//!
//! Lead 裁定 2/3/4（待用户复核；规则书未规定同槽替换 / 同种覆盖细节）：
//! 同槽已有 -> 强制替换旧装备弃置；天赋同种（同卡 id / 同名）覆盖；魔导并入天赋。
//!
use gi_genius_invokation_engine::Nat;

/// 装备槽种类（魔导不单列：Lead 裁定 4 并入天赋）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotKind {
    /// 武器槽：每角色最多 1 件，且必须武器类型匹配。
    Weapon,
    /// 圣遗物槽：每角色最多 1 件，谁都能装。
    Artifact,
    /// 天赋槽：每角色可多张，**同种覆盖**（不设总数上限）。
    Talent,
    /// 特技槽：每角色最多 1 个。
    Technique,
}

/// 角色的武器类型（来自角色牌上的武器子类标签；「其他武器」= `None`）。
///
/// 对应 tags：单手剑 / 弓 / 双手剑 / 法器 / 长柄武器；其他武器（`OtherWeapons`）没有类型。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponType {
    /// 单手剑。
    Sword,
    /// 双手剑。
    Claymore,
    /// 弓。
    Bow,
    /// 法器。
    Catalyst,
    /// 长柄武器。
    Polearm,
}

/// 同槽已占时的处理方式。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnOccupied {
    /// 强制替换，旧装备弃置（Lead 裁定 2）。
    Replace,
    /// 同种覆盖自己种类（天赋；Lead 裁定 3）。
    OverwriteSameKind,
}

/// 一条槽位规则（静态表用；`cap = None` 表示不设总数上限）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotRule {
    /// 槽位种类。
    pub kind: SlotKind,
    /// 每角色该槽位的数量上限；`None` = 不设上限（天赋）。
    pub cap: Option<Nat>,
    /// 需要的角色武器类型；`None` = 无类型门。
    pub requires_weapon_type: Option<WeaponType>,
    /// 同槽已占时的处理。
    pub on_occupied: OnOccupied,
}

/// 武器槽规则（通用）：每角色 1 件；具体子类的类型门在子类标签行给出。
pub const WEAPON_SLOT: SlotRule = SlotRule {
    kind: SlotKind::Weapon,
    cap: Some(Nat::new(1)),
    requires_weapon_type: None,
    on_occupied: OnOccupied::Replace,
};

/// 圣遗物槽规则：每角色 1 件，无类型门。
pub const ARTIFACT_SLOT: SlotRule = SlotRule {
    kind: SlotKind::Artifact,
    cap: Some(Nat::new(1)),
    requires_weapon_type: None,
    on_occupied: OnOccupied::Replace,
};

/// 特技槽规则：每角色 1 个。
pub const TECHNIQUE_SLOT: SlotRule = SlotRule {
    kind: SlotKind::Technique,
    cap: Some(Nat::new(1)),
    requires_weapon_type: None,
    on_occupied: OnOccupied::Replace,
};

/// 天赋槽规则：不设总数上限；同种覆盖。
pub const TALENT_SLOT: SlotRule = SlotRule {
    kind: SlotKind::Talent,
    cap: None,
    requires_weapon_type: None,
    on_occupied: OnOccupied::OverwriteSameKind,
};

/// 某个武器子类的槽位规则：每角色 1 件，且角色必须是该武器类型。
///
/// 依据：武器卡面「『X』角色才能装备。角色最多装备 1 件『武器』」
/// （弓 311204 / 单手剑 311503 / 法器 311104 / 双手剑 311306 / 长柄武器 311402）。
pub const fn weapon_subclass_slot(weapon_type: WeaponType) -> SlotRule {
    SlotRule {
        kind: SlotKind::Weapon,
        cap: Some(Nat::new(1)),
        requires_weapon_type: Some(weapon_type),
        on_occupied: OnOccupied::Replace,
    }
}

/// 单手剑槽（11 张卡面「『单手剑』角色才能装备」）。
pub const SWORD_SLOT: SlotRule = weapon_subclass_slot(WeaponType::Sword);
/// 弓槽（14 张卡面「『弓』角色才能装备」）。
pub const BOW_SLOT: SlotRule = weapon_subclass_slot(WeaponType::Bow);
/// 双手剑槽（10 张卡面「『双手剑』角色才能装备」）。
pub const CLAYMORE_SLOT: SlotRule = weapon_subclass_slot(WeaponType::Claymore);
/// 法器槽（14 张卡面「『法器』角色才能装备」）。
pub const CATALYST_SLOT: SlotRule = weapon_subclass_slot(WeaponType::Catalyst);
/// 长柄武器槽（9 张卡面「『长柄武器』角色才能装备」）。
pub const POLEARM_SLOT: SlotRule = weapon_subclass_slot(WeaponType::Polearm);

/// 一张装备牌的槽位需求（由具体卡的标签组合得出：武器 + 子类 -> 槽 Weapon + 类型门）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EquipCard {
    /// 槽位种类。
    pub slot: SlotKind,
    /// 需要的角色武器类型（武器牌才有）。
    pub required_weapon: Option<WeaponType>,
}

/// 一个角色的装备条件（目前只有武器类型；其余槽位没有类型门）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CharacterEquip {
    /// 角色武器类型；`None` = 「其他武器」。
    pub weapon: Option<WeaponType>,
}

impl CharacterEquip {
    /// 用武器类型构造。
    pub const fn with_weapon(weapon: WeaponType) -> CharacterEquip {
        CharacterEquip {
            weapon: Some(weapon),
        }
    }

    /// 「其他武器」角色（没有武器类型）。
    pub const OTHER_WEAPON: CharacterEquip = CharacterEquip { weapon: None };
}

/// 装备结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EquipOutcome {
    /// 直接挂上（空槽）。
    Attach,
    /// 替换掉同槽已有装备（旧装备弃置；Lead 裁定 2）。
    ReplaceExisting(SlotKind),
    /// 天赋同种覆盖（旧的那张同种天赋弃置；Lead 裁定 3）。
    OverwriteSameKind,
    /// 不能装备。
    Reject(EquipReject),
}

/// 不能装备的原因。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EquipReject {
    /// 角色是「其他武器」，装不了任何武器牌。
    OtherWeaponCannotEquip,
    /// 武器子类不匹配（如「弓」牌给「单手剑」角色）。
    WeaponTypeMismatch,
}

/// 装备判定（纯函数，不改任何状态）。
///
/// - `occupied`：该角色当前已占用的槽位（每槽位最多出现一次；调用方按需给）；
/// - `same_kind_present`：天赋专用 —— 该角色是否已装备**同种**（同卡 id / 同名）天赋。
pub fn check_equip(
    card: &EquipCard,
    character: &CharacterEquip,
    occupied: &[SlotKind],
    same_kind_present: bool,
) -> EquipOutcome {
    if let Some(required) = card.required_weapon {
        match character.weapon {
            Some(weapon) if weapon == required => {}
            Some(_) => return EquipOutcome::Reject(EquipReject::WeaponTypeMismatch),
            None => return EquipOutcome::Reject(EquipReject::OtherWeaponCannotEquip),
        }
    }
    match card.slot {
        SlotKind::Talent => {
            if same_kind_present {
                EquipOutcome::OverwriteSameKind
            } else {
                EquipOutcome::Attach
            }
        }
        SlotKind::Weapon | SlotKind::Artifact | SlotKind::Technique => {
            // 待实测：同槽已有时是强制替换（Lead 裁定 2）还是禁止打出，卡文只有「最多装备 1 件」。
            if occupied.contains(&card.slot) {
                EquipOutcome::ReplaceExisting(card.slot)
            } else {
                EquipOutcome::Attach
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(slot: SlotKind, required_weapon: Option<WeaponType>) -> EquipCard {
        EquipCard {
            slot,
            required_weapon,
        }
    }

    #[test]
    fn matching_weapon_attaches_to_empty_slot() {
        let bow_user = CharacterEquip::with_weapon(WeaponType::Bow);
        let outcome = check_equip(
            &card(SlotKind::Weapon, Some(WeaponType::Bow)),
            &bow_user,
            &[],
            false,
        );
        assert_eq!(outcome, EquipOutcome::Attach);
    }

    #[test]
    fn mismatched_weapon_is_rejected() {
        let sword_user = CharacterEquip::with_weapon(WeaponType::Sword);
        let outcome = check_equip(
            &card(SlotKind::Weapon, Some(WeaponType::Bow)),
            &sword_user,
            &[],
            false,
        );
        assert_eq!(
            outcome,
            EquipOutcome::Reject(EquipReject::WeaponTypeMismatch)
        );
    }

    #[test]
    fn other_weapon_character_cannot_equip_weapons() {
        let outcome = check_equip(
            &card(SlotKind::Weapon, Some(WeaponType::Polearm)),
            &CharacterEquip::OTHER_WEAPON,
            &[],
            false,
        );
        assert_eq!(
            outcome,
            EquipOutcome::Reject(EquipReject::OtherWeaponCannotEquip)
        );
        // 非武器装备不受影响：圣遗物谁都能装（312001 冒险家头带）。
        let artifact = check_equip(
            &card(SlotKind::Artifact, None),
            &CharacterEquip::OTHER_WEAPON,
            &[],
            false,
        );
        assert_eq!(artifact, EquipOutcome::Attach);
    }

    #[test]
    fn occupied_slot_forces_replacement() {
        let sword_user = CharacterEquip::with_weapon(WeaponType::Sword);
        let outcome = check_equip(
            &card(SlotKind::Weapon, Some(WeaponType::Sword)),
            &sword_user,
            &[SlotKind::Weapon],
            false,
        );
        assert_eq!(outcome, EquipOutcome::ReplaceExisting(SlotKind::Weapon));
        // 圣遗物同理（312001 / 312027）
        let artifact = check_equip(
            &card(SlotKind::Artifact, None),
            &sword_user,
            &[SlotKind::Artifact],
            false,
        );
        assert_eq!(
            artifact,
            EquipOutcome::ReplaceExisting(SlotKind::Artifact)
        );
    }

    #[test]
    fn talent_same_kind_overwrites_and_multiple_kinds_coexist() {
        let any = CharacterEquip::with_weapon(WeaponType::Catalyst);
        let attach = check_equip(&card(SlotKind::Talent, None), &any, &[], false);
        assert_eq!(attach, EquipOutcome::Attach);
        let overwrite = check_equip(&card(SlotKind::Talent, None), &any, &[], true);
        assert_eq!(overwrite, EquipOutcome::OverwriteSameKind);
        // 天赋不设总数上限（cap = None）
        assert_eq!(TALENT_SLOT.cap, None);
        // 其余槽位都是 1
        assert_eq!(WEAPON_SLOT.cap, Some(Nat::new(1)));
        assert_eq!(ARTIFACT_SLOT.cap, Some(Nat::new(1)));
        assert_eq!(TECHNIQUE_SLOT.cap, Some(Nat::new(1)));
    }
}