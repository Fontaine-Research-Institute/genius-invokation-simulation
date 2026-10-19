//! 统一事件对象（纯数据，D185）。
//!
//! 触发调度的「事件」= 某件事发生了，并且带上所有可能被条件 / 效果用到的**最小公共数据**。
//! 本文件只有数据，没有任何时机枚举、没有注册、没有派发：
//! 时机键由各牌族 / 类型自己定义（D185），派发在 `timeline::dispatch`。
//!
//! 字段口径（五个字段，全部是「纯数据」）：
//! - `who`：**谁**——动作 / 变化的主体（受伤的角色、行动的角色、使用技能的角色、被弃置的牌……）；
//! - `target`：**目标**——效果作用在谁身上（没有目标 -> `None`）；
//! - `amount`：**数值**——伤害 / 治疗 / 层数 / 充能……（不适用 -> `Nat::ZERO`）；
//! - `element`：**元素**——伤害 / 附着 / 反应涉及的元素（非元素事件 -> `None`）；
//! - `source`：**来源**——造成这次变化 / 施加这个效果的实体（没有来源 -> `None`）。
//!
//! 依据（规则书 / 卡面）：伤害与治疗都以「来源 -> 目标 + 数值 + 元素」为最小信息
//! （规则书·元素反应·规则：元素伤害会附着元素并引发元素反应；特殊词条 护盾：受护盾保护的角色
//! 「受到伤害时」用护盾值抵消），而结算顺序（规则书·其他规则·结算顺序）只需要知道
//! 「哪个牌手 / 哪类来源」，不需要更多字段。
//!
//! **不含**：
//! - 时机键：时机是牌族自己定义的类型（D185），由派发接口作为参数传入，事件里不重复存；
//! - 概率 / 随机：本层完全确定性；效果若分叉，概率由效果层用 `engine::prob::Fraction` 表达
//!   （分子 / 分母是 `ExactNat128`，全仓零浮点，D159 / D152 / D153）。

use crate::base::Element;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 一次触发对应的事件（**纯数据**、`Copy`）。
///
/// 字段都是公开的（D64：字段访问），由派发方（各牌族的数值操作）按实际发生的事填。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Event {
    /// 谁：动作 / 变化的主体。
    pub who: Option<EntityId>,
    /// 目标：效果作用在谁身上。
    pub target: Option<EntityId>,
    /// 数值：伤害 / 治疗 / 层数 / 充能……（不适用 -> `Nat::ZERO`）。
    pub amount: Nat,
    /// 元素：伤害 / 附着 / 反应涉及的元素。
    pub element: Option<Element>,
    /// 来源：造成这次变化 / 施加这个效果的实体。
    pub source: Option<EntityId>,
}

impl Event {
    /// 空事件：没有任何实体 / 数值 / 元素（阶段类时机常用）。
    pub const EMPTY: Event = Event {
        who: None,
        target: None,
        amount: Nat::ZERO,
        element: None,
        source: None,
    };

    /// 按五个字段组一个事件（参数顺序 = who / target / amount / element / source）。
    pub const fn new(
        who: Option<EntityId>,
        target: Option<EntityId>,
        amount: Nat,
        element: Option<Element>,
        source: Option<EntityId>,
    ) -> Event {
        Event {
            who,
            target,
            amount,
            element,
            source,
        }
    }

    /// 只有「谁」的事件：其余字段为空（单纯的动作类时机，如「使用技能后」）。
    pub const fn of(who: EntityId) -> Event {
        Event {
            who: Some(who),
            target: None,
            amount: Nat::ZERO,
            element: None,
            source: None,
        }
    }

    /// 只有「目标」的事件：其余字段为空（单纯的作用对象类时机，如「被治疗」）。
    pub const fn on(target: EntityId) -> Event {
        Event {
            who: None,
            target: Some(target),
            amount: Nat::ZERO,
            element: None,
            source: None,
        }
    }
}

impl Default for Event {
    /// 默认 = 空事件（与 [`Event::EMPTY`] 同一个值）。
    fn default() -> Event {
        Event::EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_event_has_no_entities_and_zero_amount() {
        assert_eq!(Event::EMPTY.who, None);
        assert_eq!(Event::EMPTY.target, None);
        assert_eq!(Event::EMPTY.amount, Nat::ZERO);
        assert_eq!(Event::EMPTY.element, None);
        assert_eq!(Event::EMPTY.source, None);
        assert_eq!(Event::default(), Event::EMPTY);
    }

    #[test]
    fn new_keeps_all_five_fields() {
        let event = Event::new(
            Some(Nat::new(1)),
            Some(Nat::new(2)),
            Nat::new(3),
            Some(Element::Pyro),
            Some(Nat::new(4)),
        );
        assert_eq!(event.who, Some(Nat::new(1)));
        assert_eq!(event.target, Some(Nat::new(2)));
        assert_eq!(event.amount, Nat::new(3));
        assert_eq!(event.element, Some(Element::Pyro));
        assert_eq!(event.source, Some(Nat::new(4)));
    }

    #[test]
    fn of_and_on_fill_only_their_slot() {
        let by = Event::of(Nat::new(7));
        assert_eq!(by.who, Some(Nat::new(7)));
        assert_eq!(by.target, None);
        assert_eq!(by.amount, Nat::ZERO);
        assert_eq!(by.element, None);
        assert_eq!(by.source, None);

        let on = Event::on(Nat::new(8));
        assert_eq!(on.who, None);
        assert_eq!(on.target, Some(Nat::new(8)));
    }

    #[test]
    fn event_is_a_plain_copy_value() {
        let event = Event::of(Nat::new(1));
        let copy = event;
        assert_eq!(event, copy);
        // 纯数据：改一个副本不影响原值。
        let mut changed = copy;
        changed.amount = Nat::new(9);
        assert_eq!(event.amount, Nat::ZERO);
        assert_ne!(changed, event);
    }
}
