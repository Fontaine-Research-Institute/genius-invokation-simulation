//! 【召唤物】**召唤物区**（规则书·游戏区域 IV）：上限 4、满员无法生成、同名合并、
//! 可用次数递减 / 耗尽弃置。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` 游戏区域 IV + 特殊词条 可用次数）
//!
//! - IV. 召唤物区：「一些角色技能会在召唤物区生成『召唤物』。召唤物区最多放置 4 个『召唤物』。
//!   占满后，无法生成新的『召唤物』。（同时结算多个召唤物效果时，会按照召唤物区的排列顺序依次结算。）」
//! - 特殊词条 可用次数：「每次触发这张牌的效果，都会消耗 1 次『可用次数』。『可用次数』耗尽后，
//!   会立刻弃置这张牌。」→ 递减 / 耗尽判定**复用** `rules::ops::lifetime`（本层不重写数值）。
//! - 分工计划 §4.4.15（Lead 已定）：「同名召唤物再召唤时——**次数与效果量都取最大，
//!   效果类型按最新的那个**（不新增占位）。」
//!
//! ## 口径（D182 / D183 / D185 / D187）
//!
//! - 本区**自持状态**（`entries` / 统计）：变量型、可建多个（我方 / 敌方各一个实例，D34）；
//! - 对外只给**只读指针**（`entries()` / `entry_of()` / `len()` / `stats()`）；
//! - `entries` 顺序 = **召唤先后**（结算顺序第 3 条 / `结算顺序.md` §一：同来源类别按进场先后）；
//!   同名合并不改变原有位置（不重排）；
//! - 进 / 出区域的时机键由本区自己定义（D185），走 `timeline` 派发（只排序、不执行 D186）。
//!
//! ## 待复核
//!
//! - 召唤物区**已满**时再召唤同名召唤物：本层按「先尝试同名合并、合并不了才算满员无法生成」处理
//!   ——「不新增占位」所以合并不违反上限；规则书没写这种情况；
//! - `effect_kind` = `Option<SummonEffectKind>`（`families::summon::def` 的**正式效果类型枚举**：
//!   伤害 / 治疗 / 状态 / 其它；区域层不解释，合并时「取最新」）；
//! - `points` = `Option<Nat>`（**效果量**：伤害点数 / 治疗点数 / 状态层数…；含义由召唤物定义 /
//!   执行动作解释，合并时「取最大」）。召唤物定义 `SummonDef` 已落地，`SummonDef::entry()`
//!   直接产出这两种计量（`待接入` 执行器）。

pub mod record;
pub mod stats;

use crate::base::CardId;
use crate::families::summon::def::SummonEffectKind;
use crate::families::summon::zones::stats::SummonZoneStats;
use crate::rules::ops::lifetime::{is_depleted, uses_after_trigger};
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::{nsub, EntityId, Nat};

/// 一方召唤物区上限：4（规则书·游戏区域 IV）。
pub const SUMMON_CAP: Nat = Nat::new(4);

/// 召唤物区的**时机键**（D185：由本区自己定义）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SummonTiming {
    /// 召唤物**进入**召唤物区（生成新占位）。
    Summoned,
    /// 同名召唤物**合并**（不新增占位；次数 / 效果量取最大、类型取最新）。
    Merged,
    /// 召唤物效果**触发一次**（可用次数递减；没有可用次数的召唤物也发）。
    Triggered,
    /// 可用次数**耗尽** -> 立刻弃置（规则书·特殊词条 可用次数）。
    Depleted,
    /// 召唤物被**指定弃置**（移出，无弃牌堆）。
    Discarded,
}

/// 召唤物区效果函数指针：`fn(&mut 召唤物区, &Event, owner)`。
pub type SummonEffect = fn(&mut SummonZone, &Event, EntityId);

/// 召唤物区的触发登记项。
pub type SummonTrigger = Trigger<SummonTiming, SummonEffect>;

/// 召唤物区触发注册表（每个牌手一个具名实例，D34）。
pub type SummonTriggers = TriggerRegistry<SummonTiming, SummonEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type SummonPending = Queued<SummonEffect>;

/// 一个召唤物：召唤物实体 + 召唤物卡 id + 计量（可用次数 / 效果量 / 效果类型标识）。
///
/// - `uses`：可用次数（`None` = 没有可用次数，触发不递减 —— 分工计划「三项改可选」）；
/// - `points`：效果量（伤害点数 / 重复次数…，含义由执行动作解释，`None` = 无计量）；
/// - `effect_kind`：效果类型（`SummonEffectKind`，由召唤物定义给出；区域层不解释）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SummonEntry {
    /// 召唤物实体号。
    pub summon: EntityId,
    /// 召唤物卡定义 id。
    pub card: CardId,
    /// 可用次数（None = 无可用次数）。
    pub uses: Option<Nat>,
    /// 效果量（含义由执行动作解释）。
    pub points: Option<Nat>,
    /// 效果类型（正式枚举；区域层不解释）。
    pub effect_kind: Option<SummonEffectKind>,
}

impl SummonEntry {
    /// 只给实体与卡 id 的召唤物（计量全是 None）。
    pub const fn new(summon: EntityId, card: CardId) -> SummonEntry {
        SummonEntry {
            summon,
            card,
            uses: None,
            points: None,
            effect_kind: None,
        }
    }

    /// 设可用次数（构建用；纯值）。
    pub const fn with_uses(self, uses: Option<Nat>) -> SummonEntry {
        SummonEntry { uses, ..self }
    }

    /// 设效果量（构建用；纯值）。
    pub const fn with_points(self, points: Option<Nat>) -> SummonEntry {
        SummonEntry { points, ..self }
    }

    /// 设效果类型（构建用；纯值）。
    pub const fn with_effect_kind(self, effect_kind: Option<SummonEffectKind>) -> SummonEntry {
        SummonEntry {
            effect_kind,
            ..self
        }
    }
}

/// **召唤物区**：最多 4 个召唤物，按召唤先后排列。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的区域」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct SummonZone {
    /// 召唤物条目（顺序 = 召唤先后）。
    entries: Vec<SummonEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: SummonZoneStats,
}

impl PartialEq for SummonZone {
    /// **统计不参与局面等价**（D187）。
    fn eq(&self, other: &SummonZone) -> bool {
        self.entries == other.entries
    }
}

impl Eq for SummonZone {}

/// 一次召唤物区操作的结果：新状态 + 结果 + 待执行。
#[derive(Clone, Debug)]
pub struct SummonResolution {
    /// 操作后的召唤物区。
    pub zone: SummonZone,
    /// 这次操作的结果。
    pub outcome: SummonOutcome,
    /// 派发出来的待执行（本层不执行）。
    pub pending: Vec<SummonPending>,
}

/// 召唤 / 生成的结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SummonOutcome {
    /// 占了一个新位置。
    Summoned,
    /// 同名合并：`kept` 是保留的原召唤物，`absorbed` 是这次被吸收的新召唤物（不新增占位）。
    Merged {
        /// 保留的原召唤物实体号。
        kept: EntityId,
        /// 被吸收掉的新召唤物实体号。
        absorbed: EntityId,
    },
    /// 触发一次（可用次数已递减；没有可用次数的召唤物也会给这个结果）。
    Triggered {
        /// 剩余可用次数（没有可用次数 -> None）。
        remaining: Option<Nat>,
    },
    /// 可用次数耗尽，召唤物已被弃置。
    Depleted,
    /// 被指定弃置。
    Discarded,
    /// 召唤物区已满，**无法生成新的召唤物**（状态不变；「动作算发生」但没有任何召唤物进场）。
    RejectedFull,
}

impl SummonZone {
    /// 空召唤物区。
    pub const fn new() -> SummonZone {
        SummonZone {
            entries: Vec::new(),
            stats: SummonZoneStats::EMPTY,
        }
    }

    /// 全部召唤物（**只读指针**，D183；顺序 = 召唤先后）。
    pub fn entries(&self) -> &[SummonEntry] {
        &self.entries
    }

    /// 召唤物个数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何召唤物。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 是否已满（4 个）。
    pub fn is_full(&self) -> bool {
        self.len() >= SUMMON_CAP
    }

    /// 还能放几个召唤物（已满 -> 0）。
    pub fn free_slots(&self) -> Nat {
        nsub!(SUMMON_CAP, self.len())
    }

    /// 召唤物区统计（只读指针，D183 / D187）。
    pub fn stats(&self) -> &SummonZoneStats {
        &self.stats
    }

    /// 这个召唤物实体是否在区里。
    pub fn contains(&self, summon: EntityId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.summon == summon)
    }

    /// 某个召唤物实体的条目（只读指针）。
    pub fn entry_of(&self, summon: EntityId) -> Option<&SummonEntry> {
        self.entries
            .iter()
            .find(|entry| entry.summon == summon)
    }

    /// 同**卡 id** 的召唤物条目（同名合并判定用；只读指针）。
    pub fn entry_of_card(&self, card: CardId) -> Option<&SummonEntry> {
        self.entries.iter().find(|entry| entry.card == card)
    }

    /// **召唤 / 生成**（规则书·游戏区域 IV）+ 派发「生成 / 合并」。
    ///
    /// - 同名（同卡 id）已存在：**合并**（使用次数取最大、效果量取最大、效果类型取最新），
    ///   不新增占位、不改变原位置；`outcome = Merged`；
    /// - 未满：占一个新位置；`outcome = Summoned`；
    /// - 已满且没有同名：**无法生成**，状态不变；`outcome = RejectedFull`（不派发）。
    pub fn summon_with_triggers(
        &self,
        entry: SummonEntry,
        triggers: &SummonTriggers,
    ) -> SummonResolution {
        let mut zone = self.clone();
        let mut merged: Option<(EntityId, Option<Nat>)> = None;
        if let Some(existing) = zone
            .entries
            .iter_mut()
            .find(|existing| existing.card == entry.card)
        {
            let kept = existing.summon;
            existing.uses = max_opt(existing.uses, entry.uses);
            existing.points = max_opt(existing.points, entry.points);
            // 效果类型按**最新的那个**；新的没给就保留原来的。
            existing.effect_kind = entry.effect_kind.or(existing.effect_kind);
            merged = Some((kept, existing.uses));
        }
        if let Some((kept, uses)) = merged {
            zone.stats.on_merged(Nat::new(1));
            let pending = collect(
                triggers,
                SummonTiming::Merged,
                merged_event(kept, entry.summon, uses),
            );
            return SummonResolution {
                zone,
                outcome: SummonOutcome::Merged {
                    kept,
                    absorbed: entry.summon,
                },
                pending,
            };
        }
        if zone.is_full() {
            zone.stats.on_rejected_full(Nat::new(1));
            return SummonResolution {
                zone,
                outcome: SummonOutcome::RejectedFull,
                pending: Vec::new(),
            };
        }
        zone.entries.push(entry);
        zone.stats.on_summoned(Nat::new(1));
        let pending = collect(
            triggers,
            SummonTiming::Summoned,
            Event::of(entry.summon),
        );
        SummonResolution {
            zone,
            outcome: SummonOutcome::Summoned,
            pending,
        }
    }

    /// **触发一次**（规则书·特殊词条 可用次数）：可用次数 -1；耗尽则立刻弃置。
    ///
    /// - 有可用次数且减到 0：移出 + 派发 `Depleted`；
    /// - 有可用次数且还有剩余：写回剩余次数 + 派发 `Triggered`（`amount` = 剩余次数）；
    /// - **没有可用次数**：不递减、不移除，只派发 `Triggered`（`amount` = 0）。
    ///
    /// 失败（返回 None、不改状态）：这个召唤物不在区里。
    pub fn tick_with_triggers(
        &self,
        summon: EntityId,
        triggers: &SummonTriggers,
    ) -> Option<SummonResolution> {
        let position = self
            .entries
            .iter()
            .position(|entry| entry.summon == summon)?;
        let entry = self.entries.get(position).copied()?;
        let mut zone = self.clone();
        zone.stats.on_triggered(Nat::new(1));
        let (pending, outcome) = match entry.uses {
            None => (
                collect(
                    triggers,
                    SummonTiming::Triggered,
                    triggered_event(summon, None),
                ),
                SummonOutcome::Triggered { remaining: None },
            ),
            Some(uses) => {
                let next = uses_after_trigger(uses);
                if is_depleted(next) {
                    zone.entries.remove(position);
                    zone.stats.on_depleted(Nat::new(1));
                    (
                        collect(triggers, SummonTiming::Depleted, Event::of(summon)),
                        SummonOutcome::Depleted,
                    )
                } else {
                    if let Some(target) = zone.entries.get_mut(position) {
                        target.uses = Some(next);
                    }
                    (
                        collect(
                            triggers,
                            SummonTiming::Triggered,
                            triggered_event(summon, Some(next)),
                        ),
                        SummonOutcome::Triggered {
                            remaining: Some(next),
                        },
                    )
                }
            }
        };
        Some(SummonResolution {
            zone,
            outcome,
            pending,
        })
    }

    /// **弃置指定召唤物**（移出，无弃牌堆）+ 派发 `Discarded`。
    ///
    /// 失败（返回 None、不改状态）：这个召唤物不在区里。
    pub fn discard_with_triggers(
        &self,
        summon: EntityId,
        triggers: &SummonTriggers,
    ) -> Option<SummonResolution> {
        let position = self
            .entries
            .iter()
            .position(|entry| entry.summon == summon)?;
        let mut zone = self.clone();
        zone.entries.remove(position);
        zone.stats.on_discarded(Nat::new(1));
        let pending = collect(triggers, SummonTiming::Discarded, Event::of(summon));
        Some(SummonResolution {
            zone,
            outcome: SummonOutcome::Discarded,
            pending,
        })
    }
}

/// 执行一条待执行列表（**调用方显式调用**；派发永不执行）。
pub fn execute_summon(pending: &[SummonPending], zone: &mut SummonZone) {
    for item in pending {
        (item.effect)(zone, &item.event, item.owner);
    }
}

/// 收集某个时机 + 事件的待执行。
fn collect(triggers: &SummonTriggers, timing: SummonTiming, event: Event) -> Vec<SummonPending> {
    dispatch(triggers, timing, &event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 「同名合并」事件：`who` = 保留的原召唤物，`target` = 被吸收的新召唤物，
/// `amount` = 合并后的可用次数（没有可用次数 -> 0）。
pub fn merged_event(kept: EntityId, absorbed: EntityId, uses: Option<Nat>) -> Event {
    Event::new(
        Some(kept),
        Some(absorbed),
        uses.unwrap_or(Nat::ZERO),
        None,
        Some(absorbed),
    )
}

/// 「触发一次」事件：`who` = 召唤物，`amount` = 剩余可用次数（没有可用次数 -> 0）。
pub fn triggered_event(summon: EntityId, remaining: Option<Nat>) -> Event {
    Event::new(
        Some(summon),
        None,
        remaining.unwrap_or(Nat::ZERO),
        None,
        None,
    )
}

/// 两个可选数值取最大（None 视为没有，另一个优先）。
fn max_opt(left: Option<Nat>, right: Option<Nat>) -> Option<Nat> {
    match (left, right) {
        (None, other) | (other, None) => other,
        (Some(left), Some(right)) => {
            if left >= right {
                Some(left)
            } else {
                Some(right)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ops::order::EffectSource;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    fn entry(summon: u32, card: u32) -> SummonEntry {
        SummonEntry::new(n(summon), CardId::new(card))
    }

    fn noop(_zone: &mut SummonZone, _event: &Event, _owner: EntityId) {}

    fn registry_for(timing: SummonTiming) -> SummonTriggers {
        let mut triggers = SummonTriggers::new();
        triggers.register(Trigger::new(timing, n(1), EffectSource::Summon, n(0), noop));
        triggers
    }

    /// 同时登记「触发 / 耗尽」两个时机的注册表（可用次数减到 0 时两条链都能观察）。
    fn registry_triggered_and_depleted() -> SummonTriggers {
        let mut triggers = SummonTriggers::new();
        triggers.register(Trigger::new(
            SummonTiming::Triggered,
            n(1),
            EffectSource::Summon,
            n(0),
            noop,
        ));
        triggers.register(Trigger::new(
            SummonTiming::Depleted,
            n(1),
            EffectSource::Summon,
            n(0),
            noop,
        ));
        triggers
    }

    fn registry_both() -> SummonTriggers {
        let mut triggers = SummonTriggers::new();
        triggers.register(Trigger::new(
            SummonTiming::Summoned,
            n(1),
            EffectSource::Summon,
            n(0),
            noop,
        ));
        triggers.register(Trigger::new(
            SummonTiming::Merged,
            n(1),
            EffectSource::Summon,
            n(0),
            noop,
        ));
        triggers
    }

    fn zone_with(count: u32) -> SummonZone {
        let triggers = SummonTriggers::new();
        let mut zone = SummonZone::new();
        for index in 0..count {
            zone = zone.summon_with_triggers(entry(index, index), &triggers).zone;
        }
        zone
    }

    #[test]
    fn empty_zone_basics() {
        let zone = SummonZone::new();
        assert!(zone.is_empty());
        assert_eq!(zone.len(), Nat::ZERO);
        assert_eq!(zone.free_slots(), SUMMON_CAP);
        assert!(zone.entries().is_empty());
        assert!(zone.entry_of(n(1)).is_none());
    }

    #[test]
    fn cap_is_four_and_fifth_is_rejected_without_state_change() {
        let zone = zone_with(4);
        assert!(zone.is_full());
        assert_eq!(zone.len(), n(4));
        let triggers = SummonTriggers::new();
        let fifth = zone.summon_with_triggers(edge(n(99)), &triggers);
        assert_eq!(fifth.outcome, SummonOutcome::RejectedFull);
        assert_eq!(fifth.zone, zone);
        assert!(fifth.pending.is_empty());
    }

    /// 第 5 个（不同卡 id）召唤物。
    fn edge(summon: Nat) -> SummonEntry {
        SummonEntry::new(summon, CardId::new(999))
    }

    #[test]
    fn summon_dispatches_summoned_in_zone_order() {
        let triggers = registry_for(SummonTiming::Summoned);
        let zone = SummonZone::new();
        let first = zone.summon_with_triggers(entry(1, 113041), &triggers);
        assert_eq!(first.outcome, SummonOutcome::Summoned);
        assert_eq!(first.zone.len(), n(1));
        assert_eq!(first.pending.len(), 1);
        assert_eq!(first.pending.first().expect("一条").event.who, Some(n(1)));
        // 顺序 = 召唤先后。
        let second = first
            .zone
            .summon_with_triggers(entry(2, 113042), &triggers);
        assert_eq!(
            second
                .zone
                .entries()
                .iter()
                .map(|entry| entry.summon)
                .collect::<Vec<_>>(),
            vec![n(1), n(2)]
        );
    }

    #[test]
    fn same_card_merges_uses_points_and_keeps_kind_newest() {
        let triggers = registry_both();
        let zone = SummonZone::new();
        let first = zone
            .summon_with_triggers(
                entry(1, 113041)
                    .with_uses(Some(n(3)))
                    .with_points(Some(n(2)))
                    .with_effect_kind(Some(SummonEffectKind::Damage)),
                &triggers,
            )
            .zone;
        let merged = first.summon_with_triggers(
            entry(2, 113041)
                .with_uses(Some(n(5)))
                .with_points(Some(n(1)))
                .with_effect_kind(Some(SummonEffectKind::Status)),
            &triggers,
        );
        assert_eq!(
            merged.outcome,
            SummonOutcome::Merged {
                kept: n(1),
                absorbed: n(2),
            }
        );
        assert_eq!(merged.zone.len(), n(1));
        let kept = merged.zone.entry_of(n(1)).expect("保留了原召唤物");
        // 次数 / 效果量取最大，类型取最新。
        assert_eq!(kept.uses, Some(n(5)));
        assert_eq!(kept.points, Some(n(2)));
        assert_eq!(kept.effect_kind, Some(SummonEffectKind::Status));
        // 派发「合并」：who = 保留的，target = 被吸收的。
        assert_eq!(merged.pending.len(), 1);
        let item = merged.pending.first().expect("一条");
        assert_eq!(item.event.who, Some(n(1)));
        assert_eq!(item.event.target, Some(n(2)));
    }

    #[test]
    fn merge_keeps_position_and_works_when_full() {
        let triggers = SummonTriggers::new();
        let zone = zone_with(4);
        // 第 2 个召唤物的卡 id = 1；满员时再召唤同名（卡 1）仍应合并、不新增占位。
        let merged = zone.summon_with_triggers(entry(50, 1), &triggers);
        assert!(matches!(merged.outcome, SummonOutcome::Merged { .. }));
        assert_eq!(merged.zone.len(), n(4));
        assert_eq!(
            merged
                .zone
                .entries()
                .iter()
                .map(|entry| entry.summon)
                .collect::<Vec<_>>(),
            vec![n(0), n(1), n(2), n(3)]
        );
    }

    #[test]
    fn tick_decrements_uses_and_discards_at_zero() {
        let triggers = registry_triggered_and_depleted();
        let zone = SummonZone::new()
            .summon_with_triggers(entry(1, 113041).with_uses(Some(n(2))), &triggers)
            .zone;
        let first = zone.tick_with_triggers(n(1), &triggers).expect("在区里");
        assert_eq!(first.zone.entry_of(n(1)).expect("还在").uses, Some(n(1)));
        assert_eq!(first.pending.len(), 1);
        assert_eq!(first.pending.first().expect("一条").event.amount, n(1));
        let second = first.zone.tick_with_triggers(n(1), &triggers).expect("在区里");
        assert!(second.zone.is_empty());
        assert_eq!(second.pending.len(), 1);
        // 再触发 -> 不在了
        assert!(second.zone.tick_with_triggers(n(1), &triggers).is_none());
    }

    #[test]
    fn tick_without_uses_keeps_entry_and_dispatches() {
        let triggers = registry_for(SummonTiming::Triggered);
        let zone = SummonZone::new()
            .summon_with_triggers(entry(1, 113041), &triggers)
            .zone;
        let ticked = zone.tick_with_triggers(n(1), &triggers).expect("在区里");
        assert_eq!(ticked.zone.len(), n(1));
        assert_eq!(ticked.zone.entry_of(n(1)).expect("还在").uses, None);
        assert_eq!(ticked.pending.len(), 1);
        assert_eq!(ticked.pending.first().expect("一条").event.amount, Nat::ZERO);
    }

    #[test]
    fn discard_removes_and_dispatches() {
        let triggers = registry_for(SummonTiming::Discarded);
        let zone = SummonZone::new()
            .summon_with_triggers(entry(1, 113041), &triggers)
            .zone;
        let discarded = zone.discard_with_triggers(n(1), &triggers).expect("在区里");
        assert!(discarded.zone.is_empty());
        assert_eq!(discarded.pending.len(), 1);
        assert!(discarded.zone.discard_with_triggers(n(1), &triggers).is_none());
    }

    #[test]
    fn max_opt_takes_the_larger_value() {
        assert_eq!(max_opt(None, Some(n(2))), Some(n(2)));
        assert_eq!(max_opt(Some(n(2)), None), Some(n(2)));
        assert_eq!(max_opt(Some(n(2)), Some(n(5))), Some(n(5)));
        assert_eq!(max_opt(Some(n(5)), Some(n(2))), Some(n(5)));
        assert_eq!(max_opt(None, None), None);
    }

    #[test]
    fn stats_follow_feature_flag() {
        let triggers = SummonTriggers::new();
        let zone = zone_with(4);
        let rejected = zone.summon_with_triggers(edge(n(99)), &triggers);
        if cfg!(feature = "stats") {
            assert_eq!(rejected.zone.stats().rejected_full().get(), n(1));
            assert_eq!(rejected.zone.stats().summoned().get(), n(4));
        } else {
            assert!(rejected.zone.stats().rejected_full().is_zero());
        }
    }
}
