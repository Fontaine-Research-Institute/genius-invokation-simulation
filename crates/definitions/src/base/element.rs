//! 元素：7 种。
//!
//! 顺序固定为：火 / 水 / 雷 / 草 / 冰 / 风 / 岩（与骰面、伤害类型的前 7 种一致）。
//! 本枚举**没有编号 / 优先级语义**；需要数值映射的地方（如伤害类型）各自定义，
//! 不在 base 里给 Element 加编号。

/// 元素（7 种）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Element {
    /// 火。
    Pyro,
    /// 水。
    Hydro,
    /// 雷。
    Electro,
    /// 草。
    Dendro,
    /// 冰。
    Cryo,
    /// 风。
    Anemo,
    /// 岩。
    Geo,
}

/// 全部 7 种元素，顺序固定（只用于遍历，不表示任何强度 / 优先级）。
pub const ELEMENTS: [Element; 7] = [
    Element::Pyro,
    Element::Hydro,
    Element::Electro,
    Element::Dendro,
    Element::Cryo,
    Element::Anemo,
    Element::Geo,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// 穷尽 match：编译器保证这里覆盖 Element 的每一个变体（漏一个就编译失败）。
    fn canonical_name(element: Element) -> &'static str {
        match element {
            Element::Pyro => "Pyro",
            Element::Hydro => "Hydro",
            Element::Electro => "Electro",
            Element::Dendro => "Dendro",
            Element::Cryo => "Cryo",
            Element::Anemo => "Anemo",
            Element::Geo => "Geo",
        }
    }

    #[test]
    fn element_enum_is_complete_and_ordered() {
        // 7 个、顺序固定
        assert_eq!(
            ELEMENTS,
            [
                Element::Pyro,
                Element::Hydro,
                Element::Electro,
                Element::Dendro,
                Element::Cryo,
                Element::Anemo,
                Element::Geo,
            ]
        );
    }

    #[test]
    fn elements_are_all_distinct() {
        // 7 个元素互不相同：去重后个数不变（有重复变体混进来就会变小）
        let mut names: Vec<&'static str> = ELEMENTS
            .iter()
            .map(|element| canonical_name(*element))
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ELEMENTS.len());
    }
}
