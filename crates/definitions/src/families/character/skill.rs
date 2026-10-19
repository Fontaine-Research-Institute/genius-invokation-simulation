//! 技能定义载体（D121 / D133 / D162 迁移）。
//!
//! 本文件是 `SkillCost` / `SkillEffect` / `SkillDef` 的**正式归属**：
//! 它们原先临时放在 `cards/character/mod.rs`（D162 的 TODO(迁移, P0)），现在迁到这里；
//! 具体卡牌数据在 D240 起独立到 gi-genius-invokation-cards crate，那边 `use` 本文件的类型。
//!
//! 本文件只有**定义数据**，不含任何结算 / 使用流程（D133：角色块只做底层数值操作 + 基础类型，
//! 不做高层语义封装）：
//! - 技能种类复用 `types::SkillKind`（D121 / D123–D126）；
//! - 骰子费用格式复用 `resources::dice::CostComponent`（D142 / D161：不另造第二份）。

use crate::base::SkillId;
use crate::families::character::types::SkillKind;
use crate::generated::Localized;
use crate::resources::dice::CostComponent;
use gi_genius_invokation_engine::Nat;

/// 技能效果体（**占位签名**，D162）。
///
/// 现在还没有技能使用上下文：谁使用 / 目标怎么选 / 费用怎么付 / 在哪个阶段结算
/// 都没有接口，所以效果体一律无参 `fn()`，卡牌侧写 `todo!("缺：…")`。
///
/// TODO(P0)：技能使用流程块建好后，换成**带上下文**的签名（如 `fn(&mut SkillContext)`）；
/// 现在**不发明**上下文类型。
pub type SkillEffect = fn();

/// 技能消耗（D121 / D162）。
///
/// - `dice`：元素骰费用，复用 `resources::dice::CostComponent`（D142 / D161：
///   `SpecificElement(元素, n)` = 指定元素 n 个；`AnyColor(n)` = 任意 n 个；`SameColor(n)` = 同色 n 个）；
/// - `energy`：充能费用，`Nat::ZERO` = 不要充能。
///
/// 本类型**只描述「要花什么」**；支付 / 扣减在「被花的东西」那边（骰池、角色充能，D121 / D133）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SkillCost {
    /// 元素骰费用（按 CostComponent 逐项扣）。
    pub dice: &'static [CostComponent],
    /// 充能费用（`ZERO` = 不消耗充能）。
    pub energy: Nat,
}

impl SkillCost {
    /// 无任何消耗（被动技能用）。
    pub const FREE: SkillCost = SkillCost {
        dice: &[],
        energy: Nat::ZERO,
    };

    /// 只花元素骰。
    pub const fn from_dice(dice: &'static [CostComponent]) -> SkillCost {
        SkillCost {
            dice,
            energy: Nat::ZERO,
        }
    }

    /// 元素骰 + 充能。
    pub const fn from_dice_and_energy(dice: &'static [CostComponent], energy: Nat) -> SkillCost {
        SkillCost { dice, energy }
    }

    /// 是否完全不消耗（骰子与充能都为空）。
    pub fn is_free(self) -> bool {
        self.dice.is_empty() && self.energy == Nat::ZERO
    }
}

/// 一个技能的**定义**（D121 / D162）。
///
/// 只存「识别信息 + 文案 + 种类 + 消耗 + 效果函数」；
/// 技能种类复用 `types::SkillKind`（D121 / D123–D126）。
///
/// TODO(P3)：`description` 将来应从 `generated` 按（牌族, 卡 id, 技能 id）取，
/// 而不是把中文写进卡牌代码（结构.md §4.4 规则 6；当前 D120 / D162 样板例外）。
#[derive(Clone, Copy, Debug)]
pub struct SkillDef {
    /// 技能 id（角色 id × 10 + 序号，见 `base::id`）。
    pub id: SkillId,
    /// 技能名（样板只填中文，D120）。
    pub name: Localized,
    /// 技能完整中文文案（样板只填中文，D120）。
    pub description: Localized,
    /// 技能种类。
    pub kind: SkillKind,
    /// 消耗。
    pub cost: SkillCost,
    /// 效果体（占位签名，见 [SkillEffect]）。
    pub effect: SkillEffect,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;

    /// 测试用效果体（只证明类型可调用，不做任何事）。
    fn noop() {}

    /// 测试用骰子费用（静态数组：~from_dice~ 要求 ~&'static [CostComponent]~）。
    static ANY_TWO: [CostComponent; 1] = [CostComponent::AnyColor(Nat::new(2))];
    /// 测试用骰子费用：3 个水。
    static THREE_HYDRO: [CostComponent; 1] =
        [CostComponent::SpecificElement(Element::Hydro, Nat::new(3))];

    /// 测试用技能定义（结构与爱诺样板一致）。
    static SAMPLE: SkillDef = SkillDef {
        id: SkillId::new(12161),
        name: Localized::from_parts(Some("测试技能"), None, None, None),
        description: Localized::from_parts(Some("造成2点物理伤害。"), None, None, None),
        kind: SkillKind::normal_attack(),
        cost: SkillCost::from_dice_and_energy(
            &[CostComponent::SpecificElement(Element::Hydro, Nat::new(1))],
            Nat::new(2),
        ),
        effect: noop,
    };

    #[test]
    fn free_cost_has_no_dice_and_no_energy() {
        assert!(SkillCost::FREE.is_free());
        assert!(SkillCost::FREE.dice.is_empty());
        assert_eq!(SkillCost::FREE.energy, Nat::ZERO);
    }

    #[test]
    fn dice_only_cost_keeps_energy_zero() {
        let cost = SkillCost::from_dice(&ANY_TWO);
        assert_eq!(cost.dice, &[CostComponent::AnyColor(Nat::new(2))]);
        assert_eq!(cost.energy, Nat::ZERO);
        assert!(!cost.is_free());
    }

    #[test]
    fn dice_and_energy_cost_keeps_both_parts() {
        let cost = SkillCost::from_dice_and_energy(&THREE_HYDRO, Nat::new(2));
        assert_eq!(
            cost.dice,
            &[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))]
        );
        assert_eq!(cost.energy, Nat::new(2));
        assert!(!cost.is_free());
    }

    #[test]
    fn skill_def_carries_identity_kind_cost_and_effect() {
        assert_eq!(SAMPLE.id, SkillId::new(12161));
        assert_eq!(SAMPLE.kind, SkillKind::normal_attack());
        assert!(SAMPLE.kind.is_normal_attack());
        assert_eq!(SAMPLE.cost.energy, Nat::new(2));
        // 效果体是占位 fn()：可以被取用 / 调用（现在不做任何事）。
        let effect: SkillEffect = SAMPLE.effect;
        effect();
        // 文案只有中文（D120 / D162）。
        assert_eq!(
            SAMPLE.name.get(crate::generated::Lang::ZhCn),
            crate::generated::Resolved::Exact("测试技能")
        );
    }
}
