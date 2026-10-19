//! **召唤物区的记录内容类型 + 记录入口**（D221 / D223 / D224 / D225）。
//!
//! 记录的唯一入口是 [crate::records::Recorder]；内容类型由本文件定义（D225），显示由
//! [ShowRecord] 决定。旧入口 summon_with_triggers / tick_with_triggers / discard_with_triggers
//! 原样保留（行为不变），这里新增原名 + _recorded 的带记录入口。
//!
//! feature records 关闭时 Recorder 零大小、push 空操作，记录内容不构造（D223）。
//!
//! 依据：D221 / D223 / D224 / D225。

use crate::records::{Recorder, ShowRecord};
use gi_genius_invokation_engine::{EntityId, Nat};

use super::{SummonEntry, SummonResolution, SummonTriggers, SummonZone};

/// 本模块的记录器类型。
pub type SummonZoneRecorder = Recorder<SummonZoneRecord>;

/// 召唤物区的记录内容（纯数据；显示见 ShowRecord）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SummonZoneRecord {
    /// 召唤物生成新占位。
    Summoned {
        /// 召唤物实体。
        summon: EntityId,
    },
    /// 同名召唤物合并（不新增占位）。
    Merged {
        /// 保留的召唤物实体。
        kept: EntityId,
        /// 被吸收的召唤物实体。
        absorbed: EntityId,
        /// 合并后的可用次数。
        uses: Option<Nat>,
    },
    /// 已满且没有同名：无法生成。
    RejectedFull,
    /// 触发一次：可用次数递减 / 不递减。
    Triggered {
        /// 召唤物实体。
        summon: EntityId,
        /// 剩余可用次数（None = 没有可用次数概念）。
        remaining: Option<Nat>,
    },
    /// 可用次数耗尽 -> 弃置。
    Depleted {
        /// 召唤物实体。
        summon: EntityId,
    },
    /// 被指定弃置。
    Discarded {
        /// 召唤物实体。
        summon: EntityId,
    },
}

impl ShowRecord for SummonZoneRecord {
    fn show(&self, out: &mut String) {
        match self {
            SummonZoneRecord::Summoned { summon } => {
                out.push_str("召唤物 ");
                out.push_str(&summon.to_string());
                out.push_str(" 生成");
            }
            SummonZoneRecord::Merged {
                kept,
                absorbed,
                uses,
            } => {
                out.push_str("召唤物同名合并：保留 ");
                out.push_str(&kept.to_string());
                out.push_str("，吸收 ");
                out.push_str(&absorbed.to_string());
                if let Some(uses) = uses {
                    out.push_str("，可用次数 ");
                    out.push_str(&uses.to_string());
                }
            }
            SummonZoneRecord::RejectedFull => out.push_str("召唤物区已满，无法生成"),
            SummonZoneRecord::Triggered { summon, remaining } => {
                out.push_str("召唤物 ");
                out.push_str(&summon.to_string());
                out.push_str(" 触发");
                if let Some(remaining) = remaining {
                    out.push_str("，剩余次数 ");
                    out.push_str(&remaining.to_string());
                }
            }
            SummonZoneRecord::Depleted { summon } => {
                out.push_str("召唤物 ");
                out.push_str(&summon.to_string());
                out.push_str(" 可用次数耗尽，弃置");
            }
            SummonZoneRecord::Discarded { summon } => {
                out.push_str("召唤物 ");
                out.push_str(&summon.to_string());
                out.push_str(" 弃置");
            }
        }
    }
}

impl SummonZone {
    /// 召唤（带记录：生成 / 合并 / 满员拒绝）。
    pub fn summon_recorded(
        &self,
        entry: SummonEntry,
        triggers: &SummonTriggers,
        recorder: &mut SummonZoneRecorder,
        step: Nat,
    ) -> SummonResolution {
        let resolution = self.summon_with_triggers(entry, triggers);
        #[cfg(feature = "records")]
        match resolution.outcome {
            super::SummonOutcome::Summoned => recorder.push(
                step,
                SummonZoneRecord::Summoned {
                    summon: entry.summon,
                },
            ),
            super::SummonOutcome::Merged { kept, absorbed } => recorder.push(
                step,
                SummonZoneRecord::Merged {
                    kept,
                    absorbed,
                    uses: resolution.zone.entry_of(kept).and_then(|entry| entry.uses),
                },
            ),
            super::SummonOutcome::RejectedFull => recorder.push(step, SummonZoneRecord::RejectedFull),
            _ => {}
        }
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entry);
        resolution
    }

    /// 触发一次（带记录）。
    pub fn tick_recorded(
        &self,
        summon: EntityId,
        triggers: &SummonTriggers,
        recorder: &mut SummonZoneRecorder,
        step: Nat,
    ) -> Option<SummonResolution> {
        let resolution = self.tick_with_triggers(summon, triggers)?;
        #[cfg(feature = "records")]
        match resolution.outcome {
            super::SummonOutcome::Triggered { remaining } => recorder.push(
                step,
                SummonZoneRecord::Triggered { summon, remaining },
            ),
            super::SummonOutcome::Depleted => {
                recorder.push(step, SummonZoneRecord::Depleted { summon })
            }
            _ => {}
        }
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, summon);
        Some(resolution)
    }

    /// 弃置指定召唤物（带记录）。
    pub fn discard_recorded(
        &self,
        summon: EntityId,
        triggers: &SummonTriggers,
        recorder: &mut SummonZoneRecorder,
        step: Nat,
    ) -> Option<SummonResolution> {
        let resolution = self.discard_with_triggers(summon, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(step, SummonZoneRecord::Discarded { summon });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, summon);
        Some(resolution)
    }
}

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::families::summon::zones::{SummonEntry, SummonOutcome, SummonTriggers, SummonZone};
    use gi_genius_invokation_engine::Nat;

    #[test]
    fn summon_actions_are_recorded_with_steps() {
        let zone = SummonZone::new();
        let triggers = SummonTriggers::new();
        let mut recorder = SummonZoneRecorder::new();
        let entry = SummonEntry::new(Nat::ZERO, CardId::new(5)).with_uses(Some(Nat::new(1)));
        let resolution = zone.summon_recorded(entry, &triggers, &mut recorder, Nat::new(1));
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            SummonZoneRecord::Summoned {
                summon: Nat::ZERO
            }
        );
        let mut shown = String::new();
        recorder.entries().first().expect("有记录").content.show(&mut shown);
        assert!(shown.contains("生成"));

        // 触发一次 -> 可用次数耗尽。
        let tick = resolution
            .zone
            .tick_recorded(Nat::ZERO, &triggers, &mut recorder, Nat::new(2))
            .expect("召唤物在区里");
        assert_eq!(
            recorder.entries().last().expect("有记录").content,
            SummonZoneRecord::Depleted {
                summon: Nat::ZERO
            }
        );
        assert_eq!(recorder.entries().last().expect("有记录").step, Nat::new(2));
        assert!(tick.zone.is_empty());
        // 旧入口仍可用。
        assert_eq!(
            zone.summon_with_triggers(entry, &triggers).outcome,
            SummonOutcome::Summoned
        );
    }
}
