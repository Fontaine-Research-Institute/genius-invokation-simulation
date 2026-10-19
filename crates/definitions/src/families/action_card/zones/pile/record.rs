//! **牌库区的记录内容类型 + 记录入口**（D221 / D223 / D224 / D225）。
//!
//! 记录唯一入口 [crate::records::Recorder]；内容类型由本文件定义（D225），显示由 [ShowRecord]
//! 决定。旧入口全部保持原样（行为不变），这里新增原名 + _recorded 的带记录入口；
//! feature records 关闭时 Recorder 零大小、push 空操作，记录内容不构造（D223）。

use crate::base::CardId;
use crate::records::{Recorder, ShowRecord};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{EntityId, Nat};

use crate::families::action_card::status::Mark;
use crate::families::action_card::zones::hand::HandState;

use super::{DrawBranch, PileState};

/// 牌库记录器类型。
pub type PileRecorder = Recorder<PileRecord>;

/// 牌库的记录内容。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PileRecord {
    /// 抽到一张牌。
    Drawn {
        /// 抽到的实体。
        entity: EntityId,
        /// 卡牌定义 id。
        card: CardId,
    },
    /// 一个抽牌分支抽到的张数（分支折叠前每支一条，同一 step）。
    BranchDrawn {
        /// 这一支抽到几张。
        count: Nat,
    },
    /// 舍弃一张牌。
    Discarded {
        /// 被舍弃的实体。
        entity: EntityId,
        /// 卡牌定义 id。
        card: CardId,
    },
    /// 均匀置入若干张（已在牌库里的牌换位置）。
    Inserted {
        /// 置入张数。
        count: Nat,
    },
    /// 把若干新牌放进牌组底。
    PutBottomMany {
        /// 放进张数。
        count: Nat,
    },
    /// 置底一张（牌库内部移动）。
    PutBottom {
        /// 被移动的实体。
        entity: EntityId,
    },
    /// 记录一批待展开随机状态的位置。
    PendingMarkAdded {
        /// 位置个数。
        count: Nat,
    },
}

impl ShowRecord for PileRecord {
    fn show(&self, out: &mut String) {
        match self {
            PileRecord::Drawn { entity, card } => {
                out.push_str("抽到牌 ");
                out.push_str(&entity.to_string());
                out.push_str("（卡 ");
                out.push_str(&card.0.to_string());
                out.push('）');
            }
            PileRecord::BranchDrawn { count } => {
                out.push_str("抽牌分支：抽到 ");
                out.push_str(&count.to_string());
                out.push_str(" 张");
            }
            PileRecord::Discarded { entity, card } => {
                out.push_str("舍弃牌 ");
                out.push_str(&entity.to_string());
                out.push_str("（卡 ");
                out.push_str(&card.0.to_string());
                out.push('）');
            }
            PileRecord::Inserted { count } => {
                out.push_str("均匀置入 ");
                out.push_str(&count.to_string());
                out.push_str(" 张");
            }
            PileRecord::PutBottomMany { count } => {
                out.push_str("放入牌组底 ");
                out.push_str(&count.to_string());
                out.push_str(" 张");
            }
            PileRecord::PutBottom { entity } => {
                out.push_str("置底牌 ");
                out.push_str(&entity.to_string());
            }
            PileRecord::PendingMarkAdded { count } => {
                out.push_str("记录待展开状态位置 ");
                out.push_str(&count.to_string());
                out.push_str(" 个");
            }
        }
    }
}

impl PileState {
    /// 舍弃一张牌（带记录）。
    pub fn discard_recorded(
        &self,
        entity: EntityId,
        recorder: &mut PileRecorder,
        step: Nat,
    ) -> Option<PileState> {
        let card = self.card_of(entity)?;
        let next = self.discard(entity)?;
        #[cfg(feature = "records")]
        recorder.push(step, PileRecord::Discarded { entity, card });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entity, card);
        Some(next)
    }

    /// 置底一张（带记录）。
    pub fn put_bottom_recorded(
        &self,
        entity: EntityId,
        recorder: &mut PileRecorder,
        step: Nat,
    ) -> Option<PileState> {
        let next = self.put_bottom(entity)?;
        #[cfg(feature = "records")]
        recorder.push(step, PileRecord::PutBottom { entity });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entity);
        Some(next)
    }

    /// 均匀置入已经在牌库里的牌（带记录）。
    pub fn insert_evenly_recorded(
        &self,
        entities: &[EntityId],
        recorder: &mut PileRecorder,
        step: Nat,
    ) -> Option<PileState> {
        let mut found = Nat::ZERO;
        let mut seen: Vec<EntityId> = Vec::new();
        for entity in entities {
            if seen.contains(entity) {
                continue;
            }
            seen.push(*entity);
            if self.contains(*entity) {
                found = found.saturating_add(Nat::new(1));
            }
        }
        let next = self.insert_evenly(entities)?;
        #[cfg(feature = "records")]
        recorder.push(step, PileRecord::Inserted { count: found });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entities, found);
        Some(next)
    }

    /// 把新牌放入牌组底（带记录）。
    pub fn insert_evenly_with_recorded(
        &self,
        instances: &[(EntityId, CardId)],
        recorder: &mut PileRecorder,
        step: Nat,
    ) -> Option<PileState> {
        let next = self.insert_evenly_with(instances)?;
        let count = next.len().saturating_sub(self.len());
        #[cfg(not(feature = "records"))]
        let _ = &recorder;
        #[cfg(feature = "records")]
        recorder.push(step, PileRecord::Inserted { count });
        #[cfg(not(feature = "records"))]
        let _ = (step, instances, count);
        Some(next)
    }

    /// 记录一批待展开随机状态的位置（带记录）。
    pub fn add_pending_mark_recorded(
        &self,
        mark: Mark,
        positions: &[Nat],
        recorder: &mut PileRecorder,
        step: Nat,
    ) -> Option<PileState> {
        let count = Nat::new(positions.len() as _);
        let next = self.add_pending_mark(mark, positions)?;
        #[cfg(feature = "records")]
        recorder.push(step, PileRecord::PendingMarkAdded { count });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, mark, positions, count);
        Some(next)
    }

    /// 抽 n 张（带记录：每个分支一条抽到张数，同一 step）。
    pub fn draw_many_recorded(
        &self,
        hand: &HandState,
        n: Nat,
        recorder: &mut PileRecorder,
        step: Nat,
    ) -> Vec<(DrawBranch, Fraction)> {
        let branches = self.draw_many(hand, n);
        #[cfg(not(feature = "records"))]
        let _ = &recorder;
        for (branch, _) in &branches {
            #[cfg(feature = "records")]
            recorder.push(
                step,
                PileRecord::BranchDrawn {
                    count: Nat::new(branch.drawn.len() as _),
                },
            );
            #[cfg(not(feature = "records"))]
            let _ = (step, branch);
        }
        branches
    }
}

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::families::action_card::zones::hand::HandState;
    use gi_genius_invokation_engine::Nat;

    fn deck(count: Nat) -> PileState {
        let mut pairs: Vec<(EntityId, CardId)> = Vec::new();
        let mut index = Nat::ZERO;
        while index < count {
            pairs.push((index, CardId::new(index.get())));
            index = index.saturating_add(Nat::new(1));
        }
        PileState::from_fixed(pairs)
    }

    #[test]
    fn discard_and_draw_are_recorded_with_steps() {
        let pile = deck(Nat::new(3));
        let mut recorder = PileRecorder::new();
        let next = pile
            .discard_recorded(Nat::ZERO, &mut recorder, Nat::new(1))
            .expect("0 号在牌库");
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            PileRecord::Discarded {
                entity: Nat::ZERO,
                card: CardId::new(0)
            }
        );
        let mut shown = String::new();
        recorder.entries().first().expect("有记录").content.show(&mut shown);
        assert!(shown.contains("舍弃"));

        recorder.clear();
        let branches = next.draw_many_recorded(&HandState::new(), Nat::new(2), &mut recorder, Nat::new(2));
        assert!(!branches.is_empty());
        let first = recorder.entries().first().expect("有记录");
        assert_eq!(first.step, Nat::new(2));
        assert!(matches!(first.content, PileRecord::BranchDrawn { .. }));
        // 旧入口仍可用。
        assert!(pile.put_bottom(Nat::ZERO).is_some());
    }
}
