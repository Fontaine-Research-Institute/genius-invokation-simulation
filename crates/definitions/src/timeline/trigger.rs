//! 触发登记项与注册表（D185 / D186）。
//!
//! 本文件只提供**机制**，不认识任何具体时机：
//! - **时机键 `K` 由牌族 / 类型自己定义**（零大小类型或枚举，D185），本层只做相等比较；
//! - **条件 = 函数指针** `fn(&Event) -> bool`（D186：不是闭包、不是 trait object）；
//! - **效果 = 执行函数指针** `E`（D186：不是效果数据、不是效果 enum）；默认 [`Effect`] = `fn(&Event)`，
//!   需要局面上下文的家族把它设成带自家状态参数的函数指针（如 `fn(&mut CharacterState, &Event)`），
//!   **执行由调用方做，本层只登记 + 排序**；
//! - **来源标识** [`EffectSource`]：结算顺序第 2 条的五类（角色牌 / 角色装备和状态 / 阵营出战状态 /
//!   召唤物 / 支援牌）；
//! - **进场先后** [`Nat`]：结算顺序第 3 条（同一来源类别按进场先后）；
//! - **所属实体** [`EntityId`]：这条触发「挂在谁身上」——派发结果会原样带回，供调用方定位实例
//!   （扣可用次数 / 弃置 / 取状态），本层不解释它。
//!
//! **没有全局单例、没有 `TypeId`、没有字符串键**（D185）：注册表是普通**值**，谁要派发谁就在自己的
//! 状态结构里持有一个字段（D42：区 = 状态里的字段）；条目全是 `Copy` 数据（时机键 + 函数指针 +
//! `Nat`），没有堆上的效果数据、没有所有权问题（D186）。
//!
//! 卡牌 / 状态在**初始化**时登记：逐条 [`TriggerRegistry::register`]，
//! 或从「静态数据 + 函数指针表」整批拷入 [`TriggerRegistry::register_all`]。

use super::event::Event;
use crate::rules::ops::order::EffectSource;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 触发条件：`fn(&Event) -> bool` 的函数指针（D186：函数指针，不是闭包 / trait object）。
///
/// 只看事件（谁 / 目标 / 数值 / 元素 / 来源）；返回 `true` = 这条触发命中。
pub type Condition = fn(&Event) -> bool;

/// 恒成立的条件：登记时不额外过滤。
pub const ALWAYS: Condition = always;

/// 默认效果函数指针：只吃事件（无状态家族 / 纯通知用）。
///
/// 需要局面上下文的家族把注册表的 `E` 换成带自家状态参数的函数指针类型
/// （例如 `fn(&mut CharacterState, &Event)`）；**本层只登记和排序，不调用**（D186）。
pub type Effect = fn(&Event);

fn always(_event: &Event) -> bool {
    true
}

/// 一条**触发登记项**（纯数据；`K` / `E` 都是 `Copy` 时整体 `Copy`）。
///
/// 字段与依据：
/// - `timing`：时机键（家族定义）；
/// - `owner`：这条触发挂在哪个实体上（「谁」）；
/// - `source_kind`：结算顺序第 2 条的来源类别；
/// - `entry`：进场先后（结算顺序第 3 条）；
/// - `condition`：条件函数指针（D186）；
/// - `effect`：执行函数指针（D186；派发只把它放进有序列表，不执行）。
#[derive(Clone, Copy, Debug)]
pub struct Trigger<K, E = Effect> {
    /// 时机键（由牌族 / 类型定义）。
    pub timing: K,
    /// 这条触发挂在谁身上（实体号；派发结果原样带回）。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条；由区域在登记时给出）。
    pub entry: Nat,
    /// 条件函数指针（D186）。
    pub condition: Condition,
    /// 执行函数指针（D186）。
    pub effect: E,
}

impl<K, E> Trigger<K, E> {
    /// 登记一条触发：条件恒真（[`ALWAYS`]）。
    pub const fn new(
        timing: K,
        owner: EntityId,
        source_kind: EffectSource,
        entry: Nat,
        effect: E,
    ) -> Trigger<K, E> {
        Trigger {
            timing,
            owner,
            source_kind,
            entry,
            condition: ALWAYS,
            effect,
        }
    }

    /// 登记一条触发：带条件函数指针。
    pub const fn with_condition(
        timing: K,
        owner: EntityId,
        source_kind: EffectSource,
        entry: Nat,
        condition: Condition,
        effect: E,
    ) -> Trigger<K, E> {
        Trigger {
            timing,
            owner,
            source_kind,
            entry,
            condition,
            effect,
        }
    }
}

impl<K: PartialEq, E> Trigger<K, E> {
    /// 这条登记项是否命中这次事件：**时机相等且条件成立**（派发唯一的匹配口径）。
    ///
    /// 只读：不调用效果、不改任何东西。
    pub fn applies(&self, timing: K, event: &Event) -> bool {
        self.timing == timing && (self.condition)(event)
    }
}

/// 触发注册表（**容器没有身份**，D42：它只是某个状态结构里的一个字段）。
///
/// - 泛型 `K` = 时机键类型（由家族给；D185）；
/// - 泛型 `E` = 效果函数指针类型（默认 [`Effect`]；D186）。
///
/// 只收集登记项，不做匹配 / 排序（那在 `timeline::dispatch`）；重复登记**不去重**
/// （每条登记独立结算，与 `ReactionRegistry` 同口径）。
#[derive(Clone, Debug)]
pub struct TriggerRegistry<K, E = Effect> {
    entries: Vec<Trigger<K, E>>,
}

impl<K, E> TriggerRegistry<K, E> {
    /// 空注册表。
    pub const fn new() -> TriggerRegistry<K, E> {
        TriggerRegistry {
            entries: Vec::new(),
        }
    }

    /// 全部登记项（读）。
    pub fn entries(&self) -> &[Trigger<K, E>] {
        &self.entries
    }

    /// 登记项条数（`Nat`，不是裸整型）。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何登记项。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 登记一条触发（追加；重复登记不去重）。
    pub fn register(&mut self, trigger: Trigger<K, E>) {
        self.entries.push(trigger);
    }

    /// 从一份**静态登记表**整批拷入（「静态数据 + 函数指针表」，D186）。
    ///
    /// 卡牌 / 状态的定义把「时机 -> 效果」写成 `static [Trigger; N]`，
    /// 初始化时整表登记进所属家族的注册表；数据只有一份，运行期没有额外效果数据。
    pub fn register_all(&mut self, triggers: &[Trigger<K, E>])
    where
        K: Copy,
        E: Copy,
    {
        self.entries.extend_from_slice(triggers);
    }

    /// 清空（例如该注册表跟随某个状态一起被移除）。
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<K, E> Default for TriggerRegistry<K, E> {
    /// 默认 = 空注册表。
    fn default() -> TriggerRegistry<K, E> {
        TriggerRegistry::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 示例时机键（D185：由家族定义；通用层不认识）。两个值足够验证匹配口径。
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Timing {
        Changed,
        Healed,
    }

    /// 默认效果（无状态）：只证明函数指针可以被登记 / 取出。
    fn noop(_event: &Event) {}

    /// 条件：数值 > 0。
    fn positive(event: &Event) -> bool {
        event.amount > Nat::ZERO
    }

    fn event(amount: Nat) -> Event {
        Event::new(None, None, amount, None, None)
    }

    #[test]
    fn new_uses_always_condition_and_keeps_sort_fields() {
        let trigger = Trigger::new(
            Timing::Changed,
            Nat::new(9),
            EffectSource::Summon,
            Nat::new(3),
            noop,
        );
        assert_eq!(trigger.timing, Timing::Changed);
        assert_eq!(trigger.owner, Nat::new(9));
        assert_eq!(trigger.source_kind, EffectSource::Summon);
        assert_eq!(trigger.entry, Nat::new(3));
        // 条件恒真：任何事件都成立。
        assert!(trigger.applies(Timing::Changed, &Event::EMPTY));
        assert!(trigger.applies(Timing::Changed, &event(Nat::new(5))));
    }

    #[test]
    fn with_condition_keeps_the_predicate() {
        let trigger = Trigger::with_condition(
            Timing::Healed,
            Nat::new(1),
            EffectSource::Support,
            Nat::new(0),
            positive,
            noop,
        );
        assert!(trigger.applies(Timing::Healed, &event(Nat::new(1))));
        assert!(!trigger.applies(Timing::Healed, &event(Nat::new(0))));
    }

    #[test]
    fn applies_requires_the_same_timing() {
        let trigger = Trigger::new(
            Timing::Changed,
            Nat::new(1),
            EffectSource::Summon,
            Nat::new(0),
            noop,
        );
        assert!(!trigger.applies(Timing::Healed, &Event::EMPTY));
        assert!(trigger.applies(Timing::Changed, &Event::EMPTY));
    }

    #[test]
    fn empty_registry_is_empty_and_default() {
        let registry: TriggerRegistry<Timing> = TriggerRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), Nat::ZERO);
        assert!(registry.entries().is_empty());
        assert_eq!(registry.len(), TriggerRegistry::<Timing>::default().len());
    }

    #[test]
    fn register_len_entries_and_clear() {
        let mut registry: TriggerRegistry<Timing> = TriggerRegistry::new();
        registry.register(Trigger::new(
            Timing::Changed,
            Nat::new(1),
            EffectSource::Summon,
            Nat::new(0),
            noop,
        ));
        registry.register(Trigger::new(
            Timing::Healed,
            Nat::new(2),
            EffectSource::Support,
            Nat::new(1),
            noop,
        ));
        assert_eq!(registry.len(), Nat::new(2));
        assert_eq!(registry.entries().len(), 2);
        assert_eq!(registry.entries()[0].timing, Timing::Changed);
        assert_eq!(registry.entries()[1].timing, Timing::Healed);
        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn duplicate_registrations_are_not_deduplicated() {
        // 每条登记独立结算（与 ReactionRegistry 同口径）：同一时机登记两次就命中两次。
        let mut registry: TriggerRegistry<Timing> = TriggerRegistry::new();
        // 这里显式标注类型：`Trigger::new` 的 `E` 没被注册表约束时会被推成 fn item 类型，
        // 标注后统一成函数指针 `fn(&Event)`（注册表的 E）。
        let trigger: Trigger<Timing> = Trigger::new(
            Timing::Changed,
            Nat::new(1),
            EffectSource::Summon,
            Nat::new(0),
            noop,
        );
        registry.register(trigger);
        registry.register(trigger);
        assert_eq!(registry.len(), Nat::new(2));
    }

    #[test]
    fn register_all_copies_a_static_table() {
        // 「静态数据 + 函数指针表」：定义侧是 static 数组，初始化时整表拷进注册表。
        static TABLE: [Trigger<Timing>; 2] = [
            Trigger::new(
                Timing::Changed,
                Nat::new(1),
                EffectSource::CharacterCard,
                Nat::new(0),
                noop,
            ),
            Trigger::with_condition(
                Timing::Healed,
                Nat::new(2),
                EffectSource::Support,
                Nat::new(1),
                positive,
                noop,
            ),
        ];
        let mut registry: TriggerRegistry<Timing> = TriggerRegistry::new();
        registry.register_all(&TABLE);
        assert_eq!(registry.len(), Nat::new(2));
        assert_eq!(registry.entries()[0].owner, Nat::new(1));
        assert_eq!(registry.entries()[1].source_kind, EffectSource::Support);
        // 静态表本身没有被修改（纯拷贝，数据只有一份）。
        assert_eq!(TABLE.len(), 2);
    }
}
