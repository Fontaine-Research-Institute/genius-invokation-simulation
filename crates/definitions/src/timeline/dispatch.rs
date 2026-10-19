//! 有序派发（D185 / D186）：把注册表里「这个时机、条件成立」的登记项按**结算顺序**排成待执行列表。
//!
//! **派发不执行效果**（D186）：只产出「效果函数指针 + 事件」的有序列表；执行由调用方做。
//!
//! **权威顺序文档**：`结算顺序.md` §一「同一时机内的结算顺序」（唯一权威；标「暂定」）。
//! 本模块的 [`dispatch`] / [`resolve`] 就是 §一 的落地：当前牌手优先 / 来源类别 / 进场先后 /
//! 稳定登记顺序；顺序常量复用 `rules::ops::order`（唯一一份），不在此另存。
//!
//! 结算顺序（权威原文见 `rules::ops::order`，规则书·其他规则·结算顺序）：
//! 1. 「先结算当前牌手的所有卡牌效果，再结算对方牌手的卡牌效果。」-> [`resolve`]：
//!    两个注册表就是**两个具名实例**（D34：不用 `Side` 枚举 / 不用 0/1 编号），
//!    调用方把「当前行动权那一侧」的注册表放第一个参数；
//! 2. 「同属于同一位牌手的卡牌效果，按以下顺序结算：角色牌、角色装备和状态、阵营出战状态、召唤物、支援牌。」
//!    -> [`dispatch`] 复用 `rules::ops::order::{resolves_before, EffectSource}`（不另存一份顺序表）；
//! 3. 「同属于同一位牌手的同类型卡牌效果，按进场的先后顺序结算。」-> [`dispatch`] 按 `entry`；
//! 4. 完全同序（同来源类别、同进场序）-> **稳定排序**：保持登记顺序，结果可复现。

use super::event::Event;
use super::suspend::Queued;
use super::trigger::{Trigger, TriggerRegistry};
use crate::rules::ops::order::{resolves_before, EffectSource};
use gi_genius_invokation_engine::{EntityId, Nat};
use std::cmp::Ordering;

/// 一条**待执行**（D186）：效果函数指针 + 触发它的事件，外加「该谁 / 按什么顺序」的标识。
///
/// - `effect`：注册方提供的执行函数指针（类型 `E` 由家族决定；本层不调用）；
/// - `event`：触发它的事件（纯数据；执行时原样传回）；
/// - `owner` / `source_kind` / `entry`：这条触发挂在谁身上、来源类别、进场先后
///   （调用方据此定位实例、扣可用次数 / 弃置等）。
#[derive(Clone, Copy, Debug)]
pub struct Pending<'a, E> {
    /// 执行函数指针（本层不调用）。
    pub effect: E,
    /// 触发它的事件。
    pub event: &'a Event,
    /// 这条触发挂在谁身上。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条）。
    pub entry: Nat,
}

impl<'a, E: Copy> Pending<'a, E> {
    /// 转成 **owned** 的 [`Queued`]（事件按值带走）。
    ///
    /// 用于需要跨调用保存的待执行队列 / 结算帧（D194 / D196：[`super::suspend::Resolution`]）。
    pub fn into_queued(self) -> Queued<E> {
        Queued {
            effect: self.effect,
            event: *self.event,
            owner: self.owner,
            source_kind: self.source_kind,
            entry: self.entry,
        }
    }
}

/// **单侧派发**：从一个牌手的注册表里取出「这个时机、条件成立」的登记项，按结算顺序排好。
///
/// 排序 = `结算顺序.md` §一 第 2~4 条：来源类别 -> 进场先后 -> 登记顺序（稳定排序）。
/// 第 1 条「当前牌手优先」不在本函数里：它由 [`resolve`] 用「两个注册表的先后」表达。
///
/// 只读 + 不执行：不调用任何效果函数指针，也不改注册表。
#[must_use]
pub fn dispatch<'e, K, E>(
    registry: &TriggerRegistry<K, E>,
    timing: K,
    event: &'e Event,
) -> Vec<Pending<'e, E>>
where
    K: Copy + PartialEq,
    E: Copy,
{
    // 1) 只留「时机相等 + 条件成立」的登记项。
    let mut matched: Vec<Trigger<K, E>> = registry
        .entries()
        .iter()
        .copied()
        .filter(|trigger| trigger.applies(timing, event))
        .collect();
    // 2) 按结算顺序排（稳定排序：完全同序时保持登记顺序）。
    matched.sort_by(by_resolution_order);
    // 3) 产出「函数指针 + 事件」的待执行列表（不执行）。
    matched
        .into_iter()
        .map(|trigger| Pending {
            effect: trigger.effect,
            event,
            owner: trigger.owner,
            source_kind: trigger.source_kind,
            entry: trigger.entry,
        })
        .collect()
}

/// **双方合并派发**（`结算顺序.md` §一 第 1 条「当前牌手优先」）：**当前牌手**的注册表先，对方的后。
///
/// 两个注册表是两个具名实例（D34：不传 `Side` 枚举、不用编号）：调用方把「当前行动权所在那一侧」
/// 放 `current`，另一侧放 `other`；两侧内部各自按 [`dispatch`] 的规则 2 / 3 排序。
///
/// 只读 + 不执行效果。
#[must_use]
pub fn resolve<'e, K, E>(
    current: &TriggerRegistry<K, E>,
    other: &TriggerRegistry<K, E>,
    timing: K,
    event: &'e Event,
) -> Vec<Pending<'e, E>>
where
    K: Copy + PartialEq,
    E: Copy,
{
    let mut ordered = dispatch(current, timing, event);
    ordered.extend(dispatch(other, timing, event));
    ordered
}

/// 两条登记项的结算先后：先比来源类别（规则书第 2 条），再比进场先后（规则书第 3 条）。
///
/// 来源类别的先后**复用** `rules::ops::order::resolves_before`（唯一一份顺序表）；
/// 同一类别返回 `resolves_before == false`，改比 `entry`。
fn by_resolution_order<K, E>(left: &Trigger<K, E>, right: &Trigger<K, E>) -> Ordering {
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
    use crate::base::Element;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    // ------------------------------------------------------------------
    // 示例时机键：全部由「家族 / 具体块」定义（D185），通用层不认识它们。
    // ------------------------------------------------------------------

    /// 示例：**角色族**自己的时机键（枚举；由角色块定义，不在 timeline）。
    ///
    /// 规则书 / 卡面依据：
    /// - `HpChanged`（生命值变动）：规则书·其他规则 击倒「当角色的生命值被降为0，则在技能或效果
    ///   结算完成后，角色就会被『击倒』」、免于被击倒；特殊词条 护盾「『护盾』所保护的角色受到
    ///   伤害时，将会消耗『护盾值』来抵消伤害」；卡面「受到伤害时」「受到伤害后」「治疗后」。
    /// - `Healed`（治疗后）：规则书未单列条目，依据在卡面「治疗后 / 受治疗后」。
    /// - `Defeated`（被击倒时）：规则书·其他规则 击倒（同上）。
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum CharacterTiming {
        HpChanged,
        Healed,
        Defeated,
    }

    /// 示例：**对局阶段**时机键（回合流程；由需要阶段广播的块定义）。
    ///
    /// 依据：规则书·回合流程「每个回合，都会按照『投掷阶段』『行动阶段』『结束阶段』顺序进行」；
    /// 结束阶段「从『先手牌手』开始，处理在『结束阶段』触发的卡牌效果」；
    /// 行动阶段「从『先手牌手』开始，双方『轮流进行行动』」。
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum RoundTiming {
        RoundBegin,
        RollPhase,
        ActionPhaseStart,
        EndPhase,
    }

    /// 示例：**零大小类型**时机键（D185 允许的另一形态：一个时机一个类型）。
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    struct DamageDealt;

    /// 调用方自己的局面（示例：效果函数指针接收的上下文；本层完全不认识它）。
    #[derive(Default)]
    struct World {
        log: Vec<&'static str>,
    }

    /// 示例效果函数指针类型：需要局面的家族把注册表的 `E` 设成这种签名。
    type WorldEffect = fn(&mut World, &Event);

    /// 示例效果函数指针类型（**带 owner**）：实例级过滤用 —— 效果函数拿 `Pending.owner` 判断
    /// 「这条触发是不是属于本次事件的主体」。
    type OwnerAwareEffect = fn(&mut World, &Event, EntityId);

    /// 执行一条待执行（**由调用方做**，不是 dispatch 做）。
    fn run(pending: &[Pending<'_, WorldEffect>], world: &mut World) {
        for item in pending {
            (item.effect)(world, item.event);
        }
    }

    // ------------------------------------------------------------------
    // 效果函数指针（每个来源类别一个，方便断言顺序）
    // ------------------------------------------------------------------

    fn character_card(world: &mut World, _event: &Event) {
        world.log.push("角色牌");
    }
    fn equipment_or_status(world: &mut World, _event: &Event) {
        world.log.push("角色装备和状态");
    }
    fn combat_status(world: &mut World, _event: &Event) {
        world.log.push("出战状态");
    }
    fn summon(world: &mut World, _event: &Event) {
        world.log.push("召唤物");
    }
    fn support(world: &mut World, _event: &Event) {
        world.log.push("支援牌");
    }

    fn entry_zero(world: &mut World, _event: &Event) {
        world.log.push("进场0");
    }
    fn entry_one(world: &mut World, _event: &Event) {
        world.log.push("进场1");
    }
    fn entry_two(world: &mut World, _event: &Event) {
        world.log.push("进场2");
    }

    fn first_registered(world: &mut World, _event: &Event) {
        world.log.push("先登记");
    }
    fn second_registered(world: &mut World, _event: &Event) {
        world.log.push("后登记");
    }

    /// 条件：数值 > 2。
    fn amount_above_two(event: &Event) -> bool {
        event.amount > Nat::new(2)
    }
    /// 条件：火元素。
    fn pyro_only(event: &Event) -> bool {
        event.element == Some(Element::Pyro)
    }

    /// 建一条登记项（示例里 owner 用 1，来源类别 / 进场先后由参数给）。
    fn trigger(
        timing: RoundTiming,
        source_kind: EffectSource,
        entry: Nat,
        effect: WorldEffect,
    ) -> Trigger<RoundTiming, WorldEffect> {
        Trigger::new(timing, Nat::new(1), source_kind, entry, effect)
    }

    fn end_phase_event() -> Event {
        Event::new(None, None, Nat::ZERO, None, None)
    }

    // ------------------------------------------------------------------
    // 空注册 / 时机隔离
    // ------------------------------------------------------------------

    #[test]
    fn empty_registry_dispatches_nothing() {
        let registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        let event = end_phase_event();
        let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
        assert!(pending.is_empty());
        assert_eq!(registry.len(), Nat::ZERO);
    }

    #[test]
    fn dispatch_only_matches_the_requested_timing() {
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Support,
            Nat::ZERO,
            support,
        ));
        registry.register(trigger(
            RoundTiming::ActionPhaseStart,
            EffectSource::Support,
            Nat::new(1),
            character_card,
        ));
        // 只派发结束阶段：行动阶段的登记项不命中。
        let event = end_phase_event();
        let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["支援牌"]);
        // 行动阶段开始：只命中它自己那条。
        let pending = dispatch(&registry, RoundTiming::ActionPhaseStart, &event);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["角色牌"]);
        // 没登记过的时机 -> 空。
        assert!(dispatch(&registry, RoundTiming::RollPhase, &end_phase_event()).is_empty());
    }

    // ------------------------------------------------------------------
    // 同一时机多来源排序（`结算顺序.md` §一 第 2 条 / 规则书·结算顺序）
    // ------------------------------------------------------------------

    #[test]
    fn same_timing_multiple_sources_follow_rulebook_order() {
        // 故意乱序登记：支援 -> 召唤物 -> 出战状态 -> 角色装备和状态 -> 角色牌。
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Support,
            Nat::ZERO,
            support,
        ));
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::ZERO,
            summon,
        ));
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::CombatStatus,
            Nat::ZERO,
            combat_status,
        ));
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::CharacterEquipmentOrStatus,
            Nat::ZERO,
            equipment_or_status,
        ));
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::CharacterCard,
            Nat::ZERO,
            character_card,
        ));
        let event = end_phase_event();
        let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
        assert_eq!(pending.len(), 5);
        let mut world = World::default();
        run(&pending, &mut world);
        // 规则书原文顺序：角色牌、角色装备和状态、阵营出战状态、召唤物、支援牌。
        assert_eq!(
            world.log,
            vec!["角色牌", "角色装备和状态", "出战状态", "召唤物", "支援牌"]
        );
    }

    // ------------------------------------------------------------------
    // 同一来源类别按进场先后（`结算顺序.md` §一 第 3 条）
    // ------------------------------------------------------------------

    #[test]
    fn same_source_orders_by_entry_order() {
        // 登记顺序 2 -> 0 -> 1；结算顺序必须是 0 -> 1 -> 2。
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::new(2),
            entry_two,
        ));
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::ZERO,
            entry_zero,
        ));
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::new(1),
            entry_one,
        ));
        let event = end_phase_event();
        let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["进场0", "进场1", "进场2"]);
    }

    #[test]
    fn ties_keep_registration_order_and_are_stable() {
        // 同来源类别、同进场序 -> 保持登记顺序（稳定排序，`结算顺序.md` §一 第 4 条），两次派发结果一致。
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::ZERO,
            first_registered,
        ));
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::ZERO,
            second_registered,
        ));
        let event = end_phase_event();
        for _ in 0..2 {
            let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
            let mut world = World::default();
            run(&pending, &mut world);
            assert_eq!(world.log, vec!["先登记", "后登记"]);
        }
    }

    // ------------------------------------------------------------------
    // 条件过滤
    // ------------------------------------------------------------------

    #[test]
    fn conditions_filter_entries() {
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::ZERO,
            first_registered,
        ));
        registry.register(Trigger::with_condition(
            RoundTiming::EndPhase,
            Nat::new(1),
            EffectSource::Summon,
            Nat::new(1),
            amount_above_two,
            entry_one,
        ));
        registry.register(Trigger::with_condition(
            RoundTiming::EndPhase,
            Nat::new(1),
            EffectSource::Summon,
            Nat::new(2),
            pyro_only,
            entry_two,
        ));

        // 数值 3 + 火元素 -> 三条都命中（按进场先后）。
        let hit_all = Event::new(None, None, Nat::new(3), Some(Element::Pyro), None);
        let pending = dispatch(&registry, RoundTiming::EndPhase, &hit_all);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["先登记", "进场1", "进场2"]);

        // 数值 1 + 水元素 -> 只有恒真条件那条命中。
        let hit_one = Event::new(None, None, Nat::new(1), Some(Element::Hydro), None);
        let pending = dispatch(&registry, RoundTiming::EndPhase, &hit_one);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["先登记"]);

        // 数值 0 + 无元素 -> 只有恒真条件那条命中。
        let empty = end_phase_event();
        let pending = dispatch(&registry, RoundTiming::EndPhase, &empty);
        assert_eq!(pending.len(), 1);
    }

    // ------------------------------------------------------------------
    // 派发不执行效果（D186）
    // ------------------------------------------------------------------

    #[test]
    fn dispatch_does_not_execute_effects() {
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Support,
            Nat::ZERO,
            support,
        ));
        let event = end_phase_event();
        let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
        // 只是「待执行」列表：拿到之后、调用方执行之前，效果一次都没跑。
        assert_eq!(pending.len(), 1);
        let world = World::default();
        assert!(world.log.is_empty());
        // 调用方执行（本层不替它做）。
        let mut world = world;
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["支援牌"]);
    }

    // ------------------------------------------------------------------
    // 第 1 条：当前牌手优先（`结算顺序.md` §一 第 1 条；resolve）
    // ------------------------------------------------------------------

    #[test]
    fn resolve_puts_current_player_effects_first_even_for_later_source_kinds() {
        // 当前牌手只有「支援牌」（第 2 条里最后），对方有「角色牌」（第 2 条里最前）。
        // 第 1 条优先于第 2 条：当前牌手的支援牌先结算。
        let mut current: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        current.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Support,
            Nat::ZERO,
            support,
        ));
        let mut other: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        other.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::CharacterCard,
            Nat::ZERO,
            character_card,
        ));
        let event = end_phase_event();
        let pending = resolve(&current, &other, RoundTiming::EndPhase, &event);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["支援牌", "角色牌"]);
    }

    #[test]
    fn resolve_keeps_each_sides_internal_order() {
        let mut current: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        current.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::new(1),
            entry_one,
        ));
        current.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::Summon,
            Nat::ZERO,
            entry_zero,
        ));
        let mut other: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        other.register(trigger(
            RoundTiming::EndPhase,
            EffectSource::CombatStatus,
            Nat::ZERO,
            combat_status,
        ));
        let event = end_phase_event();
        let pending = resolve(&current, &other, RoundTiming::EndPhase, &event);
        let mut world = World::default();
        run(&pending, &mut world);
        // 当前牌手内部按进场先后；然后才轮到对方。
        assert_eq!(world.log, vec!["进场0", "进场1", "出战状态"]);
    }

    // ------------------------------------------------------------------
    // 待执行项携带「该谁 / 按什么顺序」+ 事件
    // ------------------------------------------------------------------

    #[test]
    fn pending_carries_owner_source_entry_and_the_event() {
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register(Trigger::new(
            RoundTiming::EndPhase,
            Nat::new(7),
            EffectSource::Summon,
            Nat::new(4),
            summon,
        ));
        let event = Event::new(
            Some(Nat::new(1)),
            Some(Nat::new(2)),
            Nat::new(3),
            Some(Element::Pyro),
            Some(Nat::new(8)),
        );
        let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
        let item = pending.first().expect("应有一条待执行");
        assert!(std::ptr::addr_eq(item.event, &event));
        assert_eq!(item.owner, Nat::new(7));
        assert_eq!(item.source_kind, EffectSource::Summon);
        assert_eq!(item.entry, Nat::new(4));
    }

    // ------------------------------------------------------------------
    // 「静态数据 + 函数指针表」：整表登记
    // ------------------------------------------------------------------

    #[test]
    fn static_trigger_table_registers_and_dispatches() {
        static TABLE: [Trigger<RoundTiming, WorldEffect>; 2] = [
            Trigger::new(
                RoundTiming::EndPhase,
                Nat::new(1),
                EffectSource::Support,
                Nat::ZERO,
                support,
            ),
            Trigger::new(
                RoundTiming::EndPhase,
                Nat::new(2),
                EffectSource::CharacterCard,
                Nat::ZERO,
                character_card,
            ),
        ];
        let mut registry: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        registry.register_all(&TABLE);
        let event = end_phase_event();
        let pending = dispatch(&registry, RoundTiming::EndPhase, &event);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["角色牌", "支援牌"]);
    }

    // ------------------------------------------------------------------
    // 默认效果类型（fn(&Event)）+ 零大小时机键
    // ------------------------------------------------------------------

    static DEFAULT_EFFECT_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn count_default(_event: &Event) {
        DEFAULT_EFFECT_CALLS.fetch_add(1, AtomicOrdering::SeqCst);
    }

    #[test]
    fn default_effect_type_is_a_plain_fn_pointer_and_is_not_executed() {
        let mut registry: TriggerRegistry<CharacterTiming> = TriggerRegistry::new();
        registry.register(Trigger::new(
            CharacterTiming::HpChanged,
            Nat::new(1),
            EffectSource::Summon,
            Nat::ZERO,
            count_default,
        ));
        let event = Event::on(Nat::new(2));
        let pending = dispatch(&registry, CharacterTiming::HpChanged, &event);
        assert_eq!(pending.len(), 1);
        assert_eq!(DEFAULT_EFFECT_CALLS.load(AtomicOrdering::SeqCst), 0);
        // 调用方执行。
        if let Some(item) = pending.first() {
            (item.effect)(item.event);
        }
        assert_eq!(DEFAULT_EFFECT_CALLS.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn dispatch_covers_every_example_timing_value() {
        // 时机覆盖：示例时机键的每个值都能被派发命中 —— 通用层不认识任何具体时机，
        // 但机制对任意 `K` 一律成立（时机键由家族定义，D185）。
        let character_timings = [
            CharacterTiming::HpChanged,
            CharacterTiming::Healed,
            CharacterTiming::Defeated,
        ];
        let mut character: TriggerRegistry<CharacterTiming, WorldEffect> = TriggerRegistry::new();
        for timing in character_timings {
            character.register(Trigger::new(
                timing,
                Nat::new(1),
                EffectSource::Support,
                Nat::ZERO,
                support,
            ));
        }
        let event = Event::on(Nat::new(2));
        for timing in character_timings {
            let pending = dispatch(&character, timing, &event);
            assert_eq!(pending.len(), 1, "每个时机值都应命中自己那条");
        }

        let round_timings = [
            RoundTiming::RoundBegin,
            RoundTiming::RollPhase,
            RoundTiming::ActionPhaseStart,
            RoundTiming::EndPhase,
        ];
        let mut round: TriggerRegistry<RoundTiming, WorldEffect> = TriggerRegistry::new();
        for timing in round_timings {
            round.register(Trigger::new(
                timing,
                Nat::new(1),
                EffectSource::Support,
                Nat::ZERO,
                support,
            ));
        }
        for timing in round_timings {
            let pending = dispatch(&round, timing, &Event::EMPTY);
            assert_eq!(pending.len(), 1, "每个阶段时机值都应命中自己那条");
        }
    }

    /// 实例级过滤示例：同一张卡的**两个实例**都登记进同一张注册表（不是每实例一张表），
    /// 条件函数不带 owner（D186）；实例判断放在效果函数里 —— 用 `Pending.owner` 判断
    /// 「这条触发是不是属于本次事件的主体」，不是就直接返回。
    fn respond_when_owner_is_subject(world: &mut World, event: &Event, owner: EntityId) {
        if event.who == Some(owner) {
            world.log.push("实例响应");
        } else {
            world.log.push("实例跳过");
        }
    }

    #[test]
    fn same_card_two_instances_only_the_owner_responds() {
        let mut registry: TriggerRegistry<CharacterTiming, OwnerAwareEffect> =
            TriggerRegistry::new();
        // 同一张卡的两个实例（owner 10 / 20），效果函数是**同一个**函数指针。
        registry.register(Trigger::new(
            CharacterTiming::HpChanged,
            Nat::new(10),
            EffectSource::Summon,
            Nat::ZERO,
            respond_when_owner_is_subject,
        ));
        registry.register(Trigger::new(
            CharacterTiming::HpChanged,
            Nat::new(20),
            EffectSource::Summon,
            Nat::new(1),
            respond_when_owner_is_subject,
        ));
        // 事件主体是 10 号实例：两条登记都命中（时机相同、条件恒真）。
        let event = Event::of(Nat::new(10));
        let pending = dispatch(&registry, CharacterTiming::HpChanged, &event);
        assert_eq!(pending.len(), 2, "条件和时机都命中：过滤不在条件里做");
        // 调用方执行时把 owner 一起传给效果函数；只有 10 号实例真正响应。
        let mut world = World::default();
        for item in &pending {
            (item.effect)(&mut world, item.event, item.owner);
        }
        assert_eq!(world.log, vec!["实例响应", "实例跳过"]);
    }

    #[test]
    fn zero_sized_timing_key_works_as_a_timing_type() {
        let mut registry: TriggerRegistry<DamageDealt, WorldEffect> = TriggerRegistry::new();
        registry.register(Trigger::new(
            DamageDealt,
            Nat::new(1),
            EffectSource::CombatStatus,
            Nat::ZERO,
            combat_status,
        ));
        let event = Event::on(Nat::new(2));
        let pending = dispatch(&registry, DamageDealt, &event);
        assert_eq!(pending.len(), 1);
        let mut world = World::default();
        run(&pending, &mut world);
        assert_eq!(world.log, vec!["出战状态"]);
    }
}
