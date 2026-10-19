//! **手牌卡状态**（行动牌作为手牌时携带的状态；区域承载缺口之一）。
//!
//! ## 依据
//!
//! - 规则书·游戏区域 VI.手牌：「只有『手牌』中的行动牌可以被打出和用于元素调和。」——
//!   手牌里的牌是**同一张行动牌实例**，状态跟着这张牌走（D52 位置标记口径：标记跟着位置 / 牌走）。
//! - D104 / D105 / D112 / D113：减费 / 赋能这类「挂在行动牌上的状态」属于**行动牌**，
//!   通用状态列表（`StateList`）同时支持「布尔 / 必须带层数」两种形态；本层**复用**它，
//!   不重造第二套状态容器。
//!
//! ## 口径（D182 / D183 / D185 / D187）
//!
//! - 本层自持 `entries`（每个手牌实体一份 `StateList`）+ 统计；变量型、可建多个；
//! - **手牌无序**：所以每张手牌卡**一个** `StateList`，`pos` 固定 0（不表示任何顺序）；
//! - 对外只给**只读指针**（`entries()` / `states_of()` / `layers()` / `stats()`，D183）；
//! - 状态的增 / 删时机键由本层自己定义（D185），走 `timeline` 派发（只排序、不执行 D186）。
//!
//! ## 与手牌区的关系
//!
//! 手牌区（`action_card::zones::hand`）的 `HandState.entries` 只存「实体号 + 卡 id」；
//! 本层用**实体号**做键另外持状态，**不改手牌的容器**（手牌块的逻辑不动）。
//! 卡片离开手牌时调用 `remove_card`（状态跟牌一起走 / 被清理；跨区搬运由上层组合）。
//!
//! ## 待复核
//!
//! - 手牌卡上的状态**跨区搬运**（例如打出去时把减费状态带到场上）：规则书没写，
//!   先只做「手牌内」；要搬由调用方显式读 `states_of` 再写目标区。

use crate::families::action_card::areas::stats::HandCardStateStats;
use crate::families::action_card::status::{StateForm, StateIdentity, StateList, StateText};
use crate::generated::Localized;
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::{EntityId, Nat};

// ======================================================================
// 手牌状态身份（Z13：关键字数据 K3003–K3008 的 6 个手牌状态）
// ======================================================================
//
// 数据来源：generated/data/keywords.json。
// - K3003 电击 / K3005 费用降低：身份已由 `families::character::damage_chain::statuses` 提供
//   （电击布尔、费用降低层数；历史入口，未在本文件重复建实例，避免同一状态两份身份）；
// - K3004 费用增加 / K3006 赋能 / K3007 不可调和 / K3008 无效化：数据里**只有 4 语名字、
//   description 全部为 null**，本文件按「只填数据有的名字、描述留空、不编造」建身份。
//
// TODO / 待实测：
// - 4 个身份的**描述**在数据里不存在（null）——这里留空，待卡面/实测补；
// - 4 个身份的**形态**（布尔 / 层数）数据未给：费用增加按「与费用降低对称」取层数，
//   其余三个按布尔；若实测不同需调整；
// - K3003/K3005 的正式定义仍在 damage_chain 桥模块（D112 迁移归行动牌状态块，待后续）；
//   本文件只补缺的 4 个，并在查询枚举中给同一套 K 码，不重复建已有身份。

/// 手牌状态「费用增加」（K3004）的文本。
///
/// 数据只有名字（4 语），description 为 null -> 描述留空（不编造）。
static HAND_COST_INCREASE_TEXT: StateText = StateText {
    name: Localized::from_all(
        "费用增加",
        "Cost Increase",
        "コストアップ",
        "코스트 증가",
    ),
    description: Localized::EMPTY,
};

/// 手牌状态「赋能」（K3006）的文本（描述数据为 null，留空）。
static HAND_EMPOWERMENT_TEXT: StateText = StateText {
    name: Localized::from_all("赋能", "Empowerment", "エンパワー", "힘 주입"),
    description: Localized::EMPTY,
};

/// 手牌状态「不可调和」（K3007）的文本（描述数据为 null，留空）。
static HAND_UNTUNABLE_TEXT: StateText = StateText {
    name: Localized::from_all("不可调和", "Untunable", "調和不可", "조정 불가"),
    description: Localized::EMPTY,
};

/// 手牌状态「无效化」（K3008）的文本（描述数据为 null，留空）。
static HAND_INVALIDATION_TEXT: StateText = StateText {
    name: Localized::from_all("无效化", "Invalidation", "無効化", "무효화"),
    description: Localized::EMPTY,
};

/// 手牌状态「费用增加」的身份（K3004；层数形态，TODO 待实测）。
#[derive(Debug)]
pub struct HandCostIncrease(&'static StateText);

/// 手牌状态「赋能」的身份（K3006；布尔形态，TODO 待实测）。
#[derive(Debug)]
pub struct HandEmpowerment(&'static StateText);

/// 手牌状态「不可调和」的身份（K3007；布尔形态，TODO 待实测）。
#[derive(Debug)]
pub struct HandUntunable(&'static StateText);

/// 手牌状态「无效化」的身份（K3008；布尔形态，TODO 待实测）。
#[derive(Debug)]
pub struct HandInvalidation(&'static StateText);

/// 「费用增加」静态身份实例。
pub static HAND_COST_INCREASE: HandCostIncrease = HandCostIncrease(&HAND_COST_INCREASE_TEXT);
/// 「赋能」静态身份实例。
pub static HAND_EMPOWERMENT: HandEmpowerment = HandEmpowerment(&HAND_EMPOWERMENT_TEXT);
/// 「不可调和」静态身份实例。
pub static HAND_UNTUNABLE: HandUntunable = HandUntunable(&HAND_UNTUNABLE_TEXT);
/// 「无效化」静态身份实例。
pub static HAND_INVALIDATION: HandInvalidation = HandInvalidation(&HAND_INVALIDATION_TEXT);

macro_rules! impl_hand_status_identity {
    ($identity:ident, $value:expr, $form:expr) => {
        impl StateIdentity for $identity {
            fn text(&self) -> &'static StateText {
                self.0
            }

            fn form(&self) -> StateForm {
                $form
            }
        }

        impl $identity {
            /// 把自身当状态身份指针用（D95 / D96）。
            pub fn identity(&'static self) -> &'static dyn StateIdentity {
                $value
            }
        }
    };
}

impl_hand_status_identity!(HandCostIncrease, &HAND_COST_INCREASE, StateForm::Layered);
impl_hand_status_identity!(HandEmpowerment, &HAND_EMPOWERMENT, StateForm::Boolean);
impl_hand_status_identity!(HandUntunable, &HAND_UNTUNABLE, StateForm::Boolean);
impl_hand_status_identity!(HandInvalidation, &HAND_INVALIDATION, StateForm::Boolean);

/// 本文件补的 4 个手牌状态（K3004 / K3006 / K3007 / K3008）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ExtraHandCardStatus {
    /// K3004 费用增加。
    CostIncrease,
    /// K3006 赋能。
    Empowerment,
    /// K3007 不可调和。
    Untunable,
    /// K3008 无效化。
    Invalidation,
}

impl ExtraHandCardStatus {
    /// 全部 4 个（顺序 = K 码顺序，不表示规则语义）。
    pub const ALL: [ExtraHandCardStatus; 4] = [
        ExtraHandCardStatus::CostIncrease,
        ExtraHandCardStatus::Empowerment,
        ExtraHandCardStatus::Untunable,
        ExtraHandCardStatus::Invalidation,
    ];

    /// 对应的关键字数据 K 码（3004 / 3006 / 3007 / 3008）。
    pub const fn k_code(self) -> Nat {
        match self {
            ExtraHandCardStatus::CostIncrease => Nat::new(3004),
            ExtraHandCardStatus::Empowerment => Nat::new(3006),
            ExtraHandCardStatus::Untunable => Nat::new(3007),
            ExtraHandCardStatus::Invalidation => Nat::new(3008),
        }
    }

    /// 按 K 码查（只认本文件补的 4 个；K3003/K3005 的正式身份在行动牌状态块落地前仍走
    /// damage_chain 桥模块，见本文件顶部 TODO）。
    pub fn from_k_code(code: Nat) -> Option<ExtraHandCardStatus> {
        if code == Nat::new(3004) {
            Some(ExtraHandCardStatus::CostIncrease)
        } else if code == Nat::new(3006) {
            Some(ExtraHandCardStatus::Empowerment)
        } else if code == Nat::new(3007) {
            Some(ExtraHandCardStatus::Untunable)
        } else if code == Nat::new(3008) {
            Some(ExtraHandCardStatus::Invalidation)
        } else {
            None
        }
    }

    /// 对应的状态身份（静态实例指针，D95 / D96）。
    pub fn identity(self) -> &'static dyn StateIdentity {
        match self {
            ExtraHandCardStatus::CostIncrease => &HAND_COST_INCREASE,
            ExtraHandCardStatus::Empowerment => &HAND_EMPOWERMENT,
            ExtraHandCardStatus::Untunable => &HAND_UNTUNABLE,
            ExtraHandCardStatus::Invalidation => &HAND_INVALIDATION,
        }
    }
}

/// 手牌无序：每张手牌卡一个 `StateList`，位置恒为 0（不表示顺序）。
pub const HAND_STATE_POS: Nat = Nat::ZERO;

/// 手牌卡状态的**时机键**（D185：由本层自己定义）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HandCardStateTiming {
    /// 给一张手牌卡**加上**状态（进入这张牌的状态列表）。
    StateAdded,
    /// 从一张手牌卡**移除**状态。
    StateRemoved,
}

/// 手牌卡状态效果函数指针：`fn(&mut 手牌卡状态表, &Event, owner)`。
pub type HandCardStateEffect = fn(&mut HandCardStates, &Event, EntityId);

/// 手牌卡状态的触发登记项。
pub type HandCardStateTrigger = Trigger<HandCardStateTiming, HandCardStateEffect>;

/// 手牌卡状态触发注册表（每个牌手一个具名实例，D34）。
pub type HandCardStateTriggers = TriggerRegistry<HandCardStateTiming, HandCardStateEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type HandCardStatePending = Queued<HandCardStateEffect>;

/// 一张手牌卡的状态：手牌实体号 + 它的通用状态列表（`pos` 固定 0）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HandCardStateEntry {
    /// 手牌实体号。
    pub entity: EntityId,
    /// 这张牌的状态列表（复用 `StateList`，D105）。
    pub states: StateList,
}

/// **手牌卡状态表**：手牌实体 -> 状态列表。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的手牌」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct HandCardStates {
    /// 条目（顺序 = 第一次加状态的先后；手牌本身无序，这里只是为了稳定）。
    entries: Vec<HandCardStateEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: HandCardStateStats,
}

impl PartialEq for HandCardStates {
    /// **统计不参与局面等价**（D187）。
    fn eq(&self, other: &HandCardStates) -> bool {
        self.entries == other.entries
    }
}

impl Eq for HandCardStates {}

/// 一次手牌卡状态操作的结果：新状态 + 待执行。
#[derive(Clone, Debug)]
pub struct HandCardStateResolution {
    /// 操作后的状态表。
    pub states: HandCardStates,
    /// 派发出来的待执行（本层不执行）。
    pub pending: Vec<HandCardStatePending>,
}

impl HandCardStates {
    /// 空状态表。
    pub const fn new() -> HandCardStates {
        HandCardStates {
            entries: Vec::new(),
            stats: HandCardStateStats::EMPTY,
        }
    }

    /// 全部条目（**只读指针**，D183）。
    pub fn entries(&self) -> &[HandCardStateEntry] {
        &self.entries
    }

    /// 条目数（有多少张手牌卡带状态）。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何手牌卡带状态。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 手牌卡状态统计（只读指针，D183 / D187）。
    pub fn stats(&self) -> &HandCardStateStats {
        &self.stats
    }

    /// 某张手牌卡的状态列表（**只读指针**；没带状态 -> None）。
    pub fn states_of(&self, entity: EntityId) -> Option<&StateList> {
        self.entries
            .iter()
            .find(|entry| entry.entity == entity)
            .map(|entry| &entry.states)
    }

    /// 某张手牌卡是否有某个状态。
    pub fn contains_state(&self, entity: EntityId, state: &'static dyn StateIdentity) -> bool {
        self.states_of(entity)
            .is_some_and(|states| states.contains(HAND_STATE_POS, state))
    }

    /// 某张手牌卡上某个状态的层数（不在 / 没有 -> None；在 -> Some(layers)，布尔类为 Some(None)）。
    pub fn layers(
        &self,
        entity: EntityId,
        state: &'static dyn StateIdentity,
    ) -> Option<Option<Nat>> {
        self.states_of(entity)
            .and_then(|states| states.layers(HAND_STATE_POS, state))
    }

    /// 给一张手牌卡**加状态**（纯函数；不改入参）+ 派发「状态加上」。
    ///
    /// 状态形态与合并口径完全复用 `StateList::add`（D104/D105）：布尔类幂等、层数类相加。
    pub fn add_state_with_triggers(
        &self,
        entity: EntityId,
        state: &'static dyn StateIdentity,
        layers: Option<Nat>,
        triggers: &HandCardStateTriggers,
    ) -> HandCardStateResolution {
        let mut next = self.clone();
        if let Some(entry) = next
            .entries
            .iter_mut()
            .find(|entry| entry.entity == entity)
        {
            entry.states.add(HAND_STATE_POS, state, layers);
        } else {
            let mut states = StateList::new();
            states.add(HAND_STATE_POS, state, layers);
            next.entries.push(HandCardStateEntry { entity, states });
        }
        next.stats.on_added(Nat::new(1));
        let amount = effective_layers(state, layers);
        let pending = collect(
            triggers,
            HandCardStateTiming::StateAdded,
            Event::new(Some(entity), None, amount, None, None),
        );
        HandCardStateResolution {
            states: next,
            pending,
        }
    }

    /// 从一张手牌卡**移除状态** + 派发「状态移除」。
    ///
    /// 失败（返回 None、不改状态）：这张牌没带状态 / 没有这个状态。
    pub fn remove_state_with_triggers(
        &self,
        entity: EntityId,
        state: &'static dyn StateIdentity,
        triggers: &HandCardStateTriggers,
    ) -> Option<HandCardStateResolution> {
        let mut next = self.clone();
        let entry = next
            .entries
            .iter_mut()
            .find(|entry| entry.entity == entity)?;
        if !entry.states.contains(HAND_STATE_POS, state) {
            return None;
        }
        entry.states.remove(HAND_STATE_POS, state);
        next.stats.on_removed(Nat::new(1));
        let pending = collect(
            triggers,
            HandCardStateTiming::StateRemoved,
            Event::of(entity),
        );
        Some(HandCardStateResolution {
            states: next,
            pending,
        })
    }

    /// 一张手牌卡**离开手牌**：把它身上的状态整体清掉（卡片进出**手牌区**是手牌区的事实，
    /// 本层不派发；跨区搬运由上层组合）。
    ///
    /// 失败（返回 None、不改状态）：这张牌本来就没带状态。
    pub fn remove_card(&self, entity: EntityId) -> Option<HandCardStates> {
        let mut next = self.clone();
        let position = next
            .entries
            .iter()
            .position(|entry| entry.entity == entity)?;
        next.entries.remove(position);
        Some(next)
    }

    /// 一张手牌卡的全部状态条目（拷贝；调试 / 展示用）。
    pub fn states_entries_of(
        &self,
        entity: EntityId,
    ) -> Option<Vec<crate::families::action_card::status::StateEntry>> {
        self.states_of(entity)
            .map(|states| states.entries_at(HAND_STATE_POS))
    }
}

/// 执行一条待执行列表（**调用方显式调用**；派发永不执行）。
pub fn execute_hand_card_states(
    pending: &[HandCardStatePending],
    states: &mut HandCardStates,
) {
    for item in pending {
        (item.effect)(states, &item.event, item.owner);
    }
}

/// 收集某个时机 + 事件的待执行。
fn collect(
    triggers: &HandCardStateTriggers,
    timing: HandCardStateTiming,
    event: Event,
) -> Vec<HandCardStatePending> {
    dispatch(triggers, timing, &event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 这次加状态实际加了多少层（布尔类 = 0，层数类缺省 1）——事件 `amount` 用。
fn effective_layers(state: &'static dyn StateIdentity, layers: Option<Nat>) -> Nat {
    match state.form() {
        StateForm::Boolean => Nat::ZERO,
        StateForm::Layered => layers.unwrap_or(Nat::new(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::StateText;
    use crate::generated::Localized;
    use crate::rules::ops::order::EffectSource;

    static CHEAPER_TEXT: StateText = StateText {
        name: Localized::from_all("减费", "Cost Reduction", "コスト減少", "비용 감소"),
        description: Localized::EMPTY,
    };
    static EMPOWERED_TEXT: StateText = StateText {
        name: Localized::from_all("赋能", "Empowered", "強化", "강화"),
        description: Localized::EMPTY,
    };

    /// 层数状态（减费）。
    #[derive(Debug)]
    struct Layered(&'static StateText);
    impl StateIdentity for Layered {
        fn text(&self) -> &'static StateText {
            self.0
        }
        fn form(&self) -> StateForm {
            StateForm::Layered
        }
    }

    /// 布尔状态（赋能）。
    #[derive(Debug)]
    struct Boolean(&'static StateText);
    impl StateIdentity for Boolean {
        fn text(&self) -> &'static StateText {
            self.0
        }
        fn form(&self) -> StateForm {
            StateForm::Boolean
        }
    }

    static CHEAPER: Layered = Layered(&CHEAPER_TEXT);
    static EMPOWERED: Boolean = Boolean(&EMPOWERED_TEXT);

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    fn noop(_states: &mut HandCardStates, _event: &Event, _owner: EntityId) {}

    fn registry_for(timing: HandCardStateTiming) -> HandCardStateTriggers {
        let mut triggers = HandCardStateTriggers::new();
        triggers.register(Trigger::new(timing, n(1), EffectSource::CharacterCard, n(0), noop));
        triggers
    }

    #[test]
    fn empty_table_basics() {
        let states = HandCardStates::new();
        assert!(states.is_empty());
        assert_eq!(states.len(), Nat::ZERO);
        assert!(states.states_of(n(1)).is_none());
        assert!(!states.contains_state(n(1), &CHEAPER));
    }

    #[test]
    fn add_state_creates_entry_and_merges_by_state_list_rules() {
        let triggers = HandCardStateTriggers::new();
        let empty = HandCardStates::new();
        let first = empty.add_state_with_triggers(n(7), &CHEAPER, Some(n(2)), &triggers);
        assert_eq!(first.states.len(), n(1));
        assert_eq!(first.states.layers(n(7), &CHEAPER), Some(Some(n(2))));
        // 层数类相加。
        let second = first
            .states
            .add_state_with_triggers(n(7), &CHEAPER, Some(n(3)), &triggers);
        assert_eq!(second.states.layers(n(7), &CHEAPER), Some(Some(n(5))));
        assert_eq!(second.states.len(), n(1));
        // 布尔类无层、幂等。
        let third = second
            .states
            .add_state_with_triggers(n(7), &EMPOWERED, None, &triggers);
        assert_eq!(third.states.layers(n(7), &EMPOWERED), Some(None));
        let fourth = third
            .states
            .add_state_with_triggers(n(7), &EMPOWERED, None, &triggers);
        assert_eq!(fourth.states.layers(n(7), &EMPOWERED), Some(None));
        // 入参不改（纯函数）。
        assert_eq!(empty.len(), Nat::ZERO);
    }

    #[test]
    fn add_dispatches_with_amount_and_entity() {
        let triggers = registry_for(HandCardStateTiming::StateAdded);
        let added = HandCardStates::new().add_state_with_triggers(n(7), &CHEAPER, Some(n(2)), &triggers);
        assert_eq!(added.pending.len(), 1);
        let item = added.pending.first().expect("一条");
        assert_eq!(item.event.who, Some(n(7)));
        assert_eq!(item.event.amount, n(2));
        // 布尔类 amount = 0。
        let boolean = HandCardStates::new().add_state_with_triggers(n(8), &EMPOWERED, None, &triggers);
        assert_eq!(boolean.pending.first().expect("一条").event.amount, Nat::ZERO);
    }

    #[test]
    fn remove_state_and_card() {
        let triggers = registry_for(HandCardStateTiming::StateRemoved);
        let states = HandCardStates::new()
            .add_state_with_triggers(n(7), &CHEAPER, Some(n(2)), &triggers)
            .states
            .add_state_with_triggers(n(7), &EMPOWERED, None, &triggers)
            .states;
        // 移除一个状态：另一条还在。
        let removed = states
            .remove_state_with_triggers(n(7), &CHEAPER, &triggers)
            .expect("有这个状态");
        assert!(!removed.states.contains_state(n(7), &CHEAPER));
        assert!(removed.states.contains_state(n(7), &EMPOWERED));
        assert_eq!(removed.pending.len(), 1);
        // 没有这个状态 / 没有这张牌 -> None。
        assert!(removed
            .states
            .remove_state_with_triggers(n(7), &CHEAPER, &triggers)
            .is_none());
        assert!(removed
            .states
            .remove_state_with_triggers(n(99), &CHEAPER, &triggers)
            .is_none());
        // 卡片离手：整条移除。
        let after_card = removed.states.remove_card(n(7)).expect("这张牌带状态");
        assert!(after_card.is_empty());
        assert!(removed.states.remove_card(n(99)).is_none());
    }

    #[test]
    fn extra_hand_status_identities_have_names_forms_and_queries() {
        use crate::generated::Lang;

        let expected: [(ExtraHandCardStatus, &str, StateForm, Nat); 4] = [
            (
                ExtraHandCardStatus::CostIncrease,
                "费用增加",
                StateForm::Layered,
                n(3004),
            ),
            (
                ExtraHandCardStatus::Empowerment,
                "赋能",
                StateForm::Boolean,
                n(3006),
            ),
            (
                ExtraHandCardStatus::Untunable,
                "不可调和",
                StateForm::Boolean,
                n(3007),
            ),
            (
                ExtraHandCardStatus::Invalidation,
                "无效化",
                StateForm::Boolean,
                n(3008),
            ),
        ];
        let triggers = HandCardStateTriggers::new();
        for (index, (kind, name, form, k_code)) in expected.iter().enumerate() {
            let identity = kind.identity();
            assert_eq!(identity.name(), *name);
            assert_eq!(identity.form(), *form);
            assert_eq!(kind.k_code(), *k_code);
            assert_eq!(ExtraHandCardStatus::from_k_code(*k_code), Some(*kind));
            // 描述在数据里是 null：文本描述保持空（不编造）。
            assert!(identity.text().description.get(Lang::ZhCn).text().is_none());

            // 接上现成手牌状态区查询入口：加状态后按同一身份查得到。
            let entity = n(index as u32 + 10);
            let layers = match form {
                StateForm::Layered => Some(n(2)),
                StateForm::Boolean => None,
            };
            let states = HandCardStates::new()
                .add_state_with_triggers(entity, identity, layers, &triggers)
                .states;
            assert!(states.contains_state(entity, identity));
            assert_eq!(states.layers(entity, identity), Some(layers));
            assert!(!states.contains_state(n(99), identity));
        }
        assert_eq!(ExtraHandCardStatus::ALL.len(), 4);
        assert_eq!(ExtraHandCardStatus::from_k_code(n(3003)), None);
        assert_eq!(ExtraHandCardStatus::from_k_code(n(3005)), None);
        assert_eq!(ExtraHandCardStatus::from_k_code(n(1)), None);
    }

    #[test]
    fn stats_follow_feature_flag() {
        let triggers = HandCardStateTriggers::new();
        let states = HandCardStates::new()
            .add_state_with_triggers(n(7), &CHEAPER, Some(n(1)), &triggers)
            .states
            .remove_state_with_triggers(n(7), &CHEAPER, &triggers)
            .expect("有")
            .states;
        if cfg!(feature = "stats") {
            assert_eq!(states.stats().added().get(), n(1));
            assert_eq!(states.stats().removed().get(), n(1));
        } else {
            assert!(states.stats().added().is_zero());
            assert!(states.stats().removed().is_zero());
        }
    }
}
