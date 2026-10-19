//! 查询策略：不可发现 / 弃用（tags=不可发现 / 弃用）。
//!
//! **依据**：
//! - 分工计划 §4.4.15：「不进入检索池的三类：不可发现（179 张）、弃用（58 张）、自走牌（78 张）」；
//!   「不可发现」= 检索类效果（如随机生成几张圣遗物）不允许发现这些卡；
//! - 已实现于 `families/action_card/zones/query.rs`：两个过滤都没给时才应用排除规则，
//!   显式查询这三个标签时仍能查到（D101）。
//!
//! 本模块只把同一条策略**声明成数据**（供静态表与一致性测试使用），不重复实现过滤。
//! 自走牌（`AutomatonCard`）：本轮**不实现、不建规则行**（任务排除项），但它的「不可发现」
//! 过滤仍由 query.rs 负责。

/// 一条查询策略。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QueryPolicy {
    /// 两个过滤都没给时，是否出现在检索结果里。
    pub discoverable: bool,
    /// 显式按该标签查询时，是否仍能查到（D101：能）。
    pub explicitly_queryable: bool,
}

/// 「不可发现」：无过滤检索不出现；显式查询能查到。
pub const UNDISCOVERABLE: QueryPolicy = QueryPolicy {
    discoverable: false,
    explicitly_queryable: true,
};

/// 「弃用」：不进检索池；显式查询能查到。
pub const SCRAPPED: QueryPolicy = QueryPolicy {
    discoverable: false,
    explicitly_queryable: true,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::zones::query::{card_matches, is_discoverable, CardMeta};
    use crate::families::action_card::ActionCardType;
    use crate::base::CardId;
    use crate::generated::TagText;

    /// 测试用元数据：一张牌的类型 + 标签集合（标签必须是 `data()` 的静态指针）。
    struct TestMeta {
        tags: &'static [&'static TagText],
    }

    impl CardMeta for TestMeta {
        fn ty(&self, _card: CardId) -> Option<ActionCardType> {
            Some(ActionCardType::Event)
        }

        fn tags(&self, _card: CardId) -> &'static [&'static TagText] {
            self.tags
        }
    }

    /// 与既有实现保持同一口径：带「不可发现」/「弃用」标签的卡不可被无过滤检索发现，
    /// 但显式按标签查询仍能查到。
    #[test]
    fn policy_matches_query_implementation() {
        // 标签集合必须是 'static（D57）；测试里用 Box::leak 造一份 'static 切片。
        let undiscoverable_only: &'static [&'static TagText] =
            Box::leak(Box::new([crate::tags::action_card::common::undiscoverable::data()]));
        let meta = TestMeta {
            tags: undiscoverable_only,
        };
        let card = CardId::new(331004);
        assert_eq!(is_discoverable(&meta, card), UNDISCOVERABLE.discoverable);
        // 显式查「不可发现」标签能查到（D101）
        assert!(card_matches(
            &meta,
            card,
            None,
            Some(crate::tags::action_card::common::undiscoverable::data())
        ));
        // 两个过滤都不给 -> 排除
        assert!(!card_matches(&meta, card, None, None));

        let scrapped_only: &'static [&'static TagText] =
            Box::leak(Box::new([crate::tags::action_card::common::scrapped::data()]));
        let scrapped = TestMeta {
            tags: scrapped_only,
        };
        assert_eq!(is_discoverable(&scrapped, card), SCRAPPED.discoverable);
        assert!(card_matches(
            &scrapped,
            card,
            None,
            Some(crate::tags::action_card::common::scrapped::data())
        ));
        let policies = [UNDISCOVERABLE, SCRAPPED];
        assert!(policies.iter().all(|policy| policy.explicitly_queryable));
    }
}
