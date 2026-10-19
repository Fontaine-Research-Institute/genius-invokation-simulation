//! 牌库区（行动牌）：位置为键的统一列表 + 段视图 + 公共状态列表 + 抽取 / 舍弃 / 插入。
//!
//! 规格与决策：分工计划.md §4.4.2、结构.md §4.3；D42（区是字段、没有区号）、
//! D52（标记跟着位置走）、D86/D89（舍弃 / 置底的标记语义）、
//! D87（引擎 dedup 无上下文，先在 definitions 侧后置合并）、D90（起手归手牌，牌库只给抓牌原语）、
//! D91/D92（每块自己的选择列表 + 去重函数，放该块 parse.rs）、D93/D94（位置为键的统一列表 +
//! 段区间 + 同源池分组 id）、D95/D96（状态身份 = 类型化静态实例指针，不用 Nat）、
//! D97/D98（抽牌按组合折叠）、D100（层数只存与合并，钳到 0 在费用结算侧）、
//! D104/D105（**所有状态共用一个列表**；布尔类不额外存 bool，在列表里即代表有）、
//! D64/D65（字段访问、不写下标取数）。
//!
//! **结构（D93 / D105）**：
//! - entries: Vec<PileEntry>，**位置 = 下标（从顶往下数）**；每个位置带实体号 / 卡 id；
//! - segments: Vec<Segment>，段是对 entries 的**连续区间视图**（固定段有序 / 随机段无序）；
//! - states: StateList，**一个公共列表装所有状态**（位置 + 状态身份 + 层数，D105）；
//!   本块只负责在 entries 插入 / 删除时调用 StateList 的 on_insert / on_remove 让位置跟着牌走。
//!
//! **同源随机池（D94 第 2 条）**：Random { pool, start, len }；pool 是不透明分组 id、不建池表；
//! 同 pool 的段区间必须一致，抽 / 删时一起更新（两个段共享同一段 entries）。
//!
//! 抽取 / 折叠 / 选择列表在 parse.rs（本块自己的去重函数，D91/D92）。
//! 纯函数风格：操作吃 &self 返回新状态 / 分支；失败 = 不改状态 + 返回 None / 空，不 panic。

mod parse;
pub mod record;
pub mod stats;

pub use stats::PileStats;

use super::hand::HandState;
use crate::base::CardId;
use crate::families::action_card::status::{Mark, StateIdentity, StateKey, StateList};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, ndiv, nmul, nsub, EntityId, Nat};

/// 折叠的「随机塞状态」来源（D188 / D190 / D191）。
///
/// **不预先物化到具体牌上**：只记**位置列表**（从牌库顶往下数的序号）。
/// 未观测时保持紧凑（折叠），**被观测（抽到）才展开**，展开后立刻回收（不再是 permanent 展开）；
/// 展开 / 回收成对：本模块的 pending_marks_at（观测展开）与 remove_at 里调用的
/// pending_on_remove（回收 / 位置维护）。
/// 位置随牌库插删一起维护（和公共状态列表同一个位置口径），所以均匀置入之后仍指向原来那批牌。
/// 抽牌时：抽到的位置若在该列表里 -> 这张牌带状态，并从列表移除。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingMark {
    /// 要赋予的状态（状态身份 + 层数；布尔类 layers = None）。
    pub mark: Mark,
    /// 待赋予状态的位置（从顶往下数；已排序去重，随牌库插删维护）。
    pub positions: Vec<Nat>,
}

/// 牌库里的一个位置（D93：**位置为键**）。状态不在 entry 里，读公共状态列表（D105）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PileEntry {
    /// 本局唯一的实体号。
    pub entity: EntityId,
    /// 这张实体对应的卡牌定义 id。
    pub card: CardId,
}

impl PileEntry {
    /// 构造一个位置（建局 / 测试用）。
    pub const fn new(entity: EntityId, card: CardId) -> PileEntry {
        PileEntry { entity, card }
    }
}

/// 段：对 entries 的**连续区间视图**（D93/D94）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Segment {
    /// 固定段：区间内**有序**。
    Fixed {
        /// 区间起点（entries 下标）。
        start: Nat,
        /// 区间长度。
        len: Nat,
    },
    /// 随机段：区间内**无序**；同 pool 的段共享同一段 entries（同源）。
    Random {
        /// 不透明分组 id（不建池表；同 pool = 同源，区间必须一致）。
        pool: Nat,
        /// 区间起点（entries 下标）。
        start: Nat,
        /// 区间长度。
        len: Nat,
    },
}

impl Segment {
    /// 区间起点。
    pub const fn start(self) -> Nat {
        match self {
            Segment::Fixed { start, .. } | Segment::Random { start, .. } => start,
        }
    }

    /// 区间长度。
    pub const fn len(self) -> Nat {
        match self {
            Segment::Fixed { len, .. } | Segment::Random { len, .. } => len,
        }
    }

    /// 区间终点（不含）。
    pub fn end(self) -> Nat {
        nadd!(self.start(), self.len())
    }

    /// 是不是随机段。
    pub const fn is_random(self) -> bool {
        matches!(self, Segment::Random { .. })
    }

    /// 随机段的池分组 id（固定段 -> None）。
    pub const fn pool(self) -> Option<Nat> {
        match self {
            Segment::Random { pool, .. } => Some(pool),
            Segment::Fixed { .. } => None,
        }
    }

    fn set_start(&mut self, value: Nat) {
        match self {
            Segment::Fixed { start, .. } | Segment::Random { start, .. } => *start = value,
        }
    }

    fn set_len(&mut self, value: Nat) {
        match self {
            Segment::Fixed { len, .. } | Segment::Random { len, .. } => *len = value,
        }
    }
}

/// 抽牌分支：一次抽取之后的 (牌库, 手牌, 按抽取顺序的牌)。
///
/// drawn 是 PileEntry（含实体号 / 卡 id）：即使这张牌因为**手牌溢出**被弃置，
/// 调用方也能知道抽到的是哪张（不需要任何全局映射表）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawBranch {
    /// 抽取后的牌库。
    pub pile: PileState,
    /// 抽取后的手牌。
    pub hand: HandState,
    /// 这次抽到的位置（按抽取顺序；折叠后的代表顺序见 D98）。
    pub drawn: Vec<PileEntry>,
    /// 与 drawn 一一对应：这张牌抽到时携带的**待展开随机状态**（D188 展开结果；没有就是空 Vec）。
    pub drawn_marks: Vec<Vec<Mark>>,
}

/// 牌库区：**位置为键的 entries + 段视图 + 公共状态列表**（D93/D105）。
///
/// 容器没有身份（D42）：它就是状态里的一个字段。
///
/// 不变式（本类型维护）：
/// 1. 同一个实体在 entries 里最多出现一次；
/// 2. 段区间覆盖 entries（首尾相接；同 pool 的随机段共享同一区间，可能重复覆盖）；
/// 3. 同 pool 的随机段 start / len 必须一致；
/// 4. states 里的位置都指向当前 entries 的位置（entries 一变就 on_insert / on_remove，D105）；
/// 5. 空段可以存在（操作不删段，避免段下标漂移；抽取 / 置底会跳过空段）。
#[derive(Clone, Default, Debug)]
pub struct PileState {
    /// 位置列表：下标 = 从牌库顶往下数的位置（D93）。
    pub entries: Vec<PileEntry>,
    /// 段序列（前 = 牌库顶；每段是 entries 的连续区间）。
    pub segments: Vec<Segment>,
    /// **公共状态列表**（D105：一个列表装所有状态；按 pos 过滤就是某位置的状态）。
    pub states: StateList,
    /// 牌库统计（D187；feature 关闭时零大小、空操作）。
    stats: PileStats,
    /// 折叠的随机塞状态来源（D188：只记位置，不物化）。
    pending: Vec<PendingMark>,
}

impl PartialEq for PileState {
    /// **统计不参与局面等价**（D187：统计不改变游戏语义；参与会让等价判断变严）。
    /// 待展开的随机状态（pending）会影响后续抽牌，所以它**参与**等价。
    fn eq(&self, other: &PileState) -> bool {
        self.entries == other.entries
            && self.segments == other.segments
            && self.states == other.states
            && self.pending == other.pending
    }
}

impl Eq for PileState {}

impl PileState {
    /// 空牌库。
    pub const fn new() -> PileState {
        PileState {
            entries: Vec::new(),
            segments: Vec::new(),
            states: StateList::new(),
            stats: PileStats::EMPTY,
            pending: Vec::new(),
        }
    }

    /// 用一串**有序**的牌建牌库：整串作为**一个固定段**（前 = 顶）。
    ///
    /// 建局时按初始卡组列表 alloc 实体号后调用这里；空列表 -> 空牌库（不建空段）。
    pub fn from_fixed(instances: Vec<(EntityId, CardId)>) -> PileState {
        let entries: Vec<PileEntry> = instances
            .into_iter()
            .map(|(entity, card)| PileEntry::new(entity, card))
            .collect();
        let mut state = PileState {
            entries,
            segments: Vec::new(),
            states: StateList::new(),
            stats: PileStats::EMPTY,
            pending: Vec::new(),
        };
        if !state.entries.is_empty() {
            state.segments.push(Segment::Fixed {
                start: Nat::ZERO,
                len: state.len(),
            });
        }
        state
    }

    /// 用一串牌建一个**随机段**牌库（同源池分组 id 用 0）。
    pub fn from_random(instances: Vec<(EntityId, CardId)>) -> PileState {
        let entries: Vec<PileEntry> = instances
            .into_iter()
            .map(|(entity, card)| PileEntry::new(entity, card))
            .collect();
        let mut state = PileState {
            entries,
            segments: Vec::new(),
            states: StateList::new(),
            stats: PileStats::EMPTY,
            pending: Vec::new(),
        };
        if !state.entries.is_empty() {
            state.segments.push(Segment::Random {
                pool: Nat::ZERO,
                start: Nat::ZERO,
                len: state.len(),
            });
        }
        state
    }

    // ------------------------------------------------------------------
    // 查询
    // ------------------------------------------------------------------

    /// 牌库里的位置数（张数）。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 牌库是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 全部位置（从顶往下）。
    pub fn entries(&self) -> &[PileEntry] {
        &self.entries
    }

    /// 段序列（前 = 牌库顶）。
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// 公共状态列表（D105）。
    pub fn marks(&self) -> &StateList {
        &self.states
    }

    /// 牌库统计（D187；只读指针，D183；feature 关闭时读到的都是 0）。
    pub fn stats(&self) -> &PileStats {
        &self.stats
    }

    /// 待展开的「随机塞状态」来源（D188；只读指针）。
    pub fn pending_marks(&self) -> &[PendingMark] {
        &self.pending
    }

    /// 取某一段（越界 -> None）。
    pub fn segment(&self, index: Nat) -> Option<&Segment> {
        at(&self.segments, index)
    }

    /// 取某个位置（越界 -> None）。
    pub fn entry(&self, position: Nat) -> Option<&PileEntry> {
        at(&self.entries, position)
    }

    /// 这个实体在哪个位置（不在牌库 -> None）。
    pub fn position_of(&self, entity: EntityId) -> Option<Nat> {
        self.entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.entity == entity)
            .map(|(position, _)| Nat::new(position as _))
    }

    /// 这张实体的卡牌定义 id（不在牌库 -> None）。
    pub fn card_of(&self, entity: EntityId) -> Option<CardId> {
        self.entries
            .iter()
            .find(|entry| entry.entity == entity)
            .map(|entry| entry.card)
    }

    /// 这个实体是否在牌库里。
    pub fn contains(&self, entity: EntityId) -> bool {
        self.position_of(entity).is_some()
    }

    /// 某位置属于哪个段（越界 -> None；同源随机段取第一个匹配）。
    pub fn segment_of_position(&self, position: Nat) -> Option<Nat> {
        for (index, segment) in self.segments.iter().enumerate() {
            if segment.start() <= position && position < segment.end() {
                return Some(Nat::new(index as _));
            }
        }
        None
    }

    /// 位置是否落在随机段（D94 第 3 条：随机段内位置 = entries 存放序）。
    pub fn is_random_position(&self, position: Nat) -> bool {
        self.segment_of_position(position)
            .and_then(|index| self.segment(index).copied())
            .map(|segment| segment.is_random())
            .unwrap_or(false)
    }

    /// 同源随机池的区间（start / len；没有该 pool -> None）。
    pub fn pool_range(&self, pool: Nat) -> Option<(Nat, Nat)> {
        self.segments.iter().find_map(|segment| match segment {
            Segment::Random {
                pool: candidate,
                start,
                len,
            } if *candidate == pool => Some((*start, *len)),
            _ => None,
        })
    }

    // ------------------------------------------------------------------
    // 状态（D104/D105：委托给公共状态列表；这里只负责按位置过滤 + 位置维护）
    // ------------------------------------------------------------------

    /// 某个位置是否有这个状态。
    pub fn has_mark(&self, position: Nat, state: &'static dyn StateIdentity) -> bool {
        self.states.contains(position, state)
    }

    /// 某个位置这个状态的层数：不存在 -> None；存在 -> Some(layers)（布尔类为 Some(None)）。
    pub fn mark_layers(
        &self,
        position: Nat,
        state: &'static dyn StateIdentity,
    ) -> Option<Option<Nat>> {
        self.states.layers(position, state)
    }

    /// 某个位置的全部状态条目（按公共列表顺序）。
    pub fn marks_at(&self, position: Nat) -> Vec<crate::families::action_card::status::StateEntry> {
        self.states.entries_at(position)
    }

    /// 给某个位置加状态（形态由状态自己的 form 决定；D104 通用操作）。
    ///
    /// - 布尔类：在列表里即代表有、**不额外存 bool**，layers 一律 None，重复添加幂等；
    /// - 层数类：同状态同位置层数相加，只存与合并、不钳制（D100）；
    /// - 位置越界 -> None（状态不变）。
    pub fn add_mark(
        &self,
        position: Nat,
        state: &'static dyn StateIdentity,
        layers: Option<Nat>,
    ) -> Option<PileState> {
        self.entry(position)?;
        let mut next = self.clone();
        next.states.add(position, state, layers);
        Some(next)
    }

    /// 加状态，并先与相反状态抵消（opposite 由调用方给；D94 规则，通用层执行）。
    ///
    /// 位置越界 -> None（状态不变）。
    pub fn add_mark_with_opposite(
        &self,
        position: Nat,
        state: &'static dyn StateIdentity,
        layers: Option<Nat>,
        opposite: &'static dyn StateIdentity,
    ) -> Option<PileState> {
        self.entry(position)?;
        let mut next = self.clone();
        next.states.add_with_opposite(position, state, layers, opposite);
        Some(next)
    }

    /// 移除某个位置的某个状态（不存在也算成功；位置越界 -> None）。
    pub fn clear_mark(
        &self,
        position: Nat,
        state: &'static dyn StateIdentity,
    ) -> Option<PileState> {
        self.entry(position)?;
        let mut next = self.clone();
        next.states.remove(position, state);
        Some(next)
    }

    /// 某个位置在**抽牌折叠**里的状态键（D98/D105：布尔集合 + 层数条目集合，指针身份）。
    pub(crate) fn fold_mark_at(&self, position: Nat) -> StateKey {
        self.states.key_at(position)
    }

    /// 为「给多张牌 / 某个范围内的牌赋**布尔状态**」挑选目标位置（D99 / D105）。
    ///
    /// 判据 = 该位置**是否已有这个状态**（公共列表里有没有该 pos+state 的条目）：
    /// 布尔状态是「有 / 没有」，优先赋给还没有它的位置，不要无谓覆盖。
    /// 规则（保持调用方给定顺序）：
    /// 1. 先挑还没有该状态的位置，直到挑满 count；
    /// 2. 还不够时才退回已有该状态的位置（调用方决定要不要重复添加）；
    /// 3. 越界位置不会被选；count 大于可选数时返回全部可选的。
    pub fn pick_unmarked_targets(
        &self,
        positions: &[Nat],
        state: &'static dyn StateIdentity,
        count: Nat,
    ) -> Vec<Nat> {
        let mut chosen: Vec<Nat> = Vec::new();
        for position in positions {
            if Nat::new(chosen.len() as _) >= count {
                break;
            }
            if self.entry(*position).is_some() && !self.has_mark(*position, state) {
                chosen.push(*position);
            }
        }
        for position in positions {
            if Nat::new(chosen.len() as _) >= count {
                break;
            }
            if self.entry(*position).is_some() && self.has_mark(*position, state) {
                chosen.push(*position);
            }
        }
        chosen
    }

    // ------------------------------------------------------------------
    // 待展开的随机状态（D188/D190：未观测不物化，抽牌时才展开）
    // ------------------------------------------------------------------

    /// 记录一批「待赋状态的位置」（D188）：**不物化到牌上**，只存位置列表。
    ///
    /// - mark：要赋予的状态（状态身份 + 层数）；
    /// - positions：从牌库顶往下数的序号（重复 / 越界的会被忽略并去重排序）；
    /// - 位置会随牌库插删一起维护（insert_entry/remove_at 里同步），所以均匀置入后仍指向原来那批牌；
    /// - 抽牌时：抽到的位置若在列表里 -> 这张牌带状态、并从列表移除（展开成确定分支，D190）。
    ///
    /// 失败语义：没有任何合法位置 -> None（状态不变）。
    pub fn add_pending_mark(&self, mark: Mark, positions: &[Nat]) -> Option<PileState> {
        let mut valid: Vec<Nat> = positions
            .iter()
            .copied()
            .filter(|position| *position < self.len())
            .collect();
        valid.sort();
        valid.dedup();
        if valid.is_empty() {
            return None;
        }
        let mut next = self.clone();
        next.pending.push(PendingMark {
            mark,
            positions: valid,
        });
        Some(next)
    }

    /// **一次塞 N 个位置**（D209 通用「多次执行」）：而不是让调用方自己循环。
    ///
    /// - count：本次要赋予状态的位置个数；
    /// - pick：第 step 次（0 起）挑哪个位置；返回 None 表示提前结束；
    ///   越界 / 重复的位置会被忽略（不会重复塞）；
    /// - 结果与 add_pending_mark 相同：只记一个 PendingMark（位置列表），不物化。
    ///
    /// 失败语义：count == 0 / 一个合法位置都没挑到 -> None（状态不变）。
    pub fn add_pending_mark_many(
        &self,
        mark: Mark,
        count: Nat,
        pick: impl Fn(Nat) -> Option<Nat>,
    ) -> Option<PileState> {
        if count.get() == 0 {
            return None;
        }
        let mut positions: Vec<Nat> = Vec::new();
        let mut step = Nat::ZERO;
        while step < count {
            if Nat::new(positions.len() as _) >= self.len() {
                break;
            }
            let Some(position) = pick(step) else {
                break;
            };
            if position < self.len() && !positions.contains(&position) {
                positions.push(position);
            }
            step = nadd!(step, Nat::new(1));
        }
        if positions.is_empty() {
            return None;
        }
        self.add_pending_mark(mark, &positions)
    }

    /// 清空全部待展开随机状态（D188）。
    pub fn clear_pending_marks(&self) -> PileState {
        let mut next = self.clone();
        next.pending.clear();
        next
    }

    /// 某个位置上的待展开状态（展开用；按记录顺序，多个来源会返回多条）。
    pub(crate) fn pending_marks_at(&self, position: Nat) -> Vec<Mark> {
        self.pending
            .iter()
            .filter(|pending| pending.positions.contains(&position))
            .map(|pending| pending.mark)
            .collect()
    }

    /// 牌库插入一张牌后维护 pending 位置：从该位置起整体后移一位。
    fn pending_on_insert(&mut self, position: Nat) {
        for pending in &mut self.pending {
            for pending_position in &mut pending.positions {
                if *pending_position >= position {
                    *pending_position = nadd!(*pending_position, Nat::new(1));
                }
            }
        }
    }

    /// 牌库移除一张牌后维护 pending 位置：该位置上的待展开状态随牌消失，
    /// 后面的位置整体前移一位。
    fn pending_on_remove(&mut self, position: Nat) {
        for pending in &mut self.pending {
            pending.positions.retain(|pending_position| *pending_position != position);
            for pending_position in &mut pending.positions {
                if *pending_position > position {
                    *pending_position = nsub!(*pending_position, Nat::new(1));
                }
            }
        }
        self.pending
            .retain(|pending| !pending.positions.is_empty());
    }

    // ------------------------------------------------------------------
    // 内容操作（D93：位置为键；状态随位置一起搬，D105）
    // ------------------------------------------------------------------

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

    /// 从牌库**舍弃**一张牌（移除，没有弃牌堆）；状态随位置一起消失（D86/D89/D93）。
    ///
    /// 找不到 -> None（状态不变）。
    pub fn discard(&self, entity: EntityId) -> Option<PileState> {
        let position = self.position_of(entity)?;
        let mut next = self.clone();
        next.remove_at(position)?;
        next.stats.on_discarded(Nat::new(1));
        Some(next)
    }

    /// **按位置舍弃**（D94 第 7 条：位置就是 entries 下标）；返回分支列表。
    ///
    /// - 落**固定段**：目标确定 -> 单分支（概率 1.0）；
    /// - 落**随机段**：段内无序 -> 该段所有项等概率各一支；
    /// - 位置越界 -> 空 Vec（不改状态）。
    pub fn discard_at(&self, position: Nat) -> Vec<(PileState, EntityId, Fraction)> {
        parse::discard_at(self, position)
    }

    /// **随机舍弃一张牌**：牌库每个位置等概率（同源池不放大权重）。
    pub fn discard_random(&self) -> Vec<(PileState, Fraction)> {
        parse::discard_random(self)
    }

    /// **均匀置入**（D88）：把这些牌按整副牌库的平坦序均匀插入（已在牌库里的牌换位置）。
    ///
    /// 参数是**已经在牌库里的实体号**（不在牌库 / 重复出现 -> 忽略；重复只处理第一次）；
    /// 先从原位置移除（各自状态随移除消失），再按 floor(i*(m+k)/k) 从后往前插。
    /// 失败语义：一个都没找到 -> None（状态不变）。
    pub fn insert_evenly(&self, entities: &[EntityId]) -> Option<PileState> {
        let mut next = self.clone();
        let mut moved: Vec<PileEntry> = Vec::new();
        let mut handled: Vec<EntityId> = Vec::new();
        for entity in entities {
            if handled.contains(entity) {
                continue;
            }
            handled.push(*entity);
            if let Some(position) = next.position_of(*entity) {
                if let Some(entry) = next.remove_at(position) {
                    moved.push(entry);
                }
            }
        }
        if moved.is_empty() {
            return None;
        }
        let k = Nat::new(moved.len() as _);
        let m = next.len();
        let total = nadd!(m, k);
        let mut i = nsub!(k, Nat::new(1));
        loop {
            let target = ndiv!(nmul!(i, total), k);
            if let Some(entry) = at(&moved, i).copied() {
                next.insert_entry(target, entry);
            }
            if i.get() == 0 {
                break;
            }
            i = nsub!(i, Nat::new(1));
        }
        next.stats.on_inserted(k);
        Some(next)
    }

    /// 把**不在牌库里的新牌**按整副牌库的**平坦序**均匀插入（D88，手牌洗回牌库用）。
    ///
    /// instances: (实体号, 卡 id)；已在牌库的实体 / 重复出现 -> 忽略（重复只处理第一次）。
    /// 新位置没有状态；落随机段内即插入该段区间。
    /// 失败语义：全部被忽略（都在牌库 / 空入参）-> None（状态不变）。
    pub fn insert_evenly_with(&self, instances: &[(EntityId, CardId)]) -> Option<PileState> {
        let mut next = self.clone();
        let mut incoming: Vec<PileEntry> = Vec::new();
        let mut handled: Vec<EntityId> = Vec::new();
        for (entity, card) in instances {
            if handled.contains(entity) || next.contains(*entity) {
                continue;
            }
            handled.push(*entity);
            incoming.push(PileEntry::new(*entity, *card));
        }
        if incoming.is_empty() {
            return None;
        }
        let k = Nat::new(incoming.len() as _);
        let m = next.len();
        let total = nadd!(m, k);
        let mut i = nsub!(k, Nat::new(1));
        loop {
            let target = ndiv!(nmul!(i, total), k);
            if let Some(entry) = at(&incoming, i).copied() {
                next.insert_entry(target, entry);
            }
            if i.get() == 0 {
                break;
            }
            i = nsub!(i, Nat::new(1));
        }
        next.stats.on_inserted(k);
        Some(next)
    }

    /// 把某个随机段**拆成两个同源段**：在原段之后插入一个 pool / 区间完全相同的随机段。
    ///
    /// 语义（D94 第 2 条）：两段共享同一段 entries，抽 / 删时一起更新。
    /// 失败语义：段下标越界 / 不是随机段 -> None（状态不变）。
    pub fn split_random_segment(&self, segment: Nat) -> Option<PileState> {
        let target = *self.segment(segment)?;
        if !target.is_random() {
            return None;
        }
        let mut next = self.clone();
        let insert_at = nadd!(segment, Nat::new(1));
        next.segments.insert(insert_at.get() as _, target);
        Some(next)
    }

    // ------------------------------------------------------------------
    // 抽取（原语；折叠 / 选择列表在 parse.rs）
    // ------------------------------------------------------------------

    /// 抓一张：固定段 = 段顶那张；随机段 = 段内**均匀随机一张**。
    ///
    /// 从段序列 0 开始（前 = 顶），跳过空段。牌库为空 -> 空 Vec。
    pub fn draw(&self, hand: &HandState) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_one(self, hand, None)
    }

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

    /// 按**卡牌 id** 抓一张（规则同 draw_top）。
    pub fn draw_by_card(&self, hand: &HandState, card: CardId) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_one(self, hand, Some(&|candidate: CardId| candidate == card))
    }

    /// 从**指定随机段**抓一张：段内均匀随机一张。
    pub fn draw_from_random(&self, hand: &HandState, segment: Nat) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_from_random(self, hand, segment)
    }

    /// 抓 **n 张**（不够则少给）；返回折叠后的分支列表（D83/D84/D85）。
    ///
    /// - 一张都抓不到（空库 / n = 0）-> 空 Vec；
    /// - 折叠口径见 parse.rs 文件头；
    /// - 手牌溢出时退化成有序分支（概率仍精确）。**实际对局认为不会发生溢出（D99）**，
    ///   这一句只是安全兜底：保证分支后继状态与概率一致，不是待补功能。
    pub fn draw_many(&self, hand: &HandState, n: Nat) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_many(self, hand, n)
    }

    // ------------------------------------------------------------------
    // 内部：entries / segments / 状态列表维护
    // ------------------------------------------------------------------

    /// 移除某个位置，返回被移除的 entry（位置越界 -> None）。
    ///
    /// 公共状态列表：该位置的条目随牌消失、后面位置前移（D105 委托 StateList::on_remove）。
    fn remove_at(&mut self, position: Nat) -> Option<PileEntry> {
        if position >= self.len() {
            return None;
        }
        let entry = self.entries.remove(position.get() as _);
        self.states.on_remove(position);
        self.pending_on_remove(position);
        self.after_remove(position);
        Some(entry)
    }

    /// 在某个位置插入一个 entry（位置 == len 等价于尾部追加）。
    ///
    /// 公共状态列表：从该位置起序号后移（新位置没有状态）。
    fn insert_entry(&mut self, position: Nat, entry: PileEntry) {
        let position = min_nat(position, self.len());
        self.entries.insert(position.get() as _, entry);
        self.states.on_insert(position);
        self.pending_on_insert(position);
        self.after_insert(position);
    }

    /// 尾部追加一个 entry（尾部追加不移动已有位置，所以状态列表不用动）。
    fn append_entry(&mut self, entry: PileEntry) {
        let position = self.len();
        self.entries.push(entry);
        self.after_append(position);
    }

    /// 在 position 插入后维护段区间（D94 第 1 条）。
    fn after_insert(&mut self, position: Nat) {
        if self.segments.is_empty() {
            self.segments.push(Segment::Fixed {
                start: Nat::ZERO,
                len: self.len(),
            });
            return;
        }
        let owners = self.insertion_owners(position);
        for (index, segment) in self.segments.iter_mut().enumerate() {
            let start = segment.start();
            let len = segment.len();
            if owners.contains(&Nat::new(index as _)) {
                segment.set_len(nadd!(len, Nat::new(1)));
            } else if start > position || (start == position && len.get() == 0) {
                segment.set_start(nadd!(start, Nat::new(1)));
            }
        }
    }

    /// 尾部追加后维护段区间：延伸覆盖末尾的段（同 pool 一起）。
    fn after_append(&mut self, position: Nat) {
        if self.segments.is_empty() {
            self.segments.push(Segment::Fixed {
                start: Nat::ZERO,
                len: Nat::new(1),
            });
            return;
        }
        let owners = self.tail_owners(position);
        for (index, segment) in self.segments.iter_mut().enumerate() {
            if owners.contains(&Nat::new(index as _)) {
                segment.set_len(nadd!(segment.len(), Nat::new(1)));
            } else if segment.start() > position {
                segment.set_start(nadd!(segment.start(), Nat::new(1)));
            }
        }
    }

    /// 在 position 移除后维护段区间（D94 第 1 条）。
    fn after_remove(&mut self, position: Nat) {
        for segment in self.segments.iter_mut() {
            let start = segment.start();
            let end = segment.end();
            if start <= position && position < end {
                segment.set_len(nsub!(segment.len(), Nat::new(1)));
            } else if start > position {
                segment.set_start(nsub!(start, Nat::new(1)));
            }
        }
    }

    /// position 处插入时，应当吸收这个新位置的段下标（同 pool 的段会一起返回）。
    fn insertion_owners(&self, position: Nat) -> Vec<Nat> {
        let mut owners: Vec<Nat> = Vec::new();
        for (index, segment) in self.segments.iter().enumerate() {
            if segment.start() <= position && position < segment.end() {
                owners.push(Nat::new(index as _));
            }
        }
        if !owners.is_empty() {
            return owners;
        }
        for (index, segment) in self.segments.iter().enumerate() {
            if segment.start() == position && segment.len().get() == 0 {
                owners.push(Nat::new(index as _));
            }
        }
        if !owners.is_empty() {
            return owners;
        }
        self.tail_owners(position)
    }

    /// 终点在 boundary 处的非空段下标（同 pool 的段一起；尾部追加 / 末尾插入用）。
    fn tail_owners(&self, boundary: Nat) -> Vec<Nat> {
        let mut owners: Vec<Nat> = Vec::new();
        for (index, segment) in self.segments.iter().enumerate() {
            if segment.len().get() > 0 && segment.end() == boundary {
                owners.push(Nat::new(index as _));
            }
        }
        if !owners.is_empty() {
            return owners;
        }
        self.segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| segment.end() == boundary)
            .map(|(index, _)| Nat::new(index as _))
            .collect()
    }
}

/// 按 Nat 下标取切片元素（只用迭代，不写取值式下标；D65 精神）。
fn at<T>(items: &[T], index: Nat) -> Option<&T> {
    items
        .iter()
        .enumerate()
        .find(|(slot, _)| Nat::new(*slot as _) == index)
        .map(|(_, item)| item)
}

/// 两个 Nat 取小。
fn min_nat(left: Nat, right: Nat) -> Nat {
    if left < right {
        left
    } else {
        right
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::{StateForm, StateText};
    use crate::generated::Localized;
    use gi_genius_invokation_engine::nadd;
    use gi_genius_invokation_engine::nat::ExactNat128;

    static POWERED_TEXT: StateText = StateText {
        name: Localized::from_all("赋能", "Empowered", "強化", "강화"),
        description: Localized::EMPTY,
    };
    static CHEAPER_TEXT: StateText = StateText {
        name: Localized::from_all("减费", "Cost Reduction", "コスト減少", "비용 감소"),
        description: Localized::EMPTY,
    };
    static EXPENSIVE_TEXT: StateText = StateText {
        name: Localized::from_all("加费", "Cost Increase", "コスト増加", "비용 증가"),
        description: Localized::EMPTY,
    };

    /// 测试用布尔状态（D104：布尔类，无层数；D110：静态数据 = StateText）。
    #[derive(Debug)]
    struct TestBool(&'static StateText);

    impl StateIdentity for TestBool {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Boolean
        }
    }

    /// 测试用层数状态（D104：必须带层数；D110：静态数据 = StateText）。
    #[derive(Debug)]
    struct TestLayered(&'static StateText);

    impl StateIdentity for TestLayered {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Layered
        }
    }

    static POWERED: TestBool = TestBool(&POWERED_TEXT);
    static CHEAPER: TestLayered = TestLayered(&CHEAPER_TEXT);
    static EXPENSIVE: TestLayered = TestLayered(&EXPENSIVE_TEXT);

    fn ordered_deck(count: Nat) -> PileState {
        let mut pairs: Vec<(EntityId, CardId)> = Vec::new();
        let mut index = Nat::ZERO;
        while index < count {
            pairs.push((index, CardId::new(index.get())));
            index = nadd!(index, Nat::new(1));
        }
        PileState::from_fixed(pairs)
    }

    fn random_deck(count: Nat) -> PileState {
        let mut pairs: Vec<(EntityId, CardId)> = Vec::new();
        let mut index = Nat::ZERO;
        while index < count {
            pairs.push((index, CardId::new(index.get())));
            index = nadd!(index, Nat::new(1));
        }
        PileState::from_random(pairs)
    }

    fn hand_with(count: Nat) -> HandState {
        let mut hand = HandState::new();
        let mut index = Nat::ZERO;
        while index < count {
            let entity = nadd!(index, Nat::new(100));
            let (next, added) = hand
                .add(crate::families::action_card::zones::CardInstance::new(
                    entity,
                    CardId::new(entity.get()),
                ));
            assert!(added);
            hand = next;
            index = nadd!(index, Nat::new(1));
        }
        hand
    }

    fn frac(num: u128, den: u128) -> Fraction {
        Fraction::new(ExactNat128::new(num), ExactNat128::new(den))
    }

    fn third() -> Fraction {
        frac(1, 3)
    }

    fn sixth() -> Fraction {
        frac(1, 6)
    }

    fn two_thirds() -> Fraction {
        frac(2, 3)
    }

    fn entry_entities(pile: &PileState) -> Vec<EntityId> {
        pile.entries().iter().map(|entry| entry.entity).collect()
    }

    fn with_bool(pile: &PileState, position: Nat) -> PileState {
        pile.add_mark(position, &POWERED, None).expect("位置合法")
    }

    fn pile_with_bool() -> PileState {
        with_bool(&random_deck(Nat::new(3)), Nat::ZERO)
    }

    #[test]
    fn empty_pile_basics() {
        let pile = PileState::new();
        assert!(pile.is_empty());
        assert_eq!(pile.len(), Nat::ZERO);
        assert!(pile.segments().is_empty());
        assert!(pile.entries().is_empty());
        assert!(pile.marks().is_empty());
        assert_eq!(pile.put_bottom(Nat::ZERO), None);
        assert_eq!(pile.discard(Nat::ZERO), None);
        assert!(pile.draw(&HandState::new()).is_empty());
        assert!(pile.draw_many(&HandState::new(), Nat::new(2)).is_empty());
        assert!(pile.discard_random().is_empty());
        assert!(pile.discard_at(Nat::ZERO).is_empty());
    }

    #[test]
    fn fixed_and_random_decks_are_segmented() {
        let fixed = ordered_deck(Nat::new(3));
        assert_eq!(fixed.len(), Nat::new(3));
        assert_eq!(fixed.segments().len(), 1);
        assert_eq!(
            fixed.segment(Nat::ZERO),
            Some(&Segment::Fixed {
                start: Nat::ZERO,
                len: Nat::new(3)
            })
        );
        assert_eq!(fixed.position_of(Nat::new(2)), Some(Nat::new(2)));
        assert_eq!(fixed.card_of(Nat::new(1)), Some(CardId::new(1)));
        assert!(!fixed.is_random_position(Nat::ZERO));

        let random = random_deck(Nat::new(3));
        assert!(random.is_random_position(Nat::new(1)));
        assert_eq!(random.pool_range(Nat::ZERO), Some((Nat::ZERO, Nat::new(3))));
    }

    #[test]
    fn put_bottom_appends_and_drops_own_states() {
        let pile = with_bool(&ordered_deck(Nat::new(3)), Nat::new(1));
        let pile = pile.put_bottom(Nat::new(1)).expect("1 号在牌库");
        assert_eq!(entry_entities(&pile), vec![Nat::ZERO, Nat::new(2), Nat::new(1)]);
        // 被移动的牌自己的状态随移除消失（D89）
        assert!(!pile.has_mark(Nat::new(2), &POWERED));
        assert_eq!(
            pile.segment(Nat::ZERO),
            Some(&Segment::Fixed {
                start: Nat::ZERO,
                len: Nat::new(3)
            })
        );
        assert_eq!(pile.put_bottom(Nat::new(99)), None);
    }

    #[test]
    fn states_follow_insert_and_discard() {
        let pile = ordered_deck(Nat::new(4))
            .add_mark(Nat::new(2), &CHEAPER, Some(Nat::new(2)))
            .expect("位置合法");
        // 舍弃位置 0：状态跟着牌前移到位置 1
        let moved = pile.discard(Nat::ZERO).expect("0 号在牌库");
        assert_eq!(moved.position_of(Nat::new(2)), Some(Nat::new(1)));
        assert_eq!(moved.mark_layers(Nat::new(1), &CHEAPER), Some(Some(Nat::new(2))));
        // 舍弃带状态的那张本身：状态消失
        let dropped = pile.discard(Nat::new(2)).expect("2 号在牌库");
        assert!(!dropped.has_mark(Nat::new(2), &CHEAPER));
        // 均匀插入：把 3 号移到第 1 位 -> 状态跟着到位置 3
        let inserted = pile.insert_evenly(&[Nat::new(3)]).expect("3 号在牌库");
        assert_eq!(inserted.mark_layers(Nat::new(3), &CHEAPER), Some(Some(Nat::new(2))));
        // 置底一张在状态上方的牌：状态前移一位
        let moved = pile.put_bottom(Nat::ZERO).expect("0 号在牌库");
        assert_eq!(moved.mark_layers(Nat::new(1), &CHEAPER), Some(Some(Nat::new(2))));
    }

    #[test]
    fn boolean_state_existence_and_idempotence() {
        let pile = ordered_deck(Nat::new(3));
        assert!(!pile.has_mark(Nat::ZERO, &POWERED));
        let pile = pile.add_mark(Nat::ZERO, &POWERED, None).expect("位置合法");
        assert!(pile.has_mark(Nat::ZERO, &POWERED));
        assert_eq!(pile.mark_layers(Nat::ZERO, &POWERED), Some(None));
        // 重复添加幂等：公共列表里还是一条
        let pile = pile.add_mark(Nat::ZERO, &POWERED, None).expect("位置合法");
        assert_eq!(pile.marks().len(), Nat::new(1));
        let pile = pile.clear_mark(Nat::ZERO, &POWERED).expect("位置合法");
        assert!(!pile.has_mark(Nat::ZERO, &POWERED));
        assert_eq!(pile.add_mark(Nat::new(9), &POWERED, None), None);
    }

    #[test]
    fn layered_states_merge_cancel_and_coexist() {
        let pile = ordered_deck(Nat::new(2));
        let pile = pile
            .add_mark(Nat::ZERO, &CHEAPER, Some(Nat::new(2)))
            .expect("位置合法");
        let pile = pile
            .add_mark(Nat::ZERO, &CHEAPER, Some(Nat::new(3)))
            .expect("位置合法");
        assert_eq!(pile.mark_layers(Nat::ZERO, &CHEAPER), Some(Some(Nat::new(5))));
        // 相反抵消：EXPENSIVE 1 层抵掉 CHEAPER 1 层 -> CHEAPER 4 层，EXPENSIVE 不留下
        let pile = pile
            .add_mark_with_opposite(Nat::ZERO, &EXPENSIVE, Some(Nat::new(1)), &CHEAPER)
            .expect("位置合法");
        assert_eq!(pile.mark_layers(Nat::ZERO, &CHEAPER), Some(Some(Nat::new(4))));
        assert!(!pile.has_mark(Nat::ZERO, &EXPENSIVE));
        // 再来 5 层 EXPENSIVE：CHEAPER 4 层抵消完，剩 EXPENSIVE 1 层
        let pile = pile
            .add_mark_with_opposite(Nat::ZERO, &EXPENSIVE, Some(Nat::new(5)), &CHEAPER)
            .expect("位置合法");
        assert!(!pile.has_mark(Nat::ZERO, &CHEAPER));
        assert_eq!(pile.mark_layers(Nat::ZERO, &EXPENSIVE), Some(Some(Nat::new(1))));
        // 不同状态可以并存：再加 CHEAPER 2 层
        let pile = pile
            .add_mark(Nat::ZERO, &CHEAPER, Some(Nat::new(2)))
            .expect("位置合法");
        assert_eq!(pile.mark_layers(Nat::ZERO, &EXPENSIVE), Some(Some(Nat::new(1))));
        assert_eq!(pile.mark_layers(Nat::ZERO, &CHEAPER), Some(Some(Nat::new(2))));
        assert_eq!(pile.marks().len(), Nat::new(2));
    }

    #[test]
    fn fold_key_includes_boolean_and_layers() {
        let pile = pile_with_bool();
        let key_zero = pile.fold_mark_at(Nat::ZERO);
        let key_one = pile.fold_mark_at(Nat::new(1));
        assert_ne!(key_zero, key_one);
        assert_eq!(key_zero.booleans.len(), 1);
        assert!(key_zero.layered.is_empty());
        let pile = pile
            .add_mark(Nat::ZERO, &CHEAPER, Some(Nat::new(3)))
            .expect("位置合法");
        let key = pile.fold_mark_at(Nat::ZERO);
        assert_eq!(key.booleans.len(), 1);
        assert_eq!(key.layered.len(), 1);
        assert_eq!(key.layered.first().expect("一条").1, Nat::new(3));
    }

    #[test]
    fn pick_unmarked_targets_prefers_place_without_that_state() {
        let pile = with_bool(&ordered_deck(Nat::new(3)), Nat::ZERO);
        let positions = vec![Nat::ZERO, Nat::new(1), Nat::new(2)];
        assert_eq!(
            pile.pick_unmarked_targets(&positions, &POWERED, Nat::new(1)),
            vec![Nat::new(1)]
        );
        // 层数状态不影响布尔状态的「有没有」判据
        let pile = pile
            .add_mark(Nat::ZERO, &CHEAPER, Some(Nat::new(2)))
            .expect("位置合法");
        assert_eq!(
            pile.pick_unmarked_targets(&positions, &POWERED, Nat::new(1)),
            vec![Nat::new(1)]
        );
        // 全都有该布尔状态 -> 退回给定顺序第一个
        let mut all = ordered_deck(Nat::new(3));
        let mut position = Nat::ZERO;
        while position < Nat::new(3) {
            all = with_bool(&all, position);
            position = nadd!(position, Nat::new(1));
        }
        assert_eq!(
            all.pick_unmarked_targets(&positions, &POWERED, Nat::new(1)),
            vec![Nat::ZERO]
        );
        assert_eq!(
            all.pick_unmarked_targets(&positions, &POWERED, Nat::new(2)),
            vec![Nat::ZERO, Nat::new(1)]
        );
        assert_eq!(
            pile.pick_unmarked_targets(&[Nat::new(9)], &POWERED, Nat::new(1)),
            Vec::<Nat>::new()
        );
        assert_eq!(
            pile.pick_unmarked_targets(&[], &POWERED, Nat::new(1)),
            Vec::<Nat>::new()
        );
        // count 超过可选数：先未置位（1、2）再已置位（0）
        assert_eq!(
            pile.pick_unmarked_targets(&positions, &POWERED, Nat::new(5)),
            vec![Nat::new(1), Nat::new(2), Nat::ZERO]
        );
    }

    #[test]
    fn draw_single_from_fixed_is_deterministic() {
        let pile = ordered_deck(Nat::new(3));
        let hand = hand_with(Nat::new(1));
        let branches = pile.draw(&hand);
        assert_eq!(branches.len(), 1);
        let (branch, probability) = branches.first().expect("有分支");
        assert_eq!(*probability, Fraction::ONE);
        assert_eq!(branch.drawn.first().expect("一张").entity, Nat::ZERO);
        assert_eq!(branch.pile.len(), Nat::new(2));
        assert_eq!(branch.hand.len(), Nat::new(2));
    }

    #[test]
    fn draw_single_from_random_is_uniform() {
        let pile = random_deck(Nat::new(3));
        let branches = pile.draw(&HandState::new());
        assert_eq!(branches.len(), 3);
        for (branch, probability) in &branches {
            assert_eq!(*probability, third());
            assert_eq!(branch.pile.len(), Nat::new(2));
        }
    }

    #[test]
    fn draw_top_only_checks_segment_top() {
        let pile = ordered_deck(Nat::new(3));
        assert!(pile
            .draw_top(&HandState::new(), &|card| card == CardId::new(1))
            .is_empty());
        assert_eq!(
            pile.draw_top(&HandState::new(), &|card| card == CardId::new(0))
                .len(),
            1
        );
        let random = random_deck(Nat::new(4));
        let branches = random.draw_top(&HandState::new(), &|card| {
            card == CardId::new(1) || card == CardId::new(3)
        });
        assert_eq!(branches.len(), 2);
        for (_branch, probability) in &branches {
            assert_eq!(*probability, frac(1, 2));
        }
    }

    #[test]
    fn draw_by_card_finds_exact_card() {
        let pile = random_deck(Nat::new(3));
        let branches = pile.draw_by_card(&HandState::new(), CardId::new(2));
        assert_eq!(branches.len(), 1);
        assert_eq!(
            branches.first().expect("有分支").0.drawn.first().expect("一张").entity,
            Nat::new(2)
        );
    }

    #[test]
    fn draw_many_unmarked_folds_to_combinations() {
        let pile = random_deck(Nat::new(3));
        let branches = pile.draw_many(&HandState::new(), Nat::new(2));
        assert_eq!(branches.len(), 3);
        let total: Fraction = branches.iter().map(|(_, probability)| *probability).sum();
        assert!(total.is_one());
        for (_branch, probability) in &branches {
            assert_eq!(*probability, third());
        }
    }

    #[test]
    fn draw_many_marks_fold_by_combination_d98() {
        let pile = with_bool(&random_deck(Nat::new(3)), Nat::new(2));
        let branches = pile.draw_many(&HandState::new(), Nat::new(2));
        assert_eq!(branches.len(), 3);
        for (_branch, probability) in &branches {
            assert_eq!(*probability, third());
        }
    }

    #[test]
    fn draw_many_all_marked_same_key_folds_to_one() {
        let mut pile = random_deck(Nat::new(3));
        let mut position = Nat::ZERO;
        while position < Nat::new(3) {
            pile = with_bool(&pile, position);
            position = nadd!(position, Nat::new(1));
        }
        let branches = pile.draw_many(&HandState::new(), Nat::new(3));
        assert_eq!(branches.len(), 1);
        assert_eq!(branches.first().expect("有分支").1, Fraction::ONE);
    }

    #[test]
    fn draw_many_same_card_id_tests() {
        let pile = PileState::from_random(vec![
            (Nat::ZERO, CardId::new(5)),
            (Nat::new(1), CardId::new(5)),
            (Nat::new(2), CardId::new(5)),
        ]);
        let branches = pile.draw_many(&HandState::new(), Nat::new(2));
        assert_eq!(branches.len(), 1);
        assert_eq!(branches.first().expect("有分支").1, Fraction::ONE);
        let marked = with_bool(&pile, Nat::new(2));
        let branches = marked.draw_many(&HandState::new(), Nat::new(2));
        assert_eq!(branches.len(), 2);
        let mut probabilities: Vec<Fraction> =
            branches.iter().map(|(_, probability)| *probability).collect();
        probabilities.sort_by(|left, right| left.cmp_value(*right));
        assert_eq!(probabilities.first().copied().expect("非空"), third());
        assert_eq!(probabilities.last().copied().expect("非空"), two_thirds());
    }

    #[test]
    fn draw_many_aab_two_variants() {
        let cards = vec![
            (Nat::ZERO, CardId::new(7)),
            (Nat::new(1), CardId::new(7)),
            (Nat::new(2), CardId::new(9)),
        ];
        let pile = with_bool(&PileState::from_random(cards.clone()), Nat::new(1));
        let branches = pile.draw_many(&HandState::new(), Nat::new(2));
        assert_eq!(branches.len(), 3);
        for (_branch, probability) in &branches {
            assert_eq!(*probability, third());
        }
        let pile = with_bool(&PileState::from_random(cards), Nat::new(2));
        let branches = pile.draw_many(&HandState::new(), Nat::new(2));
        assert_eq!(branches.len(), 2);
        let mut probabilities: Vec<Fraction> =
            branches.iter().map(|(_, probability)| *probability).collect();
        probabilities.sort_by(|left, right| left.cmp_value(*right));
        assert_eq!(probabilities.first().copied().expect("非空"), third());
        assert_eq!(probabilities.last().copied().expect("非空"), two_thirds());
    }

    #[test]
    fn draw_many_hand_overflow_stays_ordered() {
        let hand = hand_with(Nat::new(9));
        let pile = random_deck(Nat::new(3));
        let branches = pile.draw_many(&hand, Nat::new(2));
        assert_eq!(branches.len(), 6);
        for (branch, probability) in &branches {
            assert_eq!(*probability, sixth());
            assert_eq!(branch.hand.len(), HandState::MAX);
            assert_eq!(branch.pile.len(), Nat::new(1));
        }
    }

    #[test]
    fn draw_many_fixed_then_random_combines() {
        let mut pile = ordered_deck(Nat::new(2));
        pile.entries.push(PileEntry::new(Nat::new(2), CardId::new(2)));
        pile.entries.push(PileEntry::new(Nat::new(3), CardId::new(3)));
        pile.entries.push(PileEntry::new(Nat::new(4), CardId::new(4)));
        pile.segments.push(Segment::Random {
            pool: Nat::new(7),
            start: Nat::new(2),
            len: Nat::new(3),
        });
        let branches = pile.draw_many(&HandState::new(), Nat::new(4));
        assert_eq!(branches.len(), 3);
        for (branch, probability) in &branches {
            assert_eq!(*probability, third());
            assert_eq!(branch.drawn.len(), 4);
            assert_eq!(branch.pile.len(), Nat::new(1));
        }
    }

    #[test]
    fn split_random_segment_shares_pool_and_entries() {
        let pile = random_deck(Nat::new(3));
        let pile = pile.split_random_segment(Nat::ZERO).expect("0 号段是随机段");
        assert_eq!(pile.segments().len(), 2);
        assert_eq!(
            pile.segment(Nat::ZERO).copied(),
            pile.segment(Nat::new(1)).copied()
        );
        let branches = pile.draw_from_random(&HandState::new(), Nat::ZERO);
        assert_eq!(branches.len(), 3);
        let branch = &branches.first().expect("有分支").0;
        assert_eq!(branch.pile.pool_range(Nat::ZERO), Some((Nat::ZERO, Nat::new(2))));
    }

    #[test]
    fn discard_at_fixed_and_random() {
        let fixed = ordered_deck(Nat::new(3));
        let single = fixed.discard_at(Nat::new(1));
        assert_eq!(single.len(), 1);
        let (next, entity, probability) = single.first().expect("有分支");
        assert_eq!(*entity, Nat::new(1));
        assert_eq!(*probability, Fraction::ONE);
        assert_eq!(next.len(), Nat::new(2));
        assert!(fixed.discard_at(Nat::new(9)).is_empty());

        let random = random_deck(Nat::new(3));
        let branches = random.discard_at(Nat::new(1));
        assert_eq!(branches.len(), 3);
        for (_next, _entity, probability) in &branches {
            assert_eq!(*probability, third());
        }
    }

    #[test]
    fn discard_random_is_uniform_per_position() {
        let pile = random_deck(Nat::new(3));
        let branches = pile.discard_random();
        assert_eq!(branches.len(), 3);
        for (next, probability) in &branches {
            assert_eq!(*probability, third());
            assert_eq!(next.len(), Nat::new(2));
        }
    }

    fn pending_positions(pile: &PileState) -> Vec<Nat> {
        pile.pending_marks()
            .first()
            .map(|pending| pending.positions.clone())
            .unwrap_or_default()
    }

    #[test]
    fn pending_marks_record_and_follow_positions() {
        let pile = ordered_deck(Nat::new(4))
            .add_pending_mark(
                Mark {
                    state: &POWERED,
                    layers: None,
                },
                &[Nat::new(1), Nat::new(3)],
            )
            .expect("位置合法");
        assert_eq!(pile.pending_marks().len(), 1);
        assert_eq!(pending_positions(&pile), vec![Nat::new(1), Nat::new(3)]);

        // 中间插入一张：位置整体后移
        let inserted = pile
            .insert_evenly_with(&[(Nat::new(10), CardId::new(10))])
            .expect("有新牌可插");
        assert_eq!(pending_positions(&inserted), vec![Nat::new(2), Nat::new(4)]);

        // 舍弃位置 0：后面的位置前移
        let discarded = pile.discard(Nat::ZERO).expect("0 号在牌库");
        assert_eq!(pending_positions(&discarded), vec![Nat::ZERO, Nat::new(2)]);

        // 舍弃带 pending 的那张（位置 1）：该位置条目消失、后面的前移
        let discarded = pile.discard(Nat::new(1)).expect("1 号在牌库");
        assert_eq!(pending_positions(&discarded), vec![Nat::new(2)]);

        // 越界 / 空位置 -> None
        assert_eq!(
            pile.add_pending_mark(
                Mark {
                    state: &POWERED,
                    layers: None,
                },
                &[Nat::new(9)]
            ),
            None
        );
        assert_eq!(
            pile.add_pending_mark(
                Mark {
                    state: &POWERED,
                    layers: None,
                },
                &[]
            ),
            None
        );
    }

    #[test]
    fn pending_marks_expand_when_drawn() {
        let pile = ordered_deck(Nat::new(3))
            .add_pending_mark(
                Mark {
                    state: &POWERED,
                    layers: None,
                },
                &[Nat::new(1)],
            )
            .expect("位置合法");
        // 先抽顶（位置 0，无 pending）：pending 位置 1 -> 0
        let first = pile.draw(&HandState::new());
        let branch = &first.first().expect("有分支").0;
        assert_eq!(branch.drawn.first().expect("一张").entity, Nat::ZERO);
        assert!(branch.drawn_marks.first().expect("对齐").is_empty());
        assert_eq!(pending_positions(&branch.pile), vec![Nat::ZERO]);
        // 再抽：位置 0 带 pending -> 这张牌带状态，pending 清空
        let second = branch.pile.draw(&HandState::new());
        let branch = &second.first().expect("有分支").0;
        assert_eq!(branch.drawn.first().expect("一张").entity, Nat::new(1));
        assert_eq!(
            branch.drawn_marks.first().expect("对齐"),
            &vec![Mark {
                state: &POWERED,
                layers: None,
            }]
        );
        assert!(branch.pile.pending_marks().is_empty());
    }

    #[test]
    fn pending_marks_are_part_of_fold_key() {
        let pile = random_deck(Nat::new(3))
            .add_pending_mark(
                Mark {
                    state: &POWERED,
                    layers: None,
                },
                &[Nat::new(2)],
            )
            .expect("位置合法");
        let branches = pile.draw_many(&HandState::new(), Nat::new(2));
        // 组合折叠照样 3 支、各 1/3；pending 不额外产生随机分支
        assert_eq!(branches.len(), 3);
        for (_branch, probability) in &branches {
            assert_eq!(*probability, third());
        }
        // 抽到 pending 那张的支：这张牌带状态（进了折叠键）
        let with_two = branches
            .iter()
            .find(|(branch, _)| branch.drawn.iter().any(|entry| entry.entity == Nat::new(2)))
            .expect("有含实体 2 的分支");
        let index = with_two
            .0
            .drawn
            .iter()
            .position(|entry| entry.entity == Nat::new(2))
            .expect("位置");
        assert_eq!(
            with_two.0.drawn_marks.get(index).expect("对齐"),
            &vec![Mark {
                state: &POWERED,
                layers: None,
            }]
        );
        // 没抽到 pending 那张的支：pending 位置前移、还留在牌库
        let without_two = branches
            .iter()
            .find(|(branch, _)| !branch.drawn.iter().any(|entry| entry.entity == Nat::new(2)))
            .expect("有不含实体 2 的分支");
        assert_eq!(without_two.0.pile.pending_marks().len(), 1);
    }

    #[cfg(feature = "stats")]
    #[test]
    fn pile_stats_count_updates() {
        let pile = ordered_deck(Nat::new(3));
        let branches = pile.draw(&HandState::new());
        let drawn = &branches.first().expect("有分支").0.pile;
        assert_eq!(drawn.stats().drawn().get(), Nat::new(1));
        let discarded = drawn.discard(Nat::new(1)).expect("1 号在牌库");
        assert_eq!(discarded.stats().discarded().get(), Nat::new(1));
        let inserted = discarded
            .insert_evenly_with(&[(Nat::new(10), CardId::new(10))])
            .expect("有新牌可插");
        assert_eq!(inserted.stats().inserted().get(), Nat::new(1));
        let bottom = discarded.put_bottom(Nat::new(2)).expect("2 号在牌库");
        assert_eq!(bottom.stats().put_bottom().get(), Nat::new(1));
    }

    #[cfg(not(feature = "stats"))]
    #[test]
    fn pile_stats_disabled_are_zero() {
        let pile = ordered_deck(Nat::new(3));
        let branches = pile.draw(&HandState::new());
        let drawn = &branches.first().expect("有分支").0.pile;
        assert!(drawn.stats().drawn().is_zero());
        assert_eq!(std::mem::size_of::<PileStats>(), 0);
    }

    #[test]
    fn add_pending_mark_many_places_n_positions() {
        let pile = ordered_deck(Nat::new(4));
        let mark = Mark {
            state: &POWERED,
            layers: None,
        };
        // 一次塞 3 个位置：step -> step+1（1,2,3）
        let marked = pile
            .add_pending_mark_many(mark, Nat::new(3), |step| Some(nadd!(step, Nat::new(1))))
            .expect("有合法位置");
        assert_eq!(pending_positions(&marked), vec![Nat::new(1), Nat::new(2), Nat::new(3)]);
        // count == 0 -> None
        assert_eq!(
            pile.add_pending_mark_many(mark, Nat::ZERO, |_| Some(Nat::ZERO)),
            None
        );
        // 重复挑同一位置 -> 只记一次
        let deduped = pile
            .add_pending_mark_many(mark, Nat::new(3), |_| Some(Nat::ZERO))
            .expect("位置合法");
        assert_eq!(pending_positions(&deduped), vec![Nat::ZERO]);
        // 全部越界 -> None
        assert_eq!(
            pile.add_pending_mark_many(mark, Nat::new(2), |_| Some(Nat::new(9))),
            None
        );
        // pick 提前返回 None -> 只记录已挑到的
        let early = pile
            .add_pending_mark_many(mark, Nat::new(3), |step| {
                if step.get() == 0 {
                    Some(Nat::new(2))
                } else {
                    None
                }
            })
            .expect("挑到一个");
        assert_eq!(pending_positions(&early), vec![Nat::new(2)]);
    }

    #[test]
    fn put_bottom_many_respects_order_function() {
        let pile = ordered_deck(Nat::new(2));
        let cards = [
            (Nat::new(10), CardId::new(10)),
            (Nat::new(11), CardId::new(11)),
        ];
        // 顺序 0,1：先放 10、再放 11 -> 后放的 11 在底区更上面
        let forward = pile.put_bottom_many(&cards, Some).expect("有牌可放");
        assert_eq!(
            entry_entities(&forward),
            vec![Nat::ZERO, Nat::new(1), Nat::new(11), Nat::new(10)]
        );
        // 顺序 1,0：先放 11、再放 10 -> 10 在底区更上面
        let reversed = pile
            .put_bottom_many(&cards, |step| Some(nsub!(Nat::new(1), step)))
            .expect("有牌可放");
        assert_eq!(
            entry_entities(&reversed),
            vec![Nat::ZERO, Nat::new(1), Nat::new(10), Nat::new(11)]
        );
        // order 立即结束 / 空 cards -> None
        assert_eq!(pile.put_bottom_many(&cards, |_| None), None);
        assert_eq!(pile.put_bottom_many(&[], |_| Some(Nat::ZERO)), None);
        // 越界 / 重复下标忽略；已在牌库的卡忽略
        let partial = pile
            .put_bottom_many(&cards, |step| {
                if step.get() == 0 {
                    Some(Nat::ZERO)
                } else {
                    Some(Nat::new(9))
                }
            })
            .expect("放进去一张");
        assert_eq!(
            entry_entities(&partial),
            vec![Nat::ZERO, Nat::new(1), Nat::new(10)]
        );
        let existing = [
            (Nat::ZERO, CardId::new(99)),
            (Nat::new(12), CardId::new(12)),
        ];
        let with_existing = pile.put_bottom_many(&existing, Some).expect("有新的可放");
        assert_eq!(with_existing.len(), Nat::new(3));
    }

    #[test]
    fn draw_many_amount_clamps_to_available() {
        let pile = random_deck(Nat::new(3));
        // 抽 5 张但只有 3 张：clamp 成全抽（唯一组合、概率 1）
        let branches = pile.draw_many(&HandState::new(), Nat::new(5));
        assert_eq!(branches.len(), 1);
        let (branch, probability) = branches.first().expect("有分支");
        assert_eq!(*probability, Fraction::ONE);
        assert_eq!(branch.drawn.len(), 3);
        assert!(branch.pile.is_empty());
        // n == 0 -> 空 Vec（D81-H）
        assert!(pile.draw_many(&HandState::new(), Nat::ZERO).is_empty());
    }

    #[test]
    fn insert_evenly_with_tests() {
        let pile = ordered_deck(Nat::new(2))
            .insert_evenly_with(&[
                (Nat::new(10), CardId::new(10)),
                (Nat::new(11), CardId::new(11)),
            ])
            .expect("有新牌可插");
        assert_eq!(
            entry_entities(&pile),
            vec![Nat::new(10), Nat::ZERO, Nat::new(1), Nat::new(11)]
        );
        assert_eq!(pile.segment(Nat::ZERO).map(|s| s.len()), Some(Nat::new(4)));
        let pile = pile
            .insert_evenly_with(&[(Nat::ZERO, CardId::new(999)), (Nat::new(12), CardId::new(12))])
            .expect("还有一张");
        assert_eq!(pile.len(), Nat::new(5));
        assert_eq!(pile.insert_evenly_with(&[(Nat::ZERO, CardId::new(999))]), None);
        assert_eq!(pile.insert_evenly_with(&[]), None);

        let empty = PileState::new()
            .insert_evenly_with(&[
                (Nat::new(10), CardId::new(10)),
                (Nat::new(11), CardId::new(11)),
            ])
            .expect("有新牌可插");
        assert_eq!(entry_entities(&empty), vec![Nat::new(10), Nat::new(11)]);
        assert_eq!(empty.segments().len(), 1);

        let random = random_deck(Nat::new(2))
            .insert_evenly_with(&[(Nat::new(10), CardId::new(10))])
            .expect("有新牌可插");
        assert_eq!(random.len(), Nat::new(3));
        assert_eq!(random.pool_range(Nat::ZERO), Some((Nat::ZERO, Nat::new(3))));
    }
}