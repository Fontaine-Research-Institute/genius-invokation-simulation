//! L1 牌族（顶层，也是 **id 区域**）。
//!
//! 规则（分工计划.md §4.4.0.0）：
//! 1. 一个卡牌 = 一个牌族 + 一个类型 + 若干标签；
//! 2. **牌族决定 id 区域**：不同牌族的 id 不保证不重复，引用一张卡必须带牌族；
//! 3. 上下文已确定牌族时，不填牌族（例：牌库 / 手牌里的必然是行动牌）。
//!
//! 牌族的**类型**由牌族自己定义（families 层，不在 base）：行动牌 ActionCardType、
//! 状态 StatusType；其余牌族没有类型。
//!
//! 标签**不在**类型 / 牌族里定义：它们是**独立顶层目录 tags**（与 families 同级）里的
//! **零大小类型** —— 每个标签一个文件、一个类型，既不是枚举，也不是数字 / 字符串
//! （决策日志 D53 / D54）。
//!
//! 本枚举**没有编号 / 顺序语义**（不是数字编码，不参与 id 运算）。

/// L1 牌族（id 区域）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CardFamily {
    /// 行动牌（有类型：支援 / 装备 / 事件）。
    ActionCard,
    /// 召唤物（无类型）。
    Summon,
    /// 角色牌（无类型）。
    Character,
    /// 敌人牌（无类型）。
    Enemy,
    /// 状态（无类型；角色状态 / 出战状态在牌族自己的 StatusType 里区分）。
    Status,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 穷尽 match：编译器保证这里覆盖每一个变体（漏一个就编译失败）。
    fn canonical_name(family: CardFamily) -> &'static str {
        match family {
            CardFamily::ActionCard => "ActionCard",
            CardFamily::Summon => "Summon",
            CardFamily::Character => "Character",
            CardFamily::Enemy => "Enemy",
            CardFamily::Status => "Status",
        }
    }

    const ALL_FAMILIES: [CardFamily; 5] = [
        CardFamily::ActionCard,
        CardFamily::Summon,
        CardFamily::Character,
        CardFamily::Enemy,
        CardFamily::Status,
    ];

    #[test]
    fn card_family_has_all_five_id_regions() {
        // 五个牌族齐全：逐个点名核对（漏项由穷尽 match 兜住）
        assert_eq!(ALL_FAMILIES.len(), 5);
        for expected in ALL_FAMILIES {
            assert!(ALL_FAMILIES.contains(&expected));
        }

        // 五个牌族互不相同（没有重复变体）
        let mut names: Vec<&'static str> = ALL_FAMILIES
            .iter()
            .map(|family| canonical_name(*family))
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL_FAMILIES.len());
    }

    #[test]
    fn card_family_is_not_a_numeric_encoding() {
        // 牌族可比较 / 可判等 / 可作集合键，但"大小"不表示任何游戏语义。
        assert_ne!(CardFamily::ActionCard, CardFamily::Character);
        assert_eq!(CardFamily::Status, CardFamily::Status);

        let mut regions: std::collections::HashSet<CardFamily> =
            std::collections::HashSet::new();
        for family in ALL_FAMILIES {
            regions.insert(family);
        }
        assert_eq!(regions.len(), ALL_FAMILIES.len());
    }
}
