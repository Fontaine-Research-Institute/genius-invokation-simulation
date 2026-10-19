//! 结算帧：**插入式结算** + 中断 / 恢复（D194 / D196）。
//!
//! 用户口径（D196）：「不是栈，是有些效果会先插入一次别的结算再继续原来的结算」「debug 可以导出触发栈」。
//!
//! 语义：
//! - 当前结算 = 一串**待执行步骤**（[`Queued`]）；
//! - 某些效果会在**当前位置插入**一段新的结算（**子结算**，新的一帧）；
//!   子结算执行完后**回到插入点**，继续执行原结算的剩余步骤（[`Frame`] 保留 `cursor`）；
//! - **不是**「触发器后进先出栈」：不按 LIFO 重排已排好的待执行项，原帧步骤顺序不变；
//! - **中断** = 保留帧与 `cursor`（[`Resolution`] 本身就是被保留的结算状态）；
//! - **恢复** = 先执行「免于被击倒 + 治疗」这一步（[`Resolution::insert`] 插入子结算），
//!   再回到 `cursor` 继续；
//! - **确实被击倒** = 丢弃帧（[`Resolution::discard`]，终止后续）；
//! - **debug 导出**：[`Resolution::snapshot`] 返回只读的嵌套状态快照（当前帧 / 剩余步骤 / 插入点），
//!   供打印 / 对照游戏结算顺序；不改状态、不含函数指针。
//!
//! **权威文档**：`结算顺序.md` §二「插入式结算（不是栈）」与 §五「debug」——本模块是这两节的落地。
//!
//! 措辞（D195）：击倒发生时角色**已被击倒**，治疗类效果的**目标已不存在 → 无法执行**
//! （**不是**触发被屏蔽：触发条件仍然成立，只是没有可操作对象）；
//! 免于被击倒使角色重新成为有效目标后，才补 1 点治疗，再继续被中断的流程。

use super::event::Event;
use crate::rules::ops::order::EffectSource;
use gi_genius_invokation_engine::{nadd, nsub, EntityId, Nat};

/// 一条 **owned 的待执行**：函数指针 + 事件 + 「谁 / 按什么顺序」标识。
///
/// 与 [`crate::timeline::Pending`] 字段一一对应，区别只在 `event` 是**按值**保存的 ——
/// 这样才能组成结算步骤 / 结算帧。
#[derive(Clone, Copy, Debug)]
pub struct Queued<E> {
    /// 执行函数指针（本层不调用）。
    pub effect: E,
    /// 触发它的事件（纯数据、owned）。
    pub event: Event,
    /// 这条触发挂在谁身上。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条）。
    pub entry: Nat,
}

/// **一帧结算**：待执行步骤 + `cursor`（插入点 / 下一条要执行的步骤下标）。
///
/// 插入子结算时**不动**本帧的 `cursor`（原步骤顺序不变，也不重排）；
/// 子帧执行完后回到本帧 `cursor` 继续。
#[derive(Clone, Debug)]
pub struct Frame<E> {
    steps: Vec<Queued<E>>,
    cursor: Nat,
}

impl<E> Frame<E> {
    /// 用一串待执行步骤建一帧（`cursor` 从 0 开始）。
    pub fn new(steps: Vec<Queued<E>>) -> Frame<E> {
        Frame {
            steps,
            cursor: Nat::ZERO,
        }
    }

    /// 这帧的步骤总数。
    pub fn len(&self) -> Nat {
        Nat::new(self.steps.len() as _)
    }

    /// 这帧是否没有步骤。
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// 插入点：下一条要执行的步骤下标。
    pub fn cursor(&self) -> Nat {
        self.cursor
    }

    /// 剩余步骤条数（总数 - `cursor`）。
    pub fn remaining_len(&self) -> Nat {
        nsub!(self.len(), self.cursor)
    }

    /// 这帧是否已经执行完（`cursor` 到末尾）。
    pub fn is_finished(&self) -> bool {
        self.cursor >= self.len()
    }

    /// 剩余步骤（按原顺序；从 `cursor` 起）。
    pub fn remaining(&self) -> &[Queued<E>] {
        self.steps.get(self.cursor.get() as usize..).unwrap_or(&[])
    }

    /// 取下一步（`cursor` 前进 1；只移动游标，不执行效果）；已到末尾 -> `None`。
    pub fn take_step(&mut self) -> Option<Queued<E>>
    where
        E: Copy,
    {
        let step = *self.steps.get(self.cursor.get() as usize)?;
        self.cursor = nadd!(self.cursor, Nat::new(1));
        Some(step)
    }

    /// 这一帧的只读快照（层号由外到内：0 = 最外层）。
    fn snapshot(&self, depth: Nat) -> FrameSnapshot {
        let steps = self
            .remaining()
            .iter()
            .map(|step| StepSnapshot {
                owner: step.owner,
                source_kind: step.source_kind,
                entry: step.entry,
                event: step.event,
            })
            .collect();
        FrameSnapshot {
            depth,
            cursor: self.cursor,
            remaining: self.remaining_len(),
            steps,
        }
    }
}

/// **结算帧栈**：插入式结算的当前状态（D196）。
///
/// - 最后一帧 = 当前帧；前面的帧是「被插入点保留的父结算」；
/// - [`Resolution::insert`] 在当前位置压入子结算帧；子帧做完自动弹回父帧的 `cursor`；
/// - 中断 = 保留这个值（帧与 `cursor` 都在）；确实被击倒 = [`Resolution::discard`]。
#[derive(Clone, Debug)]
pub struct Resolution<E> {
    frames: Vec<Frame<E>>,
}

impl<E> Resolution<E> {
    /// 用一串待执行步骤建单帧结算。
    pub fn new(steps: Vec<Queued<E>>) -> Resolution<E> {
        Resolution {
            frames: vec![Frame::new(steps)],
        }
    }

    /// 空结算（没有帧）。
    pub fn empty() -> Resolution<E> {
        Resolution { frames: Vec::new() }
    }

    /// 全部帧（从外到内；读）。
    pub fn frames(&self) -> &[Frame<E>] {
        &self.frames
    }

    /// 当前帧（最内层；没有帧 -> `None`）。
    pub fn current(&self) -> Option<&Frame<E>> {
        self.frames.last()
    }

    /// 嵌套层数（帧数）。
    pub fn depth(&self) -> Nat {
        Nat::new(self.frames.len() as _)
    }

    /// 整个结算是否已完成（没有剩余步骤）。
    pub fn is_finished(&self) -> bool {
        self.frames.iter().all(Frame::is_finished)
    }

    /// **插入子结算**：在当前帧当前位置压入新帧（父帧的 `cursor` 不动）。
    ///
    /// - 子结算执行完后**回到插入点**，继续父帧剩余步骤；
    /// - 不重排父帧已有步骤（不是 LIFO 触发栈）；
    /// - 空插入 = 不做任何事。
    pub fn insert(&mut self, steps: Vec<Queued<E>>) {
        if steps.is_empty() {
            return;
        }
        self.frames.push(Frame::new(steps));
    }

    /// 取下一步：当前帧有剩余就取它；当前帧做完则弹回父帧继续。
    ///
    /// 整个嵌套结算都做完 -> `None`（只移动游标，不执行效果）。
    pub fn next_step(&mut self) -> Option<Queued<E>>
    where
        E: Copy,
    {
        loop {
            let frame = self.frames.last_mut()?;
            if frame.is_finished() {
                self.frames.pop();
                continue;
            }
            return frame.take_step();
        }
    }

    /// **确实被击倒**：丢弃结算帧（终止后续，不再继续）。
    ///
    /// 消费掉整个结算对象：类型上不能再恢复。
    pub fn discard(self) {}

    /// **只读快照**：当前结算嵌套状态（当前帧 / 剩余步骤 / 插入点）。
    ///
    /// 不改状态、不含函数指针；供打印 / 导出对照游戏结算顺序。
    pub fn snapshot(&self) -> ResolutionSnapshot {
        let current = match self.frames.len() {
            0 => None,
            len => Some(nsub!(Nat::new(len as _), Nat::new(1))),
        };
        let frames = self
            .frames
            .iter()
            .enumerate()
            .map(|(depth, frame)| frame.snapshot(Nat::new(depth as _)))
            .collect();
        ResolutionSnapshot { current, frames }
    }
}

/// 当前结算嵌套状态的**只读快照**（可打印 / 导出）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolutionSnapshot {
    /// 当前帧的层号（0 = 最外层；没有帧 -> `None`）。
    pub current: Option<Nat>,
    /// 各层快照（从外到内）。
    pub frames: Vec<FrameSnapshot>,
}

/// 一帧的只读快照。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSnapshot {
    /// 层号（0 = 最外层）。
    pub depth: Nat,
    /// 插入点：下一条要执行的步骤下标。
    pub cursor: Nat,
    /// 剩余步骤条数。
    pub remaining: Nat,
    /// 剩余步骤（按原执行顺序；只含定位信息，不含函数指针）。
    pub steps: Vec<StepSnapshot>,
}

/// 一步待执行的只读快照（纯数据，可打印）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepSnapshot {
    /// 这条触发挂在谁身上。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条）。
    pub entry: Nat,
    /// 触发它的事件。
    pub event: Event,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用局面（效果函数指针的上下文；本层不认识它）。
    #[derive(Default)]
    struct World {
        log: Vec<&'static str>,
    }

    /// 测试用效果函数指针类型。
    type WorldEffect = fn(&mut World, &Event);

    fn original_one(world: &mut World, _event: &Event) {
        world.log.push("原1");
    }
    fn original_two(world: &mut World, _event: &Event) {
        world.log.push("原2");
    }
    fn sub_one(world: &mut World, _event: &Event) {
        world.log.push("子1");
    }
    fn sub_two(world: &mut World, _event: &Event) {
        world.log.push("子2");
    }
    fn nested_one(world: &mut World, _event: &Event) {
        world.log.push("嵌套1");
    }
    fn avoid_defeat(world: &mut World, _event: &Event) {
        world.log.push("不视作被击倒");
    }
    fn heal_one(world: &mut World, _event: &Event) {
        world.log.push("治疗1");
    }

    fn queued(effect: WorldEffect) -> Queued<WorldEffect> {
        Queued {
            effect,
            event: Event::EMPTY,
            owner: Nat::new(1),
            source_kind: EffectSource::Summon,
            entry: Nat::ZERO,
        }
    }

    /// 执行一步（模拟调用方执行：本层只给步骤）。
    fn run_step(step: Queued<WorldEffect>, world: &mut World) {
        (step.effect)(world, &step.event);
    }

    /// 把结算帧栈里剩余步骤全部执行。
    fn drain(resolution: &mut Resolution<WorldEffect>, world: &mut World) {
        while let Some(step) = resolution.next_step() {
            run_step(step, world);
        }
    }

    #[test]
    fn inserted_sub_resolution_returns_to_the_insertion_point() {
        // 原结算：原1 -> 原2；执行完原1 后在当前位置插入子结算。
        let mut resolution = Resolution::new(vec![queued(original_one), queued(original_two)]);
        let mut world = World::default();
        let first = resolution.next_step().expect("原1");
        run_step(first, &mut world);
        // 插入子结算（父帧 cursor 保持不动，原2 顺序不变）。
        resolution.insert(vec![queued(sub_one), queued(sub_two)]);
        assert_eq!(resolution.depth(), Nat::new(2));
        drain(&mut resolution, &mut world);
        assert_eq!(world.log, vec!["原1", "子1", "子2", "原2"]);
    }

    #[test]
    fn nested_insertions_return_level_by_level() {
        // 两层嵌套：原1 -> 子1 -> 嵌套1 -> 子2 -> 原2。
        let mut resolution = Resolution::new(vec![queued(original_one), queued(original_two)]);
        let mut world = World::default();
        let first = resolution.next_step().expect("原1");
        run_step(first, &mut world);
        resolution.insert(vec![queued(sub_one), queued(sub_two)]);
        let sub_first = resolution.next_step().expect("子1");
        run_step(sub_first, &mut world);
        // 子帧里再插入一层。
        resolution.insert(vec![queued(nested_one)]);
        assert_eq!(resolution.depth(), Nat::new(3));
        drain(&mut resolution, &mut world);
        assert_eq!(world.log, vec!["原1", "子1", "嵌套1", "子2", "原2"]);
    }

    #[test]
    fn interruption_keeps_frames_and_cursor_then_resumes() {
        // 中断 = 保留帧与 cursor；恢复 = 先执行「免于击倒 + 治疗」，再回到 cursor 继续。
        let mut resolution = Resolution::new(vec![queued(original_one), queued(original_two)]);
        let mut world = World::default();
        let first = resolution.next_step().expect("原1");
        run_step(first, &mut world);
        // 击倒中断：resolution 保留（原2 仍在 cursor 处）。
        assert_eq!(
            resolution.current().map(Frame::remaining_len),
            Some(Nat::new(1))
        );
        // 免于被击倒后：插入「不视作被击倒 -> 治疗1」子结算，再回到插入点。
        resolution.insert(vec![queued(avoid_defeat), queued(heal_one)]);
        drain(&mut resolution, &mut world);
        assert_eq!(world.log, vec!["原1", "不视作被击倒", "治疗1", "原2"]);
    }

    #[test]
    fn truly_defeated_discards_frames() {
        // 没有免于被击倒：丢弃结算帧（不是继续）。
        let resolution = Resolution::new(vec![queued(original_one), queued(original_two)]);
        assert!(!resolution.is_finished());
        assert_eq!(resolution.depth(), Nat::new(1));
        resolution.discard();
        // 丢弃后类型上无法再继续；没有任何步骤被执行。
        let world = World::default();
        assert!(world.log.is_empty());
    }

    #[test]
    fn snapshot_exposes_current_frame_remaining_steps_and_cursor() {
        let mut resolution = Resolution::new(vec![queued(original_one), queued(original_two)]);
        let first = resolution.next_step().expect("原1");
        let mut world = World::default();
        run_step(first, &mut world);
        resolution.insert(vec![queued(sub_one)]);
        // 快照：外帧 depth 0 / cursor 1 / remaining 1（原2），内帧 depth 1 / cursor 0 / remaining 1（子1）。
        let snapshot = resolution.snapshot();
        assert_eq!(snapshot.current, Some(Nat::new(1)));
        assert_eq!(snapshot.frames.len(), 2);
        let outer = &snapshot.frames[0];
        assert_eq!(outer.depth, Nat::ZERO);
        assert_eq!(outer.cursor, Nat::new(1));
        assert_eq!(outer.remaining, Nat::new(1));
        assert_eq!(outer.steps.len(), 1);
        assert_eq!(outer.steps[0].owner, Nat::new(1));
        assert_eq!(outer.steps[0].entry, Nat::ZERO);
        let inner = &snapshot.frames[1];
        assert_eq!(inner.depth, Nat::new(1));
        assert_eq!(inner.cursor, Nat::ZERO);
        assert_eq!(inner.remaining, Nat::new(1));
        // 只读：连续两次快照内容一致（不改状态）。
        assert_eq!(resolution.snapshot(), snapshot);
        // 拍完快照后继续执行，仍然是内帧的子1。
        let next = resolution.next_step().expect("子1");
        assert_eq!(next.owner, Nat::new(1));
    }

    #[test]
    fn empty_resolution_is_finished_and_next_is_none() {
        let mut resolution: Resolution<WorldEffect> = Resolution::empty();
        assert!(resolution.is_finished());
        assert!(resolution.next_step().is_none());
        let snapshot = resolution.snapshot();
        assert_eq!(snapshot.current, None);
        assert!(snapshot.frames.is_empty());
    }
}
