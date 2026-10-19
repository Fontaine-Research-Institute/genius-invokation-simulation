//! 元素共鸣 / 国家共鸣：**组牌门槛**（tags=元素共鸣；角色标签=元素 / 国家）。
//!
//! **依据**：
//! - 元素共鸣 15 张，14 张卡面括号都写「（牌组包含至少 2 个 <元素> 角色，才能加入牌组）」，
//!   如 `331602` 坚定之岩、`331301` 交织之火、`331101` 交织之冰、`331201` 交织之水、
//!   `331401` 交织之雷、`331501` 交织之风、`331701` 交织之草；
//! - 第 15 张 `331721` 月兆·满辉「（牌组包含至少 2 个『挪德卡莱』角色，才能加入牌组）」，
//!   所以组牌门槛的「按国家」也要支持。
//!
//! 这是**建局前**的组牌限制，不进入对局开局流程。

use crate::base::Element;
use gi_genius_invokation_engine::{nadd, Nat};

/// 元素共鸣：牌组至少 2 名同元素角色。
pub const MIN_SAME_ELEMENT_CHARACTERS: Nat = Nat::new(2);

/// 国家共鸣：牌组至少 2 名同国家角色（331721 月兆·满辉）。
pub const MIN_SAME_COUNTRY_CHARACTERS: Nat = Nat::new(2);

/// 统计队伍里某元素的角色数量。
pub fn characters_of_element(team: &[Element], element: Element) -> Nat {
    let mut total = Nat::ZERO;
    for candidate in team {
        if *candidate == element {
            total = nadd!(total, Nat::new(1));
        }
    }
    total
}

/// 元素共鸣组牌门槛：队伍里该元素角色数 >= 2。
pub fn element_resonance_allows(team: &[Element], element: Element) -> bool {
    MIN_SAME_ELEMENT_CHARACTERS <= characters_of_element(team, element)
}

/// 国家共鸣组牌门槛：同国家角色数 >= 2（国家标签只做查询条件，不建枚举）。
pub fn country_resonance_allows(same_country_count: Nat) -> bool {
    MIN_SAME_COUNTRY_CHARACTERS <= same_country_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_two_characters_of_the_element() {
        assert!(!element_resonance_allows(&[Element::Geo], Element::Geo));
        assert!(element_resonance_allows(
            &[Element::Geo, Element::Geo],
            Element::Geo
        ));
        assert!(element_resonance_allows(
            &[Element::Geo, Element::Geo, Element::Pyro],
            Element::Geo
        ));
        assert!(!element_resonance_allows(
            &[Element::Geo, Element::Cryo],
            Element::Geo
        ));
    }

    #[test]
    fn counts_matching_characters_only() {
        let team = [Element::Hydro, Element::Pyro, Element::Hydro];
        assert_eq!(characters_of_element(&team, Element::Hydro), Nat::new(2));
        assert_eq!(characters_of_element(&team, Element::Pyro), Nat::new(1));
        assert_eq!(characters_of_element(&team, Element::Geo), Nat::ZERO);
    }

    #[test]
    fn country_resonance_also_needs_two() {
        assert!(!country_resonance_allows(Nat::new(1)));
        assert!(country_resonance_allows(Nat::new(2)));
        assert!(country_resonance_allows(Nat::new(3)));
    }
}
