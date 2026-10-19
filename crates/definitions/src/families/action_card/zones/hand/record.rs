//! **手牌区的记录内容类型 + 记录入口**（D221 / D223 / D224 / D225）。
//!
//! 记录唯一入口 [crate::records::Recorder]；内容类型由本文件定义（D225），显示由 [ShowRecord]
//! 决定。旧入口全部保持原样（行为不变），这里新增原名 + _recorded 的带记录入口；
//! feature records 关闭时 Recorder 零大小、push 空操作，记录内容不构造（D223）。

use crate::base::CardId;
use crate::families::action_card::zones::CardInstance;
use crate::records::{Recorder, ShowRecord};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{EntityId, Nat};

use super::{HandState, PileState};

/// 手牌记录器类型。
pub type HandRecorder = Recorder<HandRecord>;

/// 手牌的记录内容。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HandRecord {
    /// 一张牌加入手牌。
    Added {
        /// 手牌里的实体。
        entity: EntityId,
        /// 卡牌定义 id。
        card: CardId,
    },
    /// 一张牌离开手牌。
    Discarded {
        /// 离开的实体。
        entity: EntityId,
    },
    /// 起手分支抓到的张数（每支一条，同一 step）。
    OpeningDrawn {
        /// 这一支抓到的张数。
        count: Nat,
    },
}

impl ShowRecord for HandRecord {
    fn show(&self, out: &mut String) {
        match self {
            HandRecord::Added { entity, card } => {
                out.push_str("手牌加入 ");
                out.push_str(&entity.to_string());
                out.push_str("（卡 ");
                out.push_str(&card.0.to_string());
                out.push('）');
            }
            HandRecord::Discarded { entity } => {
                out.push_str("手牌离开 ");
                out.push_str(&entity.to_string());
            }
            HandRecord::OpeningDrawn { count } => {
                out.push_str("起手抓 ");
                out.push_str(&count.to_string());
                out.push_str(" 张");
            }
        }
    }
}

impl HandState {
    /// 加入手牌（带记录；被拒绝时不记录）。
    pub fn add_recorded(
        &self,
        instance: CardInstance,
        recorder: &mut HandRecorder,
        step: Nat,
    ) -> (HandState, bool) {
        let (next, added) = self.add(instance);
        #[cfg(feature = "records")]
        if added {
            recorder.push(
                step,
                HandRecord::Added {
                    entity: instance.entity,
                    card: instance.card,
                },
            );
        }
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, instance, added);
        (next, added)
    }

    /// 舍弃手牌（带记录；不在手牌时不记录）。
    pub fn discard_recorded(
        &self,
        entity: EntityId,
        recorder: &mut HandRecorder,
        step: Nat,
    ) -> Option<HandState> {
        let next = self.discard(entity)?;
        #[cfg(feature = "records")]
        recorder.push(step, HandRecord::Discarded { entity });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entity);
        Some(next)
    }

    /// 起手（带记录：每个分支一条抓到张数）。
    pub fn draw_opening_recorded(
        &self,
        pile: &PileState,
        arcane: &dyn Fn(CardId) -> bool,
        recorder: &mut HandRecorder,
        step: Nat,
    ) -> Vec<(HandState, PileState, Fraction)> {
        let initial = self.len();
        let branches = self.draw_opening(pile, arcane);
        #[cfg(not(feature = "records"))]
        let _ = &recorder;
        for (hand, _, _) in &branches {
            #[cfg(feature = "records")]
            recorder.push(
                step,
                HandRecord::OpeningDrawn {
                    count: hand.len().saturating_sub(initial),
                },
            );
            #[cfg(not(feature = "records"))]
            let _ = (step, hand, initial);
        }
        branches
    }
}

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::base::CardId;
    use gi_genius_invokation_engine::Nat;

    #[test]
    fn add_discard_and_opening_are_recorded() {
        let hand = HandState::new();
        let mut recorder = HandRecorder::new();
        let (hand, added) = hand.add_recorded(
            CardInstance::new(Nat::ZERO, CardId::new(5)),
            &mut recorder,
            Nat::new(1),
        );
        assert!(added);
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            HandRecord::Added {
                entity: Nat::ZERO,
                card: CardId::new(5)
            }
        );
        let hand = hand
            .discard_recorded(Nat::ZERO, &mut recorder, Nat::new(2))
            .expect("0 号在手牌");
        assert_eq!(
            recorder.entries().last().expect("有记录").content,
            HandRecord::Discarded {
                entity: Nat::ZERO
            }
        );

        recorder.clear();
        let pile = PileState::from_fixed(vec![
            (Nat::ZERO, CardId::new(1)),
            (Nat::new(1), CardId::new(2)),
        ]);
        let branches = hand.draw_opening_recorded(&pile, &|_| false, &mut recorder, Nat::new(3));
        assert!(!branches.is_empty());
        let first = recorder.entries().first().expect("有记录");
        assert_eq!(first.step, Nat::new(3));
        assert!(matches!(first.content, HandRecord::OpeningDrawn { .. }));
        // 旧入口仍可用。
        assert!(HandState::new().discard(Nat::new(9)).is_none());
    }
}
