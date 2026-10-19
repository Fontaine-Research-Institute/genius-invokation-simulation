//! **游戏准备（开局）流程**：抓 5 张起手（秘传优先）-> 换牌 -> 选首发角色。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json`）
//!
//! - 规则书·《七圣召唤》游戏准备：
//!   「(1) 双方牌手各抓 5 张起始手牌。然后，双方各有一次机会替换任意数量的手牌。
//!   (2) 双方各自选择一张自己的『角色牌』作为『出战角色』。」
//!   English: "Both sides will draw 5 cards ... 1 chance to switch out any number of cards in their Hand."
//! - 规则书·游戏区域 VI（手牌）：「游戏开始后，从『牌堆』中抓 5 张牌，作为初始的『手牌』。」
//! - 秘传（D17 / D90）：起手第 1 步先按「秘传」标签向牌库抓，再补足到 5 张
//!   （`HandState::draw_opening` 已实现，本流程直接调它）。
//!
//! ## 顺序（唯一权威：`rules/ops/opening.rs` 的 `pregame_steps`）
//!
//! [`pregame`] 按 [`pregame_steps`] 给出的顺序逐步执行（**不另写一份顺序**）：
//! `DrawOpeningHand` -> `Mulligan` -> `ChooseFirstCharacter`。
//!
//! ## 分支与概率（D190）
//!
//! 抓牌是随机观测点：本流程返回**分支列表 + 精确概率**（`Fraction`，分子 / 分母 `ExactNat128`，
//! 零浮点）。两侧的抓牌互相独立（两个牌库），所以联合概率 = 两侧分支概率**逐个相乘**；
//! 概率的展开 / 折叠由牌库的抓牌原语负责（本层不造分布，只做组合）。
//!
//! ## 待复核
//!
//! - **换牌顺序**：规则书只说「替换任意数量」，没说换掉的牌怎么回牌库。本实现：
//!   先从手牌舍弃 -> 抓同样张数 -> 再把换掉的牌用 `PileState::insert_evenly_with`
//!   （牌库文档注明是「手牌洗回牌库用」的均匀置入原语）放回牌库。**先抓后洗**还是**先洗后抓**
//!   会改变抓到什么，待实测；
//! - **换牌后的费用表**：新抓到的牌按卡牌定义的基础费用建 `CurrentCost`；换掉的牌的 delta 随牌丢弃；
//! - **首发选择**：`CharacterZone::set_active` 在「已经是出战角色」时返回 `None`；
//!   开局默认出战角色可能已经是第一张角色牌，所以本流程把「已经是出战角色」当作**成功的无操作**。

use crate::base::CardId;
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::pile::PileState;
use crate::families::action_card::zones::CardInstance;
use crate::families::character::zones::field::CharacterZone;
use crate::resources::dice::CostComponent;
use crate::rules::ops::opening::{pregame_steps, PregameStep};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 开局时上层注入的两个查询函数（D186 风格：函数指针 / `Fn`，不是效果数据）。
pub struct PregameHooks<'a> {
    /// 这张牌是不是「秘传」（起手第 1 步按它抓；标签语义在行动牌块，本层不认识）。
    pub arcane: &'a dyn Fn(CardId) -> bool,
    /// 这张牌的基础元素骰费用（建「每张手牌的当前费用」用；费用属性在卡牌定义里）。
    pub base_cost: &'a dyn Fn(CardId) -> Vec<CostComponent>,
}

/// 玩家在游戏准备阶段做的**决策**（本层不替你选，只按给定决策推进世界）。
#[derive(Clone, Debug)]
pub struct PregameChoices {
    /// 我方要换掉的手牌实体号（不认识的实体号会被忽略；重复只算一次）。
    pub me_mulligan: Vec<EntityId>,
    /// 敌方要换掉的手牌实体号。
    pub opp_mulligan: Vec<EntityId>,
    /// 我方首发角色实体号。
    pub me_first: EntityId,
    /// 敌方首发角色实体号。
    pub opp_first: EntityId,
}

impl PregameChoices {
    /// 只给首发角色（两边都不换牌）。
    pub fn new(me_first: EntityId, opp_first: EntityId) -> PregameChoices {
        PregameChoices {
            me_mulligan: Vec::new(),
            opp_mulligan: Vec::new(),
            me_first,
            opp_first,
        }
    }

    /// 设我方的换牌名单（纯值，返回新决策）。
    pub fn with_me_mulligan(mut self, entities: Vec<EntityId>) -> PregameChoices {
        self.me_mulligan = entities;
        self
    }

    /// 设敌方的换牌名单（纯值，返回新决策）。
    pub fn with_opp_mulligan(mut self, entities: Vec<EntityId>) -> PregameChoices {
        self.opp_mulligan = entities;
        self
    }
}

// ======================================================================
// 第 1 步：抓 5 张起手（秘传优先，D17 / D90）
// ======================================================================

/// 双方各抓 5 张起手牌；返回**分支 + 精确概率**（两侧独立，概率相乘）。
///
/// 抓牌原语是 [`HandState::draw_opening`]（秘传优先 + 补足到 5）；本函数只做
/// 「两侧组合 + 费用表同步」。牌库不够 5 张时按实际张数（不 panic）。
pub fn draw_opening_hands(state: &DefState, hooks: &PregameHooks) -> Vec<(DefState, Fraction)> {
    let me_branches = draw_side(state.me(), hooks);
    let opp_branches = draw_side(state.opp(), hooks);
    let mut branches: Vec<(DefState, Fraction)> = Vec::new();
    for (me, me_probability) in &me_branches {
        for (opp, opp_probability) in &opp_branches {
            branches.push((
                state
                    .clone()
                    .with_me(me.clone())
                    .with_opp(opp.clone()),
                me_probability.exact_mul(*opp_probability),
            ));
        }
    }
    branches
}

/// 一侧的抓牌分支（`HandState::draw_opening` 原样透传 + 写回 + 费用表同步）。
fn draw_side(player: &PlayerDefs, hooks: &PregameHooks) -> Vec<(PlayerDefs, Fraction)> {
    let mut branches: Vec<(PlayerDefs, Fraction)> = Vec::new();
    for (hand, pile, probability) in player
        .hand()
        .draw_opening(player.pile(), hooks.arcane)
    {
        let mut next = player.clone();
        {
            let parts = next.parts_mut();
            *parts.hand = hand;
            *parts.pile = pile;
        }
        branches.push((next.with_synced_hand_costs(hooks.base_cost), probability));
    }
    branches
}

// ======================================================================
// 第 2 步：换牌（双方各 1 次机会，替换任意数量）
// ======================================================================

/// 换牌；返回**分支 + 精确概率**（换牌后抓的牌来自牌库，可能多分支）。
///
/// 每侧的顺序：舍弃选中的手牌 -> 抓同样张数 -> 把换掉的牌均匀洗回牌库（见文件头「待复核」）。
/// 选中的实体号不在手牌 -> 忽略；重复 -> 只算一次。
pub fn mulligan(
    state: &DefState,
    me: &[EntityId],
    opp: &[EntityId],
    hooks: &PregameHooks,
) -> Vec<(DefState, Fraction)> {
    let me_branches = mulligan_side(state.me(), me, hooks);
    let opp_branches = mulligan_side(state.opp(), opp, hooks);
    let mut branches: Vec<(DefState, Fraction)> = Vec::new();
    for (me, me_probability) in &me_branches {
        for (opp, opp_probability) in &opp_branches {
            branches.push((
                state
                    .clone()
                    .with_me(me.clone())
                    .with_opp(opp.clone()),
                me_probability.exact_mul(*opp_probability),
            ));
        }
    }
    branches
}

/// 一侧的换牌分支。
fn mulligan_side(
    player: &PlayerDefs,
    requested: &[EntityId],
    hooks: &PregameHooks,
) -> Vec<(PlayerDefs, Fraction)> {
    // 1. 只保留真在手牌里的实体（去重、保序）。
    let mut discarded: Vec<CardInstance> = Vec::new();
    for entity in requested {
        if discarded.iter().any(|instance| instance.entity == *entity) {
            continue;
        }
        if let Some(card) = player.hand().card_of(*entity) {
            discarded.push(CardInstance::new(*entity, card));
        }
    }
    if discarded.is_empty() {
        // 没换牌：仍然同步一次费用表（各分支的后置条件保持一致，调用方不必分支判断）。
        return vec![(player.with_synced_hand_costs(hooks.base_cost), Fraction::ONE)];
    }

    // 2. 舍弃。
    let mut hand = player.hand().clone();
    for instance in &discarded {
        if let Some(next) = hand.discard(instance.entity) {
            hand = next;
        }
    }

    // 3. 抓同样张数；牌库空了（draw_many 给空 Vec）-> 一个分支、状态不变。
    let count = Nat::new(discarded.len() as _);
    let draws = player.pile().draw_many(&hand, count);
    if draws.is_empty() {
        return vec![(settle(player, hand, player.pile().clone(), &discarded, hooks), Fraction::ONE)];
    }
    draws
        .into_iter()
        .map(|(branch, probability)| {
            let next = settle(player, branch.hand, branch.pile, &discarded, hooks);
            (next, probability)
        })
        .collect()
}

/// 抓完牌后：把换掉的牌洗回牌库 + 写回 + 同步费用表。
fn settle(
    player: &PlayerDefs,
    hand: HandState,
    pile: PileState,
    discarded: &[CardInstance],
    hooks: &PregameHooks,
) -> PlayerDefs {
    let returning: Vec<(EntityId, CardId)> = discarded
        .iter()
        .map(|instance| (instance.entity, instance.card))
        .collect();
    let pile = pile.insert_evenly_with(&returning).unwrap_or(pile);
    let mut next = player.clone();
    {
        let parts = next.parts_mut();
        *parts.hand = hand;
        *parts.pile = pile;
    }
    next.with_synced_hand_costs(hooks.base_cost)
}

// ======================================================================
// 第 3 步：选首发角色
// ======================================================================

/// 双方各自选一张自己的角色牌作为出战角色（规则书·游戏准备 (2)）。
///
/// 失败（返回 None、不改状态）：某一方选的角色**不在**那一方的角色区里。
/// 「已经是出战角色」视为**成功的无操作**（开局默认出战角色可能已经是它）。
pub fn choose_first_characters(
    state: &DefState,
    me: EntityId,
    opp: EntityId,
) -> Option<DefState> {
    let me_zone = choose_first(state.me().characters(), me)?;
    let opp_zone = choose_first(state.opp().characters(), opp)?;
    let mut next = state.clone();
    {
        let parts = next.me_mut().parts_mut();
        *parts.characters = me_zone;
    }
    {
        let parts = next.opp_mut().parts_mut();
        *parts.characters = opp_zone;
    }
    Some(next)
}

/// 一侧的首发选择（「已经是出战角色」= 成功无操作）。
fn choose_first(zone: &CharacterZone, character: EntityId) -> Option<CharacterZone> {
    if !zone.contains(character) {
        return None;
    }
    if zone.is_active(character) {
        return Some(zone.clone());
    }
    zone.set_active(character)
}

// ======================================================================
// 串起来：按 pregame_steps 的顺序跑完整开局
// ======================================================================

/// **完整游戏准备流程**：严格按 [`pregame_steps`] 的顺序逐步执行。
///
/// 返回分支 + 精确概率；任一步骤没有任何分支（例如选的角色不在角色区）-> 整体空 Vec
/// （**不改状态**：入参一个字节都不动）。
pub fn pregame(
    state: &DefState,
    choices: &PregameChoices,
    hooks: &PregameHooks,
) -> Vec<(DefState, Fraction)> {
    let mut branches: Vec<(DefState, Fraction)> = vec![(state.clone(), Fraction::ONE)];
    for step in pregame_steps() {
        branches = match step {
            PregameStep::DrawOpeningHand => {
                expand(branches, |current| draw_opening_hands(current, hooks))
            }
            PregameStep::Mulligan => expand(branches, |current| {
                mulligan(current, &choices.me_mulligan, &choices.opp_mulligan, hooks)
            }),
            PregameStep::ChooseFirstCharacter => branches
                .into_iter()
                .filter_map(|(current, probability)| {
                    choose_first_characters(&current, choices.me_first, choices.opp_first)
                        .map(|next| (next, probability))
                })
                .collect(),
        };
        if branches.is_empty() {
            break;
        }
    }
    branches
}

/// 对每条分支再做一步「多分支变换」，概率相乘。
fn expand(
    branches: Vec<(DefState, Fraction)>,
    step: impl Fn(&DefState) -> Vec<(DefState, Fraction)>,
) -> Vec<(DefState, Fraction)> {
    let mut next: Vec<(DefState, Fraction)> = Vec::new();
    for (state, probability) in branches {
        for (branch, branch_probability) in step(&state) {
            next.push((branch, probability.exact_mul(branch_probability)));
        }
    }
    next
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::families::action_card::zones::pile::PileState;
    use crate::resources::dice::CostComponent;
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{characters, entity, hand, n, pile, CARD_A};

    const ARCANE: CardId = CardId::new(330001);

    /// 每张牌 1 个任意骰的基础费用（测试用）。
    fn base_one(_card: CardId) -> Vec<CostComponent> {
        vec![CostComponent::AnyColor(n(1))]
    }

    fn arcane_none(_card: CardId) -> bool {
        false
    }

    fn hooks<'a>(
        arcane: &'a dyn Fn(CardId) -> bool,
        base_cost: &'a dyn Fn(CardId) -> Vec<CostComponent>,
    ) -> PregameHooks<'a> {
        PregameHooks { arcane, base_cost }
    }

    fn side(pile: PileState, characters_zone: CharacterZone) -> PlayerDefs {
        PlayerDefs::new()
            .with_pile(pile)
            .with_characters(characters_zone)
    }

    fn state() -> DefState {
        DefState::from_players(
            side(
                pile(&[101, 102, 103, 104, 105, 106, 107, 108, 109, 110]),
                characters(&[201, 202, 203]),
            ),
            side(
                pile(&[301, 302, 303, 304, 305, 306, 307, 308, 309, 310]),
                characters(&[401, 402]),
            ),
        )
    }

    #[test]
    fn pregame_draws_five_and_chooses_first_character() {
        let before = state();
        let arcane = arcane_none;
        let cost = base_one;
        let pregame_hooks = hooks(&arcane, &cost);
        // 202 不是默认出战角色（默认是 201），用来验证「显式选首发」；401 是敌方默认出战角色。
        let choices = PregameChoices::new(n(202), n(401));

        let branches = pregame(&before, &choices, &pregame_hooks);
        assert_eq!(branches.len(), 1);
        let (after, probability) = branches.first().expect("一条分支");
        assert!(probability.is_one());

        // 双方各抓 5 张、牌库各剩 5 张
        assert_eq!(after.me().hand().len(), Nat::new(5));
        assert_eq!(after.opp().hand().len(), Nat::new(5));
        assert_eq!(after.me().pile().len(), Nat::new(5));
        assert_eq!(after.opp().pile().len(), Nat::new(5));
        // 抓的是各自牌库顶的 5 张（两个牌库互不影响）
        for value in [101, 102, 103, 104, 105] {
            assert!(after.me().hand().contains(entity(value)));
        }
        for value in [301, 302, 303, 304, 305] {
            assert!(after.opp().hand().contains(entity(value)));
        }
        // 首发角色
        assert_eq!(after.me().characters().active_entity(), Some(n(202)));
        assert_eq!(after.opp().characters().active_entity(), Some(n(401)));
        // 每张手牌都有当前费用（基础 1 费）
        assert_eq!(after.me().costs().len(), Nat::new(5));
        for instance in after.me().hand().entries() {
            assert_eq!(
                after.me().cost_of(instance.entity).expect("有费用").total(),
                Nat::new(1)
            );
        }
        // 纯函数：入参一个字节都没动
        assert_eq!(before.me().hand().len(), Nat::ZERO);
        assert_eq!(before.me().characters().active_entity(), Some(n(201)));
    }

    #[test]
    fn opening_hand_prefers_arcane_card_on_top() {
        // 牌库顶是秘传牌：起手第 1 步先按秘传抓（draw_opening），它一定在手里。
        let me_pile = PileState::from_fixed(
            std::iter::once((entity(111), ARCANE))
                .chain((112..=120).map(|value| (entity(value), CARD_A)))
                .collect(),
        );
        let before = DefState::from_players(
            side(me_pile, characters(&[201])),
            side(
                pile(&[301, 302, 303, 304, 305, 306]),
                characters(&[401]),
            ),
        );
        let arcane = |card: CardId| card == ARCANE;
        let cost = base_one;
        let pregame_hooks = hooks(&arcane, &cost);
        let drawn = draw_opening_hands(&before, &pregame_hooks);
        assert_eq!(drawn.len(), 1);
        let (after, probability) = drawn.first().expect("一条分支");
        assert!(probability.is_one());
        assert_eq!(after.me().hand().len(), Nat::new(5));
        assert!(after.me().hand().contains(entity(111)));
    }

    #[test]
    fn mulligan_swaps_cards_and_returns_them_to_the_pile() {
        let arcane = arcane_none;
        let cost = base_one;
        let pregame_hooks = hooks(&arcane, &cost);

        // 先造一个「已经抓满 5 张」的局面：10 张牌库抓 5 -> 手牌 5 / 牌库 5。
        let full_pile = pile(&[101, 102, 103, 104, 105, 106, 107, 108, 109, 110]);
        let hand_before = HandState::new();
        let branch = full_pile
            .draw_many(&hand_before, Nat::new(5))
            .into_iter()
            .next()
            .expect("一条分支")
            .0;
        let me = PlayerDefs::new()
            .with_pile(branch.pile)
            .with_hand(branch.hand)
            .with_characters(characters(&[201]))
            .with_synced_hand_costs(&cost);
        let before = DefState::from_players(me, PlayerDefs::new());

        let first = before.me().hand().entries().first().expect("有手牌").entity;
        let second = before.me().hand().entries().get(1).expect("有手牌").entity;
        let branches = mulligan(&before, &[first, second], &[], &pregame_hooks);
        assert_eq!(branches.len(), 1);
        let (after, probability) = branches.first().expect("一条分支");
        assert!(probability.is_one());

        // 换掉 2 张 -> 抓回 2 张：手牌仍 5 张；洗回 2 张 -> 牌库仍 5 张
        assert_eq!(after.me().hand().len(), Nat::new(5));
        assert_eq!(after.me().pile().len(), Nat::new(5));
        // 换掉的牌不在手上、回到牌库；没换的还在手上
        assert!(!after.me().hand().contains(first));
        assert!(!after.me().hand().contains(second));
        assert!(after.me().pile().contains(first));
        assert!(after.me().pile().contains(second));
        // 费用表跟着手牌同步（仍 5 条，且都是基础 1 费）
        assert_eq!(after.me().costs().len(), Nat::new(5));
        for instance in after.me().hand().entries() {
            assert_eq!(
                after.me().cost_of(instance.entity).expect("有费用").total(),
                Nat::new(1)
            );
        }
        // 纯函数：入参不变
        assert_eq!(before.me().pile().len(), Nat::new(5));
        assert!(before.me().hand().contains(first));
    }

    #[test]
    fn mulligan_with_nothing_to_swap_changes_nothing() {
        let arcane = arcane_none;
        let cost = base_one;
        let pregame_hooks = hooks(&arcane, &cost);
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_pile(pile(&[101, 102, 103, 104, 105]))
                .with_hand(hand(&[201, 202]))
                .with_characters(characters(&[301])),
            PlayerDefs::new(),
        );
        let branches = mulligan(&before, &[], &[], &pregame_hooks);
        assert_eq!(branches.len(), 1);
        let (after, probability) = branches.first().expect("一条分支");
        assert!(probability.is_one());
        // 没换牌：手牌 / 牌库都不动；费用表按当前手牌同步出来
        assert_eq!(after.me().hand(), before.me().hand());
        assert_eq!(after.me().pile(), before.me().pile());
        assert_eq!(after.me().costs().len(), Nat::new(2));
    }

    #[test]
    fn choose_first_accepts_current_active_but_rejects_unknown_character() {
        let before = state();
        // 默认出战角色（第一张）-> 成功的无操作
        let same = choose_first_characters(&before, n(201), n(401)).expect("已是出战角色也算成功");
        assert_eq!(same, before);
        // 选一个不在角色区里的角色 -> 失败、不改状态
        assert!(choose_first_characters(&before, n(999), n(401)).is_none());
        assert!(choose_first_characters(&before, n(202), n(999)).is_none());
        assert_eq!(before.me().characters().active_entity(), Some(n(201)));
    }

    #[test]
    fn pregame_fails_as_a_whole_when_first_character_is_illegal() {
        let before = state();
        let arcane = arcane_none;
        let cost = base_one;
        let pregame_hooks = hooks(&arcane, &cost);
        let choices = PregameChoices::new(n(999), n(401));
        assert!(pregame(&before, &choices, &pregame_hooks).is_empty());
        // 整个流程不改入参
        assert_eq!(before.me().hand().len(), Nat::ZERO);
    }

    #[test]
    fn opening_keeps_both_sides_independent() {
        // 只有我方有牌库 / 角色区；敌方全空 -> 敌方抓 0 张，我方形不受影响。
        let before = DefState::from_players(
            side(
                pile(&[101, 102, 103, 104, 105, 106]),
                characters(&[201]),
            ),
            PlayerDefs::new(),
        );
        let arcane = arcane_none;
        let cost = base_one;
        let pregame_hooks = hooks(&arcane, &cost);
        let drawn = draw_opening_hands(&before, &pregame_hooks);
        assert_eq!(drawn.len(), 1);
        let (after, probability) = drawn.first().expect("一条分支");
        assert!(probability.is_one());
        assert_eq!(after.me().hand().len(), Nat::new(5));
        assert_eq!(after.opp().hand().len(), Nat::ZERO);
        assert_eq!(after.opp().pile().len(), Nat::ZERO);
    }
}