//! 元素附着 + 元素反应**注册接口**（D127 / D128–D134 / D136 / D167）。
//!
//! 归属：D130 —— 元素反应挂在「角色 / 元素附着」之下，所以本文件同时放两者。
//!
//! **元素附着 = 允许集合**（D130）：由角色 / 卡牌的定义提供「哪些元素允许附着、
//! 能保存在角色身上」（静态定义层，[AttachmentAllowSet]）；运行期 [Attachment] 是
//! **元素布尔集合**（D167）：
//! - 每个元素单独记「是否附着」；**无层数、无计数**；
//! - **多元素可共存**（例如冰 + 草这类互不反应的可以同时存在）；
//! - 附着是**可被操作**的（D134：存在「附着自身 X 元素」这类效果）：
//!   [Attachment::attach] / [Attachment::detach]；
//! - 元素**不在允许集合内 -> 不落地**（attach 返回 `false`，集合不变），
//!   但**反应仍按已有附着判定**（D134）：拒绝附着不影响 [Attachment::matches_reaction]；
//! - 附着**不是状态**，不进通用 `StateList`（用户裁决：`StateList` 是「状态」用的）；
//! - 「本次攻击元素」**不落状态**，是反应判定 / 伤害结算的**入参**（用户裁决）。
//!
//! **反应消耗参与元素**（D167）由**伤害入口**完成（见 `damage.rs`）：命中反应时移除
//! 「命中的已覆盖元素 + 本次攻击元素」，本次攻击元素不再附着；本文件只提供
//! 「附着集合」与「反应判定 / 注册接口」，不自己扣附着。
//!
//! 反应模型（D128 / D129 / D132）：每条触发条件 = { 已覆盖: 单个元素, 本次攻击: 单个元素 }
//! （[ReactionCondition]）；条件列表可写多条（实际写 2 条，先后各一条），**命中任一条即触发**；
//! 空列表 = 永不触发（D128「省略条件 = 必定不能触发」）。多元素共存时，只要集合里
//! **有**该条件的「已覆盖元素」就算命中。
//!
//! **本文件只有接口，不实现任何具体反应**（超载 / 感电 / 结晶 … 的数值与效果在别的块）。
//! 注册支持 3 种方式（D136「实现方式不统一 -> 提供多种」）：
//! 1. [ReactionRegistry::register_by_kind]：按**反应种类**注册 —— 适用「与元素对无关、
//!    只要这个反应发生就要生效」的效果（转化类反应、反应计数等）；
//! 2. [ReactionRegistry::register_by_elements]：按**元素对**注册 —— 适用「明确知道
//!    已覆盖 X + 本次攻击 Y 才生效」的效果，**先后顺序敏感**（D132 实际写两条，先后各一条）；
//!    [ReactionRegistry::register_by_elements_both_orders] 是把一对元素的两种先后一次注册完。
//! 3. [ReactionRegistry::register_any]：**通用回调** —— 收到任意反应事件，适用全局统计 /
//!    日志 / 「本回合第一次反应」这类跨反应效果。
//!
//! **敌我双方触发的对应反应都算**（D136）：注册接口不区分敌我 —— 引擎也不分敌我
//! （D29 / D34），事件只带「谁触发（source）/ 作用在谁身上（target）」两个实体号；
//! 我方触发 / 敌方触发走同一套注册槽。
//!
//! 纯函数风格：附着操作吃 `self` 返回新附着（不改入参）；注册表只收集注册项，
//! `trigger` 时按「种类 -> 元素对 -> 通用」三段、段内按注册顺序调用（每条注册独立算一次）。

use crate::base::{Element, ELEMENTS};
use gi_genius_invokation_engine::{EntityId, Nat};

// ======================================================================
// 元素附着：允许集合（静态定义层，D130）
// ======================================================================

/// 标准可附着元素：火 / 水 / 雷 / 草 / 冰（D134：岩 / 风无法附着）。
pub const ATTACHABLE_ELEMENTS: [Element; 5] = [
    Element::Pyro,
    Element::Hydro,
    Element::Electro,
    Element::Dendro,
    Element::Cryo,
];

/// 天生无法附着的元素：风 / 岩（D134）。
///
/// 它们的附着请求**不落地**；但反应照常按该角色**已有附着**判定（D134）。
pub const NEVER_ATTACHABLE_ELEMENTS: [Element; 2] = [Element::Anemo, Element::Geo];

/// 这个元素是不是「天生无法附着」（风 / 岩，D134）。
pub fn is_inherently_unattachable(element: Element) -> bool {
    NEVER_ATTACHABLE_ELEMENTS.contains(&element)
}

/// 「哪些元素允许附着」的**允许集合**（静态定义层，D130）。
///
/// 由角色 / 卡牌的定义提供：定义数据决定哪些元素允许附着、能保存在角色身上。
/// 本层不解释「为什么允许」；默认集合见 [AttachmentAllowSet::STANDARD]。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttachmentAllowSet {
    elements: &'static [Element],
}

impl AttachmentAllowSet {
    /// 标准允许集合（[ATTACHABLE_ELEMENTS]：火 / 水 / 雷 / 草 / 冰）。
    pub const STANDARD: AttachmentAllowSet = AttachmentAllowSet {
        elements: &ATTACHABLE_ELEMENTS,
    };

    /// 空集合（什么都不允许附着；用于「不能附着任何元素」的自定义定义）。
    pub const NONE: AttachmentAllowSet = AttachmentAllowSet { elements: &[] };

    /// 由一份**静态**元素列表定义允许集合（角色 / 卡牌定义提供）。
    pub const fn new(elements: &'static [Element]) -> AttachmentAllowSet {
        AttachmentAllowSet { elements }
    }

    /// 这个元素是否允许附着。
    pub fn allows(self, element: Element) -> bool {
        self.elements.contains(&element)
    }

    /// 允许附着的全部元素（定义数据本身；不拷贝）。
    pub const fn elements(self) -> &'static [Element] {
        self.elements
    }
}

// ======================================================================
// 元素附着：当前附着（运行期，布尔集合）
// ======================================================================

/// 当前元素附着（D130 / D134 / D167）：**元素布尔集合**。
///
/// - 每个元素单独记「是否附着」；**无层数、无计数**（D167）；
/// - **多元素可共存**（冰 + 草这类互不反应的可以同时存在）；
/// - 不是状态，不进通用 `StateList`；「本次攻击元素」不在这里，是判定时的入参。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Attachment {
    /// 每个元素是否附着（下标顺序 = `base::ELEMENTS`，见 [element_slot]）。
    attached: [bool; 7],
}

/// 元素在「[元素; 7] 布尔集合」里的下标（= `base::ELEMENTS` 的固定顺序）。
const fn element_slot(element: Element) -> usize {
    match element {
        Element::Pyro => 0,
        Element::Hydro => 1,
        Element::Electro => 2,
        Element::Dendro => 3,
        Element::Cryo => 4,
        Element::Anemo => 5,
        Element::Geo => 6,
    }
}

impl Attachment {
    /// 空附着（没有任何元素）。
    pub const fn none() -> Attachment {
        Attachment {
            attached: [false; 7],
        }
    }

    /// 直接设定一组附着（建局 / 测试 / 定义数据用；**不检查允许集合**）。
    ///
    /// 正常游戏流程请走 [Attachment::attach]（它按允许集合判定是否落地）。
    pub fn from_elements(elements: &[Element]) -> Attachment {
        let mut next = Attachment::none();
        for element in elements {
            next.set(*element, true);
        }
        next
    }

    /// 直接设定一个附着（建局 / 测试 / 定义数据用；**不检查允许集合**）。
    pub fn with(element: Element) -> Attachment {
        Attachment::from_elements(&[element])
    }

    /// 这个元素**是否附着**（布尔集合；无计量）。
    pub fn has(self, element: Element) -> bool {
        self.attached[element_slot(element)]
    }

    /// 有没有任何附着。
    pub fn is_empty(self) -> bool {
        !self.attached.iter().any(|attached| *attached)
    }

    /// 当前附着的全部元素（按 `base::ELEMENTS` 的固定顺序；无计量，只列出「有」的元素）。
    pub fn elements(self) -> Vec<Element> {
        ELEMENTS
            .iter()
            .copied()
            .filter(|element| self.has(*element))
            .collect()
    }

    /// **附着某元素**（D134 / D167：布尔集合、可多元素共存）：返回 (新附着, 是否落地)。
    ///
    /// - 元素在允许集合内 -> 落地（该元素置「已附着」；**已附着时幂等**，不涨层数 / 不涨计数）；
    /// - 元素**不在**允许集合内 -> **不落地**，返回 `false`，集合保持不变
    ///   （D134：无法附着的元素不落地，但反应仍按已有附着判定）。
    ///
    /// 只做附着本身：**不判定、不触发反应**（反应判定见 [Attachment::matches_reaction]）。
    pub fn attach(self, element: Element, allow: AttachmentAllowSet) -> (Attachment, bool) {
        if !allow.allows(element) {
            return (self, false);
        }
        let mut next = self;
        next.set(element, true);
        (next, true)
    }

    /// **去掉某个元素的附着**（D134：附着可被操作）；其它元素不受影响。
    pub fn detach(self, element: Element) -> Attachment {
        let mut next = self;
        next.set(element, false);
        next
    }

    /// 去掉一组元素的附着（例如反应消耗参与元素）；其它元素不受影响。
    pub fn remove_all(self, elements: &[Element]) -> Attachment {
        let mut next = self;
        for element in elements {
            next.set(*element, false);
        }
        next
    }

    /// 清空全部附着。
    pub fn clear(self) -> Attachment {
        Attachment::none()
    }

    /// **反应判定**（D131 / D132）：按**已有附着集合** + **本次攻击元素**，看条件列表是否命中。
    ///
    /// 命中口径：条件列表里**任一条**同时满足
    /// 「已覆盖元素在附着集合里（`has(covered)`）」「本次攻击 == 本次攻击元素」即命中
    /// （D132：列表可写多条，命中任一即触发）。
    /// - 空条件列表 -> `false`（D128「省略条件 = 必定不能触发」）；
    /// - 与「这次能不能附着」无关：无法附着的元素也照这样按已有附着判定（D134）。
    pub fn matches_reaction(self, attack: Element, conditions: &[ReactionCondition]) -> bool {
        self.matching_covered(attack, conditions).is_some()
    }

    /// **反应判定并返回命中的「已覆盖元素」**（D132 / D167）：
    /// 条件列表按顺序找第一条命中的，返回它的 `covered`。
    ///
    /// 多元素共存时：返回**命中的那一个**已覆盖元素（它是本次反应的参与元素之一）；
    /// 没命中 -> `None`。
    pub fn matching_covered(
        self,
        attack: Element,
        conditions: &[ReactionCondition],
    ) -> Option<Element> {
        conditions
            .iter()
            .find(|condition| condition.matches(&self, attack))
            .map(|condition| condition.covered)
    }

    /// 置 / 清一个元素的附着位（内部用；不检查允许集合）。
    fn set(&mut self, element: Element, value: bool) {
        self.attached[element_slot(element)] = value;
    }
}

// ======================================================================
// 元素反应：触发条件（D128 / D131 / D132）
// ======================================================================

/// 反应触发条件（D131 / D132）：单条 = 「已覆盖的**单个**元素 + 本次攻击的**单个**元素」。
///
/// 「列表」指**条件条目可以写多条**（实际写 2 条，先后各一条），不是「一个字段里放多个元素」。
/// 附着是布尔集合（D167）时，「已覆盖」只要求该元素**在集合里**。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ReactionCondition {
    /// 已覆盖：触发时该角色身上已有的附着元素（在附着集合里即算）。
    pub covered: Element,
    /// 本次攻击：本次攻击的元素。
    pub attack: Element,
}

impl ReactionCondition {
    /// 一条触发条件（先后固定：已覆盖 -> 本次攻击）。
    pub const fn new(covered: Element, attack: Element) -> ReactionCondition {
        ReactionCondition { covered, attack }
    }

    /// 同一对元素**两种先后**各一条（D132：实际写 2 条，先后各一条）。
    pub const fn both_orders(first: Element, second: Element) -> [ReactionCondition; 2] {
        [
            ReactionCondition::new(first, second),
            ReactionCondition::new(second, first),
        ]
    }

    /// 这条条件是否命中：已覆盖元素在 `attachment` 里，且本次攻击 == `attack`。
    pub fn matches(self, attachment: &Attachment, attack: Element) -> bool {
        attachment.has(self.covered) && self.attack == attack
    }
}

// ======================================================================
// 元素反应：注册接口（D127 / D136）—— 只做接口，不实现具体反应
// ======================================================================

/// 反应身份（与 `StateIdentity` 同款，D95 / D96）：具体反应模块定义自己的零大小类型 +
/// 一份静态实例；运行期身份 = 指向它的**那份数据**的地址（D57 / D58 口径）。
///
/// **本块不实现任何具体反应**（超载 / 感电 / 结晶 … 都定义在别的块）；
/// 注册表按**身份地址**比较，不建编号表、不用字符串匹配。
pub trait ReactionIdentity: 'static + std::fmt::Debug {
    /// 可读名字（调试 / 日志用；**不参与身份判断**）。
    fn name(&self) -> &'static str;

    /// **身份地址** = 指向该反应**定义数据**的指针（D57 / D58 口径）。
    ///
    /// 为什么不用 `&dyn ReactionIdentity` 的地址：零大小（ZST）静态实例**没有独立地址**
    /// （本仓库实测 rustc 1.98.1 下不同 ZST 静态实例地址相同）；反应身份目前都是 ZST
    /// （`pub struct Melt;` + `static MELT: Melt`），直接比 `&dyn` 会把所有反应判成同一个。
    ///
    /// 默认实现 = `name()` 返回的 `&'static str` 的**数据地址**：本仓库每个反应模块的名字
    /// 字面量各是一份独立静态数据，所以稳定且互不相同。将来反应有自己的定义数据
    /// （如 `ReactionDef`）时，实现应覆盖本方法返回那份定义的地址。
    fn identity_address(&self) -> *const () {
        self.name().as_ptr() as *const ()
    }
}

/// 两个反应身份是否是**同一份身份**（比较 `identity_address()`；D57 / D58）。
///
/// **不要**直接比 `&dyn ReactionIdentity`（`std::ptr::addr_eq`）：ZST 身份地址可能相同。
pub fn same_reaction_identity(
    left: &'static dyn ReactionIdentity,
    right: &'static dyn ReactionIdentity,
) -> bool {
    std::ptr::eq(left.identity_address(), right.identity_address())
}

/// 一次反应事件（接口入参；D136：敌我双方触发的对应反应都算）。
///
/// 不设敌我枚举（D29 / D34）：只带两个实体号 —— 调用方从「哪个具名状态实例」出发，
/// 就知道是哪一方；注册表对双方一视同仁。
#[derive(Debug)]
pub struct ReactionEvent {
    /// 反应身份（具体反应由反应表 / 卡牌定义给出；本块只做接口）。
    pub reaction: &'static dyn ReactionIdentity,
    /// 命中的已覆盖元素（多元素共存时只带「这条反应命中的那一个」，D167）。
    pub covered: Element,
    /// 本次攻击元素。
    pub attack: Element,
    /// 谁触发（例如发起攻击的角色实体号）。
    pub source: EntityId,
    /// 作用在谁身上（承载附着 / 反应的角色实体号）。
    pub target: EntityId,
}

impl ReactionEvent {
    /// 组一个反应事件（参数顺序：反应身份、已覆盖、本次攻击、谁触发、作用在谁身上）。
    pub const fn new(
        reaction: &'static dyn ReactionIdentity,
        covered: Element,
        attack: Element,
        source: EntityId,
        target: EntityId,
    ) -> ReactionEvent {
        ReactionEvent {
            reaction,
            covered,
            attack,
            source,
            target,
        }
    }
}

/// 「反应时」触发接口（D127）：注册进来的实现方实现它。
///
/// 本层只负责**通知 / 触发**，不定义效果；实现方在 `on_reaction` 里做自己的事。
pub trait ReactionHandler: 'static + std::fmt::Debug {
    /// 收到一次反应事件。
    fn on_reaction(&self, event: &ReactionEvent);
}

/// 「反应时」注册表（D127 / D136）：三种注册方式并存，互不影响。
///
/// - 同一事件可能命中多条注册（同一 handler 注册多次就触发多次，**不去重**）；
/// - 触发顺序见 [ReactionRegistry::trigger]。
#[derive(Debug, Default)]
pub struct ReactionRegistry {
    by_kind: Vec<(&'static dyn ReactionIdentity, &'static dyn ReactionHandler)>,
    by_elements: Vec<(ReactionCondition, &'static dyn ReactionHandler)>,
    any: Vec<&'static dyn ReactionHandler>,
}

impl ReactionRegistry {
    /// 空注册表。
    pub const fn new() -> ReactionRegistry {
        ReactionRegistry {
            by_kind: Vec::new(),
            by_elements: Vec::new(),
            any: Vec::new(),
        }
    }

    /// **① 按反应种类注册**（D136 多种接口之一）。
    ///
    /// 只要事件的反应身份是这个（指针相同）就触发，**不看元素对**。
    /// 适用：与元素对无关、只要这个反应发生就要生效的效果（转化类反应、反应计数）。
    pub fn register_by_kind(
        &mut self,
        reaction: &'static dyn ReactionIdentity,
        handler: &'static dyn ReactionHandler,
    ) {
        self.by_kind.push((reaction, handler));
    }

    /// **② 按元素对注册**（D136 多种接口之一）。
    ///
    /// 条件命中（事件里的已覆盖 / 本次攻击都相等）就触发；**先后顺序敏感**。
    /// 适用：明确知道「已覆盖 X + 本次攻击 Y」才生效的效果。
    pub fn register_by_elements(
        &mut self,
        condition: ReactionCondition,
        handler: &'static dyn ReactionHandler,
    ) {
        self.by_elements.push((condition, handler));
    }

    /// **② 按元素对注册（两种先后一次写完）**：等价于对
    /// [ReactionCondition::both_orders] 的两条各调一次 [ReactionRegistry::register_by_elements]。
    ///
    /// 适用：多数反应两种先后顺序都该触发（D132：实际写 2 条，先后各一条）。
    pub fn register_by_elements_both_orders(
        &mut self,
        first: Element,
        second: Element,
        handler: &'static dyn ReactionHandler,
    ) {
        for condition in ReactionCondition::both_orders(first, second) {
            self.register_by_elements(condition, handler);
        }
    }

    /// **③ 通用回调注册**（D136 多种接口之一）。
    ///
    /// 收到**任意**反应事件都触发（不看反应身份、不看元素对）。
    /// 适用：全局统计 / 日志 / 「本回合第一次反应」这类跨反应效果。
    pub fn register_any(&mut self, handler: &'static dyn ReactionHandler) {
        self.any.push(handler);
    }

    /// 这个事件会触发哪些 handler（**只读**，不调用）。
    ///
    /// 顺序固定：按种类注册的（注册顺序）-> 按元素对注册的（注册顺序）-> 通用回调（注册顺序）。
    pub fn matching(&self, event: &ReactionEvent) -> Vec<&'static dyn ReactionHandler> {
        let mut matched: Vec<&'static dyn ReactionHandler> = Vec::new();
        for (reaction, handler) in &self.by_kind {
            if same_reaction_identity(*reaction, event.reaction) {
                matched.push(*handler);
            }
        }
        for (condition, handler) in &self.by_elements {
            if condition.covered == event.covered && condition.attack == event.attack {
                matched.push(*handler);
            }
        }
        matched.extend(self.any.iter().copied());
        matched
    }

    /// **触发**：按 [ReactionRegistry::matching] 的顺序逐个调用，返回被调用的 handler 个数（`Nat`）。
    ///
    /// 敌我双方的事件走同一张表（D136）：注册表不区分 source / target 是哪一方。
    pub fn trigger(&self, event: &ReactionEvent) -> Nat {
        let matched = self.matching(event);
        for handler in &matched {
            handler.on_reaction(event);
        }
        Nat::new(matched.len() as _)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 测试用反应身份（真实反应由具体卡牌 / 反应表定义，本块不实现）。
    #[derive(Debug)]
    struct SampleReaction(&'static str);

    impl ReactionIdentity for SampleReaction {
        fn name(&self) -> &'static str {
            self.0
        }
    }

    static VAPORIZE: SampleReaction = SampleReaction("蒸发");
    static OVERLOAD: SampleReaction = SampleReaction("超载");

    /// 测试用触发器：只数被调用次数（证明「注册 -> 触发」链路通）。
    #[derive(Debug)]
    struct Counter(&'static AtomicUsize);

    impl ReactionHandler for Counter {
        fn on_reaction(&self, _event: &ReactionEvent) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// 组一个默认事件（source = 1，target = 2）。
    fn event(
        reaction: &'static dyn ReactionIdentity,
        covered: Element,
        attack: Element,
    ) -> ReactionEvent {
        ReactionEvent::new(reaction, covered, attack, Nat::new(1), Nat::new(2))
    }

    // ------------------------------------------------------------------
    // 允许集合 / 附着（D167：布尔集合、多元素共存、无计量）
    // ------------------------------------------------------------------

    #[test]
    fn standard_allow_set_is_the_five_attachable_elements() {
        let allow = AttachmentAllowSet::STANDARD;
        for element in ATTACHABLE_ELEMENTS {
            assert!(allow.allows(element), "{element:?} 应该允许附着");
        }
        for element in NEVER_ATTACHABLE_ELEMENTS {
            assert!(!allow.allows(element), "{element:?} 天生无法附着（D134）");
            assert!(is_inherently_unattachable(element));
        }
        assert_eq!(allow.elements(), ATTACHABLE_ELEMENTS.as_slice());
        assert_eq!(AttachmentAllowSet::NONE.elements().len(), 0);
        assert!(!AttachmentAllowSet::NONE.allows(Element::Pyro));
    }

    #[test]
    fn custom_allow_set_is_definition_level() {
        // 允许集合是静态定义层：自定义角色可以给出与标准不同的集合。
        static GEO_ONLY: [Element; 1] = [Element::Geo];
        let custom = AttachmentAllowSet::new(&GEO_ONLY);
        assert!(custom.allows(Element::Geo));
        assert!(!custom.allows(Element::Pyro));
    }

    #[test]
    fn attachment_is_a_boolean_set_with_multiple_elements() {
        // D167：元素只按「是否附着」存，无计量；多元素可共存（冰 + 草）。
        let allow = AttachmentAllowSet::STANDARD;
        let (ice, landed) = Attachment::none().attach(Element::Cryo, allow);
        assert!(landed);
        assert!(ice.has(Element::Cryo));
        assert_eq!(ice.elements(), vec![Element::Cryo]);
        // 再附着草：不覆盖冰，两个元素共存。
        let (both, landed) = ice.attach(Element::Dendro, allow);
        assert!(landed);
        assert!(both.has(Element::Cryo));
        assert!(both.has(Element::Dendro));
        assert_eq!(both.elements(), vec![Element::Dendro, Element::Cryo]);
        // 重复附着同一元素：布尔集合不变（无层数 / 无计数）。
        let (again, landed) = both.attach(Element::Cryo, allow);
        assert!(landed);
        assert_eq!(again, both);
        assert_eq!(again.elements().len(), 2);
    }

    #[test]
    fn non_allowed_element_does_not_land_and_keeps_existing_attachment() {
        let allow = AttachmentAllowSet::STANDARD;
        let attached = Attachment::with(Element::Hydro);
        // 风 / 岩：附着请求不落地（D134）。
        for element in NEVER_ATTACHABLE_ELEMENTS {
            let (after, landed) = attached.attach(element, allow);
            assert!(!landed);
            assert_eq!(after, attached, "拒绝附着时附着集合必须保持原值");
        }
        // 空允许集合：任何元素都不落地。
        let (after, landed) = attached.attach(Element::Pyro, AttachmentAllowSet::NONE);
        assert!(!landed);
        assert!(after.has(Element::Hydro));
        assert!(!after.has(Element::Pyro));
    }

    #[test]
    fn rejected_attachment_does_not_stop_reaction_check() {
        // D134：附着被拒 -> 不落地，但反应**仍按已有附着判定**。
        let allow = AttachmentAllowSet::STANDARD;
        let attached = Attachment::with(Element::Hydro);
        let (after_rejected, landed) = attached.attach(Element::Geo, allow);
        assert!(!landed);
        let vaporize = ReactionCondition::both_orders(Element::Hydro, Element::Pyro);
        // 已有水附着 + 本次火 -> 蒸发条件命中，与「岩附着被拒」无关。
        assert!(after_rejected.matches_reaction(Element::Pyro, &vaporize));
        assert!(!after_rejected.matches_reaction(Element::Electro, &vaporize));
    }

    #[test]
    fn detach_removes_only_that_element_and_clear_removes_all() {
        let both = Attachment::from_elements(&[Element::Pyro, Element::Cryo]);
        let without_pyro = both.detach(Element::Pyro);
        assert!(!without_pyro.has(Element::Pyro));
        assert!(without_pyro.has(Element::Cryo));
        assert_ne!(without_pyro, both);
        // 去掉不存在的元素：不变（幂等）。
        assert_eq!(both.detach(Element::Dendro), both);
        // 清空全部。
        assert!(both.clear().is_empty());
        assert!(Attachment::none().is_empty());
        assert!(!both.is_empty());
    }

    #[test]
    fn remove_all_removes_participating_elements_only() {
        // D167：反应消耗「参与元素」，其余共存元素保留。
        let with_three =
            Attachment::from_elements(&[Element::Cryo, Element::Dendro, Element::Hydro]);
        let after = with_three.remove_all(&[Element::Cryo, Element::Hydro]);
        assert_eq!(after.elements(), vec![Element::Dendro]);
    }

    // ------------------------------------------------------------------
    // 触发条件（多元素集合下）
    // ------------------------------------------------------------------

    #[test]
    fn reaction_conditions_match_against_the_attached_set() {
        let both = ReactionCondition::both_orders(Element::Hydro, Element::Pyro);
        // 集合里「有」已覆盖元素即命中（多元素共存不影响）。
        assert!(Attachment::with(Element::Hydro).matches_reaction(Element::Pyro, &both));
        assert!(Attachment::from_elements(&[Element::Cryo, Element::Hydro])
            .matches_reaction(Element::Pyro, &both));
        // 集合里没有已覆盖元素 -> 不命中。
        assert!(!Attachment::from_elements(&[Element::Cryo]).matches_reaction(Element::Pyro, &both));
        // 空条件列表 = 永不触发（D128：省略条件 = 必定不能触发）。
        assert!(!Attachment::with(Element::Hydro).matches_reaction(Element::Pyro, &[]));
        // 没有附着 -> 任何条件都不命中。
        assert!(!Attachment::none().matches_reaction(Element::Pyro, &both));
    }

    #[test]
    fn matching_covered_returns_the_element_that_reacted() {
        // D167：多元素共存时，返回命中的那一个已覆盖元素（反应要消耗它）。
        let melt = ReactionCondition::both_orders(Element::Cryo, Element::Pyro);
        let attached = Attachment::from_elements(&[Element::Dendro, Element::Cryo]);
        assert_eq!(
            attached.matching_covered(Element::Pyro, &melt),
            Some(Element::Cryo)
        );
        // 不命中 -> None。
        assert_eq!(attached.matching_covered(Element::Hydro, &melt), None);
    }

    // ------------------------------------------------------------------
    // 注册接口：三种方式都能注册与触发
    // ------------------------------------------------------------------

    #[test]
    fn register_by_kind_fires_only_for_that_reaction_kind() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&VAPORIZE, &HANDLER);
        // 反应种类命中 -> 触发一次。
        assert_eq!(
            registry.trigger(&event(&VAPORIZE, Element::Hydro, Element::Pyro)),
            Nat::new(1)
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        // 别的反应种类 -> 不触发（按种类注册不看元素对）。
        assert_eq!(
            registry.trigger(&event(&OVERLOAD, Element::Pyro, Element::Electro)),
            Nat::ZERO
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    /// 零大小测试反应身份：自身没有独立地址，身份靠 `name()` 的数据地址区分。
    macro_rules! zst_reaction {
        ($ty:ident, $name:literal) => {
            #[derive(Debug)]
            struct $ty;

            impl ReactionIdentity for $ty {
                fn name(&self) -> &'static str {
                    $name
                }
            }
        };
    }

    zst_reaction!(ZstReactionA, "零反应A");
    zst_reaction!(ZstReactionB, "零反应B");
    zst_reaction!(ZstReactionC, "零反应C");

    static ZST_REACTION_A: ZstReactionA = ZstReactionA;
    static ZST_REACTION_B: ZstReactionB = ZstReactionB;
    static ZST_REACTION_C: ZstReactionC = ZstReactionC;

    #[test]
    fn different_zst_reaction_identities_are_not_equal() {
        // 三个不同 ZST 反应身份：不能拿 `&dyn` 地址当身份，必须用 `identity_address()`。
        assert_eq!(std::mem::size_of::<ZstReactionA>(), 0);
        assert!(!same_reaction_identity(&ZST_REACTION_A, &ZST_REACTION_B));
        assert!(!same_reaction_identity(&ZST_REACTION_A, &ZST_REACTION_C));
        assert!(!same_reaction_identity(&ZST_REACTION_B, &ZST_REACTION_C));
        assert!(same_reaction_identity(&ZST_REACTION_A, &ZST_REACTION_A));
    }

    #[test]
    fn same_static_reaction_identity_matches_via_two_paths() {
        // 同一 static 经两条路径（直接取址 / 经函数返回 dyn 引用）拿到身份：必须相等。
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        fn direct() -> &'static dyn ReactionIdentity {
            &VAPORIZE
        }
        fn via_alias() -> &'static dyn ReactionIdentity {
            &VAPORIZE
        }
        let registered = direct();
        let event_kind = via_alias();
        assert!(
            same_reaction_identity(registered, event_kind),
            "同一反应身份经两条路径必须相等"
        );
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(registered, &HANDLER);
        assert_eq!(
            registry.trigger(&event(event_kind, Element::Hydro, Element::Pyro)),
            Nat::new(1)
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn register_by_elements_is_order_sensitive() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_elements(
            ReactionCondition::new(Element::Hydro, Element::Pyro),
            &HANDLER,
        );
        // 已有水 + 本次火 -> 触发。
        assert_eq!(
            registry.trigger(&event(&VAPORIZE, Element::Hydro, Element::Pyro)),
            Nat::new(1)
        );
        // 顺序反了 -> 不触发（D132：先后各写一条）。
        assert_eq!(
            registry.trigger(&event(&VAPORIZE, Element::Pyro, Element::Hydro)),
            Nat::ZERO
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn register_by_elements_both_orders_registers_two_entries() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_elements_both_orders(Element::Hydro, Element::Pyro, &HANDLER);
        // 两个先后方向各命中一条注册。
        assert_eq!(
            registry.trigger(&event(&VAPORIZE, Element::Hydro, Element::Pyro)),
            Nat::new(1)
        );
        assert_eq!(
            registry.trigger(&event(&VAPORIZE, Element::Pyro, Element::Hydro)),
            Nat::new(1)
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn register_any_receives_every_reaction() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_any(&HANDLER);
        assert_eq!(
            registry.trigger(&event(&VAPORIZE, Element::Hydro, Element::Pyro)),
            Nat::new(1)
        );
        assert_eq!(
            registry.trigger(&event(&OVERLOAD, Element::Pyro, Element::Electro)),
            Nat::new(1)
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn reactions_from_both_sides_share_the_same_slots() {
        // D136：敌我双方触发的对应反应都算 —— 注册接口不区分敌我。
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&VAPORIZE, &HANDLER);
        // 我方打敌方：source = 1 -> target = 2。
        let ours = ReactionEvent::new(
            &VAPORIZE,
            Element::Hydro,
            Element::Pyro,
            Nat::new(1),
            Nat::new(2),
        );
        // 敌方打我方：source = 2 -> target = 1（同一套注册槽）。
        let theirs = ReactionEvent::new(
            &VAPORIZE,
            Element::Hydro,
            Element::Pyro,
            Nat::new(2),
            Nat::new(1),
        );
        assert_eq!(registry.trigger(&ours), Nat::new(1));
        assert_eq!(registry.trigger(&theirs), Nat::new(1));
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn three_registration_styles_coexist_and_matching_is_read_only() {
        static KIND_CALLS: AtomicUsize = AtomicUsize::new(0);
        static ELEMENT_CALLS: AtomicUsize = AtomicUsize::new(0);
        static ANY_CALLS: AtomicUsize = AtomicUsize::new(0);
        static KIND_HANDLER: Counter = Counter(&KIND_CALLS);
        static ELEMENT_HANDLER: Counter = Counter(&ELEMENT_CALLS);
        static ANY_HANDLER: Counter = Counter(&ANY_CALLS);

        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&VAPORIZE, &KIND_HANDLER);
        registry.register_by_elements(
            ReactionCondition::new(Element::Hydro, Element::Pyro),
            &ELEMENT_HANDLER,
        );
        registry.register_any(&ANY_HANDLER);

        let ev = event(&VAPORIZE, Element::Hydro, Element::Pyro);
        // matching 只读：这次会触发 3 个 handler，但一个都还没调用。
        assert_eq!(registry.matching(&ev).len(), 3);
        assert_eq!(KIND_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(ELEMENT_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(ANY_CALLS.load(Ordering::SeqCst), 0);
        // trigger 在 matching 的基础上逐个调用。
        assert_eq!(registry.trigger(&ev), Nat::new(3));
        assert_eq!(KIND_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(ELEMENT_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(ANY_CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn same_handler_registered_multiple_ways_fires_multiple_times() {
        // 三条注册独立算：同一 handler 注册 3 种方式 -> 同一事件触发 3 次（不去重）。
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&VAPORIZE, &HANDLER);
        registry.register_by_elements(
            ReactionCondition::new(Element::Hydro, Element::Pyro),
            &HANDLER,
        );
        registry.register_any(&HANDLER);
        assert_eq!(
            registry.trigger(&event(&VAPORIZE, Element::Hydro, Element::Pyro)),
            Nat::new(3)
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn event_carries_source_and_target_entities() {
        // D136：入参带「谁触发 / 作用在谁身上」；引擎不分敌我（D29/D34），只给实体号。
        let ev = event(&VAPORIZE, Element::Hydro, Element::Pyro);
        assert_eq!(ev.source, Nat::new(1));
        assert_eq!(ev.target, Nat::new(2));
        assert_eq!(ev.covered, Element::Hydro);
        assert_eq!(ev.attack, Element::Pyro);
        assert_eq!(ev.reaction.name(), "蒸发");
    }
}
