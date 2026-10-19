//! 【行动牌】查询块：按**类型 / 标签**筛选手牌、牌库、初始牌组。
//!
//! 规格：分工计划.md §4.4（查询统一成**一个函数 + 两个可选参数**；牌库查询**按段分组**）、
//! 结构.md §4.3（`zones/query.rs`）；决策：D20（框架只给接口，具体数据谁要谁提供）、
//! D26（各块自己的解析放自己的 parse）、D42（区 = 状态里的字段）、
//! D57/D67（标签的运行期身份 = 指向静态元数据的指针，不建编号表、不用字符串）、
//! D101（本块六问裁决：注入 trait / 传入列表 / 排除只在「两个过滤都没给」时生效 /
//! `PileQuery` 不设 pools 总表 / 手牌返回实体号 / 自由函数）、
//! D102（布尔状态只存位：与查询无关，仅记录口径）。
//!
//! **查询不认识具体卡牌**（D20）：一张卡是什么类型、有哪些标签，由调用方实现
//! [`CardMeta`] 注入；本文件只按注入结果筛。标签按**指针身份**比较（D57/D67），
//! 不建「标签 → 编号」表，也不用字符串 / 数字。
//!
//! **三个范围**（返回值形状不同）：
//! - [`cards_in_initial_deck`]：`Vec<CardId>`（初始卡组列表；重复 = 实际多张；**无序**）；
//! - [`cards_in_hand`]：`Vec<EntityId>`（手牌每个**实例**一条；重复 = 实际多张；**无序**）；
//! - [`cards_in_pile`]：[`PileQuery`]（**按段分组，绝不返回扁平列表**；段从上面到下面，
//!   与 `PileState.segments()` **逐项一一对应**，含空段）。
//!
//! **过滤规则**（三个查询一致）：
//! 1. `ty = Some(t)`：只保留类型为 `t` 的卡；
//! 2. `tag = Some(g)`：只保留标签集合**含** `g`（指针相同）的卡；
//! 3. 两个过滤**都没给**：返回全部，但自动排除「不让查询」的三类 ——
//!    不可发现 / 弃用 / 自走牌（见 [`is_discoverable`]）；
//! 4. 只要给了任一个过滤，就**不再**额外排除：显式查 `tag = 不可发现` 时能查到它们。
//!
//! **纯函数**：所有函数吃 `&状态`、读元数据注入、返回新 `Vec`，不改任何状态、不 panic。
//! 数值一律用 `Nat`；遍历用迭代，不写下标取数据；不用 HashMap（遍历序必须稳定）。

use crate::base::CardId;
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::pile::{PileState, Segment};
use crate::families::action_card::ActionCardType;
use crate::generated::TagText;
use crate::tags::action_card::common::{automaton_card, scrapped, undiscoverable};
use gi_genius_invokation_engine::{nadd, EntityId, Nat};

// ======================================================================
// 元数据注入（D20 / D101）
// ======================================================================

/// 查询需要的**卡牌元数据接口**（调用方实现，框架不建注册表；D20 / D101）。
///
/// 查询块只认类型与标签，不问「这张卡叫什么、有什么效果」；具体卡牌的类型 / 标签集合
/// 将来由卡牌注册表或建局流程提供，这里只接一个注入点。
///
/// **标签集合的形态**：`&'static [&'static TagText]` —— 每个元素都是某个标签的
/// **静态元数据指针**（`crate::tags::…::data()` 的返回值，D57/D67）；比较一律
/// 按指针身份（`std::ptr::addr_eq`），不看名字、不比字符串。
///
/// **元数据缺失的卡**：`ty` 返回 `None`、`tags` 返回空切片即表示「未知」——
/// 指定类型 / 标签的查询不会命中它；两个过滤都没给时按**可发现**处理（它没有禁止标签）。
pub trait CardMeta {
    /// 这张行动牌的类型（未知 -> `None`）。
    fn ty(&self, card: CardId) -> Option<ActionCardType>;

    /// 这张行动牌的标签集合（按静态元数据指针给出；未知 -> 空切片）。
    fn tags(&self, card: CardId) -> &'static [&'static TagText];
}

// ======================================================================
// 过滤（三个查询共用同一套判定）
// ======================================================================

/// 一张卡是否命中过滤条件（**三个查询共用**，含排除规则）。
///
/// 返回 `true` 表示「这张卡要出现在查询结果里」：
/// - 给了 `ty` 就要求类型相等；给了 `tag` 就要求标签集合含这个标签（指针身份）；
/// - **两个都没给**时，还要求这张卡**可被发现**（[`is_discoverable`]）；
/// - 只给一个过滤时**不做**排除（D101：显式查不可发现标签能查到）。
///
/// 也适合直接传给牌库的抓牌谓词（如 `PileState::draw_top` 的 `&dyn Fn(CardId) -> bool`）。
pub fn card_matches(
    meta: &dyn CardMeta,
    card: CardId,
    ty: Option<ActionCardType>,
    tag: Option<&'static TagText>,
) -> bool {
    if let Some(wanted) = ty {
        if meta.ty(card) != Some(wanted) {
            return false;
        }
    }
    if let Some(wanted) = tag {
        if !card_has_tag(meta, card, wanted) {
            return false;
        }
    }
    if ty.is_none() && tag.is_none() {
        return is_discoverable(meta, card);
    }
    true
}

/// 这张卡的标签集合里**有没有**这个标签（指针身份；D57/D67）。
///
/// 只比较静态元数据地址，不比较名字、不做字符串 / 数字编号。
pub fn card_has_tag(meta: &dyn CardMeta, card: CardId, tag: &'static TagText) -> bool {
    meta.tags(card)
        .iter()
        .any(|candidate| std::ptr::addr_eq(*candidate, tag))
}

/// 这张卡**是否可被查询发现**（不进检索池的三类返回 `false`）。
///
/// 排除集合 = 三个标签的**并集**（D101）：
/// - 不可发现：`tags::action_card::common::undiscoverable`；
/// - 弃用：`tags::action_card::common::scrapped`；
/// - 自走牌：`tags::action_card::common::automaton_card`。
///
/// 标签按指针身份比较；某张卡同时带几个也只是一个 `false`。
/// 注意：排除**只在两个过滤都没给**时由 [`card_matches`] 应用；
/// 显式查这三个标签之一时仍能查到它们（D101）。
pub fn is_discoverable(meta: &dyn CardMeta, card: CardId) -> bool {
    !meta
        .tags(card)
        .iter()
        .any(|candidate| is_query_forbidden(candidate))
}

/// 这个标签是不是「不让查询」的三类之一（并集；D101）。
fn is_query_forbidden(tag: &'static TagText) -> bool {
    std::ptr::addr_eq(tag, undiscoverable::data())
        || std::ptr::addr_eq(tag, scrapped::data())
        || std::ptr::addr_eq(tag, automaton_card::data())
}

// ======================================================================
// 三个范围
// ======================================================================

/// **初始牌组**里：类型 = `ty`（`None` = 不限）、标签含 `tag`（`None` = 不限）的卡。
///
/// - 返回 **`Vec<CardId>`**：同一张卡在初始卡组里有几张就出现几次（重复 = 实际多张）；
/// - **无序**：不承诺顺序，调用方不要依赖（当前实现按传入列表顺序给出）；
/// - 空结果 = 传入列表为空，或没有任何卡命中过滤（返回空 `Vec`，不 panic）；
/// - 两个过滤都没给时应用排除规则（不可发现 / 弃用 / 自走牌）。
///
/// 初始卡组列表由调用方传入（建局流程还没实现；D101）。它可能 **30 张里带重复**，
/// 所以这里逐项判断，不去重。
pub fn cards_in_initial_deck(
    deck: &[CardId],
    meta: &dyn CardMeta,
    ty: Option<ActionCardType>,
    tag: Option<&'static TagText>,
) -> Vec<CardId> {
    deck.iter()
        .copied()
        .filter(|card| card_matches(meta, *card, ty, tag))
        .collect()
}

/// **手牌**里：类型 = `ty`（`None` = 不限）、标签含 `tag`（`None` = 不限）的**卡牌实例**。
///
/// - 返回 **`Vec<EntityId>`**：手牌是多实例容器，每个**实例**一条；
///   两张同卡 id 的不同实体都会各自出现（重复 = 实际多张）；
/// - **无序**：手牌本身不承诺顺序（手牌 `entries` 的顺序没有规则含义）；
/// - 空结果 = 空手牌，或没有实例命中（返回空 `Vec`，不 panic）；
/// - 两个过滤都没给时应用排除规则。
pub fn cards_in_hand(
    hand: &HandState,
    meta: &dyn CardMeta,
    ty: Option<ActionCardType>,
    tag: Option<&'static TagText>,
) -> Vec<EntityId> {
    hand.entries()
        .iter()
        .filter(|instance| card_matches(meta, instance.card, ty, tag))
        .map(|instance| instance.entity)
        .collect()
}

/// **牌库**里：类型 = `ty`（`None` = 不限）、标签含 `tag`（`None` = 不限）的卡 ——
/// **按段分组返回**，绝不返回扁平列表。
///
/// - 返回 [`PileQuery`]：`segments` 与 `PileState::segments()` **逐项一一对应**
///   （含空段；顺序 = 段序列顺序，前 = 牌库顶）；
/// - 固定段：`Fixed(Vec<EntityId>)`，段内**保持牌库顺序**；
/// - 随机段：`Random { pool, matches }`，`matches` 按池内位置顺序给出，但**不暗示随机段内
///   「哪张牌在哪」**（随机段是无序集合 —— 别把它当牌库顶顺序用）；
/// - 同源随机段（同 `pool`）**各自列出同一份结果**（不另设 pools 总表，D101）：
///   调用方按 `pool` 归一即可；
/// - 过滤条件一条都不满足、或牌库为空时：`segments` 仍与牌库段数一致（空段各给空结果）；
///   空牌库没有段 -> `segments` 为空。整体「无结果」用 [`PileQuery::is_empty`] 判断。
pub fn cards_in_pile(
    pile: &PileState,
    meta: &dyn CardMeta,
    ty: Option<ActionCardType>,
    tag: Option<&'static TagText>,
) -> PileQuery {
    let mut segments: Vec<SegmentQuery> = Vec::with_capacity(pile.segments().len());
    for segment in pile.segments() {
        let matches = matches_in_segment(pile, *segment, meta, ty, tag);
        segments.push(match *segment {
            Segment::Fixed { .. } => SegmentQuery::Fixed(matches),
            Segment::Random { pool, .. } => SegmentQuery::Random { pool, matches },
        });
    }
    PileQuery { segments }
}

/// 某个段区间内命中的实体（按位置顺序；段外位置不参与）。
///
/// 用**位置比较**切段（位置是 `Nat`），不把 `Nat` 转成切片下标去切片：
/// 越界 / 空段自然得到空结果，不 panic。
fn matches_in_segment(
    pile: &PileState,
    segment: Segment,
    meta: &dyn CardMeta,
    ty: Option<ActionCardType>,
    tag: Option<&'static TagText>,
) -> Vec<EntityId> {
    let start = segment.start();
    let end = segment.end();
    let mut matches: Vec<EntityId> = Vec::new();
    for (position, entry) in pile.entries().iter().enumerate() {
        let position = Nat::new(position as _);
        if start <= position && position < end && card_matches(meta, entry.card, ty, tag) {
            matches.push(entry.entity);
        }
    }
    matches
}

// ======================================================================
// PileQuery：按段分组的结果（绝不扁平）
// ======================================================================

/// 牌库查询结果：**按段分组**，绝不返回扁平列表（分工计划 §4.4「硬规矩」）。
///
/// 为什么不能压平：扁平列表会凭空造出一个「顺序」，把随机段的未知说成已知。
/// 因此结果只给「每个段各自经过查询后的结果」，调用方拿到的永远是「段 + 段内结果」。
///
/// `segments` 与查询时牌库的 `segments()` **逐项一一对应**：下标相同 = 同一个段。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PileQuery {
    /// 每个段各自的查询结果（顺序 = 牌库段序列顺序，前 = 牌库顶）。
    pub segments: Vec<SegmentQuery>,
}

/// 单个段的查询结果（固定段有序 / 随机段无序）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentQuery {
    /// 固定段：**有序**，按段内牌库顺序给出命中的实体。
    Fixed(Vec<EntityId>),
    /// 随机段：**无序集合**（不暗示段内位置）；`pool` = 同源分组 id（同 pool 的段共享同一池）。
    Random {
        /// 同源分组 id（`PileState` 的 `Segment::Random { pool, .. }` 原样带出）。
        pool: Nat,
        /// 这个池里命中的实体（按池内位置顺序；**不暗示随机性之外的顺序含义**）。
        matches: Vec<EntityId>,
    },
}

impl SegmentQuery {
    /// 这个段命中的实体（固定段按段内顺序 / 随机段按池内位置顺序）。
    pub fn matches(&self) -> &[EntityId] {
        match self {
            SegmentQuery::Fixed(matches) | SegmentQuery::Random { matches, .. } => matches,
        }
    }

    /// 命中张数。
    pub fn len(&self) -> Nat {
        Nat::new(self.matches().len() as _)
    }

    /// 这个段是否一张都没命中。
    pub fn is_empty(&self) -> bool {
        self.matches().is_empty()
    }

    /// 随机段的同源分组 id（固定段 -> `None`）。
    pub fn pool(&self) -> Option<Nat> {
        match self {
            SegmentQuery::Fixed(_) => None,
            SegmentQuery::Random { pool, .. } => Some(*pool),
        }
    }
}

impl PileQuery {
    /// 按段下标取结果（越界 -> `None`）；下标与 `PileState::segment` 一致。
    pub fn segment(&self, index: Nat) -> Option<&SegmentQuery> {
        self.segments
            .iter()
            .enumerate()
            .find(|(slot, _)| Nat::new(*slot as _) == index)
            .map(|(_, query)| query)
    }

    /// 各段命中条数之和（**不做同源池去重**：同 pool 的段各算一份）。
    pub fn total_matches(&self) -> Nat {
        self.segments
            .iter()
            .fold(Nat::ZERO, |total, segment| nadd!(total, segment.len()))
    }

    /// 是否所有段都没有命中（空牌库 / 全部不匹配都为 `true`）。
    pub fn is_empty(&self) -> bool {
        self.total_matches() == Nat::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::zones::pile::PileEntry;
    use crate::families::action_card::zones::CardInstance;
    use crate::tags::action_card::equipment::weapon;
    use crate::tags::action_card::event::food;
    use crate::tags::action_card::support::location;

    /// 测试用元数据表：卡 id -> (类型, 标签集合)。
    struct TestMeta {
        entries: Vec<(CardId, ActionCardType, &'static [&'static TagText])>,
    }

    impl CardMeta for TestMeta {
        fn ty(&self, card: CardId) -> Option<ActionCardType> {
            self.entries
                .iter()
                .find(|entry| entry.0 == card)
                .map(|entry| entry.1)
        }

        fn tags(&self, card: CardId) -> &'static [&'static TagText] {
            self.entries
                .iter()
                .find(|entry| entry.0 == card)
                .map(|entry| entry.2)
                .unwrap_or(&[])
        }
    }

    /// 把标签集合泄漏成 `'static`（测试专用：模拟标签静态元数据指针）。
    fn static_tags(tags: Vec<&'static TagText>) -> &'static [&'static TagText] {
        Box::leak(tags.into_boxed_slice())
    }

    /// 示例卡牌元数据（**不是真实卡牌**，只用来钉查询语义）：
    /// - 10 / 11：事件 + 料理；12：支援 + 场地；13：装备 + 武器；
    /// - 14：事件 + 不可发现；15：事件 + 弃用；16：支援 + 自走牌；17：支援、无标签。
    fn sample_meta() -> TestMeta {
        TestMeta {
            entries: vec![
                (
                    CardId::new(10),
                    ActionCardType::Event,
                    static_tags(vec![food::data()]),
                ),
                (
                    CardId::new(11),
                    ActionCardType::Event,
                    static_tags(vec![food::data()]),
                ),
                (
                    CardId::new(12),
                    ActionCardType::Support,
                    static_tags(vec![location::data()]),
                ),
                (
                    CardId::new(13),
                    ActionCardType::Equipment,
                    static_tags(vec![weapon::data()]),
                ),
                (
                    CardId::new(14),
                    ActionCardType::Event,
                    static_tags(vec![undiscoverable::data()]),
                ),
                (
                    CardId::new(15),
                    ActionCardType::Event,
                    static_tags(vec![scrapped::data()]),
                ),
                (
                    CardId::new(16),
                    ActionCardType::Support,
                    static_tags(vec![automaton_card::data()]),
                ),
                (CardId::new(17), ActionCardType::Support, &[]),
            ],
        }
    }

    fn fixed_pile(instances: &[(EntityId, CardId)]) -> PileState {
        PileState::from_fixed(instances.to_vec())
    }

    fn random_pile(instances: &[(EntityId, CardId)]) -> PileState {
        PileState::from_random(instances.to_vec())
    }

    fn hand(instances: &[(EntityId, CardId)]) -> HandState {
        let mut hand = HandState::new();
        for (entity, card) in instances {
            let (next, added) = hand.add(CardInstance::new(*entity, *card));
            assert!(added, "测试手牌没超过上限、实体不重复");
            hand = next;
        }
        hand
    }

    fn sorted_entities(mut entities: Vec<EntityId>) -> Vec<EntityId> {
        entities.sort();
        entities
    }

    fn sorted_cards(mut cards: Vec<CardId>) -> Vec<CardId> {
        cards.sort();
        cards
    }

    #[test]
    fn empty_pile_returns_empty_grouped_query() {
        let meta = sample_meta();
        let query = cards_in_pile(&PileState::new(), &meta, None, None);
        assert!(query.segments.is_empty(), "空牌库没有段 -> 结果也没有段");
        assert_eq!(query.total_matches(), Nat::ZERO);
        assert!(query.is_empty());
        assert_eq!(query.segment(Nat::ZERO), None);
    }

    #[test]
    fn single_fixed_segment_keeps_deck_order() {
        let meta = sample_meta();
        let pile = fixed_pile(&[
            (Nat::ZERO, CardId::new(10)),
            (Nat::new(1), CardId::new(12)),
            (Nat::new(2), CardId::new(11)),
        ]);

        // 两个过滤都没给：全部可发现 -> 3 张，按段内顺序。
        let all = cards_in_pile(&pile, &meta, None, None);
        assert_eq!(all.segments.len(), 1);
        assert_eq!(
            all.segments.first(),
            Some(&SegmentQuery::Fixed(vec![
                Nat::ZERO,
                Nat::new(1),
                Nat::new(2)
            ]))
        );

        // 按类型筛：事件 = 10、11（段内顺序：0, 2）。
        let events = cards_in_pile(&pile, &meta, Some(ActionCardType::Event), None);
        assert_eq!(
            events.segments,
            vec![SegmentQuery::Fixed(vec![Nat::ZERO, Nat::new(2)])]
        );

        // 按标签筛：场地 = 12。
        let locations = cards_in_pile(&pile, &meta, None, Some(location::data()));
        assert_eq!(
            locations.segments,
            vec![SegmentQuery::Fixed(vec![Nat::new(1)])]
        );
    }

    #[test]
    fn multi_segment_query_maps_segment_by_segment() {
        let meta = sample_meta();
        // 段 0：固定段 [0, 1] = 10（事件+料理）、14（不可发现）；
        // 段 1：随机段 [2, 3]、pool 7 = 12（支援+场地）、11（事件+料理）；
        // 段 2：空固定段 [4, 4)。
        let mut pile = fixed_pile(&[
            (Nat::ZERO, CardId::new(10)),
            (Nat::new(1), CardId::new(14)),
        ]);
        pile.entries
            .push(PileEntry::new(Nat::new(2), CardId::new(12)));
        pile.entries
            .push(PileEntry::new(Nat::new(3), CardId::new(11)));
        pile.segments.push(Segment::Random {
            pool: Nat::new(7),
            start: Nat::new(2),
            len: Nat::new(2),
        });
        pile.segments.push(Segment::Fixed {
            start: Nat::new(4),
            len: Nat::ZERO,
        });

        let all = cards_in_pile(&pile, &meta, None, None);
        assert_eq!(all.segments.len(), pile.segments().len(), "逐项一一对应");
        // 固定段保持段内顺序；14 因查全部被排除。
        assert_eq!(
            all.segments.first(),
            Some(&SegmentQuery::Fixed(vec![Nat::ZERO]))
        );
        // 随机段带出 pool 与池内命中（按池内位置顺序）。
        assert_eq!(
            all.segment(Nat::new(1)),
            Some(&SegmentQuery::Random {
                pool: Nat::new(7),
                matches: vec![Nat::new(2), Nat::new(3)],
            })
        );
        // 空段保留在结果里（index 对齐），结果是空的。
        assert_eq!(
            all.segment(Nat::new(2)),
            Some(&SegmentQuery::Fixed(Vec::new()))
        );
        assert!(!all.is_empty());

        // 按类型筛：事件 = 固定段的 10、14（显式按类型查时**不排除**，D101）+ 随机段的 11。
        let events = cards_in_pile(&pile, &meta, Some(ActionCardType::Event), None);
        assert_eq!(
            events.segments,
            vec![
                SegmentQuery::Fixed(vec![Nat::ZERO, Nat::new(1)]),
                SegmentQuery::Random {
                    pool: Nat::new(7),
                    matches: vec![Nat::new(3)],
                },
                SegmentQuery::Fixed(Vec::new()),
            ]
        );
    }

    #[test]
    fn same_pool_random_segments_list_the_same_result() {
        let meta = sample_meta();
        // 一个随机池 [0, 1] = 10（料理）、14（不可发现），拆成两个同源段。
        let pile = random_pile(&[
            (Nat::ZERO, CardId::new(10)),
            (Nat::new(1), CardId::new(14)),
        ]);
        let pile = pile.split_random_segment(Nat::ZERO).expect("0 号段是随机段");
        assert_eq!(pile.segments().len(), 2);

        let all = cards_in_pile(&pile, &meta, None, None);
        let expected = SegmentQuery::Random {
            pool: Nat::ZERO,
            matches: vec![Nat::ZERO],
        };
        assert_eq!(all.segments.len(), 2);
        assert_eq!(all.segments.first(), Some(&expected));
        assert_eq!(all.segments.last(), Some(&expected));
        // 同源段只按各自项列结果，调用方可按 pool 归一。
        assert_eq!(all.total_matches(), Nat::new(2));

        // 显式查「不可发现」：排除规则不生效（D101），两个同源段都能查到 14。
        let hidden = cards_in_pile(&pile, &meta, None, Some(undiscoverable::data()));
        let expected_hidden = SegmentQuery::Random {
            pool: Nat::ZERO,
            matches: vec![Nat::new(1)],
        };
        assert_eq!(hidden.segments.first(), Some(&expected_hidden));
        assert_eq!(hidden.segments.last(), Some(&expected_hidden));
    }

    #[test]
    fn filter_by_tag_matches_any_of_card_tags_by_pointer() {
        let meta = sample_meta();
        let deck = vec![
            CardId::new(10),
            CardId::new(12),
            CardId::new(13),
            CardId::new(17),
        ];
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, Some(food::data())),
            vec![CardId::new(10)]
        );
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, Some(weapon::data())),
            vec![CardId::new(13)]
        );
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, Some(location::data())),
            vec![CardId::new(12)]
        );
        // 没有这个标签的卡（17 无标签）不会被命中。
        assert!(!cards_in_initial_deck(&deck, &meta, None, Some(weapon::data()))
            .contains(&CardId::new(17)));
    }

    #[test]
    fn combined_type_and_tag_filters_require_both() {
        let meta = sample_meta();
        let deck = vec![CardId::new(10), CardId::new(11), CardId::new(13)];
        // 装备 + 武器 -> 13；事件 + 武器 -> 空。
        assert_eq!(
            cards_in_initial_deck(
                &deck,
                &meta,
                Some(ActionCardType::Equipment),
                Some(weapon::data())
            ),
            vec![CardId::new(13)]
        );
        assert!(cards_in_initial_deck(
            &deck,
            &meta,
            Some(ActionCardType::Event),
            Some(weapon::data())
        )
        .is_empty());
        // 事件 + 料理 -> 10、11。
        assert_eq!(
            sorted_cards(cards_in_initial_deck(
                &deck,
                &meta,
                Some(ActionCardType::Event),
                Some(food::data())
            )),
            vec![CardId::new(10), CardId::new(11)]
        );
    }

    #[test]
    fn query_all_excludes_non_discoverable_three_tags() {
        let meta = sample_meta();
        // 10 可发现；14 不可发现；15 弃用；16 自走牌。
        assert!(is_discoverable(&meta, CardId::new(10)));
        assert!(!is_discoverable(&meta, CardId::new(14)));
        assert!(!is_discoverable(&meta, CardId::new(15)));
        assert!(!is_discoverable(&meta, CardId::new(16)));

        let deck = vec![
            CardId::new(10),
            CardId::new(12),
            CardId::new(14),
            CardId::new(15),
            CardId::new(16),
            CardId::new(17),
        ];
        // 查全部 -> 排除三类；17（无标签）仍算可发现。
        assert_eq!(
            sorted_cards(cards_in_initial_deck(&deck, &meta, None, None)),
            vec![CardId::new(10), CardId::new(12), CardId::new(17)]
        );

        // 只给类型 / 只给标签 -> 不额外排除（D101）。
        assert_eq!(
            sorted_cards(cards_in_initial_deck(
                &deck,
                &meta,
                Some(ActionCardType::Event),
                None
            )),
            vec![CardId::new(10), CardId::new(14), CardId::new(15)]
        );
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, Some(undiscoverable::data())),
            vec![CardId::new(14)]
        );
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, Some(scrapped::data())),
            vec![CardId::new(15)]
        );
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, Some(automaton_card::data())),
            vec![CardId::new(16)]
        );
    }

    #[test]
    fn hand_query_returns_one_entity_per_instance() {
        let meta = sample_meta();
        // 两张同卡 id（10）不同实体 + 一张不可发现（14）。
        let hand = hand(&[
            (Nat::ZERO, CardId::new(10)),
            (Nat::new(1), CardId::new(10)),
            (Nat::new(2), CardId::new(14)),
        ]);

        // 查全部：排除 14，两张 10 各一条（重复 = 实际多张）。
        assert_eq!(
            sorted_entities(cards_in_hand(&hand, &meta, None, None)),
            vec![Nat::ZERO, Nat::new(1)]
        );
        // 按类型筛：事件 -> 两张 10 + 14（显式类型放行）。
        assert_eq!(
            sorted_entities(cards_in_hand(&hand, &meta, Some(ActionCardType::Event), None)),
            vec![Nat::ZERO, Nat::new(1), Nat::new(2)]
        );
        // 按标签筛：不可发现 -> 14。
        assert_eq!(
            cards_in_hand(&hand, &meta, None, Some(undiscoverable::data())),
            vec![Nat::new(2)]
        );
        // 空手牌 -> 空结果（不 panic）。
        assert!(cards_in_hand(&HandState::new(), &meta, None, None).is_empty());
    }

    #[test]
    fn initial_deck_query_keeps_duplicates_and_excludes_by_default() {
        let meta = sample_meta();
        let deck = vec![
            CardId::new(10),
            CardId::new(10),
            CardId::new(12),
            CardId::new(14),
        ];
        // 查全部：重复的 10 出现两次；14 被排除。
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, None),
            vec![CardId::new(10), CardId::new(10), CardId::new(12)]
        );
        // 按标签筛：料理两张。
        assert_eq!(
            cards_in_initial_deck(&deck, &meta, None, Some(food::data())),
            vec![CardId::new(10), CardId::new(10)]
        );
        // 空初始卡组 -> 空结果。
        assert!(cards_in_initial_deck(&[], &meta, None, None).is_empty());
    }

    #[test]
    fn empty_metadata_is_treated_as_unknown_but_discoverable() {
        let meta = sample_meta();
        // 99 不在元数据表里：类型未知 -> 指定类型不命中；标签未知 -> 指定标签不命中；
        // 查全部时没有禁止标签 -> 算可发现。
        assert!(!card_matches(
            &meta,
            CardId::new(99),
            Some(ActionCardType::Event),
            None
        ));
        assert!(!card_matches(&meta, CardId::new(99), None, Some(food::data())));
        assert!(card_matches(&meta, CardId::new(99), None, None));
        assert!(is_discoverable(&meta, CardId::new(99)));
    }
}
