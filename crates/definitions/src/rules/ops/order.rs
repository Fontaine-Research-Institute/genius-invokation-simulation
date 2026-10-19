//! 结算顺序（规则书·其他规则·结算顺序）。
//!
//! **权威原文（中文）**：
//! 1. 先结算当前牌手的所有卡牌效果，再结算对方牌手的卡牌效果。
//! 2. 同属于同一位牌手的卡牌效果，按以下顺序结算：角色牌、角色装备和状态、阵营出战状态、召唤物、支援牌。
//! 3. 同属于同一位牌手的同类型卡牌效果，按进场的先后顺序结算。
//!
//! **English（消除中文歧义用）**：Calculation Order ——
//! "Character Cards, Character Equipment and Statuses, Team Combat Status, Summons, and Support Cards"；
//! "Card effects of the same type belonging to the same player will be resolved via the order of their entry."
//!
//! 本模块只给「谁先谁后」的纯判定；真正的时间线调度归后续 timeline 块（// 待接入）。

/// 效果来源类别（规则书第 2 条的 5 类；数组顺序**就是**结算顺序）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectSource {
    /// 角色牌（角色技能 / 角色牌本身的效果）。
    CharacterCard,
    /// 角色装备和状态。
    CharacterEquipmentOrStatus,
    /// 阵营出战状态（英文：Team Combat Status）。
    CombatStatus,
    /// 召唤物。
    Summon,
    /// 支援牌。
    Support,
}

/// 同一牌手内、不同来源类别的结算顺序（规则书第 2 条）。
pub const SOURCE_ORDER: [EffectSource; 5] = [
    EffectSource::CharacterCard,
    EffectSource::CharacterEquipmentOrStatus,
    EffectSource::CombatStatus,
    EffectSource::Summon,
    EffectSource::Support,
];

/// 同一时间点、同一牌手内：first 是否先于 second 结算（规则书第 2 条）。
///
/// 同一类别 -> false（同类按进场先后，规则书第 3 条，见 same_source_uses_entry_order）。
pub fn resolves_before(first: EffectSource, second: EffectSource) -> bool {
    // 同一类别不比先后（同类按进场先后，规则书第 3 条）。
    if first == second {
        return false;
    }
    let mut seen_first = false;
    for source in SOURCE_ORDER {
        if source == first {
            if seen_first {
                return false;
            }
            seen_first = true;
        }
        if source == second {
            return seen_first;
        }
    }
    false
}

/// 同一牌手、同一类别：按**进场先后**结算（规则书第 3 条；进场顺序由区域状态持有）。
pub fn same_source_uses_entry_order() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_sources_are_in_rulebook_order() {
        assert_eq!(
            SOURCE_ORDER,
            [
                EffectSource::CharacterCard,
                EffectSource::CharacterEquipmentOrStatus,
                EffectSource::CombatStatus,
                EffectSource::Summon,
                EffectSource::Support,
            ]
        );
        // 相邻类别：前者先于后者
        for pair in SOURCE_ORDER.windows(2) {
            assert!(resolves_before(pair[0], pair[1]));
            assert!(!resolves_before(pair[1], pair[0]));
        }
        // 首尾
        assert!(resolves_before(
            EffectSource::CharacterCard,
            EffectSource::Support
        ));
        assert!(!resolves_before(
            EffectSource::Support,
            EffectSource::CharacterCard
        ));
        // 同类不比先后
        assert!(!resolves_before(EffectSource::Summon, EffectSource::Summon));
        assert!(same_source_uses_entry_order());
    }
}
