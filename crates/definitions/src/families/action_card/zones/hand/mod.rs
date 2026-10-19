//! 手牌区（行动牌）。
//!
//! 手牌是**多实体容器**，但容器本身没有身份（D42）：它只是 DefState.me / DefState.opp
//! 里的一个字段，这里不出现区号、不登记区、不建「实体 -> 区」表。
//!
//! 规格与决策：
//! - 上限 10（HandState::MAX）；
//! - **加入 / 生成溢出 -> 不允许加入**（返回失败、状态不变）；
//! - **抓牌溢出**不在这里做：抓牌在牌库的抽取分支里判断「手牌已满 -> 抓到的这张弃置」；
//! - **起手流程归手牌（D90）**：手牌先按「秘传」标签向牌库抓，再补足到 5 张；
//!   牌库只提供抓牌原语（按谓词抓最上面一张 / 抓 n 张），起手的循环与补足在本文件；
//! - 手牌**无序**：容器不承诺顺序，舍弃语义是「从手牌移除」，没有弃牌堆。
//!
//! 纯函数风格：所有操作吃 &self、返回新状态；失败返回 None / false 且状态不变，不 panic。

pub mod parse;
pub mod record;
pub mod stats;

pub use stats::HandStats;

use super::CardInstance;
use super::pile::PileState;
use crate::base::CardId;
use gi_genius_invokation_engine::{nsub, EntityId, Nat};
use gi_genius_invokation_engine::prob::Fraction;

/// 手牌区：一个多实体容器（元素 = 实体号 + 卡 id 一对），**无序、上限 10**。
///
/// 「这是谁的手牌」不靠参数：状态层用两个字段 me.hand / opp.hand（两个具名实例）表达，
/// 操作函数由状态层写成对入口（hand_me / hand_opp），规则只写一份。
#[derive(Clone, Debug, Default)]
pub struct HandState {
    /// 手牌里的卡牌实例。字段公开是为了让状态层 / 查询块能直接按字段访问（D64）；
    /// 修改请走本类型的操作，别绕过上限规则。
    pub entries: Vec<CardInstance>,
    /// 手牌统计（D187；feature 关闭时零大小、空操作）。
    stats: HandStats,
}

impl PartialEq for HandState {
    /// **统计不参与局面等价**（D187：统计不改变游戏语义）。
    fn eq(&self, other: &HandState) -> bool {
        self.entries == other.entries
    }
}

impl Eq for HandState {}

impl HandState {
    /// 手牌上限：10 张。
    pub const MAX: Nat = Nat::new(10);

    /// 起手手牌张数：5 张（D17/D90）。
    pub const OPENING_SIZE: Nat = Nat::new(5);

    /// 空手牌。
    pub const fn new() -> HandState {
        HandState {
            entries: Vec::new(),
            stats: HandStats::EMPTY,
        }
    }

    /// 手牌张数（Nat，不是裸整型）。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 手牌是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 手牌里的全部卡牌实例（**无序**：不要假设顺序有含义）。
    pub fn entries(&self) -> &[CardInstance] {
        &self.entries
    }

    /// 手牌统计（D187；只读指针，D183；feature 关闭时读到的都是 0）。
    pub fn stats(&self) -> &HandStats {
        &self.stats
    }

    /// 这张实体是否在手牌里。
    pub fn contains(&self, entity: EntityId) -> bool {
        self.entries.iter().any(|entry| entry.entity == entity)
    }

    /// 这张实体的卡牌定义 id（不在手牌里 -> None）。
    ///
    /// 手牌自己存着「实体号 + 卡 id」，所以**不需要任何全局映射表**。
    pub fn card_of(&self, entity: EntityId) -> Option<CardId> {
        self.entries
            .iter()
            .find(|entry| entry.entity == entity)
            .map(|entry| entry.card)
    }

    /// 还能再放几张（MAX - len；已满 -> 0）。
    pub fn free_slots(&self) -> Nat {
        nsub!(Self::MAX, self.len())
    }

    /// 手牌是否还有空位。
    pub fn has_space(&self) -> bool {
        self.len() < Self::MAX
    }

    /// 把一张**已存在**的卡牌实体加入手牌；返回 (新状态, 是否加入)。
    ///
    /// 边界 / 失败语义（都不改状态）：
    /// - 手牌已满（10 张）-> (self.clone(), false)；
    /// - 这张实体**已经在手牌里** -> (self.clone(), false)（同一实体不能重复占位）。
    ///
    /// 注意（D99）：实际对局**认为不会发生手牌溢出**；上面「满 -> 不加」只是安全兜底，
    /// 不是正常流程。
    ///
    /// 新牌放在容器末尾（手牌无序，位置不表示任何语义）。
    pub fn add(&self, instance: CardInstance) -> (HandState, bool) {
        if !self.has_space() || self.contains(instance.entity) {
            return (self.clone(), false);
        }
        let mut next = self.clone();
        next.entries.push(instance);
        next.stats.on_added(Nat::new(1));
        (next, true)
    }

    /// 从手牌**舍弃**一张牌（移除，没有弃牌堆）。
    ///
    /// 返回新状态；这张实体不在手牌里 -> None（状态不变）。
    /// 手牌无序，移除后不承诺剩余元素的顺序。
    pub fn discard(&self, entity: EntityId) -> Option<HandState> {
        let mut next = self.clone();
        let index = next
            .entries
            .iter()
            .position(|entry| entry.entity == entity)?;
        next.entries.swap_remove(index);
        next.stats.on_discarded(Nat::new(1));
        Some(next)
    }

    /// **起手**（D17 / D90）：先按「秘传」标签向牌库抓，再补足到 OPENING_SIZE 张。
    ///
    /// 参数：
    /// - pile：起手要抓的牌库（牌库快照，不改入参）；
    /// - arcane：调用方注入的谓词（拿 CardId 判断是不是「秘传」；zones 不认识标签语义）。
    ///
    /// 行为：
    /// 1. 循环：只要手牌没到 5 张、且牌库顶段还有匹配秘传的牌 -> 按谓词抓最上面一张；
    ///    抓不到匹配牌就进入下一步；
    /// 2. 补足：循环抓牌直到手牌 5 张或牌库空了；
    /// 3. 每一步的随机分支都由牌库的抓牌原语（内部走引擎 enumerate_random）给出，
    ///    这里只做**跨步分支的组合**：概率 = 前面各步概率之积（引擎给的权重相乘，不自己造分布）。
    ///
    /// 返回 (手牌, 牌库, 概率) 的分支列表；一条分支都没有 -> 空 Vec（不 panic）。
    pub fn draw_opening(
        &self,
        pile: &PileState,
        arcane: &dyn Fn(CardId) -> bool,
    ) -> Vec<(HandState, PileState, Fraction)> {
        let mut branches: Vec<(HandState, PileState, Fraction)> =
            vec![(self.clone(), pile.clone(), Fraction::ONE)];

        // 第 1 步：优先抓秘传（按谓词抓最上面一张，直到抓满或牌库顶部不再有匹配牌）。
        loop {
            let (next, progressed) = expand_branches(branches, |hand, pile| {
                pile.draw_top(hand, arcane)
            });
            branches = next;
            if !progressed || !needs_more(&branches) {
                break;
            }
        }

        // 第 2 步：补足到 5 张。
        while needs_more(&branches) {
            let (next, progressed) = expand_branches(branches, |hand, pile| {
                let remaining = nsub!(HandState::OPENING_SIZE, hand.len());
                pile.draw_many(hand, remaining)
            });
            branches = next;
            if !progressed {
                break;
            }
        }
        // D187：起手阶段单独统计抓到的张数（各分支分别累计）。
        let initial = self.len();
        branches
            .into_iter()
            .map(|(mut hand, pile, probability)| {
                let drawn = nsub!(hand.len(), initial);
                hand.stats.on_opening_drawn(drawn);
                (hand, pile, probability)
            })
            .collect()
    }
}

/// 是否还有分支没抓满起手张数。
fn needs_more(branches: &[(HandState, PileState, Fraction)]) -> bool {
    branches
        .iter()
        .any(|(hand, _, _)| hand.len() < HandState::OPENING_SIZE)
}

/// 对每个分支展开一次抓牌，并把概率相乘组合；返回 (新分支, 是否有分支抓到了牌)。
fn expand_branches(
    branches: Vec<(HandState, PileState, Fraction)>,
    draw: impl Fn(&HandState, &PileState) -> Vec<(super::pile::DrawBranch, Fraction)>,
) -> (Vec<(HandState, PileState, Fraction)>, bool) {
    let mut next: Vec<(HandState, PileState, Fraction)> = Vec::new();
    let mut progressed = false;
    for (hand, pile, probability) in branches {
        if hand.len() >= HandState::OPENING_SIZE {
            next.push((hand, pile, probability));
            continue;
        }
        let draws = draw(&hand, &pile);
        if draws.is_empty() {
            next.push((hand, pile, probability));
            continue;
        }
        progressed = true;
        for (branch, draw_probability) in draws {
            next.push((
                branch.hand,
                branch.pile,
                probability.exact_mul(draw_probability),
            ));
        }
    }
    (next, progressed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::zones::pile::PileState;
    use gi_genius_invokation_engine::nadd;

    /// 造一个卡牌实例（测试辅助；实体号 / 卡 id 都由 Nat 给，避免裸整型）。
    fn instance(entity: Nat, card: Nat) -> CardInstance {
        CardInstance::new(entity, CardId::new(card.get()))
    }

    fn hand_of(count: Nat) -> HandState {
        let mut hand = HandState::new();
        let mut index = Nat::ZERO;
        while index < count {
            let (next, added) = hand.add(instance(index, index));
            assert!(added);
            hand = next;
            index = nadd!(index, Nat::new(1));
        }
        hand
    }

    fn deck(count: Nat) -> PileState {
        let mut cards: Vec<(EntityId, CardId)> = Vec::new();
        let mut index = Nat::ZERO;
        while index < count {
            cards.push((index, CardId::new(index.get())));
            index = nadd!(index, Nat::new(1));
        }
        PileState::from_fixed(cards)
    }

    #[test]
    fn empty_hand_basics() {
        let hand = HandState::new();
        assert!(hand.is_empty());
        assert_eq!(hand.len(), Nat::ZERO);
        assert_eq!(hand.free_slots(), HandState::MAX);
        assert!(hand.entries().is_empty());
        assert_eq!(hand.card_of(Nat::new(3)), None);
    }

    #[test]
    fn add_fills_up_to_ten_then_rejects() {
        let hand = hand_of(HandState::MAX);
        assert_eq!(hand.len(), HandState::MAX);
        assert!(!hand.has_space());
        assert_eq!(hand.free_slots(), Nat::ZERO);

        let (rejected, added) = hand.add(instance(Nat::new(99), Nat::new(999)));
        assert!(!added);
        assert_eq!(rejected, hand);
    }

    #[test]
    fn add_rejects_duplicate_entity() {
        let hand = hand_of(Nat::new(1));
        let (same, added) = hand.add(instance(Nat::ZERO, Nat::new(777)));
        assert!(!added);
        assert_eq!(same, hand);
    }

    #[test]
    fn add_records_card_identity_without_global_table() {
        let hand = HandState::new();
        let (hand, added) = hand.add(instance(Nat::new(4), Nat::new(321004)));
        assert!(added);
        assert_eq!(hand.card_of(Nat::new(4)), Some(CardId::new(321004)));
    }

    #[test]
    fn discard_removes_present_and_rejects_absent() {
        let hand = hand_of(Nat::new(3));
        let smaller = hand.discard(Nat::new(1)).expect("手牌里有 1 号实体");
        assert_eq!(smaller.len(), Nat::new(2));
        assert!(!smaller.contains(Nat::new(1)));
        assert_eq!(hand.discard(Nat::new(42)), None);
    }

    #[test]
    fn opening_hand_draws_five_then_runs_out() {
        let hand = HandState::new();
        let pile = deck(Nat::new(3));

        // 没有秘传：直接补足，牌库只有 3 张 -> 每支都是 3 张手牌
        let branches = hand.draw_opening(&pile, &|_card| false);
        assert!(!branches.is_empty());
        for (hand, _pile, _probability) in &branches {
            assert_eq!(hand.len(), Nat::new(3));
        }
        let total: Fraction = branches.iter().map(|(_, _, probability)| *probability).sum();
        assert!(total.is_one());
    }

    #[test]
    fn opening_hand_prefers_arcane_cards() {
        let hand = HandState::new();
        // 牌库：0 号不是秘传，1 号是秘传（都在固定段里，段顶是 0 号）
        let pile = deck(Nat::new(4));

        // 谓词：卡 id == 1 的算秘传
        let arcane = |card: CardId| card == CardId::new(1);
        let branches = hand.draw_opening(&pile, &arcane);

        // 固定段顶是 0 号（不是秘传）-> 按 D88「固定段只看段顶」，秘传一张都抓不到，
        // 直接补足 4 张（牌库只有 4 张）。
        assert!(!branches.is_empty());
        for (hand, _pile, probability) in &branches {
            assert_eq!(hand.len(), Nat::new(4));
            assert_eq!(*probability, Fraction::ONE);
        }
    }

    #[test]
    fn opening_hand_with_arcane_on_top_fetches_it_first() {
        let hand = HandState::new();
        // 固定段：0 号在顶（秘传），1~2 号普通
        let pile = deck(Nat::new(3));
        let arcane = |card: CardId| card == CardId::new(0);
        let branches = hand.draw_opening(&pile, &arcane);
        assert_eq!(branches.len(), 1);
        let (hand, _pile, probability) = branches.first().expect("有分支");
        assert_eq!(hand.len(), Nat::new(3));
        assert!(hand.contains(Nat::ZERO));
        assert_eq!(*probability, Fraction::ONE);
    }

    #[test]
    fn hand_stats_consistent_with_feature() {
        let hand = HandState::new();
        let (hand, added) = hand.add(instance(Nat::ZERO, Nat::new(5)));
        assert!(added);
        let hand = hand.discard(Nat::ZERO).expect("0 号在手牌");
        if cfg!(feature = "stats") {
            assert_eq!(hand.stats().added().get(), Nat::new(1));
            assert_eq!(hand.stats().discarded().get(), Nat::new(1));
        } else {
            assert!(hand.stats().added().is_zero());
            assert!(hand.stats().discarded().is_zero());
            assert_eq!(std::mem::size_of::<HandStats>(), 0);
        }
    }

    #[test]
    fn opening_drawn_stat_consistent_with_feature() {
        let pile = deck(Nat::new(3));
        let branches = HandState::new().draw_opening(&pile, &|_card| false);
        for (hand, _pile, _probability) in &branches {
            if cfg!(feature = "stats") {
                assert_eq!(hand.stats().opening_drawn().get(), hand.len());
            } else {
                assert!(hand.stats().opening_drawn().is_zero());
            }
        }
    }
}
