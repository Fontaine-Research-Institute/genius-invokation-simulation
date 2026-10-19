//! **准备技能**（K53）的状态机 —— 普通函数接口（本轮**不写宏**；D165 / D166 的宏化留到以后）。
//!
//! ## 权威依据（按优先级）
//!
//! 1. 权威关键字 **K53 准备技能**（`generated/data/keywords.json`，中/英/日/韩四语）原文：
//!    > 有些技能无法被直接使用，而是需要一定次数的行动轮来进行准备。
//!    > 轮到一位牌手行动时，如果该牌手的「出战角色」正在准备技能，则跳过该牌手的行动；
//!    > 如果技能已经完成准备，角色将会在此时直接使用此技能。（此类需要准备的技能，无法触发
//!    > 「使用技能后」「角色引发元素反应后」等效果。）
//!    > 只有「出战角色」才能准备技能；如果正在准备技能的「出战角色」被切换到后台，技能的准备就会被打断。
//! 2. 卡面：`（需准备1个行动轮）` / `（需准备2个行动轮）`；「然后准备技能：X」/
//!    「分别准备技能：X 和 Y」。
//!
//! ## 状态机（本文件的四个操作 = 交付要求的「开始 / 完成 / 打断 / 清除」）
//!
//! - **开始准备** [`PreparedSkills::begin`]：追加一个「准备中」槽（`rounds_left` = 还需几个行动轮）；
//! - **过一个行动轮** [`PreparedSkills::tick_action_round`]：所有「准备中」槽的 `rounds_left` -1
//!   （到 0 = **已完成准备**，随时可直接使用）；
//! - **完成准备** [`PreparedSkills::complete`]：技能已完成准备并**被直接使用** -> 移除该槽；
//!   还没就绪 -> `None`（准备没完成就不能算完成）；
//! - **被打断** [`PreparedSkills::interrupt`] / [`PreparedSkills::interrupt_all`]：
//!   切换角色（出战 -> 后台）时按 K53 打断；
//! - **清除** [`PreparedSkills::clear`]：角色被击倒 / 离场等把槽全部清掉。
//!
//! 「轮到行动时做什么」由 [`PreparedSkills::pending_action`] 回答（[`PreparedAction`]）：
//! 有已完成准备的 -> 直接使用它；否则有准备中的 -> 跳过本次行动，否则正常行动。
//!
//! ## 边界（本层不做）
//!
//! - **不写宏**（本轮口径）：这里是普通函数 + 值类型；将来宏化时替换实现、保留接口；
//! - **不判定「谁是出战角色」**：K53 的「只有出战角色才能准备技能」由调用方保证（本类型只是
//!   **一个角色**身上的准备槽，放在哪里由角色区 / 出战角色区自己决定）；
//! - **不执行技能效果**：完成准备只负责移除槽；技能效果由调用方执行（且**不派发**「使用技能后」，
//!   见 K53；派发见 `skill_flow::timing`）；
//! - **不结算不确定量**（D190）：准备 / 打断都是确定事实，没有分支与概率。

use crate::base::SkillId;
use gi_genius_invokation_engine::{nsub, Nat};

/// 一个**准备槽**：某个技能正在准备，或已完成准备。
///
/// 字段私有：外部只能通过只读方法看（D183 的只读指针口径；本类型是普通值，不是统计容器）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedSkill {
    skill: SkillId,
    rounds_left: Nat,
}

impl PreparedSkill {
    /// 这个槽是哪个技能。
    pub fn skill(&self) -> SkillId {
        self.skill
    }

    /// 还需几个行动轮才能完成准备（`ZERO` = 已完成准备）。
    pub fn rounds_left(&self) -> Nat {
        self.rounds_left
    }

    /// 是否已完成准备（可以直接使用）。
    pub fn is_ready(&self) -> bool {
        self.rounds_left == Nat::ZERO
    }
}

/// 「轮到该角色行动时」应该做什么（K53 原文口径）。
///
/// - `None`：没有准备中的技能，正常行动；
/// - [`PreparedAction::Skip`]：正在准备 -> **跳过该牌手的行动**（调用方在跳过时
///   [`PreparedSkills::tick_action_round`] 推进一个行动轮）；
/// - [`PreparedAction::Use`]：已完成准备 -> 角色**直接使用**这个技能（不触发「使用技能后」等效果）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedAction {
    /// 正在准备这个技能：跳过本次行动。
    Skip {
        /// 正在准备的技能。
        skill: SkillId,
    },
    /// 这个技能已完成准备：此时直接使用它。
    Use {
        /// 已完成准备的技能。
        skill: SkillId,
    },
}

/// 一个角色身上的**准备技能槽**（D182：普通值类型，由区自己持有；没有全局状态）。
///
/// 可以有**多个**槽（卡面「分别准备技能：长延涤流和终幕涤流」），顺序 = 开始准备的先后。
/// 同一个技能允许重复准备（按顺序逐个别完成 / 打断）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparedSkills {
    slots: Vec<PreparedSkill>,
}

impl PreparedSkills {
    /// 空容器（没有准备中的技能）。
    pub const fn new() -> PreparedSkills {
        PreparedSkills { slots: Vec::new() }
    }

    /// 准备槽条数。
    pub fn len(&self) -> Nat {
        Nat::new(self.slots.len() as _)
    }

    /// 是否没有准备槽。
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// 按下标取槽（只读；越界 -> `None`）。
    pub fn get(&self, index: Nat) -> Option<&PreparedSkill> {
        self.slots.get(index.get() as usize) // bare-int-ok: 容器下标，非业务数值
    }

    /// 按开始准备的先后只读遍历。
    pub fn iter(&self) -> impl Iterator<Item = &PreparedSkill> {
        self.slots.iter()
    }

    /// **开始准备**：追加一个准备槽，`rounds` = 还需几个行动轮（`ZERO` = 立刻完成准备）。
    ///
    /// 纯函数：返回新容器，不改入参。只应由「出战角色」调用（K53；本层不校验，见模块文档）。
    pub fn begin(&self, skill: SkillId, rounds: Nat) -> PreparedSkills {
        let mut next = self.clone();
        next.slots.push(PreparedSkill {
            skill,
            rounds_left: rounds,
        });
        next
    }

    /// **过一个行动轮**：所有「准备中」槽的剩余轮数 -1（`ZERO` 保持不变）。
    ///
    /// 在「轮到该牌手行动、因其正在准备技能而跳过」时调用一次（K53）。
    pub fn tick_action_round(&self) -> PreparedSkills {
        let mut next = self.clone();
        for slot in &mut next.slots {
            slot.rounds_left = nsub!(slot.rounds_left, Nat::new(1));
        }
        next
    }

    /// 已完成准备的技能（第一个；没有 -> `None`）。
    pub fn ready_skill(&self) -> Option<SkillId> {
        self.slots
            .iter()
            .find(|slot| slot.is_ready())
            .map(|slot| slot.skill)
    }

    /// 这个技能是否已完成准备（可以直接使用）。
    pub fn is_ready(&self, skill: SkillId) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.skill == skill && slot.is_ready())
    }

    /// 「轮到该角色行动时」应该做什么（K53）。
    ///
    /// 判定顺序：**已完成准备的先直接用**（K53「如果技能已经完成准备，角色将会在此时直接使用此技能」）；
    /// 没有就绪的、但有准备中的 -> 跳过行动；都没有 -> `None`（正常行动）。
    pub fn pending_action(&self) -> Option<PreparedAction> {
        if let Some(skill) = self.ready_skill() {
            return Some(PreparedAction::Use { skill });
        }
        self.slots
            .iter()
            .find(|slot| !slot.is_ready())
            .map(|slot| PreparedAction::Skip { skill: slot.skill })
    }

    /// **完成准备**：把「已完成准备」的槽移除（= 该技能被直接使用后消耗掉）。
    ///
    /// 还没有完成准备（`rounds_left > 0`）-> `None`（槽不动）；技能不存在 -> `None`。
    pub fn complete(&self, skill: SkillId) -> Option<PreparedSkills> {
        let position = self
            .slots
            .iter()
            .position(|slot| slot.skill == skill && slot.is_ready())?;
        let mut next = self.clone();
        next.slots.remove(position);
        Some(next)
    }

    /// **被打断**：移除该技能的准备槽（不分就绪 / 准备中；K53「被切换到后台，准备就会被打断」）。
    ///
    /// 没有这个技能 -> `None`。
    pub fn interrupt(&self, skill: SkillId) -> Option<PreparedSkills> {
        let position = self.slots.iter().position(|slot| slot.skill == skill)?;
        let mut next = self.clone();
        next.slots.remove(position);
        Some(next)
    }

    /// **全部被打断**（切换角色：出战角色被切到后台 -> 所有准备槽打断，K53）。
    pub fn interrupt_all(&self) -> PreparedSkills {
        PreparedSkills::new()
    }

    /// **清除**（角色被击倒 / 离场：准备槽全部清掉）。
    pub fn clear(&self) -> PreparedSkills {
        PreparedSkills::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLASH: SkillId = SkillId::new(25025);
    const FINALE: SkillId = SkillId::new(25026);

    #[test]
    fn new_is_empty_and_default() {
        let prepared = PreparedSkills::new();
        assert!(prepared.is_empty());
        assert_eq!(prepared.len(), Nat::ZERO);
        assert_eq!(prepared.pending_action(), None);
        assert_eq!(prepared.ready_skill(), None);
        assert_eq!(prepared.get(Nat::ZERO), None);
        assert_eq!(prepared, PreparedSkills::default());
    }

    #[test]
    fn begin_marks_preparing_and_skips_action() {
        let prepared = PreparedSkills::new().begin(SLASH, Nat::new(1));
        assert_eq!(prepared.len(), Nat::new(1));
        assert!(!prepared.is_ready(SLASH));
        assert_eq!(
            prepared.pending_action(),
            Some(PreparedAction::Skip { skill: SLASH })
        );
        let slot = prepared.get(Nat::ZERO).expect("一个槽");
        assert_eq!(slot.skill(), SLASH);
        assert_eq!(slot.rounds_left(), Nat::new(1));
        assert!(!slot.is_ready());
    }

    #[test]
    fn begin_with_no_rounds_is_ready_immediately() {
        let prepared = PreparedSkills::new().begin(SLASH, Nat::ZERO);
        assert!(prepared.is_ready(SLASH));
        assert_eq!(prepared.ready_skill(), Some(SLASH));
        assert_eq!(
            prepared.pending_action(),
            Some(PreparedAction::Use { skill: SLASH })
        );
    }

    #[test]
    fn tick_action_round_counts_down_to_ready() {
        // 「（需准备2个行动轮）」：两次跳过后就绪。
        let preparing = PreparedSkills::new().begin(SLASH, Nat::new(2));
        let after_one = preparing.tick_action_round();
        assert_eq!(
            after_one.pending_action(),
            Some(PreparedAction::Skip { skill: SLASH })
        );
        assert_eq!(
            after_one.get(Nat::ZERO).map(PreparedSkill::rounds_left),
            Some(Nat::new(1))
        );
        let after_two = after_one.tick_action_round();
        assert!(after_two.is_ready(SLASH));
        assert_eq!(
            after_two.pending_action(),
            Some(PreparedAction::Use { skill: SLASH })
        );
        // 已就绪后再过行动轮：保持就绪（不会回到准备中）。
        let after_three = after_two.tick_action_round();
        assert!(after_three.is_ready(SLASH));
        // 纯函数：原容器仍是「准备中」。
        assert!(!preparing.is_ready(SLASH));
    }

    #[test]
    fn ready_skill_takes_priority_over_preparing() {
        // 两个槽：先开始的要 1 轮，后开始的立刻完成 -> 先直接用已完成的那个。
        let prepared = PreparedSkills::new()
            .begin(SLASH, Nat::new(1))
            .begin(FINALE, Nat::ZERO);
        assert_eq!(
            prepared.pending_action(),
            Some(PreparedAction::Use { skill: FINALE })
        );
        assert_eq!(prepared.ready_skill(), Some(FINALE));
    }

    #[test]
    fn complete_removes_only_ready_slots() {
        let preparing = PreparedSkills::new().begin(SLASH, Nat::new(1));
        // 还没就绪 -> 不能完成准备（返回 None，状态不变）。
        assert_eq!(preparing.complete(SLASH), None);
        let ready = preparing.tick_action_round();
        let completed = ready.complete(SLASH).expect("已完成准备");
        assert!(completed.is_empty());
        // 不存在的技能 -> None。
        assert_eq!(ready.complete(FINALE), None);
    }

    #[test]
    fn interrupt_works_for_preparing_and_ready_slots() {
        let preparing = PreparedSkills::new().begin(SLASH, Nat::new(2));
        assert!(preparing.interrupt(SLASH).expect("打断").is_empty());
        let ready = preparing.tick_action_round().tick_action_round();
        assert!(ready.interrupt(SLASH).expect("打断").is_empty());
        assert_eq!(ready.interrupt(FINALE), None);
    }

    #[test]
    fn interrupt_all_and_clear_remove_every_slot() {
        // 切换角色（K53：出战 -> 后台，所有准备打断）。
        let prepared = PreparedSkills::new()
            .begin(SLASH, Nat::new(1))
            .begin(FINALE, Nat::ZERO);
        assert_eq!(prepared.interrupt_all(), PreparedSkills::new());
        assert_eq!(prepared.clear(), PreparedSkills::new());
        // 原容器不受影响（纯函数）。
        assert_eq!(prepared.len(), Nat::new(2));
    }

    #[test]
    fn duplicates_are_removed_one_at_a_time() {
        let prepared = PreparedSkills::new()
            .begin(SLASH, Nat::ZERO)
            .begin(SLASH, Nat::ZERO);
        assert_eq!(prepared.len(), Nat::new(2));
        let once = prepared.complete(SLASH).expect("完成一个");
        assert_eq!(once.len(), Nat::new(1));
        let twice = once.complete(SLASH).expect("完成另一个");
        assert!(twice.is_empty());
    }

    #[test]
    fn slots_iterate_in_begin_order() {
        let prepared = PreparedSkills::new()
            .begin(SLASH, Nat::new(1))
            .begin(FINALE, Nat::new(2));
        let listed: Vec<SkillId> = prepared.iter().map(PreparedSkill::skill).collect();
        assert_eq!(listed, vec![SLASH, FINALE]);
        assert_eq!(
            prepared.get(Nat::new(1)).map(PreparedSkill::rounds_left),
            Some(Nat::new(2))
        );
    }
}
