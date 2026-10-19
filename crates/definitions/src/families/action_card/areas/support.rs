//! **支援区**（规则书·游戏区域 III）：上限 4、满 4 时选择弃置一张（分支）、值三选一。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` 游戏区域 III + 卡面）
//!
//! - III. 支援区：「『支援牌』打出后，会放置在支援区。支援区最多放置 4 张『支援牌』。
//!   占满后打出新的支援牌时，需要先选择一张支援区内的支援牌弃置。
//!   （同时结算多个支援牌效果时，会按照支援区的排列顺序依次结算。）」
//! - 卡面：322014 鲸井小弟「如果对方的支援区未满，则将此牌转移到对方的支援区」；
//!   302213 芙佳的声援「随机生成『伙伴』到场上，直到填满双方支援区」。
//! - 分工计划 §4.4.15（Lead 已定）：**满 4 时由玩家选择弃哪张**（多一个选择分支，
//!   走「选择」原语 `enumerate_select`）；**支援的值** = `None | Flag(bool) | Count(Nat)` 三选一（互斥）。
//!
//! ## 口径（D182 / D183 / D185 / D187）
//!
//! - 支援牌**就是行动牌本身**（规则书 / 游戏内文本描述：`GCG_CARD_ASSIST`，一个 id），
//!   打到支援区只是**同一张卡换区**、不产生新卡牌定义（分工计划 §4.4.3 / §4.4.15）；
//! - 本区**自持状态**（`entries` 有序 / 统计）：变量型、可建多个（我方 / 敌方各一个实例，D34）；
//! - `entries` 顺序 = 支援区**排列顺序**（= 进场先后）；规则书 III「按照支援区的排列顺序依次结算」，
//!   与 `结算顺序.md` §一 第 3 条一致；
//! - 对外只给**只读指针**（`entries()` / `entry_of()` / `value_of()` / `len()` / `stats()`）；
//! - 满 4 时的**选择弃置**用引擎选择原语 `enumerate_select`（列表类型在本文件，
//!   遵循「各块自己的选择列表放自己块里」D91/D92）。
//!
//! ## 待复核
//!
//! - 派发顺序：满 4 时先 `Discarded`（被选中的旧支援）再 `Entered`（新的）—— 规则书只说
//!   「需要先选择一张…弃置」，先弃后进；
//! - 「弃置」= 移除、无弃牌堆（分工计划术语）。

use crate::base::CardId;
use crate::families::action_card::areas::stats::SupportStats;
use crate::rules::ops::support::{support_enter, SupportEnter};
/// 一方支援区上限：4（规则书·游戏区域 III；复用 `rules::ops::support::SUPPORT_CAP`）。
pub use crate::rules::ops::support::SUPPORT_CAP;
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{enumerate_select, nsub, EntityId, Nat, SelectionSource};

/// 支援牌的「值」（分工计划 §4.4.15）：三选一、互斥。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupportValue {
    /// 都没有（合法）。
    None,
    /// 布尔值（如赋能）。
    Flag(bool),
    /// 计数（如层数 / 次数）。
    Count(Nat),
}

/// 支援区的**时机键**（D185：由本区自己定义）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SupportTiming {
    /// 支援牌**进入**支援区。
    Entered,
    /// 支援牌**离开**支援区（弃置：满 4 选择弃置 / 被效果移除）。
    Discarded,
}

/// 支援区效果函数指针：`fn(&mut 支援区, &Event, owner)`。
pub type SupportEffect = fn(&mut SupportZone, &Event, EntityId);

/// 支援区的触发登记项。
pub type SupportTrigger = Trigger<SupportTiming, SupportEffect>;

/// 支援区触发注册表（每个牌手一个具名实例，D34）。
pub type SupportTriggers = TriggerRegistry<SupportTiming, SupportEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type SupportPending = Queued<SupportEffect>;

/// 支援牌的**持续回合表达**（Z14：显式区分「有限回合」与「整局」）。
///
/// 规则书·特殊词条「持续回合」：「每个结束阶段，都会使『持续回合』-1。『持续回合』耗尽后，
/// 会立刻弃置这张牌。」整局状态（K60 / 无持续回合的支援牌）不参与递减，所以用
/// SupportDuration::WholeGame **显式**表达，而不是靠 duration = None 隐含。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupportDuration {
    /// 有限回合：每个结束阶段 -1，耗尽弃置。
    Round(Nat),
    /// 整局：不按回合递减（结束阶段跳过）。
    WholeGame,
}

impl SupportDuration {
    /// 是否整局（不递减）。
    pub const fn is_whole_game(self) -> bool {
        matches!(self, SupportDuration::WholeGame)
    }

    /// 有限回合数（整局 -> None）。
    pub const fn rounds(self) -> Option<Nat> {
        match self {
            SupportDuration::Round(rounds) => Some(rounds),
            SupportDuration::WholeGame => None,
        }
    }
}

/// 一张支援牌：卡 id（= 行动牌 id，唯一）+ 实体号 + 值 + 持续回合。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SupportEntry {
    /// 支援牌的定义 id（**就是行动牌 id**，支援区不产生新卡牌定义）。
    pub card: CardId,
    /// 支援牌实体号。
    pub entity: EntityId,
    /// 支援牌的值（三选一）。
    pub value: SupportValue,
    /// **持续回合**（D13 / 规则书·特殊词条「持续回合」）：显式区分有限回合与整局。
    ///
    /// 待复核 / TODO：规则书 S8.6 只给出「每个结束阶段 -1、耗尽立刻弃置」的通用条款，
    /// 支援牌本身**没有专条**说明哪些支援牌带持续回合；本字段由调用方（打牌 / 效果）给，
    /// 数据层暂不反推，待实测哪些支援牌（如有）使用它。
    pub duration: SupportDuration,
}

impl SupportEntry {
    /// 建一张支援牌条目（**不带持续回合**；旧签名保持可用）。
    pub const fn new(card: CardId, entity: EntityId, value: SupportValue) -> SupportEntry {
        SupportEntry {
            card,
            entity,
            value,
            duration: SupportDuration::WholeGame,
        }
    }

    /// 带持续回合的条目（旧签名兼容：Some(n) = 有限 n 回合；None = 整局）。
    ///
    /// 显式表达请用 SupportEntry::with_support_duration。
    pub const fn with_duration(self, duration: Option<Nat>) -> SupportEntry {
        let duration = match duration {
            Some(rounds) => SupportDuration::Round(rounds),
            None => SupportDuration::WholeGame,
        };
        self.with_support_duration(duration)
    }

    /// 带**显式**持续回合表达的条目（Z14）。
    pub const fn with_support_duration(self, duration: SupportDuration) -> SupportEntry {
        SupportEntry {
            card: self.card,
            entity: self.entity,
            value: self.value,
            duration,
        }
    }
}

/// **支援区**：最多 4 张，按排列顺序（= 进场先后）。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的区域」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct SupportZone {
    /// 支援牌条目（顺序 = 进场先后 / 排列顺序）。
    entries: Vec<SupportEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: SupportStats,
}

impl PartialEq for SupportZone {
    /// **统计不参与局面等价**（D187）。
    fn eq(&self, other: &SupportZone) -> bool {
        self.entries == other.entries
    }
}

impl Eq for SupportZone {}

/// 一个「加支援牌」分支：加完之后的支援区 + 这个分支弃置了谁。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportAddBranch {
    /// 加完之后的支援区。
    pub zone: SupportZone,
    /// 这个分支弃置的旧支援牌（没满 -> None）。
    pub discarded: Option<EntityId>,
}

/// 一个「加支援牌」分支的最终结果：状态 + 待执行。
#[derive(Clone, Debug)]
pub struct SupportAddResult {
    /// 加完之后的支援区。
    pub zone: SupportZone,
    /// 这个分支弃置的旧支援牌（没满 -> None）。
    pub discarded: Option<EntityId>,
    /// 派发出来的待执行（本层不执行）。
    pub pending: Vec<SupportPending>,
}

/// 一次支援区操作的结果：新状态 + 待执行。
#[derive(Clone, Debug)]
pub struct SupportResolution {
    /// 操作后的支援区。
    pub zone: SupportZone,
    /// 派发出来的待执行（本层不执行）。
    pub pending: Vec<SupportPending>,
}

/// **结束阶段持续回合递减**的结果（规则书·特殊词条「持续回合」）。
#[derive(Clone, Debug)]
pub struct SupportTickResolution {
    /// 递减后的支援区（耗尽的已移出）。
    pub zone: SupportZone,
    /// 本次耗尽的支援牌实体（按排列顺序）。
    pub depleted: Vec<EntityId>,
    /// 派发出来的待执行（耗尽的 Discarded；本层不执行）。
    pub pending: Vec<SupportPending>,
}

impl SupportZone {
    /// 空支援区。
    pub const fn new() -> SupportZone {
        SupportZone {
            entries: Vec::new(),
            stats: SupportStats::EMPTY,
        }
    }

    /// 全部支援牌（**只读指针**，D183；顺序 = 排列顺序 / 进场先后）。
    pub fn entries(&self) -> &[SupportEntry] {
        &self.entries
    }

    /// 支援牌张数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何支援牌。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 是否已满（4 张）。
    pub fn is_full(&self) -> bool {
        self.len() >= SUPPORT_CAP
    }

    /// 还能放几张（已满 -> 0）。
    pub fn free_slots(&self) -> Nat {
        nsub!(SUPPORT_CAP, self.len())
    }

    /// 支援区统计（只读指针，D183 / D187）。
    pub fn stats(&self) -> &SupportStats {
        &self.stats
    }

    /// 这张支援牌是否在区里。
    pub fn contains(&self, entity: EntityId) -> bool {
        self.entries.iter().any(|entry| entry.entity == entity)
    }

    /// 某个支援牌实体的条目（只读指针）。
    pub fn entry_of(&self, entity: EntityId) -> Option<&SupportEntry> {
        self.entries.iter().find(|entry| entry.entity == entity)
    }

    /// 某张支援牌的值（不在区里 -> None；在 -> Some(值)）。
    pub fn value_of(&self, entity: EntityId) -> Option<SupportValue> {
        self.entry_of(entity).map(|entry| entry.value)
    }

    /// 某张支援牌的**持续回合**（旧口径兼容：整局 -> None；不在区里 -> None）。
    pub fn duration_of(&self, entity: EntityId) -> Option<Option<Nat>> {
        self.entry_of(entity).map(|entry| entry.duration.rounds())
    }

    /// 某张支援牌的**显式持续回合表达**（Z14；不在区里 -> None）。
    pub fn support_duration_of(&self, entity: EntityId) -> Option<SupportDuration> {
        self.entry_of(entity).map(|entry| entry.duration)
    }

    /// 按**卡 id** 找第一张（卡面「将这张牌转移到…」这类按卡 id 的操作）。
    pub fn entry_of_card(&self, card: CardId) -> Option<&SupportEntry> {
        self.entries.iter().find(|entry| entry.card == card)
    }

    /// 支援区**进场判定**（复用 `rules::ops::support::support_enter`）。
    pub fn enter_kind(&self) -> SupportEnter {
        support_enter(self.len())
    }

    /// **加一张支援牌**（规则书·游戏区域 III）：未满 -> 直接进；满 4 -> **选择弃置一张**（多分支）。
    ///
    /// 返回值 = 每个分支（状态 + 弃置了谁 + 待执行）与**精确概率**：
    /// - 未满：1 个分支、概率 1；
    /// - 满 4：4 个分支、每支概率 1/4（`enumerate_select` 的既定 v1 语义：count = 1 时均匀）。
    pub fn add_with_triggers(
        &self,
        entry: SupportEntry,
        triggers: &SupportTriggers,
    ) -> Vec<(SupportAddResult, Fraction)> {
        let entered_event = Event::of(entry.entity);
        if !self.is_full() {
            let zone = self.with_entry(entry);
            let pending = collect(triggers, SupportTiming::Entered, entered_event);
            return vec![(
                SupportAddResult {
                    zone,
                    discarded: None,
                    pending,
                },
                Fraction::ONE,
            )];
        }
        let candidates: Vec<EntityId> = self.entries.iter().map(|existing| existing.entity).collect();
        let list = SupportDiscardList {
            zone: self,
            entry: &entry,
            candidates,
        };
        let start = SupportAddBranch {
            zone: self.clone(),
            discarded: None,
        };
        enumerate_select(&start, &list, Nat::new(1))
            .into_iter()
            .map(|(branch, probability)| {
                let mut pending: Vec<SupportPending> = Vec::new();
                if let Some(discarded) = branch.discarded {
                    pending.extend(collect(
                        triggers,
                        SupportTiming::Discarded,
                        Event::of(discarded),
                    ));
                }
                pending.extend(collect(triggers, SupportTiming::Entered, entered_event));
                (
                    SupportAddResult {
                        zone: branch.zone,
                        discarded: branch.discarded,
                        pending,
                    },
                    probability,
                )
            })
            .collect()
    }

    /// **弃置指定支援牌**（移出，无弃牌堆）+ 派发 `Discarded`。
    ///
    /// 失败（返回 None、不改状态）：这张支援牌不在区里。
    pub fn discard_with_triggers(
        &self,
        entity: EntityId,
        triggers: &SupportTriggers,
    ) -> Option<SupportResolution> {
        let zone = self.without(entity)?;
        let pending = collect(triggers, SupportTiming::Discarded, Event::of(entity));
        Some(SupportResolution { zone, pending })
    }

    /// 把 `entity` 移出（纯函数内部用；统计 +1 弃置）。
    /// **结束阶段**：所有带持续回合的支援牌 -1；耗尽的**立刻弃置**（规则书·特殊词条「持续回合」）。
    ///
    /// 语义（与出战状态区的 tick_end_phase_with_triggers 同口径）：
    /// - 整局（SupportDuration::WholeGame）不递减、不派发；
    /// - 有限回合（SupportDuration::Round）按**排列顺序**逐条处理：剩余 > 1 -> 减 1 并保留；
    ///   剩余 = 1（或 0）-> 移出支援区 + 派发 Discarded（本层只派发、不执行，D186）；
    /// - 返回的 pending 按排列顺序排列（规则书 III「按支援区排列顺序依次结算」）。
    ///
    /// TODO / 待实测：规则书 S8.6 只有通用条款，没说支援牌持续回合在**哪个时机**递减
    /// （本实现按「结束阶段」，与卡牌 / 状态持续回合的通用条文一致；若实测是回合开始需调整）。
    pub fn tick_end_phase_with_triggers(
        &self,
        triggers: &SupportTriggers,
    ) -> SupportTickResolution {
        let mut zone = self.clone();
        let mut depleted: Vec<EntityId> = Vec::new();
        let mut pending: Vec<SupportPending> = Vec::new();
        let mut kept: Vec<SupportEntry> = Vec::with_capacity(zone.entries.len());
        let entries = zone.entries.clone();
        for mut entry in entries {
            match entry.duration {
                SupportDuration::WholeGame => kept.push(entry),
                SupportDuration::Round(remaining) => {
                    if remaining <= Nat::new(1) {
                        depleted.push(entry.entity);
                        pending.extend(collect(
                            triggers,
                            SupportTiming::Discarded,
                            Event::of(entry.entity),
                        ));
                        zone.stats.on_discarded(Nat::new(1));
                    } else {
                        entry.duration = SupportDuration::Round(nsub!(remaining, Nat::new(1)));
                        kept.push(entry);
                    }
                }
            }
        }
        zone.entries = kept;
        SupportTickResolution {
            zone,
            depleted,
            pending,
        }
    }

    fn without(&self, entity: EntityId) -> Option<SupportZone> {
        let position = self
            .entries
            .iter()
            .position(|entry| entry.entity == entity)?;
        let mut next = self.clone();
        next.entries.remove(position);
        next.stats.on_discarded(Nat::new(1));
        Some(next)
    }

    /// 把 `entry` 加进末尾（纯函数内部用；统计 +1 进场）。
    fn with_entry(&self, entry: SupportEntry) -> SupportZone {
        let mut next = self.clone();
        next.entries.push(entry);
        next.stats.on_entered(Nat::new(1));
        next
    }
}

/// 满 4 时的**选择弃置列表**（选择原语的列表类型；列表 = 当前支援区的 4 张）。
///
/// 每条分支 = 「弃置第 index 张 + 把新支援牌加进末尾」，概率由 `enumerate_select` 给。
struct SupportDiscardList<'a> {
    zone: &'a SupportZone,
    entry: &'a SupportEntry,
    candidates: Vec<EntityId>,
}

impl SelectionSource<SupportAddBranch> for SupportDiscardList<'_> {
    // bare-int-ok: 引擎选择列表的 len 契约，非业务数值
    fn len(&self) -> usize {
        self.candidates.len()
    }

    // bare-int-ok: 引擎选择列表的下标契约，非业务数值
    fn apply(&self, _state: &SupportAddBranch, index: usize) -> (SupportAddBranch, Fraction) {
        let Some(entity) = self.candidates.get(index).copied() else {
            return (
                SupportAddBranch {
                    zone: self.zone.clone(),
                    discarded: None,
                },
                Fraction::ONE,
            );
        };
        let Some(discarded_zone) = self.zone.without(entity) else {
            return (
                SupportAddBranch {
                    zone: self.zone.clone(),
                    discarded: None,
                },
                Fraction::ONE,
            );
        };
        (
            SupportAddBranch {
                zone: discarded_zone.with_entry(*self.entry),
                discarded: Some(entity),
            },
            Fraction::ONE,
        )
    }
}

/// 执行一条待执行列表（**调用方显式调用**；派发永不执行）。
pub fn execute_support(pending: &[SupportPending], zone: &mut SupportZone) {
    for item in pending {
        (item.effect)(zone, &item.event, item.owner);
    }
}

/// 收集某个时机 + 事件的待执行。
fn collect(triggers: &SupportTriggers, timing: SupportTiming, event: Event) -> Vec<SupportPending> {
    dispatch(triggers, timing, &event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ops::order::EffectSource;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    fn entry(entity: u32, card: u32) -> SupportEntry {
        SupportEntry::new(CardId::new(card), n(entity), SupportValue::None)
    }

    fn noop(_zone: &mut SupportZone, _event: &Event, _owner: EntityId) {}

    fn registry_for(timing: SupportTiming) -> SupportTriggers {
        let mut triggers = SupportTriggers::new();
        triggers.register(Trigger::new(timing, n(1), EffectSource::Support, n(0), noop));
        triggers
    }

    fn zone_with(count: u32) -> SupportZone {
        let triggers = SupportTriggers::new();
        let mut zone = SupportZone::new();
        for index in 0..count {
            let branch = zone
                .add_with_triggers(entry(index, index), &triggers)
                .into_iter()
                .next()
                .expect("未满时只有一支");
            zone = branch.0.zone;
        }
        zone
    }

    #[test]
    fn empty_zone_basics() {
        let zone = SupportZone::new();
        assert!(zone.is_empty());
        assert_eq!(zone.len(), Nat::ZERO);
        assert_eq!(zone.free_slots(), SUPPORT_CAP);
        assert_eq!(zone.enter_kind(), SupportEnter::Accept);
        assert!(zone.value_of(n(1)).is_none());
    }

    #[test]
    fn not_full_adds_in_one_branch_with_probability_one() {
        let triggers = registry_for(SupportTiming::Entered);
        let zone = SupportZone::new();
        let branches = zone.add_with_triggers(entry(1, 321004), &triggers);
        assert_eq!(branches.len(), 1);
        let (result, probability) = branches.first().expect("一支");
        assert_eq!(*probability, Fraction::ONE);
        assert_eq!(result.zone.len(), n(1));
        assert_eq!(result.discarded, None);
        assert_eq!(result.pending.len(), 1);
        assert_eq!(result.pending.first().expect("一条").event.who, Some(n(1)));
        // 顺序 = 进场先后。
        let after = result
            .zone
            .add_with_triggers(entry(2, 321005), &triggers)
            .into_iter()
            .next()
            .expect("一支");
        assert_eq!(
            after
                .0
                .zone
                .entries()
                .iter()
                .map(|entry| entry.entity)
                .collect::<Vec<_>>(),
            vec![n(1), n(2)]
        );
    }

    #[test]
    fn full_zone_gives_one_branch_per_discard_candidate() {
        let triggers = SupportTriggers::new();
        let zone = zone_with(4);
        assert!(zone.is_full());
        assert_eq!(zone.enter_kind(), SupportEnter::MustChooseDiscard);
        let branches = zone.add_with_triggers(entry(99, 302213), &triggers);
        assert_eq!(branches.len(), 4);
        // 每支都保留 4 张、都包含新支援牌、都不包含被弃置的那张。
        let mut seen_discarded: Vec<EntityId> = Vec::new();
        let first_probability = branches.first().expect("有分支").1;
        for (result, probability) in &branches {
            // 均匀：每支概率相同（配合下面「和 = 1」即 1/4）。
            assert_eq!(*probability, first_probability);
            assert_eq!(result.zone.len(), n(4));
            assert!(result.zone.contains(n(99)));
            let discarded = result.discarded.expect("满 4 一定有弃置");
            assert!(!result.zone.contains(discarded));
            seen_discarded.push(discarded);
        }
        seen_discarded.sort();
        seen_discarded.dedup();
        assert_eq!(seen_discarded.len(), 4);
        // 概率和 = 1。
        let total: Fraction = branches.iter().map(|(_, probability)| *probability).sum();
        assert!(total.is_one());
    }

    #[test]
    fn full_zone_branches_dispatch_discard_then_enter() {
        let mut triggers = SupportTriggers::new();
        triggers.register(Trigger::new(
            SupportTiming::Discarded,
            n(1),
            EffectSource::Support,
            n(0),
            noop,
        ));
        triggers.register(Trigger::new(
            SupportTiming::Entered,
            n(1),
            EffectSource::Support,
            n(0),
            noop,
        ));
        let zone = zone_with(4);
        let branches = zone.add_with_triggers(entry(99, 302213), &triggers);
        for (result, _probability) in &branches {
            assert_eq!(result.pending.len(), 2);
            assert!(result.discarded.is_some());
            assert_eq!(
                result.pending.first().expect("弃置").event.who,
                result.discarded
            );
            assert_eq!(
                result.pending.get(1).expect("进场").event.who,
                Some(n(99))
            );
        }
    }

    #[test]
    fn discard_specific_removes_and_dispatches() {
        let triggers = registry_for(SupportTiming::Discarded);
        let zone = zone_with(2);
        let discarded = zone.discard_with_triggers(n(0), &triggers).expect("在区里");
        assert_eq!(discarded.zone.len(), n(1));
        assert!(!discarded.zone.contains(n(0)));
        assert_eq!(discarded.pending.len(), 1);
        assert!(discarded.zone.discard_with_triggers(n(0), &triggers).is_none());
    }

    #[test]
    fn end_phase_duration_ticks_and_depleted_support_is_discarded() {
        let triggers = registry_for(SupportTiming::Discarded);
        let zone = SupportZone::new()
            .with_entry(entry(0, 1).with_duration(Some(n(2))))
            .with_entry(entry(1, 2).with_duration(Some(n(1))))
            .with_entry(entry(2, 3));
        assert_eq!(zone.duration_of(n(0)), Some(Some(n(2))));
        assert_eq!(zone.duration_of(n(1)), Some(Some(n(1))));
        assert_eq!(zone.duration_of(n(2)), Some(None));
        assert_eq!(zone.duration_of(n(9)), None);

        // 第一次结束阶段：0 剩 1；1 耗尽弃置；没有持续回合的 2 不动。
        let first = zone.tick_end_phase_with_triggers(&triggers);
        assert_eq!(first.depleted, vec![n(1)]);
        assert_eq!(first.zone.len(), n(2));
        assert_eq!(first.zone.duration_of(n(0)), Some(Some(n(1))));
        assert!(!first.zone.contains(n(1)));
        assert_eq!(first.zone.duration_of(n(2)), Some(None));
        assert_eq!(first.pending.len(), 1);
        assert_eq!(first.pending.first().expect("弃置").event.who, Some(n(1)));

        // 第二次结束阶段：0 也耗尽弃置，只剩没有持续回合的 2。
        let second = first.zone.tick_end_phase_with_triggers(&triggers);
        assert_eq!(second.depleted, vec![n(0)]);
        assert_eq!(second.zone.len(), n(1));
        assert!(second.zone.contains(n(2)));
        assert_eq!(second.pending.len(), 1);
        assert_eq!(second.pending.first().expect("弃置").event.who, Some(n(0)));

        // 第三次：没有带持续回合的牌 -> 什么都不发生。
        let third = second.zone.tick_end_phase_with_triggers(&triggers);
        assert!(third.depleted.is_empty());
        assert!(third.pending.is_empty());
        assert_eq!(third.zone.len(), n(1));
    }

    #[test]
    fn whole_game_duration_skips_tick_and_round_duration_still_ticks() {
        let triggers = registry_for(SupportTiming::Discarded);
        // 显式整局 + 显式有限回合 + 旧口径（None = 整局）。
        let zone = SupportZone::new()
            .with_entry(
                entry(0, 1).with_support_duration(SupportDuration::WholeGame),
            )
            .with_entry(
                entry(1, 2).with_support_duration(SupportDuration::Round(n(2))),
            )
            .with_entry(entry(2, 3).with_duration(None));
        assert_eq!(
            zone.support_duration_of(n(0)),
            Some(SupportDuration::WholeGame)
        );
        assert_eq!(
            zone.support_duration_of(n(1)),
            Some(SupportDuration::Round(n(2)))
        );
        assert!(zone
            .support_duration_of(n(0))
            .expect("在区里")
            .is_whole_game());

        // 第一次结束阶段：整局的 0 / 2 都不动，有限回合的 1 递减到 1。
        let first = zone.tick_end_phase_with_triggers(&triggers);
        assert!(first.depleted.is_empty());
        assert_eq!(first.zone.len(), n(3));
        assert_eq!(
            first.zone.support_duration_of(n(1)),
            Some(SupportDuration::Round(n(1)))
        );
        assert_eq!(
            first.zone.support_duration_of(n(0)),
            Some(SupportDuration::WholeGame)
        );

        // 第二次：1 耗尽弃置；整局的 0 / 2 仍留在区里。
        let second = first.zone.tick_end_phase_with_triggers(&triggers);
        assert_eq!(second.depleted, vec![n(1)]);
        assert_eq!(second.zone.len(), n(2));
        assert!(second.zone.contains(n(0)));
        assert!(second.zone.contains(n(2)));

        // 再多 tick 几次也不会移除整局条目。
        let third = second.zone.tick_end_phase_with_triggers(&triggers);
        let fourth = third.zone.tick_end_phase_with_triggers(&triggers);
        assert!(fourth.depleted.is_empty());
        assert_eq!(fourth.zone.len(), n(2));
    }

    #[test]
    fn value_is_one_of_three_kinds() {
        let zone = SupportZone::new()
            .with_entry(SupportEntry::new(CardId::new(1), n(1), SupportValue::None))
            .with_entry(SupportEntry::new(
                CardId::new(2),
                n(2),
                SupportValue::Flag(true),
            ))
            .with_entry(SupportEntry::new(
                CardId::new(3),
                n(3),
                SupportValue::Count(n(2)),
            ));
        assert_eq!(zone.value_of(n(1)), Some(SupportValue::None));
        assert_eq!(zone.value_of(n(2)), Some(SupportValue::Flag(true)));
        assert_eq!(zone.value_of(n(3)), Some(SupportValue::Count(n(2))));
        assert_eq!(zone.value_of(n(9)), None);
        assert_eq!(zone.entry_of_card(CardId::new(2)).expect("有").entity, n(2));
    }

    #[test]
    fn stats_follow_feature_flag() {
        let triggers = SupportTriggers::new();
        let zone = zone_with(4);
        let branches = zone.add_with_triggers(entry(99, 302213), &triggers);
        let result = &branches.first().expect("有分支").0;
        if cfg!(feature = "stats") {
            assert_eq!(result.zone.stats().entered().get(), n(5));
            assert_eq!(result.zone.stats().discarded().get(), n(1));
        } else {
            assert!(result.zone.stats().entered().is_zero());
            assert!(result.zone.stats().discarded().is_zero());
        }
    }
}
