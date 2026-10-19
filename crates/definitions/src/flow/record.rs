//! **flow 的记录内容类型**（D225：内容类型单独一个文件；显示由内容类型自己实现）。
//!
//! 记录器 [crate::records::Recorder] 只提供「属于哪一步 + 任意内容」；flow 用本类型实例化
//! 自己的记录器。写入在业务接线处（flow/recorded.rs：谁干活谁记录，D225），本文件只有内容
//! 类型与显示实现，不含任何结算逻辑。
//!
//! ## 记录哪些事件（D227）
//!
//! - 一次「使用技能」的汇总（技能 / 行动方 / 效果体路径 / 分支数）；
//! - 一次「打出行动牌」的汇总（牌实体 / 类型 / 效果体路径 / 分支数）；
//! - 一个**一步**（D204）的执行（步号 / 操作数 / 分支数）；
//! - **步末 flush**（D204：步内禁止插入，待执行在步末统一执行）；
//! - **步边界插入**（D196：「使用技能后」等子结算只插在步与步之间）。
//!
//! 这些事件都带「步号」语义（记录器的 Record.step）：flow 把同一次结算的事件都记在调用方
//! 给的同一个步号下，再用 Recorder::step_entries 按步读（D223 / D224）。

use crate::base::SkillId;
use crate::records::ShowRecord;
use gi_genius_invokation_engine::{EntityId, Nat};

/// **flow 的记录内容**（记录器不预设种类；这里每种事件一个变体）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowRecord {
    /// 一次「使用技能」的汇总（在结算完成后记，含分支数）。
    UseSkill {
        /// 技能 id。
        skill: SkillId,
        /// 行动方描述（我方 / 敌方使用）。
        action: &'static str,
        /// 效果体走的是哪条路径（旧的效果体计划 / 关键字一步）。
        effect: &'static str,
        /// 结算分支数（反应效果 / 随机效果可能多支）。
        branches: Nat,
    },
    /// 一次「打出行动牌」的汇总。
    PlayCard {
        /// 打出的牌实体号。
        entity: EntityId,
        /// 类型（支援 / 装备 / 事件；中文）。
        card_type: &'static str,
        /// 效果体走的是哪条路径（无效果体 / 关键字一步）。
        effect: &'static str,
        /// 结算分支数。
        branches: Nat,
    },
    /// 执行了一个**一步**（D204：步内禁止插入）。
    Step {
        /// 步号（同一次结算内从 0 起）。
        index: Nat,
        /// 步内操作条数。
        ops: Nat,
        /// 这一步展开出的分支数。
        branches: Nat,
    },
    /// **步末 flush**：把这一步派发出来的待执行统一执行（D204：插入只能发生在步边界）。
    Flush {
        /// 在第几步之后 flush。
        after_step: Nat,
        /// 执行了多少条待执行。
        pending: Nat,
    },
    /// **步边界插入**（D196：不是栈，子结算做完回到插入点）。
    Insert {
        /// 在第几步之后插入。
        after_step: Nat,
        /// 插入的待执行条数。
        pending: Nat,
        /// 插入位置描述（例如「使用技能后」）。
        at: &'static str,
    },
}

impl ShowRecord for FlowRecord {
    fn show(&self, out: &mut String) {
        match self {
            FlowRecord::UseSkill {
                skill,
                action,
                effect,
                branches,
            } => {
                out.push_str(&format!(
                    "{}：技能 {}（{}），结算分支 {}",
                    action,
                    skill.0.get(),
                    effect,
                    branches.get()
                ));
            }
            FlowRecord::PlayCard {
                entity,
                card_type,
                effect,
                branches,
            } => {
                out.push_str(&format!(
                    "打出牌：实体 {}，{}（{}），结算分支 {}",
                    entity.get(),
                    card_type,
                    effect,
                    branches.get()
                ));
            }
            FlowRecord::Step {
                index,
                ops,
                branches,
            } => {
                out.push_str(&format!(
                    "一步 #{}：操作 {} 条，分支 {}",
                    index.get(),
                    ops.get(),
                    branches.get()
                ));
            }
            FlowRecord::Flush {
                after_step,
                pending,
            } => {
                out.push_str(&format!(
                    "步 {} 之后 flush：执行 {} 条待执行",
                    after_step.get(),
                    pending.get()
                ));
            }
            FlowRecord::Insert {
                after_step,
                pending,
                at,
            } => {
                out.push_str(&format!(
                    "步 {} 之后插入（{}）：{} 条",
                    after_step.get(),
                    at,
                    pending.get()
                ));
            }
        }
    }
}

/// **兼容别名**（D225 之前叫 SkillUseRecord；现在 flow 的汇总事件统一走 [FlowRecord]）。
pub type SkillUseRecord = FlowRecord;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::Recorder;

    #[test]
    fn every_flow_record_has_a_display_line() {
        let mut recorder: Recorder<FlowRecord> = Recorder::new();
        recorder.push(
            Nat::ZERO,
            FlowRecord::UseSkill {
                skill: SkillId::new(12162),
                action: "使用技能（我方）",
                effect: "关键字一步",
                branches: Nat::new(1),
            },
        );
        recorder.push(
            Nat::ZERO,
            FlowRecord::PlayCard {
                entity: Nat::new(211),
                card_type: "事件",
                effect: "无效果体",
                branches: Nat::new(1),
            },
        );
        recorder.push(
            Nat::ZERO,
            FlowRecord::Step {
                index: Nat::ZERO,
                ops: Nat::new(2),
                branches: Nat::new(1),
            },
        );
        recorder.push(
            Nat::ZERO,
            FlowRecord::Flush {
                after_step: Nat::ZERO,
                pending: Nat::new(3),
            },
        );
        recorder.push(
            Nat::new(1),
            FlowRecord::Insert {
                after_step: Nat::new(1),
                pending: Nat::new(2),
                at: "使用技能后",
            },
        );
        let text = recorder.render();
        assert!(text.contains("使用技能（我方）：技能 12162"));
        assert!(text.contains("打出牌：实体 211"));
        assert!(text.contains("一步 #0"));
        assert!(text.contains("flush"));
        assert!(text.contains("插入（使用技能后）"));
        assert_eq!(text.lines().count(), 5);
    }

    #[test]
    fn records_are_grouped_by_step() {
        let mut recorder: Recorder<FlowRecord> = Recorder::new();
        recorder.push(
            Nat::ZERO,
            FlowRecord::Step {
                index: Nat::ZERO,
                ops: Nat::new(1),
                branches: Nat::new(1),
            },
        );
        recorder.push(
            Nat::ZERO,
            FlowRecord::Flush {
                after_step: Nat::ZERO,
                pending: Nat::new(1),
            },
        );
        recorder.push(
            Nat::new(2),
            FlowRecord::Insert {
                after_step: Nat::new(2),
                pending: Nat::new(0),
                at: "打出后",
            },
        );
        assert_eq!(recorder.step_entries(Nat::ZERO).count(), 2);
        assert_eq!(recorder.step_entries(Nat::new(1)).count(), 0);
        assert_eq!(recorder.step_entries(Nat::new(2)).count(), 1);
    }
}
