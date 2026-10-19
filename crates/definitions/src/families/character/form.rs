//! **转换形态**（覆盖报告 R4）：形态抽象 + 纯转换 + 转换时机的派发。
//!
//! ## 依据（唯一来源）
//!
//! 规则书·其他规则 **转换形态**：
//! 「部分角色可以通过一些效果『转换形态』。转换为其他形态后，角色的名称、元素类型、标签、技能等
//! 都有可能发生改变。（在冒险挑战中的敌人，转换形态后会重新生成行动意图。）」
//!
//! 权威关键字（`generated/data/keywords.json`）：
//! - **K1013「『焚尽的炽炎魔女』」**：「『女士』可以永久地转换为『焚尽的炽炎魔女』形态。转换后，
//!   『女士』的元素类型会从冰元素转换为火元素。转换后，此角色可以使用『焚尽的炽炎魔女』的技能，
//!   但是不再能使用『女士』原本的技能。」
//! - **K1014「『失能形态』」**：「……转换为『失能形态』。……处于『失能形态』时无法使用原本的技能，
//!   只能使用『失能形态』的技能。『失能形态』下的技能不会造成元素伤害，只能造成物理伤害。」
//! - **K1012「汲取对应元素的力量」**：「若陀龙王……根据所汲取的元素类型，获得技能……」
//!   —— 技能列表变化也是形态变化的一种（技能**替换 / 按形态给**）。
//!
//! 卡牌数据（只读依据，**本模块不导入卡牌数据**）：
//! `tcgcharactercards/lasignora.json` 的 `transformsinto: [6301]`；
//! `crimsonwitchofembers.json` 的 `istransformation: true`（**转换后的形态本身也是一张角色卡**）；
//! `azhdaha*.json` 的 4 个汲取形态 6601~6604（元素标签仍是岩、只换技能 —— 形态的「元素类型」
//! 与「技能造成什么元素伤害」是两件事）。
//!
//! ## 语义口径
//!
//! - **形态 = 一套（名称 / 元素类型 / 标签 / 技能列表）的替换**；
//! - `FormDef` 是**定义数据**（静态、只此一份）；`Form` 只是**指向定义**的只读指针载体：
//!   不内联、不拷贝名称 / 标签 / 技能数据（技能列表直接复用同一条 `&'static [&'static SkillDef]`）；
//! - 转换后**所有读接口都走当前形态**（名称 / 元素 / 标签 / 技能 / 按技能 id 查找）；
//! - `FormState::transform_to` 是**纯函数**：吃 `&self`、返回新状态、不改入参；
//! - 转换**是否发生**由效果决定（效果持有目标形态），本层不判断合法性（规则书没有「形态图」条规）；
//!   「恢复基础形态」用 `FormState::restore`（失能形态这类可逆性的判定见「待复核」）。
//!
//! ## 标签为什么是 `&'static [&'static TagText]` 而不是类型元组
//!
//! D54 的标签是零大小类型，但**转换形态会换标签集合**：类型级元组在运行期无法替换（类型不同）。
//! 所以形态的标签按 **D67 的运行期身份**存：`&'static TagText` 指针切片（`&TAGS.<字段>`）。
//! 这与 `families::action_card::zones::query` 的标签筛选是**同一口径**：指针比较、不用数字 /
//! 字符串键、不用 `HashMap`；标签文本数据仍然只有一份（`generated::TAGS`），这里只存指针。
//!
//! ## 时机派发（复用 `timeline`）
//!
//! - 时机键 = **`CharacterTiming::FormTransformed`**（角色族自己定义的键，D185；通用层不认识它）；
//! - 派发 = `timeline::dispatch` 的薄封装 [`dispatch_form`]：只排序、不执行（D186）；
//! - 效果签名 = [`FormEffect`] = `fn(&mut FormState, &Event, EntityId)`：第三个参数 = `Pending.owner`，
//!   实例过滤约定与 `trigger.rs` 相同（用 owner 判断是不是本次事件主体）；
//! - **与 `CharacterTriggers` 分开登记**：形态效果要读 / 改当前形态，而 `CharacterEffect` 只拿到
//!   `CharacterState`。待接入：统一「角色状态 + 形态」上下文（见文件末「待接入」）。
//!
//! ## 待接入 / 待复核
//!
//! - **待接入**：`CharacterState` 增加当前形态字段（`character.rs` 本轮不在可写范围，故本文件提供
//!   独立的 [`FormState`] 作为该字段的载体；接入时把 `FormState` 作为角色状态 / 槽位的伴随字段，
//!   或把两个 `Form` 字段搬进 `CharacterState`）；
//! - **待接入**：形态触发注册表与 `CharacterTriggers` 合并（需要统一的角色上下文类型）；
//! - **待复核**：基础形态的 `name` / `skills` 与角色牌定义（`CharacterDef`）是**同一份数据的两处
//!   引用**：`FormDef.skills` 直接用同一条 `&'static [&'static SkillDef]`（不拷贝技能数据），
//!   名称是取值拷贝。接入时可让基础形态**直接指向** `CharacterDef`（本轮不这么做：`CharacterDef`
//!   没有元素字段、标签是泛型元组，无法在非泛型形态里引用）；
//! - **待复核**：转换时机的事件只有五字段（`timeline::Event`），**装不下形态身份**：条件函数只能按
//!   元素 / 主体筛选；「从哪个形态到哪个形态」由效果函数读 `FormState` 判断；
//! - **待复核**：K1014 失能形态是否可逆（卡面数据未入库该关键字的具体卡），`restore` 只提供机制。

use crate::base::{CardId, Element, SkillId};
use crate::families::character::skill::SkillDef;
use crate::families::character::trigger::CharacterTiming;
use crate::generated::{Localized, TagText};
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::{EntityId, Nat};

/// 一个**形态的定义数据**（静态、只此一份）。
///
/// 只放「转换时会被替换的那一套」：名称 / 元素类型 / 标签 / 技能列表 + 识别信息；
/// 生命 / 充能上限不在形态里（规则书只说名称 / 元素类型 / 标签 / 技能「有可能改变」）。
#[derive(Clone, Copy, Debug)]
pub struct FormDef {
    /// 形态对应的角色牌 id：转换后的形态本身也是一张角色卡（数据 `istransformation: true`，
    /// 如「焚尽的炽炎魔女」= 6301）；基础形态就是角色自己的 card id。
    pub card_id: CardId,
    /// 形态名称（转换后可能不同：「女士」-> 「焚尽的炽炎魔女」）。
    pub name: Localized,
    /// 形态的**元素类型**（转换后可能不同：冰 -> 火）。
    ///
    /// 注意与「技能造成什么元素伤害」分开：若陀龙王的汲取形态元素标签仍是岩、只是换技能。
    pub element: Element,
    /// 形态的标签（D67 运行期身份：`&TAGS.<字段>` 指针切片，不拷贝标签数据）。
    pub tags: &'static [&'static TagText],
    /// 形态的技能列表（**替换**而非追加；技能数据只有一份，这里复用同一条静态切片）。
    pub skills: &'static [&'static SkillDef],
    /// 这个形态是不是**转换形态**（数据 `istransformation`）；基础形态为 `false`。
    pub is_transformation: bool,
}

/// 一个**形态**（数据载体）：只持有指向 [`FormDef`] 的静态引用，**不内联、不拷贝**定义数据。
///
/// 相等 = **同一份静态定义**（指针相等；D67 口径：运行期身份 = 地址）。
#[derive(Clone, Copy, Debug)]
pub struct Form {
    /// 形态定义（静态、只此一份）。
    def: &'static FormDef,
}

impl Form {
    /// 用一份静态形态定义建形态载体（只存指针）。
    pub const fn new(def: &'static FormDef) -> Form {
        Form { def }
    }

    /// 形态定义（**只读指针**，数据只有一份）。
    pub const fn def(self) -> &'static FormDef {
        self.def
    }

    /// 形态对应的角色牌 id。
    pub const fn card_id(self) -> CardId {
        self.def.card_id
    }

    /// 形态名称（读**当前形态**，不是基础形态）。
    pub const fn name(self) -> Localized {
        self.def.name
    }

    /// 形态的元素类型（读**当前形态**）。
    pub const fn element(self) -> Element {
        self.def.element
    }

    /// 形态的标签（只读指针切片；D67 运行期身份）。
    pub const fn tags(self) -> &'static [&'static TagText] {
        self.def.tags
    }

    /// 是否有某个标签（**指针比较**，与查询层同口径；不用名字 / 编号）。
    pub fn has_tag(self, tag: &'static TagText) -> bool {
        self.tags()
            .iter()
            .any(|candidate| std::ptr::eq(*candidate, tag))
    }

    /// 形态的技能列表（只读；顺序 = 展示 / 使用顺序：普攻、战技、爆发、被动）。
    pub const fn skills(self) -> &'static [&'static SkillDef] {
        self.def.skills
    }

    /// 按技能 id 取技能定义（**失败 -> `None`**：这个形态没有这个技能）。
    pub fn skill(self, id: SkillId) -> Option<&'static SkillDef> {
        self.skills()
            .iter()
            .copied()
            .find(|skill| skill.id == id)
    }

    /// 这个形态是否有某个技能（K1013 / K1014：转换后只能用转换后的技能）。
    pub fn has_skill(self, id: SkillId) -> bool {
        self.skill(id).is_some()
    }

    /// 是不是转换形态（数据 `istransformation`）。
    pub const fn is_transformation(self) -> bool {
        self.def.is_transformation
    }
}

impl PartialEq for Form {
    /// 相等 = 同一份静态形态定义（**指针相等**；数据只有一份）。
    fn eq(&self, other: &Form) -> bool {
        std::ptr::eq(self.def, other.def)
    }
}

impl Eq for Form {}

/// 一个角色的**形态状态**：基础形态 + 当前形态（对应「角色当前形态字段」）。
///
/// 基础形态 = 角色原本那张角色卡的形态；当前形态 = 转换之后的形态（未转换时两者相同）。
///
/// 纯函数风格：`transform_to` / `restore` 都吃 `&self`、返回新状态、不改入参。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormState {
    /// 基础形态（角色原本的角色卡）。
    base: Form,
    /// 当前形态（转换后；未转换时 = `base`）。
    current: Form,
}

impl FormState {
    /// 建一个**未转换**的形态状态（基础形态 = 当前形态）。
    pub const fn new(base: Form) -> FormState {
        FormState {
            base,
            current: base,
        }
    }

    /// 基础形态（只读）。
    pub const fn base(self) -> Form {
        self.base
    }

    /// 当前形态（只读）。
    pub const fn current(self) -> Form {
        self.current
    }

    /// 是否处于转换后的形态（当前形态与基础形态不是同一份定义）。
    pub fn is_transformed(self) -> bool {
        self.current != self.base
    }

    /// **转换到某个形态**（纯函数：返回新状态，不改入参）。
    ///
    /// 转换**是否允许**由效果层判断（效果持有目标形态）；本函数只做状态替换。
    /// 转换后名称 / 元素 / 标签 / 技能的所有读接口都走新形态。
    pub const fn transform_to(self, form: Form) -> FormState {
        FormState {
            base: self.base,
            current: form,
        }
    }

    /// **恢复基础形态**（纯函数；给可逆形态用，如失能形态 —— 可逆性由卡面效果决定，待复核）。
    pub const fn restore(self) -> FormState {
        FormState {
            base: self.base,
            current: self.base,
        }
    }

    /// 当前形态对应的角色牌 id。
    pub const fn card_id(self) -> CardId {
        self.current.card_id()
    }

    /// 当前形态的名称（读**当前形态**：转换后名字变了，这里就变了）。
    pub const fn name(self) -> Localized {
        self.current.name()
    }

    /// 当前形态的元素类型（读**当前形态**：转换后元素变了，这里就变了）。
    pub const fn element(self) -> Element {
        self.current.element()
    }

    /// 当前形态的标签（读**当前形态**）。
    pub const fn tags(self) -> &'static [&'static TagText] {
        self.current.tags()
    }

    /// 当前形态是否有某个标签。
    pub fn has_tag(self, tag: &'static TagText) -> bool {
        self.current.has_tag(tag)
    }

    /// 当前形态的技能列表（读**当前形态**：转换后只能用转换后的技能）。
    pub const fn skills(self) -> &'static [&'static SkillDef] {
        self.current.skills()
    }

    /// 按技能 id 在当前形态里取技能（**失败 -> `None`**：原本技能在转换后不再可用）。
    pub fn skill(self, id: SkillId) -> Option<&'static SkillDef> {
        self.current.skill(id)
    }

    /// 当前形态是否有某个技能。
    pub fn has_skill(self, id: SkillId) -> bool {
        self.current.has_skill(id)
    }
}

// ======================================================================
// 转换时机的派发（复用 timeline；时机键 = CharacterTiming::FormTransformed）
// ======================================================================

/// 形态效果函数指针：`fn(&mut 形态状态, &Event, owner)`。
///
/// 与 `CharacterEffect`（`fn(&mut CharacterState, &Event, EntityId)`）同形，第三个参数 =
/// `Pending.owner`，供实例级过滤（同一张卡多实例只让属于主体那一条生效）。
pub type FormEffect = fn(&mut FormState, &Event, EntityId);

/// 形态的触发登记项（时机键 + 条件 + 效果 + 排序标识；D185 / D186）。
pub type FormTrigger = Trigger<CharacterTiming, FormEffect>;

/// 形态触发注册表（每个牌手一个**具名实例**，D34；不用全局单例）。
pub type FormTriggers = TriggerRegistry<CharacterTiming, FormEffect>;

/// 一条**待执行**（owned；事件按值带走）。由调用方执行（本层只排序）。
pub type FormPending = Queued<FormEffect>;

/// 一次带触发的形态转换结果：新形态状态 + 派发出来的待执行。
#[derive(Clone, Debug)]
pub struct FormResolution {
    /// 转换后的形态状态（与直接调 `transform_to` 的结果一致）。
    pub state: FormState,
    /// 派发出来的待执行（由调用方执行；本层不执行）。
    pub pending: Vec<FormPending>,
}

/// 「转换形态」事件（转换**之后**的事实）：`who` / `target` 都是这个角色，
/// `amount` = 0，`element` = 转换后的元素类型（供条件按元素筛选），`source` = 转换的来源。
///
/// 形态身份本身不在事件里（`timeline::Event` 只有五字段）：效果函数读 `FormState` 判断
/// 「从哪个形态到哪个形态」（待复核，见模块文档）。
pub fn transformed_event(
    owner: EntityId,
    new_element: Option<Element>,
    source: Option<EntityId>,
) -> Event {
    Event::new(Some(owner), Some(owner), Nat::ZERO, new_element, source)
}

/// 主动派发：从形态注册表里取出「这个时机、条件成立」的登记项，按结算顺序排成**待执行**。
///
/// 排序 / 条件 / 稳定顺序全部复用 `timeline::dispatch`（规则书·其他规则·结算顺序）；
/// 本函数只把借用的 `Pending` 转成 owned，**不执行效果**。
pub fn dispatch_form(
    triggers: &FormTriggers,
    timing: CharacterTiming,
    event: &Event,
) -> Vec<FormPending> {
    dispatch(triggers, timing, event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 执行一条待执行列表（**调用方显式调用**；`timeline` / 本模块的派发永不执行）。
pub fn execute_form(pending: &[FormPending], state: &mut FormState) {
    for item in pending {
        (item.effect)(state, &item.event, item.owner);
    }
}

/// **转换形态** + 派发 `CharacterTiming::FormTransformed`。
///
/// 顺序：先做转换（纯函数得到新状态），再派发「转换形态」时机（事件带转换后的元素类型）。
/// 派发只产出待执行列表；执行由调用方做（`execute_form`）。
pub fn transform_with_triggers(
    state: &FormState,
    owner: EntityId,
    form: Form,
    source: Option<EntityId>,
    triggers: &FormTriggers,
) -> FormResolution {
    let next = state.transform_to(form);
    let event = transformed_event(owner, Some(form.element()), source);
    let pending = dispatch_form(triggers, CharacterTiming::FormTransformed, &event);
    FormResolution {
        state: next,
        pending,
    }
}

/// **恢复基础形态** + 派发 `CharacterTiming::FormTransformed`（与 `transform_with_triggers` 对称）。
pub fn restore_with_triggers(
    state: &FormState,
    owner: EntityId,
    source: Option<EntityId>,
    triggers: &FormTriggers,
) -> FormResolution {
    let next = state.restore();
    let event = transformed_event(owner, Some(next.element()), source);
    let pending = dispatch_form(triggers, CharacterTiming::FormTransformed, &event);
    FormResolution {
        state: next,
        pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::SkillId;
    use crate::families::character::types::SkillKind;
    use crate::generated::{Lang, Resolved, TAGS};
    use crate::rules::ops::order::EffectSource;
    use crate::timeline::Event;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 测试用技能定义（效果体是占位 `fn()`，与 `skill.rs` 同口径）。
    fn noop() {}

    /// 基础形态的普攻技能。
    static BASE_SKILL: SkillDef = SkillDef {
        id: SkillId::new(90011),
        name: Localized::from_parts(Some("基础普攻"), None, None, None),
        description: Localized::EMPTY,
        kind: SkillKind::normal_attack(),
        cost: crate::families::character::skill::SkillCost::FREE,
        effect: noop,
    };

    /// 基础形态的元素战技。
    static BASE_SKILL_TWO: SkillDef = SkillDef {
        id: SkillId::new(90012),
        name: Localized::from_parts(Some("基础战技"), None, None, None),
        description: Localized::EMPTY,
        kind: SkillKind::ElementalSkill,
        cost: crate::families::character::skill::SkillCost::FREE,
        effect: noop,
    };

    /// 转换后形态的元素爆发（**id 不同 / 名称不同 / 种类不同**）。
    static FORM_SKILL: SkillDef = SkillDef {
        id: SkillId::new(90021),
        name: Localized::from_parts(Some("转换爆发"), None, None, None),
        description: Localized::EMPTY,
        kind: SkillKind::ElementalBurst,
        cost: crate::families::character::skill::SkillCost::FREE,
        effect: noop,
    };

    /// 基础形态标签：水元素 / 双手剑（与标签数据同一地址，D67）。
    static BASE_TAGS: [&TagText; 2] = [&TAGS.hydro, &TAGS.claymore];

    /// 转换形态标签：火元素 / 其他武器。
    static FORM_TAGS: [&TagText; 2] = [&TAGS.pyro, &TAGS.other_weapons];

    /// 基础形态的技能列表。
    static BASE_SKILLS: [&SkillDef; 2] = [&BASE_SKILL, &BASE_SKILL_TWO];

    /// 转换形态的技能列表（只有一条，且与基础形态完全不同）。
    static FORM_SKILLS: [&SkillDef; 1] = [&FORM_SKILL];

    /// 基础形态定义（样例数据，不是具体卡）。
    static BASE_FORM: FormDef = FormDef {
        card_id: CardId::new(9001),
        name: Localized::from_parts(Some("样例·基础形态"), None, None, None),
        element: Element::Hydro,
        tags: &BASE_TAGS,
        skills: &BASE_SKILLS,
        is_transformation: false,
    };

    /// 转换形态定义（样例数据：名称 / 元素 / 标签 / 技能全换）。
    static ALT_FORM: FormDef = FormDef {
        card_id: CardId::new(9002),
        name: Localized::from_parts(Some("样例·转换形态"), None, None, None),
        element: Element::Pyro,
        tags: &FORM_TAGS,
        skills: &FORM_SKILLS,
        is_transformation: true,
    };

    fn base() -> Form {
        Form::new(&BASE_FORM)
    }

    fn alt() -> Form {
        Form::new(&ALT_FORM)
    }

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 计数效果（只证明「派发不执行、执行由调用方做」）。
    static FORM_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn count_form_call(_state: &mut FormState, _event: &Event, _owner: EntityId) {
        FORM_CALLS.fetch_add(1, Ordering::SeqCst);
    }

    /// 实例过滤示例：只有「属于本次事件主体」的那条生效（owner 约定同 trigger.rs）。
    fn restore_if_subject(state: &mut FormState, event: &Event, owner: EntityId) {
        if event.who == Some(owner) {
            *state = state.restore();
        }
    }

    /// 条件：转换后的元素是火。
    fn pyro_only(event: &Event) -> bool {
        event.element == Some(Element::Pyro)
    }

    fn registry_with(owner: EntityId, effect: FormEffect) -> FormTriggers {
        let mut triggers = FormTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::FormTransformed,
            owner,
            EffectSource::CharacterCard,
            Nat::ZERO,
            effect,
        ));
        triggers
    }

    #[test]
    fn transform_switches_name_element_tags_and_skills() {
        let state = FormState::new(base());
        // 转换前：基础形态的四个读接口。
        assert_eq!(
            state.name().get(Lang::ZhCn),
            Resolved::Exact("样例·基础形态")
        );
        assert_eq!(state.element(), Element::Hydro);
        assert!(state.has_tag(&TAGS.hydro));
        assert!(!state.has_tag(&TAGS.pyro));
        assert_eq!(state.skills().len(), 2);
        assert!(state.has_skill(SkillId::new(90011)));
        assert!(!state.has_skill(SkillId::new(90021)));

        // 转换。
        let transformed = state.transform_to(alt());
        // 转换后：名称 / 元素 / 标签 / 技能**全部走当前形态**。
        assert_eq!(
            transformed.name().get(Lang::ZhCn),
            Resolved::Exact("样例·转换形态")
        );
        assert_eq!(transformed.element(), Element::Pyro);
        assert!(transformed.has_tag(&TAGS.pyro));
        assert!(transformed.has_tag(crate::tags::character::other_weapons::data()));
        assert!(!transformed.has_tag(&TAGS.hydro));
        assert_eq!(transformed.skills().len(), 1);
        assert!(transformed.has_skill(SkillId::new(90021)));
        assert!(!transformed.has_skill(SkillId::new(90011)));
        assert_eq!(transformed.card_id(), CardId::new(9002));
        assert!(transformed.is_transformed());
        assert!(!state.is_transformed());
    }

    #[test]
    fn transform_to_is_pure_and_does_not_touch_the_input() {
        let state = FormState::new(base());
        let _transformed = state.transform_to(alt());
        // 入参一个字节都不改（纯函数口径，与 character.rs / zones 一致）。
        assert_eq!(state.current(), base());
        assert_eq!(state.name().get(Lang::ZhCn), Resolved::Exact("样例·基础形态"));
        assert_eq!(state.element(), Element::Hydro);
        assert_eq!(state.skills().len(), 2);
        assert!(!state.is_transformed());
    }

    #[test]
    fn skill_lookup_is_fallible_and_form_scoped() {
        let state = FormState::new(base());
        // 基础形态：有 90011，没有 90021。
        let skill = state.skill(SkillId::new(90011)).expect("基础形态有这个技能");
        assert_eq!(skill.name.get(Lang::ZhCn), Resolved::Exact("基础普攻"));
        assert!(state.skill(SkillId::new(90021)).is_none());
        // 转换后：反过来。
        let transformed = state.transform_to(alt());
        let skill = transformed
            .skill(SkillId::new(90021))
            .expect("转换形态有这个技能");
        assert_eq!(skill.kind, SkillKind::ElementalBurst);
        assert!(transformed.skill(SkillId::new(90011)).is_none());
        // 两种形态都没有的 id -> None。
        assert!(transformed.skill(SkillId::new(99999)).is_none());
    }

    #[test]
    fn form_points_to_the_static_definition_without_copying() {
        // 数据只一份：`Form::def()` 返回的就是那份静态定义的地址。
        let form = base();
        assert!(std::ptr::eq(form.def(), &BASE_FORM));
        assert!(std::ptr::eq(form.def(), base().def())); // 同一个静态定义 = 相等
        assert_ne!(base(), alt());
        // 标签也复用同一份标签数据（D67：运行期身份 = 地址）。
        assert!(std::ptr::eq(form.tags()[0], &TAGS.hydro));
        assert!(std::ptr::eq(
            form.tags()[0],
            crate::tags::character::element::hydro::data()
        ));
    }

    #[test]
    fn is_transformation_flag_comes_from_the_definition() {
        assert!(!base().is_transformation());
        assert!(alt().is_transformation());
    }

    #[test]
    fn restore_returns_to_the_base_form() {
        let transformed = FormState::new(base()).transform_to(alt());
        assert!(transformed.is_transformed());
        let restored = transformed.restore();
        assert!(!restored.is_transformed());
        assert_eq!(restored.current(), base());
        assert_eq!(restored.name().get(Lang::ZhCn), Resolved::Exact("样例·基础形态"));
        assert_eq!(restored.element(), Element::Hydro);
        assert!(restored.has_skill(SkillId::new(90011)));
        assert!(!restored.has_skill(SkillId::new(90021)));
        // 多步转换：基础 -> 转换 -> 基础，base 一直是基础形态。
        assert_eq!(restored.base(), base());
    }

    #[test]
    fn transformed_event_carries_subject_element_and_source() {
        let event = transformed_event(n(1), Some(Element::Pyro), Some(n(2)));
        assert_eq!(event.who, Some(n(1)));
        assert_eq!(event.target, Some(n(1)));
        assert_eq!(event.amount, Nat::ZERO);
        assert_eq!(event.element, Some(Element::Pyro));
        assert_eq!(event.source, Some(n(2)));
    }

    #[test]
    fn transform_dispatches_but_does_not_execute() {
        FORM_CALLS.store(0, Ordering::SeqCst);
        let state = FormState::new(base());
        let triggers = registry_with(n(1), count_form_call);
        let resolution = transform_with_triggers(&state, n(1), alt(), None, &triggers);
        // 转换本身成立（数值语义 = transform_to）。
        assert_eq!(resolution.state, state.transform_to(alt()));
        // 派发只产出待执行；调用方执行之前效果一次都没跑。
        assert_eq!(resolution.pending.len(), 1);
        assert_eq!(resolution.pending[0].owner, n(1));
        assert_eq!(FORM_CALLS.load(Ordering::SeqCst), 0);
        // 执行由调用方做。
        let mut after = resolution.state;
        execute_form(&resolution.pending, &mut after);
        assert_eq!(FORM_CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dispatch_only_matches_the_transformed_timing() {
        let triggers = registry_with(n(1), restore_if_subject);
        let event = transformed_event(n(1), Some(Element::Pyro), None);
        assert_eq!(
            dispatch_form(&triggers, CharacterTiming::FormTransformed, &event).len(),
            1
        );
        // 其它时机不命中（派发按时机相等匹配）。
        assert!(dispatch_form(&triggers, CharacterTiming::HpChanged, &event).is_empty());
        // 空注册表 -> 无待执行，转换本身仍成立。
        let empty = FormTriggers::new();
        let resolution = transform_with_triggers(&FormState::new(base()), n(1), alt(), None, &empty);
        assert!(resolution.pending.is_empty());
        assert_eq!(resolution.state.current(), alt());
    }

    #[test]
    fn form_effect_can_observe_and_change_the_current_form() {
        // 效果函数拿到的就是「转换后」的形态状态；实例过滤用 owner（与 trigger.rs 同约定）。
        let state = FormState::new(base());
        // 注册在 1 号实例上：事件主体是 1 -> 生效（恢复基础形态）。
        let triggers = registry_with(n(1), restore_if_subject);
        let resolution = transform_with_triggers(&state, n(1), alt(), None, &triggers);
        let mut after = resolution.state;
        execute_form(&resolution.pending, &mut after);
        assert!(!after.is_transformed());
        assert_eq!(after.current(), base());
        // 注册在 2 号实例上：事件主体是 1 -> 跳过（形态保持转换后的）。
        let other = registry_with(n(2), restore_if_subject);
        let resolution = transform_with_triggers(&state, n(1), alt(), None, &other);
        let mut after = resolution.state;
        execute_form(&resolution.pending, &mut after);
        assert!(after.is_transformed());
        assert_eq!(after.current(), alt());
    }

    #[test]
    fn condition_can_filter_by_the_new_element() {
        let mut triggers = FormTriggers::new();
        triggers.register(Trigger::with_condition(
            CharacterTiming::FormTransformed,
            n(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            pyro_only,
            count_form_call,
        ));
        // 转到火 -> 条件成立。
        let to_pyro = transform_with_triggers(&FormState::new(base()), n(1), alt(), None, &triggers);
        assert_eq!(to_pyro.pending.len(), 1);
        // 恢复基础形态（水）-> 条件不成立。
        let to_base = restore_with_triggers(&to_pyro.state, n(1), None, &triggers);
        assert!(to_base.pending.is_empty());
        assert_eq!(to_base.state.element(), Element::Hydro);
    }

    #[test]
    fn restore_with_triggers_dispatches_and_is_pure() {
        let transformed = FormState::new(base()).transform_to(alt());
        let triggers = registry_with(n(1), count_form_call);
        let resolution = restore_with_triggers(&transformed, n(1), Some(n(3)), &triggers);
        assert!(!resolution.state.is_transformed());
        assert_eq!(resolution.state.current(), base());
        assert_eq!(resolution.pending.len(), 1);
        // 入参不变。
        assert!(transformed.is_transformed());
        assert_eq!(transformed.current(), alt());
    }

    #[test]
    fn static_trigger_table_registers_and_dispatches() {
        static TABLE: [FormTrigger; 1] = [FormTrigger::new(
            CharacterTiming::FormTransformed,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            count_form_call,
        )];
        let mut triggers = FormTriggers::new();
        triggers.register_all(&TABLE);
        assert_eq!(triggers.len(), n(1));
        let event = transformed_event(n(1), Some(Element::Pyro), None);
        let pending = dispatch_form(&triggers, CharacterTiming::FormTransformed, &event);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].event, event);
    }
}
