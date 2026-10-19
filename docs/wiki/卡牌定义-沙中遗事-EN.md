# Card Research: Lost Legacies in the Sand

> [[卡牌定义-沙中遗事|中文]] · English

*Scope: the event card Lost Legacies in the Sand - card text (Chinese / English + keyword expansion + related rulebook entries), implementation code (current real source + missing work), test records, and notes.*

---

## 1. Card Text

### 1.1 Card data

| Item | Content |
|---|---|
| Name | Lost Legacies in the Sand |
| Type | Event card |
| Tags | Arcane Legend / Undiscoverable |
| Cost | 1 Arcane Legend cost |

> [!NOTE]
> Type, tags, and cost are listed one item per row; this section contains only card-face text and essential metadata.

### 1.2 Description

The two card modes are written in three lines: mode name / role and condition / the card description as a `>` quote (copied exactly, original punctuation kept).

Mode 1
Affects the opponent's hand; condition: highest Current Elemental Dice Cost
> Place 1 card with the highest Current Elemental Dice Cost from your opponent's Hand at the bottom of their deck.

Mode 2
Affects your hand
> Place all cards from your Hand at the bottom of your deck and then draw the same number of cards plus 1.

Restriction
Arcane Legend: one per player per match, guaranteed in the starting hand
> (Only one "Arcane Legend" card can be played per match. This card will be in your starting hand.)

### 1.3 Keyword expansion

| Keyword | Description | Where it appears |
|---|---|---|
| Select | Choose 1 card from a specific set and activate its effect. | choose one of the two modes |
| Current Elemental Dice Cost | Cost Increase / Cost Reduction attached to an Action Card changes its current cost; play-time dice-cost reductions do not affect it. | the value compared by mode 1 |
| Arcane Edict | Each player can only use 1 Arcane Legend card per match; an Arcane Legend can only be played while the Edict exists; playing it consumes the chance. | per-match limit |

### 1.4 Related rulebook entries (excerpts)

> Rulebook, Taking Actions, Fast Action [Play Card]:
> Use Elemental Dice to pay the relevant cost and play a card.
> [Event Cards] Play these to immediately trigger all their effects.

> Rulebook, Game Areas V, Draw Pile:
> When drawing cards, draw Action Cards from the top of the Draw Pile and add them to your Hand. If the Draw Pile has no card left, then no new card will be added to Hand.

> Rulebook, Game Areas VI, Hand:
> Your Hand is limited to 10 cards, after which you will automatically discard any cards you draw.

> Rulebook, Special Terms, Arcane Legend:
> Each player can only use 1 Arcane Legend card per match.
> The Arcane Legend card will be in your starting hand. (If you have more Arcane Legend cards in your deck than there are starting hand cards, random Arcane Legend cards will appear in your starting hand.)

> Rulebook, Other Rules, Settlement Order: when multiple card effects need to settle at the same timing: 1. resolve the current player's effects first, then the opponent's; 2. within one player, by source category; 3. within a category, by entry order.

## 2. Implementation Code

### 2.1 Status and key conclusions

> [!WARNING]
> **The card definition and the choose-one branch are not implemented.** What follows is the landed generic primitives (pile bottom / draw / Arcane bookkeeping).
> Wiring still needs: the card definition, the choose-one branch, and mode 1 target selection - see 2.5.

Mode 2 settlement summary (matching the card text): at step start after playing, the hand size is N = H - 1 (H = hand size before playing; Lost Legacies has left the hand);
place each hand card at the bottom (**processing order new -> old**, so **later-acquired cards end up higher**, D273), then draw **N + 1** from the top of the pile. The hand returns to H.

> [!IMPORTANT]
> **D273 bottom order is bound to the specific primitive**: `put_bottom` appends to the very bottom, so processing new -> old directly satisfies "later-acquired higher";
> `put_bottom_many` inserts at the top of the bottom region (**later-processed is higher**), so to reach the same final order the `order` must be inverted (**old -> new**).
> Feeding D273's "new -> old" directly to `put_bottom_many` reverses the final order (the test `put_bottom_many_respects_order_function` pins this semantics).

### 2.2 Pile bottom and draw (real source)

`new/crates/definitions/src/families/action_card/zones/pile/mod.rs::put_bottom` - single-card bottom (true append to the end):

```rust
    /// **置底**：把这张牌移到牌库最下面。
    ///
    /// 语义（D89/D93）：
    /// 1. 实体不在牌库 -> None（状态不变）；
    /// 2. 先按「移除」处理：它自己的状态随位置一起消失；
    /// 3. 追加到 entries 末尾；尾部追加不影响其它位置。最后一段的区间长度 +1
    ///    （同 pool 的随机段一起 +1）。
    pub fn put_bottom(&self, entity: EntityId) -> Option<PileState> {
        let position = self.position_of(entity)?;
        let mut next = self.clone();
        let entry = next.remove_at(position)?;
        next.append_entry(entry);
        next.stats.on_put_bottom();
        Some(next)
    }
```

`pile/mod.rs::put_bottom_many` - multi-card bottom (only a homogeneous `&[(EntityId, CardId)]` list; order supplied by the `order` function):

```rust
    /// **通用「把若干新牌放牌组底」**（D209）：放几张、按什么顺序放都由调用方指定。
    ///
    /// - cards：要放到底部的新牌（实体号 + 卡 id；已在牌库里的 / 重复的会被忽略）；
    /// - order：第 step 次（0 起）放 cards 里的哪一张（返回 Some(cards 的下标)）；None 表示结束；
    ///   越界 / 重复下标会被忽略；
    /// - 放置语义（例：330012「将我方所有手牌置于牌组底」）：逐张放到**底部区域顶端**——
    ///   即每次都插在「本次已经放好的那叠」的最上面，所以**后放的在底区更上面**；
    /// - 卡 id / 实体都来自调用方，本函数不建映射表。
    ///
    /// 失败语义：没有一张真正放进去 -> None（状态不变）。
    pub fn put_bottom_many(
        &self,
        cards: &[(EntityId, CardId)],
        order: impl Fn(Nat) -> Option<Nat>,
    ) -> Option<PileState> {
        if cards.is_empty() {
            return None;
        }
        let card_count = Nat::new(cards.len() as _);
        let anchor = self.len();
        let mut next = self.clone();
        let mut used: Vec<Nat> = Vec::new();
        let mut placed = Nat::ZERO;
        let mut step = Nat::ZERO;
        while step < card_count {
            let Some(index) = order(step) else {
                break;
            };
            step = nadd!(step, Nat::new(1));
            if index >= card_count || used.contains(&index) {
                continue;
            }
            let Some((entity, card)) = at(cards, index).copied() else {
                continue;
            };
            if next.contains(entity) {
                continue;
            }
            used.push(index);
            next.insert_entry(anchor, PileEntry::new(entity, card));
            placed = nadd!(placed, Nat::new(1));
        }
        if placed.get() == 0 {
            return None;
        }
        let mut count = Nat::ZERO;
        while count < placed {
            next.stats.on_put_bottom();
            count = nadd!(count, Nat::new(1));
        }
        Some(next)
    }
```

`pile/mod.rs::draw_top` - draw the top card (fixed segment = segment top; random segment = uniform within the pool):

```rust
    /// 按类型 / 标签抽最上面一张；matches 是调用方注入的谓词（拿 CardId 查类型 / 标签，D20/D26）。
    ///
    /// 命中规则（D88）：固定段 = 只看段顶那张；随机段 = 池内匹配者等概率。
    pub fn draw_top(
        &self,
        hand: &HandState,
        matches: &dyn Fn(CardId) -> bool,
    ) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_one(self, hand, Some(matches))
    }
```

`pile/mod.rs::draw_many` - draw n cards (fewer if the pile is short; returns exact probability branches):

```rust
    /// 抓 **n 张**（不够则少给）；返回折叠后的分支列表（D83/D84/D85）。
    ///
    /// - 一张都抓不到（空库 / n = 0）-> 空 Vec；
    /// - 折叠口径见 parse.rs 文件头；
    /// - 手牌溢出时退化成有序分支（概率仍精确）。**实际对局认为不会发生溢出（D99）**，
    ///   这一句只是安全兜底：保证分支后继状态与概率一致，不是待补功能。
    pub fn draw_many(&self, hand: &HandState, n: Nat) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_many(self, hand, n)
    }
```

### 2.3 Keyword: all-hand-to-bottom + dynamic draw (real source)

`new/crates/definitions/src/keywords/op.rs` - all-hand-bottom arguments and default order:

```rust
/// **把某一方全部手牌置于牌组底**（沙中遗事；D208）。
///
/// 顺序口径（用户口径）：**后处理的牌在牌组底更上面**。处理顺序**不写死**：由调用方
/// 注入 [OrderFn]（D209），默认 [default_put_bottom_order]（按手牌容器顺序从后往前）。
/// **待实测**（手牌无序，顺序只按容器顺序）。
#[derive(Clone, Copy, Debug)]
pub struct PutHandToPileBottomArgs {
    /// 哪一方（相对视角方）。
    pub side: SideScope,
    /// 处理序 -> 手牌容器下标（注入顺序，D209）。
    pub order: OrderFn,
}

impl PartialEq for PutHandToPileBottomArgs {
    fn eq(&self, other: &PutHandToPileBottomArgs) -> bool {
        self.side == other.side && self.order as usize == other.order as usize
    }
}

impl Eq for PutHandToPileBottomArgs {}

/// 默认置底顺序：**从后往前**处理手牌（后处理的牌最终在牌组底更上面）。
///
/// 输入 (处理序号 i, 手牌数 n)，输出手牌下标 = n - 1 - i。
pub fn default_put_bottom_order(index: Nat, total: Nat) -> Nat {
    if total == Nat::ZERO {
        return Nat::ZERO;
    }
    gi_genius_invokation_engine::nsub!(
        gi_genius_invokation_engine::nsub!(total, Nat::new(1)),
        index
    )
}

impl PutHandToPileBottomArgs {
    /// 用默认顺序建参数。
    pub const fn new(side: SideScope) -> PutHandToPileBottomArgs {
        PutHandToPileBottomArgs {
            side,
            order: default_put_bottom_order,
        }
    }
}
```

`op.rs` - dynamic draw count (`HandAtStepStartPlus(1)` = hand size at step start + 1):

```rust
/// **抽几张**的表达（有动态量：沙中遗事抓「原手牌数 + 1」）。
///
/// 动态度在**一步开始时**取（D204：一步是原子结算单位），所以「先把手牌放回牌组、
/// 再抓原手牌数 + 1」这种写法不会因为手牌已经离手而算错。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawCount {
    /// 固定张数。
    Fixed(Nat),
    /// **本步开始时**那一方手牌数 + 偏移（沙中遗事 = +1）。
    HandAtStepStartPlus(Nat),
}

impl DrawCount {
    /// 固定一张。
    pub const fn one() -> DrawCount {
        DrawCount::Fixed(Nat::new(1))
    }
}

/// **抽牌**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DrawCardsArgs {
    /// 哪一方抽（相对视角方）。
    pub side: SideScope,
    /// 抽几张（不够则少给）。
    pub count: DrawCount,
}
```

`keywords/keyword.rs::put_hand_to_pile_bottom` - executing all-hand-to-bottom:

```rust
/// **把某一方全部手牌置于牌组底**（沙中遗事；D208）。
///
/// 顺序口径：**后处理的牌在牌组底更上面**。实现上按手牌容器顺序**从后往前**追加到
/// 牌库末尾（末尾 = 底），于是容器里靠后的手牌最终落在底区更上面。
///
/// **待接入 / 待实测**：
/// - 牌库区没有「把不在牌库的牌插到底部」的公开原语（families 本块只读），这里用
///   PileState::from_fixed 重建整副牌库 —— 会丢掉随机段 / 公共状态列表；
///   需要牌库区补一个 insert_bottom_with(instance) 原语；
/// - 手牌容器声明无序，这里只按容器顺序（可复现，不承诺是玩家选择顺序）。
pub fn put_hand_to_pile_bottom(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &PutHandToPileBottomArgs,
) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    for side in sides.iter().take(count) {
        let player = player_of(&current, *side);
        let hand_entries = player.hand().entries();
        let total = Nat::new(hand_entries.len() as _);
        if total == Nat::ZERO {
            continue;
        }
        let mut instances: Vec<(EntityId, CardId)> = player
            .pile()
            .entries()
            .iter()
            .map(|entry| (entry.entity, entry.card))
            .collect();
        // 顺序由调用方注入（D209）；默认「从后往前」，于是后处理的牌在底区更上面。
        let mut processed = Nat::ZERO;
        while processed < total {
            let index = (args.order)(processed, total);
            if let Some(instance) = hand_entries.get(index.get() as usize) {
                instances.push((instance.entity, instance.card));
            }
            processed = nadd!(processed, Nat::new(1));
        }
        let pile = PileState::from_fixed(instances);
        let next_player = player
            .clone()
            .with_pile(pile)
            .with_hand(HandState::new());
        current = set_player(&current, *side, next_player);
    }
    KeywordResult::single(current, PendingSet::new(), env.alloc())
}
```

`keyword.rs::draw_cards` - executing the draw:

```rust
/// **抽牌**：每张抽取的折叠分支原样展开（PileState::draw_many 的精确概率）。
pub fn draw_cards(state: &DefState, env: &EffectEnv<'_>, args: &DrawCardsArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut branches: Vec<KeywordBranch> = vec![KeywordBranch::determined(state.clone())];
    for side in sides.iter().take(count) {
        let draw_count = match args.count {
            DrawCount::Fixed(value) => value,
            DrawCount::HandAtStepStartPlus(extra) => nadd!(env.step_start_hand(*side), extra),
        };
        if draw_count == Nat::ZERO {
            continue;
        }
        let mut next_branches: Vec<KeywordBranch> = Vec::new();
        for branch in &branches {
            let player = player_of(&branch.state, *side);
            let draws = player.pile().draw_many(player.hand(), draw_count);
            if draws.is_empty() {
                next_branches.push(branch.clone());
                continue;
            }
            for (draw, probability) in draws {
                let next_player = player.clone().with_pile(draw.pile).with_hand(draw.hand);
                next_branches.push(KeywordBranch::new(
                    set_player(&branch.state, *side, next_player),
                    branch.probability.exact_mul(probability),
                    branch.pending.clone(),
                ));
            }
        }
        branches = next_branches;
    }
    KeywordResult {
        branches,
        alloc: env.alloc(),
    }
}
```

`new/crates/definitions/src/keywords/effect.rs` - hand sizes at step start (source of the dynamic count):

```rust
    /// 填「本步开始时的双方手牌数」（动态张数用；步执行器在步开始时自动填）。
    pub fn with_step_start_hands(mut self, me: Nat, opp: Nat) -> EffectEnv<'a> {
        self.step_start_me_hand = me;
        self.step_start_opp_hand = opp;
        self
    }

    /// 本步开始时某一方的手牌数。
    pub fn step_start_hand(&self, side: Perspective) -> Nat {
        match side {
            Perspective::Me => self.step_start_me_hand,
            Perspective::Opp => self.step_start_opp_hand,
        }
    }
```

> [!WARNING]
> Known gaps (partly admitted in the source comments): `put_hand_to_pile_bottom` rebuilds the whole pile with `PileState::from_fixed` (**dropping segments / states / pending**);
> it only calls `with_hand(HandState::new())` and does **not** clean `HandCosts` / `HandCardStates`; and its order is only the hand container order (`HandState` is unordered, D90 / `留空.md` #133).

### 2.4 Arcane bookkeeping (real source)

`new/crates/definitions/src/families/status/arcane_edict.rs` - the per-player Arcane Edict container:

*Keyword codes in comments were replaced with names; code lines unchanged.*

```rust
/// 每位牌手一份的**秘传揭令容器**（使用机会 + 已打出张数）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArcaneEdict {
    /// 秘传揭令是否仍存在（存在 = 还能打出秘传）。
    available: bool,
    /// 本局该牌手已打出的秘传张数。
    played: Nat,
}

impl ArcaneEdict {
    /// 整局每位牌手最多打出 1 张秘传（规则书 + 秘传揭令；复用规则层常量）。
    pub const PER_GAME: Nat = ARCANE_LEGEND_PER_GAME;

    /// 开局状态：揭令存在、还没有打出过秘传。
    pub const fn new() -> ArcaneEdict {
        ArcaneEdict {
            available: true,
            played: Nat::ZERO,
        }
    }

    /// 秘传揭令是否仍存在。
    pub const fn available(&self) -> bool {
        self.available
    }

    /// 本局已打出的秘传张数。
    pub const fn played(&self) -> Nat {
        self.played
    }

    /// 现在能不能打出秘传（复用规则层的判定顺序：先看已打出张数，再看揭令）。
    pub fn can_play(&self) -> ArcanePlay {
        can_play(self.played, self.available)
    }

    /// 只**消耗**揭令（不打牌；供「移除揭令」类效果使用，待实测）。
    ///
    /// 本来就不存在 -> None（状态不变）。
    pub const fn consume(&self) -> Option<ArcaneEdict> {
        if self.available {
            Some(ArcaneEdict {
                available: false,
                played: self.played,
            })
        } else {
            None
        }
    }

    /// 打出秘传：允许时记录「已打出 +1」并按 秘传揭令 消耗揭令；不允许 -> None（状态不变）。
    pub fn record_play(&self) -> Option<ArcaneEdict> {
        match self.can_play() {
            ArcanePlay::Allowed => Some(ArcaneEdict {
                available: if consumes_edict() {
                    false
                } else {
                    self.available
                },
                played: nadd!(self.played, Nat::new(1)),
            }),
            ArcanePlay::AlreadyPlayed | ArcanePlay::NoEdict => None,
        }
    }

    /// 这个容器的状态身份（供状态列表 / 查询按静态身份指针使用）。
    pub fn status_identity(&self) -> &'static dyn StateIdentity {
        &ARCANE_EDICT
    }
}
```

`new/crates/definitions/src/rules/ops/arcane.rs` - rule-layer check (one per match + live Edict):

```rust
/// 整局每位牌手最多打出 1 张秘传（规则书第 1 条 + 关键字 #70）。
pub const ARCANE_LEGEND_PER_GAME: Nat = Nat::new(1);

/// 秘传一定在起始手牌中（规则书第 2 条；数量超过起始手牌数时只随机出现一部分）。
pub const ARCANE_LEGEND_IN_OPENING_HAND: bool = true;

/// 是否「所有秘传都在起始手牌中」：牌组里的秘传数不超过起始手牌数时成立（规则书第 2 条）。
pub fn all_arcane_guaranteed(arcane_in_deck: Nat, opening_hand: Nat) -> bool {
    arcane_in_deck <= opening_hand
}

/// 起始手牌中出现的秘传张数：等于秘传总数；超过起始手牌数时取起始手牌数（随机抽）。
pub fn arcane_in_opening_hand(arcane_in_deck: Nat, opening_hand: Nat) -> Nat {
    if arcane_in_deck < opening_hand {
        arcane_in_deck
    } else {
        opening_hand
    }
}

/// 秘传：起手一定在手上（用于把「秘传」标签转成起手抓牌谓词）。
pub fn guaranteed_in_opening_hand() -> bool {
    ARCANE_LEGEND_IN_OPENING_HAND
}

/// 打出判定结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArcanePlay {
    /// 可以打出。
    Allowed,
    /// 本局已经打出过秘传（每人 1 张）。
    AlreadyPlayed,
    /// 秘传揭令已消耗 / 不存在。
    NoEdict,
}

/// 打出判定：played_this_game = 本局该牌手已打出的秘传张数，
/// edict_available = 秘传揭令是否仍存在。
pub fn can_play(played_this_game: Nat, edict_available: bool) -> ArcanePlay {
    if played_this_game >= ARCANE_LEGEND_PER_GAME {
        ArcanePlay::AlreadyPlayed
    } else if !edict_available {
        ArcanePlay::NoEdict
    } else {
        ArcanePlay::Allowed
    }
}

/// 打出后是否消耗秘传揭令（关键字 #70：会消耗）。
pub fn consumes_edict() -> bool {
    true
}
```

`new/crates/definitions/src/flow/play_card.rs` - Arcane play entry (pre-check + bookkeeping only on success):

*Keyword codes in comments were replaced with names; code lines unchanged.*

```rust
/// **秘传打出的结果**（Z12 / 秘传揭令）：打牌结果 + **打出后**的秘传揭令容器。
#[derive(Clone, Debug)]
pub struct ArcanePlayOutcome {
    /// 普通打牌流程的完整结果（分支 / 前后派发 / 时机 / 记录）。
    pub play: PlayCardOutcome,
    /// 打出后的秘传揭令容器（已 record_play：本局张数 +1、按 秘传揭令 消耗揭令）。
    pub edict: ArcaneEdict,
}

/// **打出「秘传」行动牌**（我方；Z12 / 秘传揭令）。
///
/// - 打牌**前置**：edict.can_play() 必须是 Allowed（揭令仍存在 + 本局未打出过）；
///   否则 Err(ArcaneNotPlayable(原因))、入参不变；
/// - 打牌**后置**：成功结算后 record_play（张数 +1、消耗揭令），新容器放进
///   ArcanePlayOutcome.edict（调用方写回自己持有的揭令状态）。
///
/// 与 play_card_me 的区别只在「秘传判定 + 揭令记账」：普通牌不要走这个入口。
pub fn play_card_me_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, true, false)
}

/// **打出「秘传」行动牌**（我方；按牌是否带「战斗行动」规则判时机；S8.8）。
pub fn play_card_me_arcane_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, true, has_combat_action_tag)
}

/// **打出「秘传」行动牌**（敌方；与 play_card_me_arcane 共用一份实现，D35）。
pub fn play_card_opp_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, false, false)
}

/// **打出「秘传」行动牌**（敌方；按牌是否带「战斗行动」规则判时机）。
pub fn play_card_opp_arcane_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, false, has_combat_action_tag)
}

/// 秘传打牌的共用实现：前置判定（秘传揭令）-> 普通打牌流程 -> 后置 record_play。
fn play_card_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    playing_me: bool,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    // 0) 手牌检查优先（与普通入口一致）：不在手牌 -> NotInHand，先于秘传 / 条件判定。
    let user = if playing_me { state.me() } else { state.opp() };
    if !user.hand().contains(request.entity) {
        return Err(PlayCardError::NotInHand);
    }
    // 1) 前置：秘传揭令仍存在 + 本局该牌手还没打出过秘传（规则书·特殊词条 秘传 + 秘传揭令）。
    let verdict = edict.can_play();
    if verdict != ArcanePlay::Allowed {
        return Err(PlayCardError::ArcaneNotPlayable(verdict));
    }
    // 2) 普通打牌流程（纯函数；失败 -> 入参不变、不消耗揭令）。
    let play = play(state, env, request, playing_me, has_combat_action_tag, None)?;
    // 3) 后置：成功才记账（张数 +1 并消耗揭令）。前置已确认 Allowed，这里不会失败。
    let next = edict
        .record_play()
        .ok_or(PlayCardError::ArcaneNotPlayable(edict.can_play()))?;
    Ok(ArcanePlayOutcome { play, edict: next })
}
```

`play_card.rs::play_card_arcane` - shared implementation (whole function):

*Keyword codes in comments were replaced with names; code lines unchanged.*

```rust
/// 秘传打牌的共用实现：前置判定（秘传揭令）-> 普通打牌流程 -> 后置 record_play。
fn play_card_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    playing_me: bool,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    // 0) 手牌检查优先（与普通入口一致）：不在手牌 -> NotInHand，先于秘传 / 条件判定。
    let user = if playing_me { state.me() } else { state.opp() };
    if !user.hand().contains(request.entity) {
        return Err(PlayCardError::NotInHand);
    }
    // 1) 前置：秘传揭令仍存在 + 本局该牌手还没打出过秘传（规则书·特殊词条 秘传 + 秘传揭令）。
    let verdict = edict.can_play();
    if verdict != ArcanePlay::Allowed {
        return Err(PlayCardError::ArcaneNotPlayable(verdict));
    }
    // 2) 普通打牌流程（纯函数；失败 -> 入参不变、不消耗揭令）。
    let play = play(state, env, request, playing_me, has_combat_action_tag, None)?;
    // 3) 后置：成功才记账（张数 +1 并消耗揭令）。前置已确认 Allowed，这里不会失败。
    let next = edict
        .record_play()
        .ok_or(PlayCardError::ArcaneNotPlayable(edict.can_play()))?;
    Ok(ArcanePlayOutcome { play, edict: next })
}
```

### 2.5 Not implemented / what is missing (by capability)

| Capability | Status | Missing |
|---|---|---|
| Card definition | **not implemented** | `ActionCardDef` (event) + the two-mode `Step`s |
| "Choose one" branch | **not implemented** | `Step` is a static op list with no choose-one-effect op; decide before building the request, or add a choose-one op |
| Mode 1: pick the opponent's highest-current-cost hand card | **not implemented** | a "place one hand card at the bottom" keyword op + max-`CurrentCost::total()` selection + tie policy |
| Mode 2 wiring | primitives exist, **not wired to a card** | compose `PutHandToPileBottom` + `DrawCards(HandAtStepStartPlus(1))` into one step; migrate to `put_bottom_many` (invert order) and clean `HandCosts` / `HandCardStates` |
| Hand acquisition order (D273 precondition) | **missing** | `HandState` unordered (D90); `留空.md` #133 needs an acquisition index / timestamp |
| Arcane cost (`GCG_COST_LEGEND`) | **missing** | `CostComponent` has only three categories (D168); no LEGEND mapping |
| Pile-enter / hand-enter triggers | **missing** | pile has only stats / record, no trigger registry |
| Mulligan | **not implemented** | `PregameStep::Mulligan` is pending |

## 3. Tests

| Item | Position / operation | In-game result | Our result | Conclusion |
|---|---|---|---|---|
| Bottom order (D273) | Hand of 5, acquisition order a<b<c<d<e; pile of 10; play Lost Legacies, choose mode 2 | pending test (expected bottom region top -> bottom = e,d,c,b,a) | current keyword implementation uses container order back-to-front (matches if container order = acquisition order); test `put_hand_to_pile_bottom_keeps_later_cards_above` verifies later container cards are higher; no acquisition order (#133) | partially verified (container order); **pending test** |
| `put_bottom` order | Pile `[0,1]`; `put_bottom(10)` then `put_bottom(11)` (10 newer, 11 older) | n/a (engineering) | top -> bottom `[0,1,10,11]` (10 above 11, newer above) | verified (matches D273) |
| `put_bottom_many` order | Pile `[0,1]`; cards = `[10(newer), 11(older)]` | n/a (engineering) | order 0,1 (new -> old) -> `[0,1,11,10]` (older above, **contradicts D273**); order 1,0 (old -> new) -> `[0,1,10,11]` (newer above, matches D273); test `put_bottom_many_respects_order_function` | verified (**D273 needs inverted order**) |
| Draw-count timing | Hand H=4 (including Lost Legacies), pile 10; play mode 2 | pending test (expected to draw 4 = H) | `HandAtStepStartPlus(1)` captures N=3 at step start -> draws 4; no card-level test (code derivation only) | code derivation matches; **pending test** |
| Does the draw include just-placed cards | Hand H=4, pile only 2; play mode 2 | pending test | deterministically draws just-placed c and b (enters the bottom region when the pile above is insufficient, starting with later-acquired cards) | **pending test** |
| Empty-hand degenerate | Hand contains only Lost Legacies; play mode 2 | pending test (expected draw 1) | N=0 -> draw 0+1=1 (code derivation) | **pending test** |
| Arcane frequency | One Arcane already played this match; try to play another | rejected (one per player per match, 秘传揭令) | `flow/play_card.rs` test `arcane_play_consumes_the_edict_and_rejects_the_second_play` passes | match (engineering verified) |
| Arcane in opening hand | Deck contains 1 Arcane | guaranteed in the starting hand | `HandState::draw_opening` draws it first; `opening_hand_prefers_arcane_cards` passes | match (engineering verified) |
| Mode 1 tie for highest | Opponent has 2 cards tied at the highest current cost; play mode 1 | pending test | not implemented | **pending test / not implemented** |
| Arcane cost | Inspect playcost | `GCG_COST_LEGEND` x 1; payment method pending test | no `CostComponent` mapping | **pending test / not implemented** |
| Random tail append | Opponent's pile tail is a random segment; place 1 card at the bottom | pending test | `after_append` extends the tail segment -> the placed card joins the random pool | **pending test / pending confirmation** |
| Full primitive test suite | `cd new; cargo test --offline -p gi-genius-invokation-definitions --lib` | n/a | not re-run for this page; test names in 4.3 | engineering item (local test run is authoritative) |

**Reproducible verification steps (in-game / after wiring)**:

1. **Bottom order**: build a 5-card opening hand (record acquisition order a<b<c<d<e) and a pile of at least 10; play Lost Legacies and choose mode 2; check the bottom region top -> bottom is e,d,c,b,a.
2. **Draw count**: hand of 4 (including Lost Legacies), pile of 10; choose mode 2; check that exactly 4 cards are drawn (= hand size before playing).
3. **Just-placed cards**: hand of 4, pile of only 2; choose mode 2; check whether the 4 drawn cards include the 2 just-placed cards and in what order.
4. **Empty-hand degenerate**: play down to only Lost Legacies in hand; play it and choose mode 2; check that 1 card is drawn.
5. **Mode 1 tie**: give the opponent 2 hand cards with the same current cost; play mode 1; check which one is placed at the bottom.
6. **Arcane frequency**: after one Arcane has been played this match, try to play another Arcane; check that it is rejected.

## 4. Notes

### 4.1 Pitfalls

| Pitfall | Detail |
|---|---|
| Feeding D273's new -> old directly to `put_bottom_many` | that primitive makes later-processed cards higher, so the order must be inverted (old -> new) |
| Rebuilding the pile with `from_fixed` | drops segments / states / pending; use `put_bottom_many` (or add `insert_bottom_with`) |
| Not cleaning costs / hand states after bottom placement | current `put_hand_to_pile_bottom` only clears the hand container; `HandCosts` / `HandCardStates` go stale |
| Reading "the same number" as the hand size after placing | the count is captured at **step start** (N = H - 1), not re-counted after the hand is emptied (which would be 0) |
| Assuming the draw never includes just-placed cards | it does when the pile above is insufficient; the rulebook is silent - pending test |
| Treating D273 as rulebook text | the rulebook has no "place at deck bottom" entry; D273 is a user ruling, pending test |
| Treating hand container order as acquisition order | `HandState` is declared unordered (D90); see `留空.md` #133 |

### 4.2 Related D numbers

| No. | Conclusion |
|---|---|
| D204 | a step is atomic; actions in written order; no insertion inside a step, only at step boundaries |
| D208 / D209 | Lost Legacies' one-by-one placement is one step; the order function is injectable |
| D273 | placing multiple cards: later-acquired higher (user ruling, pending test) |
| D264 | the Arcane Edict is per player |
| D90 / D17 | draw Arcane first in the opening hand, then fill to 5; the hand is unordered |

### 4.3 Related pages and tests

| Item | Content |
|---|---|
| [[卡牌定义-EN]] | column index and the 4-section writing requirements |
| [[卡牌定义-爱诺-EN]] | another studied card (character implementation) |
| [[Crate-Cards-EN]] | concrete-card crate |
| [[定义层-Action-Card-EN]] | action cards / pile / hand |
| [[定义层-State-EN]] | match state and areas |
| Related tests | `put_hand_to_pile_bottom_keeps_later_cards_above`, `injected_order_changes_the_bottom_order`, `put_bottom_many_respects_order_function`, `put_bottom_appends_and_drops_own_states`, `draw_many_amount_clamps_to_available`, `opening_hand_prefers_arcane_cards`, `fresh_container_allows_arcane_and_records_play`, `arcane_play_consumes_the_edict_and_rejects_the_second_play` |

---

Last verified: 2026-10-19 (code: `new/crates/definitions/src/families/action_card/zones/pile/mod.rs:put_bottom_many`)

