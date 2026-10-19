//! 角色充能的**数值修正接口**（Lead 拍板：**不做「充能变动」时机键**，只给数值）。
//!
//! 用户检索结论：「充能变动 / 充能改变 / 充能变化」在全部卡面**命中 0**；卡面里出现的是
//! 「获得 1 点充能」（数值操作）与「没有充能的角色获得充能时，获得量 +1」
//! （114103 灯中幽精 / 214101 酌盈剂虚）这类**数值修正**。
//!
//! 因此本文件只做**「获得充能量」的数值叠加**，不做派发（没有 `EnergyChanged` 时机键）：
//! - 基础获得量（如「获得 1 点充能」的 1）先叠加各条修正；
//! - 结果交给 `CharacterState::energy_add`（饱和到充能上限 —— `character.rs` 语义不变）。
//!
//! 条件（例如「当前充能为 0」）由卡牌效果自己判断，不在本层判断。
//! 依据：规则书·进行行动 使用技能「角色每次使用『普通攻击』或『元素战技』后，会获得 1 点『充能』」；
//! 卡面 114103 / 214101 的「获得量 +1」修正。

use gi_genius_invokation_engine::{nadd, Nat};

/// 叠加「获得充能」的全部修正，得到实际交给 `CharacterState::energy_add` 的获得量。
///
/// - `base`：基础获得量（如「获得 1 点充能」的 1）；
/// - `modifiers`：各条「获得量 +N」修正（顺序不影响结果；空切片 = 不修正）。
///
/// 只做数值：**不判断条件**（如「当前没有充能」由卡牌效果自己判）、**不钳上限**
/// （上限由 `CharacterState::energy_add` 管）、不做任何派发。
pub fn energy_gain(base: Nat, modifiers: &[Nat]) -> Nat {
    let mut total = base;
    for modifier in modifiers {
        total = nadd!(total, *modifier);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::character::CharacterState;

    /// 建角色状态（当前生命 / 生命上限 / 当前充能 / 充能上限）。
    fn character(energy: Nat, max_energy: Nat) -> CharacterState {
        CharacterState::new(Nat::new(5), Nat::new(5), energy, max_energy)
    }

    #[test]
    fn gain_without_modifiers_is_the_base() {
        assert_eq!(energy_gain(Nat::new(1), &[]), Nat::new(1));
    }

    #[test]
    fn gain_modifiers_add_up_in_any_order() {
        assert_eq!(energy_gain(Nat::new(1), &[Nat::new(1)]), Nat::new(2));
        assert_eq!(
            energy_gain(Nat::new(1), &[Nat::new(1), Nat::new(2)]),
            Nat::new(4)
        );
        assert_eq!(
            energy_gain(Nat::new(1), &[Nat::new(2), Nat::new(1)]),
            Nat::new(4)
        );
    }

    #[test]
    fn lamp_spirit_style_bonus_then_energy_add_caps_at_max() {
        // 灯中幽精式：角色当前没有充能，获得 1 点充能时获得量 +1 -> 实际 2 点。
        // 条件（当前充能为 0）由卡牌效果自己判断。
        let empty = character(Nat::ZERO, Nat::new(3));
        let bonus = if empty.energy() == Nat::ZERO {
            Nat::new(1)
        } else {
            Nat::ZERO
        };
        let gain = energy_gain(Nat::new(1), &[bonus]);
        assert_eq!(gain, Nat::new(2));
        assert_eq!(empty.energy_add(gain).energy(), Nat::new(2));

        // 充能上限仍由 energy_add 钳住（本层只做叠加，不钳上限）。
        let near_full = character(Nat::new(2), Nat::new(3));
        let gain = energy_gain(Nat::new(1), &[Nat::new(5)]);
        assert_eq!(near_full.energy_add(gain).energy(), Nat::new(3));
    }
}
