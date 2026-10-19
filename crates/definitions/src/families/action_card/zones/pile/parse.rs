//! 牌库这块自己的**选择列表 + 去重（折叠）函数**（D26 / D91 / D92）。
//!
//! - D91：每个随机点自己写自己的去重函数、自己传；框架不提供「通用去重」。
//!   牌库抽牌的折叠函数就写在牌库文件夹的 parse.rs 里，**私有于牌库**，别的块不复用。
//! - D92：选择列表类型也放该块的 parse.rs。
//! - D87：引擎 DedupFn 是无上下文的函数指针，拿不到 n / k / 每张牌的键；本轮先枚举
//!   **全部有序序列**（引擎给均匀概率），再在 definitions 侧按等价类**后置合并**、权重相加。
//!   分布与「抽取那一刻折叠」逐位一致；将来引擎 dedup 支持上下文时只改 draw_many 的调用处。
//!
//! 折叠口径（最终版 = D98「按组合折叠」+ D105 状态列表）：
//! - 每张牌的键 = (卡牌 id, 标记键)；标记键 = 该位置在**公共状态列表**里按 pos 过滤出来的
//!   (布尔状态集合, 层数条目集合)（D105；状态身份 = 指针，D95）；
//! - 一条序列的等价类键 = 上面这些键**排序后的多重集（组合）**；
//! - **顺序一律不区分**：两支序列等价 ⟺ 组合相同；
//! - 权重 = 组内原始有序序列条数；概率 = 权重 / P(n,k)（不是均匀）；
//! - 例：池 A/B/C 抽 2、C 有标记 -> 组合 {A,B} / {A,C} / {B,C} -> 3 支，权重 [2,2,2]，概率各 1/3；
//!   三张全同卡 id 无标记抽 2 -> 1 支、概率 1；三张同卡 id 其中一张有标记抽 2
//!   -> 2 支、权重 [2,4]、概率 [1/3,2/3]。
//!
//! 注意：状态列表按 pos 存，抽牌会把牌抽走、位置随之消失；所以折叠键必须在**抽之前**
//! 按实体号记下来（keyed_marks），再拿它给每张抽到的牌配键。
//!
//! 手牌溢出时「哪张进了手牌」随抽取顺序变化（不是组合等价），所以这种情形**不折叠**（退化成有序分支）。

use super::{DrawBranch, PileEntry, PileState, Segment};
use crate::base::CardId;
use crate::families::action_card::status::{Mark, state_address};
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::{fold_equivalent, FoldCard, FoldMark};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{
    enumerate_random, nadd, ndiv, nmul, nsub, EntityId, Nat, SelectionSource,
};

/// 抽牌计划里的一步。
#[derive(Clone, Debug)]
struct DrawStep {
    /// 这一步的来源。
    source: StepSource,
    /// 这一步抽几张。
    count: Nat,
    /// 随机步：计划时池里可用张数（解码有序序列用）。
    size: Nat,
    /// 这一步的可区分有序取值数（固定步 = 1；随机步 = P(size, count)）。
    options: Nat,
}

/// 抽取来源。
#[derive(Clone, Debug)]
enum StepSource {
    /// 固定段：按顺序确定的若干位置。
    Fixed {
        /// 这些位置（按段内顺序）。
        entries: Vec<PileEntry>,
    },
    /// 随机池：抽 count 张，顺序未知。
    Random {
        /// 池分组 id（同源）。
        pool: Nat,
    },
}

/// 抽牌用的初始分支。
fn initial_branch(pile: &PileState, hand: &HandState) -> DrawBranch {
    DrawBranch {
        pile: pile.clone(),
        hand: hand.clone(),
        drawn: Vec::new(),
        drawn_marks: Vec::new(),
    }
}

// ----------------------------------------------------------------------
// 对外（对 pile/mod.rs）的抽取入口
// ----------------------------------------------------------------------

/// 抓一张：从段序列顶部找第一个能提供候选的段。
pub(super) fn draw_one(
    pile: &PileState,
    hand: &HandState,
    matches: Option<&dyn Fn(CardId) -> bool>,
) -> Vec<(DrawBranch, Fraction)> {
    let Some(candidates) = first_draw_candidates(pile, matches) else {
        return Vec::new();
    };
    let choices = DrawOne {
        pile,
        hand,
        candidates,
    };
    enumerate_random(&initial_branch(pile, hand), &choices, false, false, Nat::new(1), None)
}

/// 从指定随机段抓一张（段内均匀随机）。
pub(super) fn draw_from_random(
    pile: &PileState,
    hand: &HandState,
    segment: Nat,
) -> Vec<(DrawBranch, Fraction)> {
    let Some(target) = pile.segment(segment) else {
        return Vec::new();
    };
    let Some((start, len)) = segment_range(*target) else {
        return Vec::new();
    };
    if len.get() == 0 {
        return Vec::new();
    }
    let Some(candidates) = segment_entries(pile, start, len) else {
        return Vec::new();
    };
    let choices = DrawOne {
        pile,
        hand,
        candidates,
    };
    enumerate_random(&initial_branch(pile, hand), &choices, false, false, Nat::new(1), None)
}

/// 抓 n 张（不够则少给）；按 D84 折叠后返回。
pub(super) fn draw_many(
    pile: &PileState,
    hand: &HandState,
    n: Nat,
) -> Vec<(DrawBranch, Fraction)> {
    if n.get() == 0 {
        return Vec::new();
    }
    let steps = build_draw_steps(pile, n);
    if steps.is_empty() {
        return Vec::new();
    }
    let mut total = Nat::new(1);
    let mut drawn_total = Nat::ZERO;
    for step in &steps {
        total = nmul!(total, step.options);
        drawn_total = nadd!(drawn_total, step.count);
    }
    // 状态按 pos 存在公共列表里；抽牌会移除位置，所以折叠键要**抽之前**按实体号记下来。
    let keyed = keyed_marks(pile);
    let choices = OrderedDraw {
        pile,
        hand,
        steps,
        total,
    };
    let branches = enumerate_random(
        &initial_branch(pile, hand),
        &choices,
        false,
        false,
        Nat::new(1),
        None,
    );
    // 安全兜底（D99：实际对局认为不会发生手牌溢出，这里只是防状态/概率不一致）：
    // 手牌溢出会让「哪张进了手牌」随顺序变化，这时不能折叠，直接给有序分支。
    //
    // 原因：组合等价的前提是「顺序不同 -> 后继状态相同」。但空闲格 free 介于 0 和抽数之间时，
    // 先抽到的 free 张进手牌、其余被弃置；同一组合的不同排列会把**不同的牌**放进手牌，
    // 后继状态真的不同（手牌内容不同），所以不能按组合合并，只能保留有序分支。
    // free == 0（全弃置）或 free >= 抽数（全进手牌）时顺序不影响后继状态，才折叠。
    let free = hand.free_slots();
    let fold = free >= drawn_total || free.get() == 0;
    if fold {
        fold_draw_branches(branches, &keyed)
    } else {
        branches
    }
}

/// 按位置舍弃（D94 第 7 条）：固定段单分支；随机段内每张等概率一支。
pub(super) fn discard_at(pile: &PileState, position: Nat) -> Vec<(PileState, EntityId, Fraction)> {
    if position >= pile.len() {
        return Vec::new();
    }
    let random = pile.is_random_position(position);
    if !random {
        let mut next = pile.clone();
        let Some(entry) = next.remove_at(position) else {
            return Vec::new();
        };
        next.stats.on_discarded(Nat::new(1));
        return vec![(next, entry.entity, Fraction::ONE)];
    }
    // 随机段：该段区间内每个位置等概率
    let Some(segment_index) = pile.segment_of_position(position) else {
        return Vec::new();
    };
    let Some(segment) = pile.segment(segment_index).copied() else {
        return Vec::new();
    };
    let Some((start, len)) = segment_range(segment) else {
        return Vec::new();
    };
    let Some(candidates) = segment_entries(pile, start, len) else {
        return Vec::new();
    };
    let choices = DiscardOne { pile, candidates };
    enumerate_random(
        &(pile.clone(), Nat::ZERO),
        &choices,
        false,
        false,
        Nat::new(1),
        None,
    )
    .into_iter()
    .map(|((next, entity), probability)| (next, entity, probability))
    .collect()
}

/// 随机舍弃一张：每个位置等概率（同源池不放大权重）。
pub(super) fn discard_random(pile: &PileState) -> Vec<(PileState, Fraction)> {
    if pile.entries().is_empty() {
        return Vec::new();
    }
    let candidates: Vec<PileEntry> = pile.entries().to_vec();
    let choices = DiscardOne { pile, candidates };
    enumerate_random(
        &(pile.clone(), Nat::ZERO),
        &choices,
        false,
        false,
        Nat::new(1),
        None,
    )
    .into_iter()
    .map(|((next, _entity), probability)| (next, probability))
    .collect()
}

// ----------------------------------------------------------------------
// 计划 / 候选
// ----------------------------------------------------------------------

/// 段的 (start, len)；不是区间段（理论上不会有）-> None。
fn segment_range(segment: Segment) -> Option<(Nat, Nat)> {
    match segment {
        Segment::Fixed { start, len } | Segment::Random { start, len, .. } => Some((start, len)),
    }
}

/// 取某段区间里的全部位置（越界 -> None）。
fn segment_entries(pile: &PileState, start: Nat, len: Nat) -> Option<Vec<PileEntry>> {
    if nadd!(start, len) > pile.len() {
        return None;
    }
    Some(
        pile.entries()
            .iter()
            .skip(start.get() as _)
            .take(len.get() as _)
            .copied()
            .collect(),
    )
}

/// 找「最上面能抽到的候选」：固定段只看段顶一张，随机段 = 匹配的池内牌（D88）。
fn first_draw_candidates(
    pile: &PileState,
    matches: Option<&dyn Fn(CardId) -> bool>,
) -> Option<Vec<PileEntry>> {
    let mut seen_pools: Vec<Nat> = Vec::new();
    for segment in pile.segments() {
        let (start, len) = segment_range(*segment)?;
        if len.get() == 0 {
            continue;
        }
        match *segment {
            Segment::Fixed { .. } => {
                let top = super::at(pile.entries(), start).copied()?;
                let hit = match matches {
                    None => true,
                    Some(matches) => matches(top.card),
                };
                if hit {
                    return Some(vec![top]);
                }
            }
            Segment::Random { pool, .. } => {
                if seen_pools.contains(&pool) {
                    continue;
                }
                seen_pools.push(pool);
                let entries = segment_entries(pile, start, len)?;
                let found: Vec<PileEntry> = match matches {
                    None => entries,
                    Some(matches) => entries
                        .into_iter()
                        .filter(|entry| matches(entry.card))
                        .collect(),
                };
                if !found.is_empty() {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// 建抽 n 张的抽取计划：从段序列顶往下、逐段拿，直到拿够 n 或没牌。
///
/// 同源池只在第一次出现的段处计入（同时后续再出现同一 pool 的段跳过，避免重复计数）。
fn build_draw_steps(pile: &PileState, n: Nat) -> Vec<DrawStep> {
    let mut steps = Vec::new();
    let mut remaining = n;
    let mut seen_pools: Vec<Nat> = Vec::new();
    let mut pool_remaining: Vec<(Nat, Nat)> = Vec::new();
    for segment in pile.segments() {
        if remaining.get() == 0 {
            break;
        }
        match *segment {
            Segment::Fixed { start, len } => {
                let take = min_nat(remaining, len);
                if take.get() == 0 {
                    continue;
                }
                let Some(entries) = segment_entries(pile, start, take) else {
                    continue;
                };
                steps.push(DrawStep {
                    source: StepSource::Fixed { entries },
                    count: take,
                    size: take,
                    options: Nat::new(1),
                });
                remaining = nsub!(remaining, take);
            }
            Segment::Random { pool, len, .. } => {
                if seen_pools.contains(&pool) {
                    continue;
                }
                seen_pools.push(pool);
                let take = min_nat(remaining, len);
                if take.get() == 0 {
                    continue;
                }
                pool_remaining.push((pool, nsub!(len, take)));
                steps.push(DrawStep {
                    source: StepSource::Random { pool },
                    count: take,
                    size: len,
                    options: permutation_count(len, take),
                });
                remaining = nsub!(remaining, take);
            }
        }
    }
    steps
}

/// 抽牌前：每张牌（按实体号）在公共状态列表里的折叠键（D105）。
///
/// 抽牌会把牌从 entries 里移除、位置随之消失，所以键必须先按实体号记下来。
fn keyed_marks(pile: &PileState) -> Vec<(EntityId, FoldMark)> {
    pile.entries()
        .iter()
        .enumerate()
        .map(|(position, entry)| (entry.entity, pile.fold_mark_at(Nat::new(position as _))))
        .collect()
}

/// 一张抽到的牌 -> 折叠判据看到的牌（D98/D105/D188）。
///
/// 折叠键 = 抽之前该位置已有的物化状态（keyed）+ 本次抽牌时展开的待赋状态（pending）。
fn fold_card(entry: &PileEntry, keyed: &[(EntityId, FoldMark)], pending: &[Mark]) -> FoldCard {
    let mut mark = keyed
        .iter()
        .find(|(entity, _)| *entity == entry.entity)
        .map(|(_, mark)| mark.clone())
        .unwrap_or_else(FoldMark::empty);
    if !pending.is_empty() {
        for applied in pending {
            match applied.layers {
                None => mark.booleans.push(state_address(applied.state)),
                Some(layers) => mark.layered.push((state_address(applied.state), layers)),
            }
        }
        mark = FoldMark::new(mark.booleans, mark.layered);
    }
    FoldCard {
        card: entry.card,
        mark,
    }
}

// ----------------------------------------------------------------------
// 选择列表
// ----------------------------------------------------------------------

/// enumerate_random 的列表：每项 = 一条**完整有序抽取序列**（按分支序号解码）。
struct OrderedDraw<'a> {
    pile: &'a PileState,
    hand: &'a HandState,
    steps: Vec<DrawStep>,
    /// 有序序列总数（= 各步 options 之积）。
    total: Nat,
}

impl SelectionSource<DrawBranch> for OrderedDraw<'_> {
    // bare-int-ok: 引擎列表下标契约，非业务数值
    fn len(&self) -> usize {
        self.total.get() as _
    }

    // bare-int-ok: 引擎列表下标契约，非业务数值
    fn apply(&self, _state: &DrawBranch, index: usize) -> (DrawBranch, Fraction) {
        let mut ordinal = Nat::new(index as _);
        let mut branch = initial_branch(self.pile, self.hand);
        for step in &self.steps {
            // 从高位开始取出这一步的有序序列序号（末位变化最快，D1 约定）。
            let digit = nsub!(ordinal, nmul!(ndiv!(ordinal, step.options), step.options));
            ordinal = ndiv!(ordinal, step.options);
            match &step.source {
                StepSource::Fixed { entries } => {
                    for entry in entries {
                        take_entry(&mut branch, entry.entity);
                    }
                }
                StepSource::Random { pool } => {
                    decode_random_step(*pool, step, digit, &mut branch);
                }
            }
        }
        (branch, Fraction::ONE)
    }
}

/// 单张抽取的列表。
struct DrawOne<'a> {
    pile: &'a PileState,
    hand: &'a HandState,
    candidates: Vec<PileEntry>,
}

impl SelectionSource<DrawBranch> for DrawOne<'_> {
    // bare-int-ok: 引擎列表下标契约，非业务数值
    fn len(&self) -> usize {
        self.candidates.len()
    }

    // bare-int-ok: 引擎列表下标契约，非业务数值
    fn apply(&self, _state: &DrawBranch, index: usize) -> (DrawBranch, Fraction) {
        let mut branch = initial_branch(self.pile, self.hand);
        let Some(entry) = self.candidates.get(index).copied() else {
            return (branch, Fraction::ONE);
        };
        take_entry(&mut branch, entry.entity);
        (branch, Fraction::ONE)
    }
}

/// 单张舍弃的列表：每支 = (新牌库, 被舍弃实体号)。
struct DiscardOne<'a> {
    pile: &'a PileState,
    candidates: Vec<PileEntry>,
}

impl SelectionSource<(PileState, EntityId)> for DiscardOne<'_> {
    // bare-int-ok: 引擎列表下标契约，非业务数值
    fn len(&self) -> usize {
        self.candidates.len()
    }

    // bare-int-ok: 引擎列表下标契约，非业务数值
    fn apply(&self, _state: &(PileState, EntityId), index: usize) -> ((PileState, EntityId), Fraction) {
        let Some(entry) = self.candidates.get(index).copied() else {
            return ((self.pile.clone(), Nat::MAX), Fraction::ONE);
        };
        let Some(position) = self.pile.position_of(entry.entity) else {
            return ((self.pile.clone(), Nat::MAX), Fraction::ONE);
        };
        let mut next = self.pile.clone();
        match next.remove_at(position) {
            Some(removed) => {
                next.stats.on_discarded(Nat::new(1));
                ((next, removed.entity), Fraction::ONE)
            }
            None => ((self.pile.clone(), Nat::MAX), Fraction::ONE),
        }
    }
}

// ----------------------------------------------------------------------
// 解码 / 折叠 / 小工具
// ----------------------------------------------------------------------

/// 从牌库取走一张放进分支的手牌（满了就不进 = 弃置），并记进 drawn。
fn take_entry(branch: &mut DrawBranch, entity: EntityId) {
    let Some(position) = branch.pile.position_of(entity) else {
        return;
    };
    // D188/D190：抽到的位置若在 pending 列表里 -> 这张牌带状态（**被观测才展开**）；
    // 具体位置就在当下确定，不需要额外随机维度。
    let applied_marks = branch.pile.pending_marks_at(position);
    let Some(entry) = branch.pile.remove_at(position) else {
        return;
    };
    branch.drawn.push(entry);
    branch.drawn_marks.push(applied_marks);
    branch.pile.stats.on_drawn(Nat::new(1));
    let instance = super::super::CardInstance::new(entry.entity, entry.card);
    let (hand, added) = branch.hand.add(instance);
    if added {
        branch.hand = hand;
    }
}

/// 随机步：按 digit（有序序列序号）解码出「从当前池里依次取哪几张」。
fn decode_random_step(pool: Nat, step: &DrawStep, digit: Nat, branch: &mut DrawBranch) {
    let mut ordinal = digit;
    let mut size = step.size;
    let mut count = step.count;
    while count.get() > 0 {
        let divisor = permutation_count(nsub!(size, Nat::new(1)), nsub!(count, Nat::new(1)));
        let pick = ndiv!(ordinal, divisor);
        ordinal = nsub!(ordinal, nmul!(pick, divisor));
        let Some((start, len)) = branch.pile.pool_range(pool) else {
            return;
        };
        let Some(available) = segment_entries(&branch.pile, start, len) else {
            return;
        };
        let Some(entry) = super::at(&available, pick).copied() else {
            return;
        };
        take_entry(branch, entry.entity);
        size = nsub!(size, Nat::new(1));
        count = nsub!(count, Nat::new(1));
    }
}

/// 把「全部有序序列」的分支按等价类折叠（D98/D105）：分组、权重相加、每组只留代表元。
///
/// 这是**牌库这块自己的折叠入口**（D91/D92）；判据与分组实现来自 action_card zones 共享层，
/// 手牌那边另有自己的入口（hand/parse.rs），但用的是同一套判据。
fn fold_draw_branches(
    branches: Vec<(DrawBranch, Fraction)>,
    keyed: &[(EntityId, FoldMark)],
) -> Vec<(DrawBranch, Fraction)> {
    fold_equivalent(branches, |branch| {
        branch
            .drawn
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let pending = branch
                    .drawn_marks
                    .get(index)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                fold_card(entry, keyed, pending)
            })
            .collect()
    })
}

/// 两个 Nat 取小。
fn min_nat(left: Nat, right: Nat) -> Nat {
    if left < right {
        left
    } else {
        right
    }
}

/// 排列数 P(size, count) = size × (size-1) × …（不重复、有序的取值数）。
///
/// count > size 时返回 0（调用方不会这么传；这里给出确定结果，不 panic）。
fn permutation_count(size: Nat, count: Nat) -> Nat {
    if count > size {
        return Nat::ZERO;
    }
    let mut result = Nat::new(1);
    let mut index = Nat::ZERO;
    while index < count {
        result = nmul!(result, nsub!(size, index));
        index = nadd!(index, Nat::new(1));
    }
    result
}
