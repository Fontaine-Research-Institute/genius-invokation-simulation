//! **支援区 / 手牌卡状态 / 当前费用 的记录内容类型 + 记录入口**（D221 / D223 / D224 / D225）。
//!
//! 记录唯一入口 [crate::records::Recorder]；内容类型由本文件定义（D225），显示由 [ShowRecord] 决定。
//! 旧入口全部保持原样（行为不变），这里新增原名 + _recorded 的带记录入口；
//! feature records 关闭时 Recorder 零大小、push 空操作，记录内容不构造（D223）。

use crate::records::{Recorder, ShowRecord};
use crate::resources::dice::CostDelta;
use gi_genius_invokation_engine::{EntityId, Nat};

use super::cost::CurrentCost;
use super::hand_state::{HandCardStateResolution, HandCardStateTriggers, HandCardStates};
use super::support::{
    SupportAddResult, SupportEntry, SupportResolution, SupportTriggers, SupportZone,
};

use gi_genius_invokation_engine::prob::Fraction;

/// 支援区记录器类型。
pub type SupportRecorder = Recorder<SupportRecord>;

/// 支援区的记录内容。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SupportRecord {
    /// 支援牌进入支援区（可能同时弃置了旧牌）。
    Entered {
        /// 进入的支援牌实体。
        entity: EntityId,
        /// 满员时被弃置的旧牌（没满 -> None）。
        discarded: Option<EntityId>,
    },
    /// 支援牌离开支援区。
    Discarded {
        /// 离开的支援牌实体。
        entity: EntityId,
    },
}

impl ShowRecord for SupportRecord {
    fn show(&self, out: &mut String) {
        match self {
            SupportRecord::Entered { entity, discarded } => {
                out.push_str("支援牌 ");
                out.push_str(&entity.to_string());
                out.push_str(" 进入支援区");
                if let Some(discarded) = discarded {
                    out.push_str("（满员弃置 ");
                    out.push_str(&discarded.to_string());
                    out.push('）');
                }
            }
            SupportRecord::Discarded { entity } => {
                out.push_str("支援牌 ");
                out.push_str(&entity.to_string());
                out.push_str(" 离开支援区");
            }
        }
    }
}

impl SupportZone {
    /// 加支援牌（带记录；满员时每个选择分支各记一条）。
    pub fn add_recorded(
        &self,
        entry: SupportEntry,
        triggers: &SupportTriggers,
        recorder: &mut SupportRecorder,
        step: Nat,
    ) -> Vec<(SupportAddResult, Fraction)> {
        let branches = self.add_with_triggers(entry, triggers);
        #[cfg(not(feature = "records"))]
        let _ = &recorder;
        for (branch, _) in &branches {
            #[cfg(feature = "records")]
            recorder.push(
                step,
                SupportRecord::Entered {
                    entity: entry.entity,
                    discarded: branch.discarded,
                },
            );
            #[cfg(not(feature = "records"))]
            let _ = (step, entry, branch);
        }
        branches
    }

    /// 弃置指定支援牌（带记录）。
    pub fn discard_recorded(
        &self,
        entity: EntityId,
        triggers: &SupportTriggers,
        recorder: &mut SupportRecorder,
        step: Nat,
    ) -> Option<SupportResolution> {
        let resolution = self.discard_with_triggers(entity, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(step, SupportRecord::Discarded { entity });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entity);
        Some(resolution)
    }
}

/// 手牌卡状态记录器类型。
pub type HandCardStateRecorder = Recorder<HandCardStateRecord>;

/// 手牌卡状态的记录内容。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HandCardStateRecord {
    /// 给手牌卡加状态。
    StateAdded {
        /// 手牌卡实体。
        entity: EntityId,
        /// 层数（布尔类 None）。
        layers: Option<Nat>,
    },
    /// 从手牌卡减状态。
    StateRemoved {
        /// 手牌卡实体。
        entity: EntityId,
    },
    /// 手牌卡离开手牌（它身上的状态整体清掉）。
    CardRemoved {
        /// 手牌卡实体。
        entity: EntityId,
    },
}

impl ShowRecord for HandCardStateRecord {
    fn show(&self, out: &mut String) {
        match self {
            HandCardStateRecord::StateAdded { entity, layers } => {
                out.push_str("手牌卡 ");
                out.push_str(&entity.to_string());
                out.push_str(" 加状态");
                if let Some(layers) = layers {
                    out.push_str("，层数 ");
                    out.push_str(&layers.to_string());
                }
            }
            HandCardStateRecord::StateRemoved { entity } => {
                out.push_str("手牌卡 ");
                out.push_str(&entity.to_string());
                out.push_str(" 减状态");
            }
            HandCardStateRecord::CardRemoved { entity } => {
                out.push_str("手牌卡 ");
                out.push_str(&entity.to_string());
                out.push_str(" 离开手牌，状态清空");
            }
        }
    }
}

impl HandCardStates {
    /// 加状态（带记录）。
    pub fn add_state_recorded(
        &self,
        entity: EntityId,
        state: &'static dyn crate::families::action_card::status::StateIdentity,
        layers: Option<Nat>,
        triggers: &HandCardStateTriggers,
        recorder: &mut HandCardStateRecorder,
        step: Nat,
    ) -> HandCardStateResolution {
        let resolution = self.add_state_with_triggers(entity, state, layers, triggers);
        #[cfg(feature = "records")]
        recorder.push(step, HandCardStateRecord::StateAdded { entity, layers });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entity, state, layers);
        resolution
    }

    /// 减状态（带记录）。
    pub fn remove_state_recorded(
        &self,
        entity: EntityId,
        state: &'static dyn crate::families::action_card::status::StateIdentity,
        triggers: &HandCardStateTriggers,
        recorder: &mut HandCardStateRecorder,
        step: Nat,
    ) -> Option<HandCardStateResolution> {
        let resolution = self.remove_state_with_triggers(entity, state, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(step, HandCardStateRecord::StateRemoved { entity });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entity, state);
        Some(resolution)
    }

    /// 手牌卡离开手牌（带记录）。
    pub fn remove_card_recorded(
        &self,
        entity: EntityId,
        recorder: &mut HandCardStateRecorder,
        step: Nat,
    ) -> Option<HandCardStates> {
        let states = self.remove_card(entity)?;
        #[cfg(feature = "records")]
        recorder.push(step, HandCardStateRecord::CardRemoved { entity });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entity);
        Some(states)
    }
}

/// 当前费用记录器类型。
pub type CostRecorder = Recorder<CostRecord>;

/// 当前费用的记录内容。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CostRecord {
    /// 基础费用合计。
    Base {
        /// 基础费用总骰数。
        total: Nat,
    },
    /// 叠加一次加费 / 减费。
    Delta {
        /// 是加费（true）还是减费（false）。
        increase: bool,
        /// 数量。
        amount: Nat,
        /// 叠加后的当前费用总骰数。
        after: Nat,
    },
    /// 当前费用合计（快照用）。
    Total {
        /// 当前费用总骰数。
        total: Nat,
    },
}

impl ShowRecord for CostRecord {
    fn show(&self, out: &mut String) {
        match self {
            CostRecord::Base { total } => {
                out.push_str("基础元素骰费用合计 ");
                out.push_str(&total.to_string());
            }
            CostRecord::Delta {
                increase,
                amount,
                after,
            } => {
                out.push_str(if *increase { "加费 " } else { "减费 " });
                out.push_str(&amount.to_string());
                out.push_str("，当前合计 ");
                out.push_str(&after.to_string());
            }
            CostRecord::Total { total } => {
                out.push_str("当前元素骰费用合计 ");
                out.push_str(&total.to_string());
            }
        }
    }
}

impl CurrentCost {
    /// 叠加一次加费 / 减费（带记录）。
    pub fn with_delta_recorded(
        &self,
        delta: CostDelta,
        recorder: &mut CostRecorder,
        step: Nat,
    ) -> CurrentCost {
        let next = self.with_delta(delta);
        #[cfg(feature = "records")]
        {
            let (increase, amount) = match delta {
                CostDelta::Increase { amount } => (true, amount),
                CostDelta::Decrease { amount } => (false, amount),
            };
            recorder.push(
                step,
                CostRecord::Delta {
                    increase,
                    amount,
                    after: next.total(),
                },
            );
        }
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, delta);
        next
    }

    /// 记录一次「基础值 + 当前值」快照（两条记录，同一 step）。
    pub fn record_snapshot(&self, recorder: &mut CostRecorder, step: Nat) {
        #[cfg(feature = "records")]
        {
            recorder.push(step, CostRecord::Base { total: self.base_total() });
            recorder.push(step, CostRecord::Total { total: self.total() });
        }
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step);
    }

    /// 基础费用总骰数（用空增减费的 CurrentCost 复用既有合计口径）。
    pub fn base_total(&self) -> Nat {
        CurrentCost::new(self.base().to_vec()).total()
    }
}

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::families::action_card::areas::hand_state::HandCardStateTriggers;
    use crate::families::action_card::areas::support::{
        SupportEntry, SupportTriggers, SupportValue, SupportZone,
    };
    use crate::families::action_card::status::{StateForm, StateIdentity, StateText};
    use crate::generated::Localized;
    use crate::resources::dice::CostComponent;

    static TEST_TEXT: StateText = StateText {
        name: Localized::from_all("测试状态", "Test", "テスト", "테스트"),
        description: Localized::EMPTY,
    };

    #[derive(Debug)]
    struct TestState(&'static StateText);

    impl StateIdentity for TestState {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Boolean
        }
    }

    static TEST_STATE: TestState = TestState(&TEST_TEXT);

    #[test]
    fn support_and_hand_card_states_are_recorded() {
        let triggers = SupportTriggers::new();
        let mut recorder = SupportRecorder::new();
        let zone = SupportZone::new();
        let entry = SupportEntry::new(CardId::new(1), Nat::ZERO, SupportValue::None);
        let branches = zone.add_recorded(entry, &triggers, &mut recorder, Nat::new(1));
        assert_eq!(branches.len(), 1);
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            SupportRecord::Entered {
                entity: Nat::ZERO,
                discarded: None
            }
        );
        let zone = branches.first().expect("有分支").0.zone.clone();
        zone.discard_recorded(Nat::ZERO, &triggers, &mut recorder, Nat::new(2))
            .expect("支援牌在区里");
        assert_eq!(
            recorder.entries().last().expect("有记录").content,
            SupportRecord::Discarded {
                entity: Nat::ZERO
            }
        );

        let triggers = HandCardStateTriggers::new();
        let mut recorder = HandCardStateRecorder::new();
        let states = HandCardStates::new();
        let resolution = states.add_state_recorded(
            Nat::new(7),
            &TEST_STATE,
            None,
            &triggers,
            &mut recorder,
            Nat::new(3),
        );
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            HandCardStateRecord::StateAdded {
                entity: Nat::new(7),
                layers: None
            }
        );
        assert!(resolution.states.contains_state(Nat::new(7), &TEST_STATE));
        // 旧入口仍可用。
        assert!(states.remove_card(Nat::new(9)).is_none());
    }

    #[test]
    fn cost_delta_is_recorded() {
        let cost = CurrentCost::new(vec![CostComponent::AnyColor(Nat::new(2))]);
        let mut recorder = CostRecorder::new();
        let next = cost.with_delta_recorded(
            CostDelta::Increase {
                amount: Nat::new(1),
            },
            &mut recorder,
            Nat::new(1),
        );
        assert_eq!(next.total(), Nat::new(3));
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            CostRecord::Delta {
                increase: true,
                amount: Nat::new(1),
                after: Nat::new(3)
            }
        );
        let mut shown = String::new();
        recorder.entries().first().expect("有记录").content.show(&mut shown);
        assert!(shown.contains("加费"));
        // 快照：基础值 + 当前值。
        recorder.clear();
        next.record_snapshot(&mut recorder, Nat::new(2));
        assert_eq!(recorder.entries().len(), 2);
        assert_eq!(recorder.entries().first().expect("有记录").step, Nat::new(2));
        // 旧入口仍可用。
        let _ = cost.with_delta(CostDelta::Decrease { amount: Nat::new(1) });
    }
}
