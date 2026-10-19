//! 「使用技能」的**作用域派发**（D185 / D186；Lead 拍板：时机键合并进角色族**唯一一张**注册表）。
//!
//! 时机键本身在 `families::character::trigger`：
//! `CharacterTiming::SkillUsed { phase: SkillPhase, scope: SkillScope }`。
//! 本文件只做三件事：
//!
//! 1. **别名**：[`SkillTriggers`] **就是** [`CharacterTriggers`]
//!    （`TriggerRegistry<CharacterTiming, CharacterEffect>`）、[`SkillFrames`] 就是
//!    [`CharacterFrames`] —— 角色族只有一张注册表，「使用技能时 / 后」与「生命值变动 / 治疗后 /
//!    被击倒 / 免于被击倒」都登记在同一张表里；
//! 2. [`skill_timings`]：一次技能使用会命中的三档作用域时机键（任意 -> 某一类 -> 具体技能）；
//! 3. [`dispatch_skill`]：三趟派发 + 按**结算顺序**归并（`结算顺序.md` §一；顺序常量复用
//!    `rules::ops::order`）；只产出「执行函数指针 + 事件」的有序列表，**不执行**（D186）。
//!
//! ## 为什么技能身份进时机键（不是进 Event）
//!
//! 卡面措辞（`generated/data/descriptions.json` 统计）有四种粒度：
//! - 「使用技能后 / 使用技能时」——**任意**技能（119 + 52 处）；
//! - 「使用元素爆发后」10 处、「使用普通攻击后」1 处——**某一类**技能；
//! - 「所附属角色使用 `$[S13173]` 后」10 处——**某个具体**技能。
//!
//! 而 `timeline::Event` 是通用五字段（who / target / amount / element / source），**不含技能 id**，
//! 触发条件又固定是 `fn(&Event) -> bool`（D186）—— 所以技能身份**只能由时机键表达**：登记时选
//! `SkillScope::Any` / `SkillScope::Kind` / `SkillScope::Skill`，派发时三档都查一遍再归并。
//!
//! ## 三档趟序（暂定，待复核）
//!
//! 三档顺序（`Any` -> `Kind` -> `Skill`）只影响「来源类别与进场序完全相同」的并列项
//! （规则书未规定；本块按「先宽后窄」暂定，Lead 已接受、标**待复核**）。并列项之外的顺序一律按
//! 结算顺序第 2 / 3 条，与登记顺序无关。
//!
//! ## 不做
//!
//! - **不执行效果**（D186）：只给有序列表，由调用方执行（便捷入口
//!   `families::character::trigger::execute_character`，类型完全相同）；
//! - **不认识特技**（K58）：「使用『特技』时不视为使用技能，无法触发『使用技能后』」——特技不是技能，
//!   另建流程；本层**不会**为特技派发 `SkillUsed`；
//! - **准备技能的使用也不派发**（K53 原文），见 `families::character::skill_flow::prepared`；
//! - **不采样、不展开不确定量**（D190）：派发只看「已经发生」的事件，事件是确定的。

use crate::families::character::skill::SkillDef;
use crate::families::character::trigger::{
    CharacterFrames, CharacterPending, CharacterTrigger, CharacterTriggers,
};
use crate::rules::ops::order::resolves_before;
use crate::timeline::{dispatch, Event};
use gi_genius_invokation_engine::{EntityId, Nat};
use std::cmp::Ordering;

pub use crate::families::character::trigger::{CharacterTiming, SkillPhase, SkillScope};

/// 技能侧的触发登记项：**就是** [`CharacterTrigger`]（角色族唯一一张表里的条目）。
pub type SkillTrigger = CharacterTrigger;

/// 技能侧的触发注册表：**就是** [`CharacterTriggers`]
/// （`TriggerRegistry<CharacterTiming, CharacterEffect>`）。
///
/// 没有第二张表（Lead 拍板）：技能使用时机与角色族其它时机共用同一份注册表 / 同一套结算帧。
pub type SkillTriggers = CharacterTriggers;

/// 一条**待执行**（owned；事件按值带走）。由调用方执行，本层不执行。
pub type SkillPending = CharacterPending;

/// 技能结算的**帧栈**（D194 / D196：插入式结算；中断 = 保留帧与 cursor）。
///
/// **就是** [`CharacterFrames`]（`Resolution<CharacterEffect>`）：技能流程与角色族其它时机共用
/// 一套结算帧，子结算用 `Resolution::insert` 插到当前位置，做完回到插入点继续。
pub type SkillFrames = CharacterFrames;

/// 「使用技能时」在**任意**技能上成立（卡面「使用技能时」，最常见的登记形态）。
pub const BEFORE_ANY: CharacterTiming = CharacterTiming::SkillUsed {
    phase: SkillPhase::Before,
    scope: SkillScope::Any,
};

/// 「使用技能后」在**任意**技能上成立（卡面「使用技能后」，最常见的登记形态）。
pub const AFTER_ANY: CharacterTiming = CharacterTiming::SkillUsed {
    phase: SkillPhase::After,
    scope: SkillScope::Any,
};

/// 使用这个技能时，某个阶段的**全部**时机键（从宽到窄：任意 -> 某一类 -> 具体技能）。
pub fn skill_timings(phase: SkillPhase, skill: &SkillDef) -> [CharacterTiming; 3] {
    [
        CharacterTiming::skill_used(phase, SkillScope::Any),
        CharacterTiming::skill_used(phase, SkillScope::Kind(skill.kind)),
        CharacterTiming::skill_used(phase, SkillScope::Skill(skill.id)),
    ]
}

/// 「使用技能」事件：`who` = 使用技能的角色；`target` = 技能目标（无目标 -> `None`）。
///
/// - **技能身份不进事件**：`timeline::Event` 是通用五字段，不含技能 id；技能身份由时机键的
///   [`SkillScope`] 表达（见本文件顶部说明）。
/// - `source` 留空：技能不是实体（`EntityId` 只发给实体），「谁用的」就是 `who`。
pub fn skill_event(who: EntityId, target: Option<EntityId>) -> Event {
    Event::new(Some(who), target, Nat::ZERO, None, None)
}

/// **一次技能使用的有序派发**（D186：只排序不执行）。
///
/// 步骤：
/// 1. 按 [`skill_timings`] 三档作用域各派发一次（`timeline::dispatch`，条件 + 时机匹配；
///    单侧内部已按来源类别 / 进场先后排序）；
/// 2. 三趟结果按**结算顺序**归并（规则书第 2 / 3 条）；同来源类别、同进场序的并列项保持
///    「任意 -> 某一类 -> 具体技能」的趟序（暂定、待复核）。
///
/// 只读 + 不执行：不调用任何效果函数指针，也不改注册表与事件。
#[must_use]
pub fn dispatch_skill(
    triggers: &SkillTriggers,
    phase: SkillPhase,
    skill: &SkillDef,
    event: &Event,
) -> Vec<SkillPending> {
    let mut ordered: Vec<SkillPending> = Vec::new();
    for timing in skill_timings(phase, skill) {
        ordered.extend(
            dispatch(triggers, timing, event)
                .into_iter()
                .map(|item| item.into_queued()),
        );
    }
    ordered.sort_by(by_resolution_order);
    ordered
}

/// 两条待执行的结算先后：先来源类别（规则书第 2 条），再进场先后（第 3 条）。
///
/// 来源类别的先后**复用** `rules::ops::order::resolves_before`（唯一一份顺序表），不另存常量。
fn by_resolution_order(left: &SkillPending, right: &SkillPending) -> Ordering {
    if left.source_kind == right.source_kind {
        return left.entry.cmp(&right.entry);
    }
    if resolves_before(left.source_kind, right.source_kind) {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::SkillId;
    use crate::families::character::character::CharacterState;
    use crate::families::character::skill::SkillCost;
    use crate::families::character::trigger::{dispatch_character, CharacterTiming};
    use crate::families::character::types::SkillKind;
    use crate::generated::Localized;
    use crate::rules::ops::order::EffectSource;
    use gi_genius_invokation_engine::{nadd, nmul};

    /// 登记效果：把生命值**追加一位数字**（`hp = hp * 10 + digit`），用来断言执行顺序。
    fn mark_one(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let scaled = nmul!(state.hp(), Nat::new(10));
        let next = state.hp_set(nadd!(scaled, Nat::new(1)));
        *state = next;
    }
    fn mark_two(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let scaled = nmul!(state.hp(), Nat::new(10));
        let next = state.hp_set(nadd!(scaled, Nat::new(2)));
        *state = next;
    }
    fn mark_three(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let scaled = nmul!(state.hp(), Nat::new(10));
        let next = state.hp_set(nadd!(scaled, Nat::new(3)));
        *state = next;
    }

    /// 测试用空效果体（`SkillDef.effect` 是占位 `fn()`，D162）。
    fn noop() {}

    /// 测试用技能：普通攻击 12161。
    static NORMAL: SkillDef = SkillDef {
        id: SkillId::new(12161),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::normal_attack(),
        cost: SkillCost::FREE,
        effect: noop,
    };

    /// 测试用技能：元素爆发 12163。
    static BURST: SkillDef = SkillDef {
        id: SkillId::new(12163),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::ElementalBurst,
        cost: SkillCost::FREE,
        effect: noop,
    };

    /// 测试用角色状态：生命 0 / 上限 MAX（顺序记号不受钳制）。
    fn character() -> CharacterState {
        CharacterState::new(Nat::ZERO, Nat::MAX, Nat::ZERO, Nat::new(3))
    }

    /// 执行一批待执行（模拟调用方；本层不执行）。
    fn run(pending: &[SkillPending], state: &mut CharacterState) {
        crate::families::character::trigger::execute_character(pending, state);
    }

    fn event() -> Event {
        skill_event(Nat::new(1), Some(Nat::new(2)))
    }

    #[test]
    fn skill_triggers_are_the_character_triggers() {
        // Lead 拍板：角色族只有一张注册表 —— 两个类型是同一种（同一张表）。
        let triggers: CharacterTriggers = SkillTriggers::new();
        assert!(triggers.is_empty());
        let _same_trigger: CharacterTrigger = SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        );
    }

    #[test]
    fn empty_registry_dispatches_nothing() {
        let triggers = SkillTriggers::new();
        assert!(dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event()).is_empty());
    }

    #[test]
    fn skill_timings_goes_from_wide_to_narrow() {
        let [any, kind, skill] = skill_timings(SkillPhase::After, &BURST);
        assert_eq!(any, AFTER_ANY);
        assert_eq!(
            kind,
            CharacterTiming::SkillUsed {
                phase: SkillPhase::After,
                scope: SkillScope::Kind(SkillKind::ElementalBurst),
            }
        );
        assert_eq!(
            skill,
            CharacterTiming::SkillUsed {
                phase: SkillPhase::After,
                scope: SkillScope::Skill(SkillId::new(12163)),
            }
        );
    }

    #[test]
    fn any_scope_matches_every_skill() {
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        let for_normal = dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event());
        let for_burst = dispatch_skill(&triggers, SkillPhase::After, &BURST, &event());
        assert_eq!(for_normal.len(), 1);
        assert_eq!(for_burst.len(), 1);
    }

    #[test]
    fn kind_scope_only_matches_that_kind() {
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            CharacterTiming::skill_used(
                SkillPhase::After,
                SkillScope::Kind(SkillKind::ElementalBurst),
            ),
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        assert!(dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event()).is_empty());
        assert_eq!(
            dispatch_skill(&triggers, SkillPhase::After, &BURST, &event()).len(),
            1
        );
    }

    #[test]
    fn skill_scope_only_matches_that_skill() {
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            CharacterTiming::skill_used(
                SkillPhase::After,
                SkillScope::Skill(SkillId::new(12163)),
            ),
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        assert!(dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event()).is_empty());
        assert_eq!(
            dispatch_skill(&triggers, SkillPhase::After, &BURST, &event()).len(),
            1
        );
    }

    #[test]
    fn before_and_after_are_separate_phases() {
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            BEFORE_ANY,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        assert_eq!(
            dispatch_skill(&triggers, SkillPhase::Before, &NORMAL, &event()).len(),
            1
        );
        assert!(dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event()).is_empty());
    }

    #[test]
    fn skill_timing_and_other_character_timings_share_one_registry() {
        // 同一张表里既有「使用技能后」也有「生命值变动」：各自只被自己的派发入口命中。
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        triggers.register(SkillTrigger::new(
            CharacterTiming::HpChanged,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::new(1),
            mark_two,
        ));
        let after_skill = dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event());
        assert_eq!(after_skill.len(), 1);
        let hp_changed = dispatch_character(&triggers, CharacterTiming::HpChanged, &event());
        assert_eq!(hp_changed.len(), 1);
        // 技能派发不命中其它时机，反之亦然。
        assert!(dispatch_skill(&triggers, SkillPhase::Before, &NORMAL, &event()).is_empty());
        assert!(dispatch_character(&triggers, CharacterTiming::Healed, &event()).is_empty());
    }

    #[test]
    fn three_scopes_merge_in_rulebook_source_order() {
        // 三档作用域各登记一条（来源类别不同、进场序都为 0）：
        // 任意 -> 支援牌；类别 -> 角色牌；具体技能 -> 召唤物。
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::Support,
            Nat::ZERO,
            mark_one,
        ));
        triggers.register(SkillTrigger::new(
            CharacterTiming::skill_used(
                SkillPhase::After,
                SkillScope::Kind(SkillKind::ElementalBurst),
            ),
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_two,
        ));
        triggers.register(SkillTrigger::new(
            CharacterTiming::skill_used(
                SkillPhase::After,
                SkillScope::Skill(SkillId::new(12163)),
            ),
            Nat::new(1),
            EffectSource::Summon,
            Nat::ZERO,
            mark_three,
        ));
        let pending = dispatch_skill(&triggers, SkillPhase::After, &BURST, &event());
        assert_eq!(pending.len(), 3);
        let mut state = character();
        run(&pending, &mut state);
        // 规则书第 2 条：角色牌 -> 召唤物 -> 支援牌（与三趟派发的先后无关）。
        assert_eq!(state.hp(), Nat::new(231));
    }

    #[test]
    fn ties_keep_wide_to_narrow_scope_order() {
        // 同来源类别、同进场序：并列项按「任意 -> 某一类 -> 具体技能」趟序（暂定、待复核）。
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::Support,
            Nat::ZERO,
            mark_one,
        ));
        triggers.register(SkillTrigger::new(
            CharacterTiming::skill_used(
                SkillPhase::After,
                SkillScope::Kind(SkillKind::ElementalBurst),
            ),
            Nat::new(1),
            EffectSource::Support,
            Nat::ZERO,
            mark_two,
        ));
        triggers.register(SkillTrigger::new(
            CharacterTiming::skill_used(
                SkillPhase::After,
                SkillScope::Skill(SkillId::new(12163)),
            ),
            Nat::new(1),
            EffectSource::Support,
            Nat::ZERO,
            mark_three,
        ));
        let pending = dispatch_skill(&triggers, SkillPhase::After, &BURST, &event());
        let mut state = character();
        run(&pending, &mut state);
        assert_eq!(state.hp(), Nat::new(123));
    }

    #[test]
    fn same_source_orders_by_entry_order() {
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::Summon,
            Nat::new(1),
            mark_two,
        ));
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::Summon,
            Nat::ZERO,
            mark_one,
        ));
        let pending = dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event());
        let mut state = character();
        run(&pending, &mut state);
        assert_eq!(state.hp(), Nat::new(12));
    }

    #[test]
    fn event_carries_who_and_target_only() {
        let event = skill_event(Nat::new(7), Some(Nat::new(8)));
        assert_eq!(event.who, Some(Nat::new(7)));
        assert_eq!(event.target, Some(Nat::new(8)));
        assert_eq!(event.amount, Nat::ZERO);
        assert_eq!(event.element, None);
        assert_eq!(event.source, None);
        // 无目标 -> target 为空。
        assert_eq!(skill_event(Nat::new(7), None).target, None);
    }

    #[test]
    fn dispatch_does_not_execute_effects() {
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        let mut state = character();
        let pending = dispatch_skill(&triggers, SkillPhase::After, &NORMAL, &event());
        // 派发只产出列表，状态一个字节都不改。
        assert_eq!(pending.len(), 1);
        assert_eq!(state.hp(), Nat::ZERO);
        run(&pending, &mut state);
        assert_eq!(state.hp(), Nat::new(1));
    }
}
