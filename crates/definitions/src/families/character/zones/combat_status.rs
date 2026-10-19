//! **出战状态区**（规则书·游戏区域 II(3)）：附在出战角色上、切换角色时**转移**给新出战角色。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` 游戏区域 II(3) + 特殊词条 持续回合）
//!
//! - II：「每张『角色牌』，都可以附属『装备牌』『状态』『出战状态』。」
//! - II(3)：「一些角色技能会生成『出战状态』，并附属在出战的角色上。出战角色被切换为后台角色时，
//!   『出战状态』会转移到新的出战角色上。」
//! - 特殊词条 持续回合：「每个结束阶段，都会使『持续回合』-1。『持续回合』耗尽后，会立刻弃置这张牌。」
//!   → 递减 / 耗尽判定**复用** `rules::ops::lifetime`（本层不重写数值）。
//!
//! ## 口径（D182 / D183 / D185 / D187）
//!
//! - 本区**自持状态**：`attached_to`（挂在哪个出战角色上）+ `entries`（状态列表）+ 统计；
//!   变量型、可建多个（我方 / 敌方各一个实例，D34），不出现区号、不建区表；
//! - 对外只给**只读指针**（`entries()` / `attached_to()` / `layers()` / `duration()` / `stats()`）；
//! - 状态身份复用**通用状态身份**（`StateIdentity`，D95/D96/D110：类型化静态实例指针），
//!   本层不建状态编号表、不解释具体状态语义（D108）；
//! - **转移** = 只改 `attached_to`，条目原样不动（列表顺序 = 进场先后，`结算顺序.md` §一 第 3 条）；
//! - 进 / 出区域的时机键由本区自己定义（D185），走 `timeline` 派发（只排序、不执行 D186）。
//!
//! ## 待复核
//!
//! - 同名出战状态再次附着时：层数**相加**（沿用 `StateList` 同口径）、持续回合**保留原值不刷新**
//!   —— 规则书未规定，先按这条实现；
//! - `Transferred` 事件只发一条（不是每个状态发一条）—— 规则书只说「会转移」。

use crate::families::action_card::status::{same_state_identity, StateForm, StateIdentity};
use crate::families::character::zones::stats::CombatStatusStats;
use crate::rules::ops::lifetime::{duration_after_end_phase, is_depleted};
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::{nadd, EntityId, Nat};

/// 出战状态区的**时机键**（D185：由本区自己定义）。
///
/// 依据：规则书·游戏区域 II(3)（出战状态生成 / 转移）+ 特殊词条 持续回合（耗尽弃置）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CombatStatusTiming {
    /// 出战状态**进入**出战状态区（生成 / 附着）。
    Attached,
    /// 出战状态随「切换角色」**转移**给新出战角色。
    Transferred,
    /// 出战状态**移出**出战状态区（被效果移除）。
    Removed,
    /// 结束阶段：持续回合 -1（还没有耗尽）。
    DurationTicked,
    /// 持续回合**耗尽** -> 立刻弃置（规则书·特殊词条 持续回合）。
    Depleted,
}

/// 出战状态区效果函数指针：`fn(&mut 出战状态区, &Event, owner)`。
pub type CombatStatusEffect = fn(&mut CombatStatusZone, &Event, EntityId);

/// 出战状态区的触发登记项。
pub type CombatStatusTrigger = Trigger<CombatStatusTiming, CombatStatusEffect>;

/// 出战状态区触发注册表（每个牌手一个具名实例，D34）。
pub type CombatStatusTriggers = TriggerRegistry<CombatStatusTiming, CombatStatusEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type CombatStatusPending = Queued<CombatStatusEffect>;

/// 一条出战状态：状态身份 + 层数 + 持续回合。
///
/// - `layers`：布尔类 `None`（在列表里即代表有，D104/D105 同口径）、层数类 `Some(n)`；
/// - `duration`：持续回合（`None` = 没有持续回合；每结束阶段 -1，耗尽弃置）。
#[derive(Clone, Copy)]
pub struct CombatStatusEntry {
    /// 状态身份（类型化静态实例指针，D95/D96）。
    pub status: &'static dyn StateIdentity,
    /// 层数（布尔类 None；层数类 Some）。
    pub layers: Option<Nat>,
    /// 持续回合（None = 不按结束阶段递减）。
    pub duration: Option<Nat>,
}

impl PartialEq for CombatStatusEntry {
    fn eq(&self, other: &CombatStatusEntry) -> bool {
        same_state_identity(self.status, other.status)
            && self.layers == other.layers
            && self.duration == other.duration
    }
}

impl Eq for CombatStatusEntry {}

impl std::fmt::Debug for CombatStatusEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatStatusEntry")
            .field("status", &self.status.name())
            .field("layers", &self.layers)
            .field("duration", &self.duration)
            .finish()
    }
}

/// **出战状态区**：挂在某个出战角色上的状态列表。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的区域」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct CombatStatusZone {
    /// 当前挂载的出战角色（没有出战角色 -> None）。
    attached_to: Option<EntityId>,
    /// 出战状态条目（顺序 = 进场先后）。
    entries: Vec<CombatStatusEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: CombatStatusStats,
}

impl PartialEq for CombatStatusZone {
    /// **统计不参与局面等价**（D187）。
    fn eq(&self, other: &CombatStatusZone) -> bool {
        self.attached_to == other.attached_to && self.entries == other.entries
    }
}

impl Eq for CombatStatusZone {}

/// 一次出战状态区操作的结果：新状态 + 待执行。
#[derive(Clone, Debug)]
pub struct CombatStatusResolution {
    /// 操作后的出战状态区。
    pub zone: CombatStatusZone,
    /// 派发出来的待执行（本层不执行）。
    pub pending: Vec<CombatStatusPending>,
}

impl CombatStatusZone {
    /// 空出战状态区（没有挂载角色、没有状态）。
    pub const fn new() -> CombatStatusZone {
        CombatStatusZone {
            attached_to: None,
            entries: Vec::new(),
            stats: CombatStatusStats::EMPTY,
        }
    }

    /// 当前挂载的出战角色（只读）。
    pub fn attached_to(&self) -> Option<EntityId> {
        self.attached_to
    }

    /// 全部出战状态（**只读指针**，D183；顺序 = 进场先后）。
    pub fn entries(&self) -> &[CombatStatusEntry] {
        &self.entries
    }

    /// 状态条目数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何出战状态。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 出战状态区统计（只读指针，D183 / D187）。
    pub fn stats(&self) -> &CombatStatusStats {
        &self.stats
    }

    /// 这个状态是否在区里。
    pub fn contains(&self, status: &'static dyn StateIdentity) -> bool {
        self.entries
            .iter()
            .any(|entry| same_state_identity(entry.status, status))
    }

    /// 这个状态的条目（只读指针；不在区里 -> None）。
    pub fn entry(&self, status: &'static dyn StateIdentity) -> Option<&CombatStatusEntry> {
        self.entries
            .iter()
            .find(|entry| same_state_identity(entry.status, status))
    }

    /// 这个状态的层数（不在区里 -> None；在区里 -> Some(layers)，布尔类为 Some(None)）。
    pub fn layers(&self, status: &'static dyn StateIdentity) -> Option<Option<Nat>> {
        self.entry(status).map(|entry| entry.layers)
    }

    /// 这个状态的持续回合（不在区里 / 没有持续回合 -> None）。
    pub fn duration(&self, status: &'static dyn StateIdentity) -> Option<Nat> {
        self.entry(status).and_then(|entry| entry.duration)
    }

    /// **进入出战状态区**（生成 / 附着）+ 派发「附着」。
    ///
    /// - `active`：这些状态挂在哪个出战角色上（同时更新 `attached_to`）；
    /// - 同名状态已存在：层数**相加**（同 `StateList` 口径），持续回合**保留原值**（待复核）；
    /// - 形态归一：布尔类 `layers = None`；层数类缺省按 1 层。
    pub fn attach_with_triggers(
        &self,
        active: EntityId,
        status: &'static dyn StateIdentity,
        layers: Option<Nat>,
        duration: Option<Nat>,
        triggers: &CombatStatusTriggers,
    ) -> CombatStatusResolution {
        let normalized = normalize_layers(status, layers);
        let mut zone = self.clone();
        zone.attached_to = Some(active);
        if let Some(entry) = zone
            .entries
            .iter_mut()
            .find(|entry| same_state_identity(entry.status, status))
        {
            entry.layers = merge_layers(entry.layers, normalized);
        } else {
            zone.entries.push(CombatStatusEntry {
                status,
                layers: normalized,
                duration,
            });
        }
        zone.stats.on_attached(Nat::new(1));
        let amount = normalized.unwrap_or(Nat::ZERO);
        let pending = collect(
            triggers,
            CombatStatusTiming::Attached,
            status_event(Some(active), None, amount),
        );
        CombatStatusResolution { zone, pending }
    }

    /// **移出**一个出战状态 + 派发「移出」。
    ///
    /// 失败（返回 None、不改状态）：这个状态不在区里。
    pub fn remove_with_triggers(
        &self,
        status: &'static dyn StateIdentity,
        triggers: &CombatStatusTriggers,
    ) -> Option<CombatStatusResolution> {
        let position = self
            .entries
            .iter()
            .position(|entry| same_state_identity(entry.status, status))?;
        let mut zone = self.clone();
        zone.entries.remove(position);
        let pending = collect(
            triggers,
            CombatStatusTiming::Removed,
            status_event(zone.attached_to, None, Nat::ZERO),
        );
        Some(CombatStatusResolution { zone, pending })
    }

    /// **切换角色时转移**（规则书·游戏区域 II(3)）：只改挂载的出战角色，条目原样保留。
    ///
    /// 失败（返回 None、不改状态）：本来就没有挂载角色 / 新出战角色没有变化。
    pub fn transfer_with_triggers(
        &self,
        new_active: EntityId,
        triggers: &CombatStatusTriggers,
    ) -> Option<CombatStatusResolution> {
        let old_active = self.attached_to?;
        if old_active == new_active {
            return None;
        }
        let mut zone = self.clone();
        zone.attached_to = Some(new_active);
        zone.stats.on_transferred(Nat::new(1));
        let pending = collect(
            triggers,
            CombatStatusTiming::Transferred,
            Event::new(
                Some(new_active),
                Some(old_active),
                zone.len(),
                None,
                None,
            ),
        );
        Some(CombatStatusResolution { zone, pending })
    }

    /// **结束阶段**：所有带持续回合的出战状态 -1；耗尽的**立刻弃置**。
    ///
    /// 递减 / 耗尽判定复用 `rules::ops::lifetime`（`duration_after_end_phase` / `is_depleted`）。
    /// 没有持续回合的条目不动、不派发。待执行按条目顺序（= 进场先后）给出：
    /// 未耗尽的发 `DurationTicked`（`amount` = 剩余回合），耗尽的发 `Depleted` 并移出。
    pub fn tick_end_phase_with_triggers(
        &self,
        triggers: &CombatStatusTriggers,
    ) -> CombatStatusResolution {
        // 先拷一份条目再遍历：统计要 &mut，借不到 &zone.entries。
        let entries = self.entries.clone();
        let attached_to = self.attached_to;
        let mut zone = self.clone();
        let mut pending: Vec<CombatStatusPending> = Vec::new();
        let mut kept: Vec<CombatStatusEntry> = Vec::new();
        for entry in &entries {
            let Some(duration) = entry.duration else {
                kept.push(*entry);
                continue;
            };
            let next = duration_after_end_phase(duration);
            if is_depleted(next) {
                zone.stats.on_depleted(Nat::new(1));
                pending.extend(collect(
                    triggers,
                    CombatStatusTiming::Depleted,
                    status_event(attached_to, None, Nat::ZERO),
                ));
            } else {
                let mut updated = *entry;
                updated.duration = Some(next);
                kept.push(updated);
                pending.extend(collect(
                    triggers,
                    CombatStatusTiming::DurationTicked,
                    status_event(attached_to, None, next),
                ));
            }
        }
        zone.entries = kept;
        CombatStatusResolution { zone, pending }
    }
}

/// 执行一条待执行列表（**调用方显式调用**；派发永不执行）。
pub fn execute_combat_status(pending: &[CombatStatusPending], zone: &mut CombatStatusZone) {
    for item in pending {
        (item.effect)(zone, &item.event, item.owner);
    }
}

/// 收集某个时机 + 事件的待执行。
fn collect(
    triggers: &CombatStatusTriggers,
    timing: CombatStatusTiming,
    event: Event,
) -> Vec<CombatStatusPending> {
    dispatch(triggers, timing, &event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 出战状态事件：`who` = 挂载的出战角色（状态身份不是实体，不能进 event），`target` = 可选的另一方，
/// `amount` = 层数 / 剩余回合 / 转移条数（含义由时机决定）。
fn status_event(attached_to: Option<EntityId>, target: Option<EntityId>, amount: Nat) -> Event {
    Event::new(attached_to, target, amount, None, None)
}

/// 按状态形态把层数归一：布尔类恒 None；层数类缺省按 1 层（与 `StateList` 同口径）。
fn normalize_layers(state: &'static dyn StateIdentity, layers: Option<Nat>) -> Option<Nat> {
    match state.form() {
        StateForm::Boolean => None,
        StateForm::Layered => Some(layers.unwrap_or(Nat::new(1))),
    }
}

/// 合并层数：None + None = None；None + Some(n) = Some(n)；Some + Some 相加（不钳制，D100）。
fn merge_layers(left: Option<Nat>, right: Option<Nat>) -> Option<Nat> {
    match (left, right) {
        (None, None) => None,
        (None, Some(layers)) | (Some(layers), None) => Some(layers),
        (Some(left), Some(right)) => Some(nadd!(left, right)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Localized;
    use crate::rules::ops::order::EffectSource;
    use crate::families::action_card::status::StateText;

    /// 测试用静态定义数据（D110：与 TagText 同构）。
    static SHIELD_TEXT: StateText = StateText {
        name: Localized::from_all("护盾", "Shield", "シールド", "보호막"),
        description: Localized::EMPTY,
    };
    static SHARP_TEXT: StateText = StateText {
        name: Localized::from_all("锐利", "Sharp", "鋭利", "예리"),
        description: Localized::EMPTY,
    };

    /// 布尔出战状态（护盾一类：在列表里即代表有）。
    #[derive(Debug)]
    struct BooleanStatus(&'static StateText);

    impl StateIdentity for BooleanStatus {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Boolean
        }
    }

    /// 层数出战状态。
    #[derive(Debug)]
    struct LayeredStatus(&'static StateText);

    impl StateIdentity for LayeredStatus {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Layered
        }
    }

    static SHIELD: BooleanStatus = BooleanStatus(&SHIELD_TEXT);
    static SHARP: LayeredStatus = LayeredStatus(&SHARP_TEXT);

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 示例效果：只证明「派发 -> 执行」链路；具体效果由卡牌自己写。
    fn noop(_zone: &mut CombatStatusZone, _event: &Event, _owner: EntityId) {}

    fn registry_for(timing: CombatStatusTiming) -> CombatStatusTriggers {
        let mut triggers = CombatStatusTriggers::new();
        triggers.register(Trigger::new(timing, n(1), EffectSource::CombatStatus, n(0), noop));
        triggers
    }

    #[test]
    fn empty_zone_basics() {
        let zone = CombatStatusZone::new();
        assert!(zone.is_empty());
        assert_eq!(zone.len(), Nat::ZERO);
        assert_eq!(zone.attached_to(), None);
        assert!(zone.entries().is_empty());
        assert!(!zone.contains(&SHIELD));
        assert_eq!(zone.layers(&SHIELD), None);
    }

    #[test]
    fn attach_boolean_status_keeps_no_layer_and_records_active() {
        let triggers = CombatStatusTriggers::new();
        let zone = CombatStatusZone::new();
        let attached = zone.attach_with_triggers(n(1), &SHIELD, None, None, &triggers);
        assert_eq!(attached.zone.attached_to(), Some(n(1)));
        assert_eq!(attached.zone.layers(&SHIELD), Some(None));
        assert_eq!(attached.zone.duration(&SHIELD), None);
        // 重复附着幂等（布尔类仍是一条）。
        let again = attached
            .zone
            .attach_with_triggers(n(1), &SHIELD, Some(n(3)), None, &triggers);
        assert_eq!(again.zone.len(), n(1));
        assert_eq!(again.zone.layers(&SHIELD), Some(None));
    }

    #[test]
    fn attach_layered_status_merges_layers_and_keeps_duration() {
        let triggers = CombatStatusTriggers::new();
        let zone = CombatStatusZone::new();
        let first = zone.attach_with_triggers(n(1), &SHARP, Some(n(2)), Some(n(2)), &triggers);
        assert_eq!(first.zone.layers(&SHARP), Some(Some(n(2))));
        assert_eq!(first.zone.duration(&SHARP), Some(n(2)));
        // 再附着：层数相加、持续回合保留原值（待复核）。
        let second = first
            .zone
            .attach_with_triggers(n(1), &SHARP, Some(n(3)), Some(n(9)), &triggers);
        assert_eq!(second.zone.layers(&SHARP), Some(Some(n(5))));
        assert_eq!(second.zone.duration(&SHARP), Some(n(2)));
        // 缺省层数按 1（层数类）。
        let third = second
            .zone
            .attach_with_triggers(n(1), &SHARP, None, None, &triggers);
        assert_eq!(third.zone.layers(&SHARP), Some(Some(n(6))));
    }

    #[test]
    fn same_status_identity_from_two_paths_merges_on_attach() {
        // 同一 static 经两条路径传入：attach 必须合并（层数相加），而不是新增第二条。
        fn direct() -> &'static dyn StateIdentity {
            &SHARP
        }
        fn via_alias() -> &'static dyn StateIdentity {
            &SHARP
        }
        let first = direct();
        let second = via_alias();
        assert!(
            same_state_identity(first, second),
            "同一身份经两条路径必须相等"
        );
        let triggers = CombatStatusTriggers::new();
        let zone = CombatStatusZone::new()
            .attach_with_triggers(n(1), first, Some(n(2)), None, &triggers)
            .zone
            .attach_with_triggers(n(1), second, Some(n(3)), None, &triggers)
            .zone;
        assert_eq!(zone.len(), n(1));
        assert_eq!(zone.layers(&SHARP), Some(Some(n(5))));
    }

    #[test]
    fn attach_dispatches_attached_event() {
        let triggers = registry_for(CombatStatusTiming::Attached);
        let attached = CombatStatusZone::new().attach_with_triggers(n(1), &SHARP, Some(n(2)), None, &triggers);
        assert_eq!(attached.pending.len(), 1);
        let item = attached.pending.first().expect("一条");
        assert_eq!(item.event.who, Some(n(1)));
        assert_eq!(item.event.amount, n(2));
        // 执行由调用方做（本层只派发）。
        let mut zone = attached.zone.clone();
        execute_combat_status(&attached.pending, &mut zone);
        assert_eq!(zone, attached.zone);
    }

    #[test]
    fn transfer_moves_attachment_and_keeps_entries() {
        let triggers = registry_for(CombatStatusTiming::Transferred);
        let zone = CombatStatusZone::new()
            .attach_with_triggers(n(1), &SHARP, Some(n(2)), Some(n(3)), &triggers)
            .zone;
        let transferred = zone.transfer_with_triggers(n(2), &triggers).expect("有挂载角色");
        assert_eq!(transferred.zone.attached_to(), Some(n(2)));
        // 条目原样（层数 / 持续回合都不变）。
        assert_eq!(transferred.zone.layers(&SHARP), Some(Some(n(2))));
        assert_eq!(transferred.zone.duration(&SHARP), Some(n(3)));
        assert_eq!(transferred.pending.len(), 1);
        let item = transferred.pending.first().expect("一条");
        assert_eq!(item.event.who, Some(n(2)));
        assert_eq!(item.event.target, Some(n(1)));
        // 新角色相同 / 没有挂载角色 -> None
        assert!(transferred.zone.transfer_with_triggers(n(2), &triggers).is_none());
        assert!(CombatStatusZone::new().transfer_with_triggers(n(1), &triggers).is_none());
    }

    #[test]
    fn end_phase_ticks_duration_and_discards_depleted() {
        let triggers = registry_for(CombatStatusTiming::DurationTicked);
        let zone = CombatStatusZone::new()
            .attach_with_triggers(n(1), &SHARP, Some(n(1)), Some(n(2)), &triggers)
            .zone;
        // 第 1 个结束阶段：2 -> 1，保留。
        let first = zone.tick_end_phase_with_triggers(&triggers);
        assert_eq!(first.zone.duration(&SHARP), Some(n(1)));
        assert_eq!(first.pending.len(), 1);
        assert_eq!(first.pending.first().expect("一条").event.amount, n(1));
        // 第 2 个结束阶段：1 -> 0，立刻弃置。
        let second = first.zone.tick_end_phase_with_triggers(&triggers);
        assert!(second.zone.is_empty());
        assert_eq!(second.zone.attached_to(), Some(n(1)));
        // 没有持续回合的状态不受影响。
        let with_shield = zone
            .attach_with_triggers(n(1), &SHIELD, None, None, &triggers)
            .zone;
        let ticked = with_shield.tick_end_phase_with_triggers(&triggers);
        assert_eq!(ticked.zone.duration(&SHARP), Some(n(1)));
        assert!(ticked.zone.contains(&SHIELD));
    }

    #[test]
    fn remove_takes_status_out_of_zone() {
        let triggers = registry_for(CombatStatusTiming::Removed);
        let zone = CombatStatusZone::new()
            .attach_with_triggers(n(1), &SHARP, Some(n(2)), None, &triggers)
            .zone;
        let removed = zone.remove_with_triggers(&SHARP, &triggers).expect("在区里");
        assert!(removed.zone.is_empty());
        assert_eq!(removed.pending.len(), 1);
        assert!(removed.zone.remove_with_triggers(&SHARP, &triggers).is_none());
    }

    #[test]
    fn stats_follow_feature_flag() {
        let triggers = CombatStatusTriggers::new();
        let zone = CombatStatusZone::new()
            .attach_with_triggers(n(1), &SHARP, Some(n(1)), Some(n(1)), &triggers)
            .zone
            .tick_end_phase_with_triggers(&triggers)
            .zone;
        if cfg!(feature = "stats") {
            assert_eq!(zone.stats().attached().get(), n(1));
            assert_eq!(zone.stats().depleted().get(), n(1));
        } else {
            assert!(zone.stats().attached().is_zero());
            assert!(zone.stats().depleted().is_zero());
        }
    }
}
