//! 角色牌的基础类型（D121 / D123–D126 / D133）。
//!
//! 本文件**只有类型**：技能种类（含普通攻击的形态）、普通攻击的特殊形态、伤害类型。
//! 没有任何操作、没有具体技能 / 卡牌定义 —— D133：角色块只做底层数值操作 + 基础类型，
//! 不做高层语义封装。
//!
//! 依据：
//! - D121：角色通用层清单（技能类型 4 种；后续被 D133 / D138 收窄）；
//! - D123 / D126：重击 / 下落攻击是**普通攻击的特殊枚举值**，不建独立技能种类；
//! - D124：重击 / 下落攻击仍属普通攻击类（对普通攻击生效的效果同样生效）；
//! - D125：普通攻击的形态默认（空）就是「普通攻击」；
//! - D133：只做底层类型，不做语义封装。

use crate::base::Element;

/// 技能的 4 个种类（D121）。
///
/// 普通攻击是「一类」而不是单个值：它的**形态**由 `SpecialNormalAttack` 表达
/// （D123 / D124 / D125 / D126）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SkillKind {
    /// 普通攻击。
    ///
    /// - `None` = 普通攻击本体（默认形态，D125）；
    /// - `Some(形态)` = 特殊普通攻击（重击 / 下落攻击，D126）。
    ///
    /// 两种都属于普通攻击类：对普通攻击生效的效果对特殊形态同样生效（D124），
    /// 判断一律用 `SkillKind::is_normal_attack`。
    NormalAttack(Option<SpecialNormalAttack>),
    /// 元素战技。
    ElementalSkill,
    /// 元素爆发。
    ElementalBurst,
    /// 被动技能。
    Passive,
}

/// 普通攻击的**特殊形态**（D123 / D126）。
///
/// 重击 / 下落攻击**不是**独立的技能种类（D123），只是普通攻击的一种形态，
/// 所以没有 `SkillKind::ChargedAttack` 这类变体；它们仍然属于普通攻击类（D124）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SpecialNormalAttack {
    /// 重击。
    Charged,
    /// 下落攻击。
    Plunging,
}

impl SkillKind {
    /// 普通攻击的**默认形态**：`NormalAttack(None)`（D125）。
    pub const fn normal_attack() -> SkillKind {
        SkillKind::NormalAttack(None)
    }

    /// 重击：普通攻击 + 特殊形态（D123 / D126）。
    pub const fn charged_attack() -> SkillKind {
        SkillKind::NormalAttack(Some(SpecialNormalAttack::Charged))
    }

    /// 下落攻击：普通攻击 + 特殊形态（D123 / D126）。
    pub const fn plunging_attack() -> SkillKind {
        SkillKind::NormalAttack(Some(SpecialNormalAttack::Plunging))
    }

    /// 是不是**普通攻击类**（含重击 / 下落攻击；D124）。
    ///
    /// 判断「对普通攻击生效的效果」时用这个，**不要**写成
    /// `kind == SkillKind::NormalAttack(None)` —— 那样会把重击 / 下落攻击漏掉（D124）。
    pub const fn is_normal_attack(self) -> bool {
        matches!(self, SkillKind::NormalAttack(_))
    }

    /// 普通攻击的形态：不是普通攻击 -> `None`；
    /// 是普通攻击 -> `Some(None)`（本体）或 `Some(Some(形态))`（特殊形态）。
    ///
    /// 外层 `Option` 区分「是不是普通攻击」，内层区分「特殊形态 / 默认形态」。
    pub const fn special_normal_attack(self) -> Option<Option<SpecialNormalAttack>> {
        match self {
            SkillKind::NormalAttack(form) => Some(form),
            SkillKind::ElementalSkill | SkillKind::ElementalBurst | SkillKind::Passive => None,
        }
    }
}

/// 伤害类型（D121）：元素 / 物理 / 穿透。
///
/// 本文件只标类型，**不做任何结算**（D133）：加减乘除、减免、穿透不吃减免等
/// 由伤害结算侧处理。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DamageType {
    /// 元素伤害：带具体元素（伤害的元素属性）。
    Elemental(Element),
    /// 物理伤害。
    Physical,
    /// 穿透伤害。
    Piercing,
}

impl DamageType {
    /// 元素伤害（带具体元素）。
    pub const fn elemental(element: Element) -> DamageType {
        DamageType::Elemental(element)
    }

    /// 元素伤害的元素；不是元素伤害 -> `None`。
    pub const fn element(self) -> Option<Element> {
        match self {
            DamageType::Elemental(element) => Some(element),
            DamageType::Physical | DamageType::Piercing => None,
        }
    }

    /// 是不是元素伤害。
    pub const fn is_elemental(self) -> bool {
        matches!(self, DamageType::Elemental(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::ELEMENTS;

    /// 穷尽 match：编译器保证覆盖 `SkillKind` 的每一个变体（含普通攻击的三种形态）。
    fn skill_kind_name(kind: SkillKind) -> &'static str {
        match kind {
            SkillKind::NormalAttack(None) => "NormalAttack",
            SkillKind::NormalAttack(Some(SpecialNormalAttack::Charged)) => "ChargedAttack",
            SkillKind::NormalAttack(Some(SpecialNormalAttack::Plunging)) => "PlungingAttack",
            SkillKind::ElementalSkill => "ElementalSkill",
            SkillKind::ElementalBurst => "ElementalBurst",
            SkillKind::Passive => "Passive",
        }
    }

    /// 穷尽 match：编译器保证覆盖 `DamageType` 的每一个变体。
    fn damage_type_name(ty: DamageType) -> &'static str {
        match ty {
            DamageType::Elemental(_) => "Elemental",
            DamageType::Physical => "Physical",
            DamageType::Piercing => "Piercing",
        }
    }

    #[test]
    fn default_normal_attack_has_empty_form() {
        // D125：默认（空）就是普通攻击。
        assert_eq!(SkillKind::normal_attack(), SkillKind::NormalAttack(None));
        assert_eq!(
            SkillKind::normal_attack().special_normal_attack(),
            Some(None)
        );
        assert!(SkillKind::normal_attack().is_normal_attack());
    }

    #[test]
    fn special_forms_still_count_as_normal_attack() {
        // D124：重击 / 下落攻击仍属普通攻击类。
        for kind in [SkillKind::charged_attack(), SkillKind::plunging_attack()] {
            assert!(kind.is_normal_attack(), "特殊形态必须仍算普通攻击（D124）");
        }
        // 其它三种技能种类都不算普通攻击。
        for kind in [
            SkillKind::ElementalSkill,
            SkillKind::ElementalBurst,
            SkillKind::Passive,
        ] {
            assert!(!kind.is_normal_attack());
            assert_eq!(kind.special_normal_attack(), None);
        }
    }

    #[test]
    fn special_normal_attack_forms_are_distinct() {
        // D123 / D126：两种形态是两个不同的枚举值，不是独立技能种类。
        assert_ne!(
            SkillKind::charged_attack(),
            SkillKind::plunging_attack(),
            "重击与下落攻击必须可区分"
        );
        assert_eq!(
            SkillKind::charged_attack().special_normal_attack(),
            Some(Some(SpecialNormalAttack::Charged))
        );
        assert_eq!(
            SkillKind::plunging_attack().special_normal_attack(),
            Some(Some(SpecialNormalAttack::Plunging))
        );
        assert_ne!(SpecialNormalAttack::Charged, SpecialNormalAttack::Plunging);
    }

    #[test]
    fn four_skill_kinds_are_covered() {
        // D121：技能类型 4 种；普通攻击的两个特殊形态属于同一种类（不新增种类）。
        let kinds = [
            SkillKind::normal_attack(),
            SkillKind::charged_attack(),
            SkillKind::plunging_attack(),
            SkillKind::ElementalSkill,
            SkillKind::ElementalBurst,
            SkillKind::Passive,
        ];
        let names: Vec<&'static str> = kinds.iter().copied().map(skill_kind_name).collect();
        assert_eq!(
            names,
            [
                "NormalAttack",
                "ChargedAttack",
                "PlungingAttack",
                "ElementalSkill",
                "ElementalBurst",
                "Passive",
            ]
        );
        // 4 个「种类」：普通攻击（3 种形态同属一类）/ 战技 / 爆发 / 被动。
        assert_ne!(SkillKind::ElementalSkill, SkillKind::ElementalBurst);
        assert_ne!(SkillKind::ElementalBurst, SkillKind::Passive);
    }

    #[test]
    fn elemental_damage_carries_every_element() {
        // 7 种元素都能作为元素伤害的元素属性（顺序 / 编号不表示语义）。
        for element in ELEMENTS {
            let ty = DamageType::elemental(element);
            assert!(ty.is_elemental());
            assert_eq!(ty.element(), Some(element));
            assert_eq!(damage_type_name(ty), "Elemental");
        }
    }

    #[test]
    fn physical_and_piercing_are_not_elemental() {
        for ty in [DamageType::Physical, DamageType::Piercing] {
            assert!(!ty.is_elemental());
            assert_eq!(ty.element(), None);
            assert_ne!(ty, DamageType::elemental(Element::Pyro));
        }
        assert_eq!(damage_type_name(DamageType::Physical), "Physical");
        assert_eq!(damage_type_name(DamageType::Piercing), "Piercing");
        // 物理与穿透是两个不同的类型（穿透不吃减免属于结算语义，本块只标类型）。
        assert_ne!(DamageType::Physical, DamageType::Piercing);
    }
}
