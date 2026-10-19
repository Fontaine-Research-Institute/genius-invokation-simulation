//! 护盾 / 减伤（tags=护盾 / 减伤；types=出战状态）。
//!
//! **权威依据（规则书·特殊词条）**：
//! - 护盾：「一些效果会提供『护盾』，来保护特定角色。『护盾』所保护的角色受到伤害时，
//!   将会消耗『护盾值』来抵消伤害。」English: "the Shield points will be deducted to block some DMG, if not all."
//! - 穿透伤害：「『穿透伤害』不会受到其他效果的伤害加成，但是也不会被护盾等效果抵消。」
//!   English: "Such DMG cannot be boosted by any effects, but they will not be blocked by any Shields either."
//! - 卡面：护盾 7 张（422021 等「提供 N 点护盾，保护我方出战角色」）；
//!   减伤 11 张（422001 丘丘人箭塔「受到的伤害 -2」、113041 兔兔伯爵「抵消 2 点伤害」）。
//!
//! Lead 裁定 8（待用户复核）：先护盾吸收、再减伤扣减；规则书没写先后与叠加 → 标 待实测。
//!
use crate::families::character::types::DamageType;
use gi_genius_invokation_engine::{nadd, nsub, Nat};

/// 一次伤害修正的结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mitigation {
    /// 最终扣到生命值的伤害。
    pub to_hp: Nat,
    /// 被护盾消耗掉的点数。
    pub shield_used: Nat,
}

/// 伤害修正（纯函数）：先护盾吸收，再减伤扣减；穿透伤害两者都不吃。
///
/// - `shield`：当前护盾点数（可叠加后的总量）；
/// - `reductions`：本次伤害适用的各条减伤数值（按调用方给定的顺序相加；顺序不影响总和）。
pub fn mitigate(damage: Nat, ty: DamageType, shield: Nat, reductions: &[Nat]) -> Mitigation {
    // 穿透：不参与任何抵消（关键字 #4）。
    if ty == DamageType::Piercing {
        return Mitigation {
            to_hp: damage,
            shield_used: Nat::ZERO,
        };
    }
    // 第 1 步：护盾吸收（最多吸到伤害为 0）。
    // 待实测：护盾与减伤的先后、多个减伤的叠加方式，卡文无法确定（Lead 裁定 8 暂按先盾后减）。
    let shield_used = if shield < damage { shield } else { damage };
    let after_shield = nsub!(damage, shield_used);
    // 第 2 步：减伤扣减（各条数值相加后一次性扣；不足则扣到 0）。
    let mut reduction_total = Nat::ZERO;
    for reduction in reductions {
        reduction_total = nadd!(reduction_total, *reduction);
    }
    Mitigation {
        to_hp: nsub!(after_shield, reduction_total),
        shield_used,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;

    #[test]
    fn shield_absorbs_then_reduction_subtracts() {
        // 5 点伤害：护盾 3 先吸 3 -> 剩 2；减伤 1 -> 扣血 1。
        let result = mitigate(
            Nat::new(5),
            DamageType::Elemental(Element::Pyro),
            Nat::new(3),
            &[Nat::new(1)],
        );
        assert_eq!(result.shield_used, Nat::new(3));
        assert_eq!(result.to_hp, Nat::new(1));
    }

    #[test]
    fn shield_larger_than_damage_fully_absorbs() {
        let result = mitigate(
            Nat::new(2),
            DamageType::Physical,
            Nat::new(6),
            &[],
        );
        assert_eq!(result.shield_used, Nat::new(2));
        assert_eq!(result.to_hp, Nat::ZERO);
    }

    #[test]
    fn reductions_never_make_negative_damage() {
        let result = mitigate(
            Nat::new(2),
            DamageType::Physical,
            Nat::ZERO,
            &[Nat::new(1), Nat::new(5)],
        );
        assert_eq!(result.to_hp, Nat::ZERO);
        assert_eq!(result.shield_used, Nat::ZERO);
    }

    #[test]
    fn piercing_ignores_shield_and_reduction() {
        // 兔兔伯爵（113041）式的「抵消」与丘丘人箭塔（422001）式的「-2」都不吃穿透。
        let result = mitigate(
            Nat::new(3),
            DamageType::Piercing,
            Nat::new(10),
            &[Nat::new(2)],
        );
        assert_eq!(result.to_hp, Nat::new(3));
        assert_eq!(result.shield_used, Nat::ZERO);
    }
}