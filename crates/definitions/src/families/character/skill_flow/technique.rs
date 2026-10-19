//! **特技流程**（覆盖报告 L4 / 规则书·特殊词条 特技 + K58）—— 特技不是技能。
//!
//! ## 依据（规则书 > 关键字数据）
//!
//! 规则书·特殊词条 **特技**（S8.4，原文）：
//! 1. 出战角色装备有「特技」卡牌时，就可以使用对应的「特技」。
//! 2. 使用「特技」是一种「战斗行动」。
//! 3. 角色因冻结、石化、眩晕等原因无法使用技能时，也会同时无法使用「特技」。
//! 4. 角色使用「特技」时不视为使用技能，无法触发「使用技能后」「角色引发元素反应后」
//!    等效果；角色使用「特技」造成的伤害不被视为角色造成的伤害。
//!
//! 权威关键字 **K58 特技** 同文（四语）。
//!
//! ## 口径
//!
//! - 槽位：装备栏的 SlotKind::Technique（每角色最多 1 个；rules::ops::equip）；
//! - use_technique 只做**判定 + 结果**：不支付（规则书未写特技费用）、不派发、不执行效果；
//! - **不派发**「使用技能时 / 后」：dispatch_technique 恒返回空（K58 第 4 条），
//!   与 timing::dispatch_skill 明确分开；
//! - 伤害来源：K58「不被视为角色造成的伤害」-> TechniqueUse::damage_is_character_damage = false；
//!   调用方只能按「非角色来源」（DamageSourceKind::Other 或状态类）结算，不能用普通攻击 /
//!   战技 / 爆发类别；
//! - 行动权：战斗行动（technique_timing = Timing::Combat）。
//!
//! ## 与 SkillKind 的并列关系
//!
//! 特技**不是** SkillKind 的变体（K58：不视为使用技能；D123 / D126 只给普通攻击形态）——
//! 用 SkillUseClass 把「普通攻击 / 元素战技 / 元素爆发 / 被动 / 特技」并列成一个判定点：
//! is_skill / is_combat_action / dispatches_skill_used / damage_is_character_damage / gains_energy。
//!
//! ## 待接入 / 待复核
//!
//! - 端到端 flow 入口（flow::use_technique_me/opp）不在本轮写范围；本模块给判定与结果；
//! - 「不触发『角色引发元素反应后』」由派发层按伤害来源类别过滤（本层只标「不是角色伤害」）；
//! - 特技费用规则书未写：本层不收费（卡面若有费用由效果自己支付）；
//! - 使用特技是否占用「每回合使用次数」：规则书未写 -> 待实测（本层不记次数）；
//! - 特技的**效果体**：特技牌是行动牌（装备），效果仍在 cards/ 层，本层只给流程入口。

use crate::base::CardId;
use crate::families::character::character::CharacterState;
use crate::families::character::skill::SkillDef;
use crate::families::character::skill_flow::forbidden::{forbidden_by, ForbiddenState};
use crate::families::character::skill_flow::{SkillPending, SkillTriggers};
use crate::families::character::types::SkillKind;
use crate::families::character::zones::equipment::EquipmentZone;
use crate::rules::ops::equip::SlotKind;
use crate::rules::ops::stage::Timing;
use gi_genius_invokation_engine::EntityId;

/// **可主动使用入口的分类**：把「技能」与「特技」并列成一个判定点。
///
/// 为什么不用 SkillKind 表达特技：K58 原文「使用『特技』时不视为使用技能」，
/// 且 D123 / D126 只把重击 / 下落攻击作为普通攻击的形态 —— 特技不是技能种类，
/// 所以单独用一个分类枚举承载「流程语义」。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SkillUseClass {
    /// 普通攻击（含重击 / 下落攻击形态）。
    NormalAttack,
    /// 元素战技。
    ElementalSkill,
    /// 元素爆发。
    ElementalBurst,
    /// 被动技能（不能主动使用，只作为分类存在）。
    Passive,
    /// 特技（不是技能；战斗行动；不派发 SkillUsed；伤害不算角色造成）。
    Technique,
}

impl SkillUseClass {
    /// 技能定义 -> 主动使用分类（便于调用方直接从 SkillDef 判定）。
    pub const fn of_skill_def(def: &SkillDef) -> SkillUseClass {
        SkillUseClass::of_skill(def.kind)
    }

    /// 这个入口该走哪条流程：技能流程 / 特技流程（并列判定点）。
    pub const fn flow(self) -> UseFlow {
        match self {
            SkillUseClass::Technique => UseFlow::TechniqueFlow,
            SkillUseClass::NormalAttack
            | SkillUseClass::ElementalSkill
            | SkillUseClass::ElementalBurst
            | SkillUseClass::Passive => UseFlow::SkillFlow,
        }
    }

    /// 技能种类 -> 主动使用分类。
    pub const fn of_skill(kind: SkillKind) -> SkillUseClass {
        match kind {
            SkillKind::NormalAttack(_) => SkillUseClass::NormalAttack,
            SkillKind::ElementalSkill => SkillUseClass::ElementalSkill,
            SkillKind::ElementalBurst => SkillUseClass::ElementalBurst,
            SkillKind::Passive => SkillUseClass::Passive,
        }
    }

    /// 特技的分类（= SkillUseClass::Technique，供调用方显式引用）。
    pub const TECHNIQUE: SkillUseClass = SkillUseClass::Technique;

    /// 是不是「技能」（K58：特技不视为使用技能 -> false）。
    pub const fn is_skill(self) -> bool {
        !matches!(self, SkillUseClass::Technique)
    }

    /// 结算后是否轮到对方（战斗行动）：技能 / 特技 / 被动之外的主动入口都是战斗行动。
    ///
    /// 规则书：使用技能 = 战斗行动；特技 = 战斗行动（S8.4 第 2 条）；被动不能主动使用 -> false。
    pub const fn is_combat_action(self) -> bool {
        match self {
            SkillUseClass::NormalAttack
            | SkillUseClass::ElementalSkill
            | SkillUseClass::ElementalBurst
            | SkillUseClass::Technique => true,
            SkillUseClass::Passive => false,
        }
    }

    /// 是否派发「使用技能时 / 后」：技能 3 类 -> 派发；特技 / 被动 -> 不派发（K58）。
    pub const fn dispatches_skill_used(self) -> bool {
        match self {
            SkillUseClass::NormalAttack
            | SkillUseClass::ElementalSkill
            | SkillUseClass::ElementalBurst => true,
            SkillUseClass::Passive | SkillUseClass::Technique => false,
        }
    }

    /// 伤害是否算「角色造成的伤害」：只有特技例外（K58 第 4 条）。
    pub const fn damage_is_character_damage(self) -> bool {
        !matches!(self, SkillUseClass::Technique)
    }

    /// 结算后是否给角色**充能**：规则书「每次使用普通攻击或元素战技后获得 1 点充能」。
    pub const fn gains_energy(self) -> bool {
        matches!(
            self,
            SkillUseClass::NormalAttack | SkillUseClass::ElementalSkill
        )
    }

    /// 中文显示名（只用于记录 / 日志，不参与判定）。
    pub const fn label(self) -> &'static str {
        match self {
            SkillUseClass::NormalAttack => "普通攻击",
            SkillUseClass::ElementalSkill => "元素战技",
            SkillUseClass::ElementalBurst => "元素爆发",
            SkillUseClass::Passive => "被动技能",
            SkillUseClass::Technique => "特技",
        }
    }
}

/// 一次「主动使用」该走的**流程**（与技能流程并列的判定点）。
///
/// 特技不是技能（K58）-> 走 TechniqueFlow；其余技能种类 -> 走既有的技能流程。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum UseFlow {
    /// 技能流程（skill_flow::can_use / use_skill）。
    SkillFlow,
    /// 特技流程（can_use_technique / use_technique）。
    TechniqueFlow,
}

/// 特技使用判定的**只读上下文**（D42：容器没有身份，就是个值）。
#[derive(Clone, Copy, Debug)]
pub struct TechniqueContext<'a> {
    /// 使用特技的角色实体。
    pub who: EntityId,
    /// 该角色的状态（技能禁用查询读这里）。
    pub character: &'a CharacterState,
    /// 该角色的装备栏（特技槽在这里）。
    pub equipment: &'a EquipmentZone,
    /// 该角色是不是**出战角色**（S8.4 第 1 条：出战角色装备特技才能使用）。
    pub is_active: bool,
}

/// 不能使用特技的原因（判定顺序固定，见 can_use_technique）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CannotUseTechnique {
    /// 不是出战角色（S8.4 第 1 条）。
    NotActiveCharacter,
    /// 装备栏里没有特技（没装备特技牌）。
    NoTechniqueEquipped,
    /// 传入的装备栏不属于这个角色（调用方给错了区）。
    EquipmentOwnerMismatch {
        /// 装备栏声明的 owner。
        equipment_owner: EntityId,
    },
    /// 状态禁用技能（冻结 / 石化 / 眩晕；S8.4 第 3 条）。
    ForbiddenByStatus {
        /// 命中的禁用状态（身份 + 被禁用的入口）。
        forbidden: ForbiddenState,
    },
}

/// 一次特技使用的结果（判定通过后的纯数据；不执行效果、不派发）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TechniqueUse {
    /// 使用特技的角色实体。
    pub who: EntityId,
    /// 提供特技的装备实体。
    pub equip: EntityId,
    /// 特技牌的定义 id。
    pub technique: CardId,
    /// 主动使用分类（恒为 SkillUseClass::Technique）。
    pub class: SkillUseClass,
    /// 行动权：战斗行动（S8.4 第 2 条）。
    pub timing: Timing,
}

impl TechniqueUse {
    /// K58：特技伤害不算角色造成的伤害。
    pub const fn damage_is_character_damage(&self) -> bool {
        self.class.damage_is_character_damage()
    }

    /// K58：特技不派发「使用技能时 / 后」。
    pub const fn dispatches_skill_used(&self) -> bool {
        self.class.dispatches_skill_used()
    }

    /// 特技不给充能（规则书：只有普通攻击 / 元素战技给）。
    pub const fn gains_energy(&self) -> bool {
        self.class.gains_energy()
    }

    /// 是不是战斗行动（恒 true；特技是行动权入口）。
    pub const fn is_combat_action(&self) -> bool {
        self.class.is_combat_action()
    }
}

/// 使用特技的行动权：**战斗行动**（规则书 S8.4 第 2 条）。
pub const fn technique_timing() -> Timing {
    Timing::Combat
}

/// **能否使用特技**（只读；成功 -> Ok，失败 -> 第一个命中的原因）。
///
/// 判定顺序（固定）：
/// 1. NotActiveCharacter：不是出战角色；
/// 2. NoTechniqueEquipped：装备栏没有特技槽；
/// 3. EquipmentOwnerMismatch：装备栏 owner 与 who 不一致（调用方给错区）；
/// 4. ForbiddenByStatus：冻结 / 石化 / 眩晕（S8.4 第 3 条）。
///
/// **不检查**：费用（规则书未写特技费用）、每回合次数（未规定）、目标合法性。
pub fn can_use_technique(context: &TechniqueContext<'_>) -> Result<(), CannotUseTechnique> {
    if !context.is_active {
        return Err(CannotUseTechnique::NotActiveCharacter);
    }
    if context.equipment.slot_entry(SlotKind::Technique).is_none() {
        return Err(CannotUseTechnique::NoTechniqueEquipped);
    }
    let equipment_owner = context.equipment.owner();
    if equipment_owner != context.who {
        return Err(CannotUseTechnique::EquipmentOwnerMismatch { equipment_owner });
    }
    if let Some(forbidden) = forbidden_by(&context.character.states) {
        return Err(CannotUseTechnique::ForbiddenByStatus { forbidden });
    }
    Ok(())
}

/// **使用特技**：判定通过后返回结果（不支付 / 不派发 / 不执行效果）。
///
/// 依据：S8.4 / K58。调用方拿到 TechniqueUse 后：
/// 1. 按战斗行动结算（Timing::Combat）；
/// 2. **不要**派发「使用技能时 / 后」（dispatch_technique 恒为空）；
/// 3. 效果造成的伤害按「非角色造成」处理。
pub fn use_technique(context: &TechniqueContext<'_>) -> Result<TechniqueUse, CannotUseTechnique> {
    can_use_technique(context)?;
    let entry = context
        .equipment
        .slot_entry(SlotKind::Technique)
        .expect("can_use_technique 已确认特技存在");
    Ok(TechniqueUse {
        who: context.who,
        equip: entry.equip,
        technique: entry.card,
        class: SkillUseClass::Technique,
        timing: technique_timing(),
    })
}

/// **特技的「使用技能时 / 后」派发**：恒为空（K58 第 4 条：特技不视为使用技能）。
///
/// 这个函数存在的意义是把「特技不派发 SkillUsed」变成**可测试的显式入口**：
/// 调用方若沿用了技能流程也必须走这里，得到空列表。
pub fn dispatch_technique(
    _triggers: &SkillTriggers,
    _who: EntityId,
    _target: Option<EntityId>,
) -> Vec<SkillPending> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::StateIdentity;
    use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
    use crate::families::character::skill_flow::forbidden::{PETRIFIED, STUN};
    use crate::families::character::zones::equipment::{
        EquipmentEntry, EquipmentTriggers, EquipmentZone,
    };
    use crate::families::status::known::FROZEN;
    use crate::rules::ops::equip::CharacterEquip;
    use crate::rules::ops::order::EffectSource;
    use crate::timeline::Event;
    use gi_genius_invokation_engine::Nat;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 装了一张特技的装备栏（owner = who）。
    fn with_technique(who: EntityId) -> EquipmentZone {
        EquipmentZone::new(who)
            .equip_with_triggers(
                EquipmentEntry {
                    equip: n(900),
                    card: CardId::new(321001),
                    slot: SlotKind::Technique,
                },
                None,
                &CharacterEquip::OTHER_WEAPON,
                &EquipmentTriggers::new(),
            )
            .zone
    }

    fn character() -> CharacterState {
        CharacterState::new(n(10), n(10), Nat::ZERO, n(2))
    }

    fn context<'a>(
        who: EntityId,
        character: &'a CharacterState,
        equipment: &'a EquipmentZone,
        is_active: bool,
    ) -> TechniqueContext<'a> {
        TechniqueContext {
            who,
            character,
            equipment,
            is_active,
        }
    }

    #[test]
    fn technique_needs_active_character_and_equipped_slot() {
        // S8.4 第 1 条：出战角色装备特技才能用。
        let state = character();
        let equipment = with_technique(n(1));
        assert_eq!(
            can_use_technique(&context(n(1), &state, &equipment, false)),
            Err(CannotUseTechnique::NotActiveCharacter)
        );
        // 装备栏是空的 -> 没有特技。
        let empty = EquipmentZone::new(n(1));
        assert_eq!(
            can_use_technique(&context(n(1), &state, &empty, true)),
            Err(CannotUseTechnique::NoTechniqueEquipped)
        );
        // 装备栏 owner 给错。
        let other_owner = with_technique(n(2));
        assert_eq!(
            can_use_technique(&context(n(1), &state, &other_owner, true)),
            Err(CannotUseTechnique::EquipmentOwnerMismatch {
                equipment_owner: n(2)
            })
        );
        // 正常：出战 + 装备特技。
        assert_eq!(can_use_technique(&context(n(1), &state, &equipment, true)), Ok(()));
    }

    #[test]
    fn technique_is_a_combat_action_but_not_a_skill() {
        // S8.4 第 2 条：特技是战斗行动；K58：不视为使用技能、伤害不算角色造成。
        let state = character();
        let equipment = with_technique(n(1));
        let used = use_technique(&context(n(1), &state, &equipment, true)).expect("可用");
        assert_eq!(used.class, SkillUseClass::Technique);
        assert_eq!(used.timing, Timing::Combat);
        assert!(used.is_combat_action());
        assert!(!used.damage_is_character_damage());
        assert!(!used.dispatches_skill_used());
        assert!(!used.gains_energy());
        assert!(!used.class.is_skill());
        assert_eq!(used.technique, CardId::new(321001));
        assert_eq!(used.equip, n(900));
        // 技能分类的并列属性。
        assert!(SkillUseClass::NormalAttack.is_skill());
        assert!(SkillUseClass::NormalAttack.dispatches_skill_used());
        assert!(SkillUseClass::NormalAttack.gains_energy());
        assert!(SkillUseClass::ElementalSkill.gains_energy());
        assert!(!SkillUseClass::ElementalBurst.gains_energy());
        assert!(!SkillUseClass::Passive.is_combat_action());
        // 并列判定点：特技走特技流程，其它技能种类走技能流程。
        assert_eq!(SkillUseClass::Technique.flow(), UseFlow::TechniqueFlow);
        assert_eq!(SkillUseClass::NormalAttack.flow(), UseFlow::SkillFlow);
        assert_eq!(SkillUseClass::ElementalSkill.flow(), UseFlow::SkillFlow);
        assert_eq!(SkillUseClass::ElementalBurst.flow(), UseFlow::SkillFlow);
        assert_eq!(SkillUseClass::Passive.flow(), UseFlow::SkillFlow);
        assert_eq!(SkillUseClass::TECHNIQUE, SkillUseClass::Technique);
    }

    #[test]
    fn forbidden_status_blocks_technique_too() {
        // S8.4 第 3 条：冻结 / 石化 / 眩晕时也无法使用特技。
        let equipment = with_technique(n(1));
        let forbidding: [&'static dyn StateIdentity; 3] = [&FROZEN, &STUN, &PETRIFIED];
        for identity in forbidding {
            let mut state = character();
            state.states.add(CHARACTER_STATE_POS, identity, None);
            let reason = can_use_technique(&context(n(1), &state, &equipment, true));
            assert!(matches!(
                reason,
                Err(CannotUseTechnique::ForbiddenByStatus { .. })
            ));
        }
        // 没有状态 -> 可用。
        let state = character();
        assert!(can_use_technique(&context(n(1), &state, &equipment, true)).is_ok());
    }

    #[test]
    fn technique_dispatches_no_skill_used_trigger() {
        // K58 第 4 条：特技不触发「使用技能时 / 后」——即使登记了任意技能触发器，派发也为空。
        fn noop(_state: &mut CharacterState, _event: &Event, _owner: EntityId) {}
        let mut triggers = SkillTriggers::new();
        triggers.register(crate::families::character::skill_flow::SkillTrigger::new(
            crate::families::character::skill_flow::BEFORE_ANY,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop,
        ));
        triggers.register(crate::families::character::skill_flow::SkillTrigger::new(
            crate::families::character::skill_flow::AFTER_ANY,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop,
        ));
        assert!(dispatch_technique(&triggers, n(1), Some(n(2))).is_empty());
    }
}
