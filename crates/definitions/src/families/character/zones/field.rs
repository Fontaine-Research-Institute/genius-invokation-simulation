//! **角色区**（规则书·游戏区域 I）：角色槽位 + 当前出战角色 + 「切换角色」操作。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` 游戏区域 I / takeaction 战斗行动【切换角色】）
//!
//! - I(3)：「每位牌手都会有一名『出战角色』。只有『出战角色』才能使用技能，同时只有『出战角色』
//!   会受到对方的技能攻击。」
//! - I(4)：「其他未出战的角色被称为『后台角色』。可以通过『切换角色』行动来将他们切换为『出战角色』。」
//! - I(5)：「如果『出战角色』被击倒，牌手就需要选择一个『后台角色』成为新的『出战角色』。」
//! - 战斗行动【切换角色】：「支付 1 个任意类型的元素骰，将一个我方的『后台角色』切换为『出战角色』。」
//!   → 骰子费用支付**不在本层**（D121 / D133：支付归「被花的东西」；本层只做区域与时机）。
//!
//! ## 状态口径（D182 / D183 / D185 / D187）
//!
//! - 角色区**自持状态**（`slots` / `active` / `stats`）：变量型、可建多个（我方 / 敌方各一个实例，
//!   由持有它的状态层写成两个具名字段，D34），本层不出现区号、不做区登记、不建「实体 -> 区」表；
//! - 对外只给**只读指针**：`slots()` / `active()` / `slot_of()` / `stats()` 都是 `&self` 读；
//! - **进 / 出区域的时机**由角色区自己定义（`CharacterZoneTiming`，D185），走 `timeline` 派发：
//!   `Entered` / `Left` / `SwitchOut` / `SwitchIn` / `ActiveChanged`；
//! - 派发**只排序、不执行**（D186）：返回待执行列表，由调用方执行（`execute_character_zone`）。
//!
//! ## 槽位顺序 = 进场先后（结算顺序第 3 条）
//!
//! 槽位 `Vec` 的顺序就是**进入角色区的先后**（开局按牌组顺序放置），**切换出战不重排槽位**：
//! 出战角色是独立指针，不是「槽位 0」。所以「同来源类别按进场先后结算」（`结算顺序.md` §一 第 3 条 /
//! 规则书·其他规则 结算顺序）可以直接按 `slots()` 顺序读。
//!
//! ## 纯函数风格
//!
//! 所有游戏操作吃 `&self`、返回新状态（或分支列表），不改入参；失败语义统一为
//! 「不改状态 + 返回 `None`」，不 panic。数值一律 `Nat`（饱和运算），不用 `HashMap`。

use crate::families::character::character::CharacterState;
use crate::families::character::zones::stats::CharacterZoneStats;
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::{nsub, EntityId, Nat};

/// 一方角色区的槽位数：3。
///
/// 依据：标准牌组是 3 张角色牌、开局全部进入角色区（规则书·游戏准备 / 游戏区域 I）。
/// 待复核：分工计划 §4.4.5 曾记「我方 2~4、场上最多 4」；本轮按用户口径固定 3，
/// 若实测存在 4 槽玩法只改这一个常量。
pub const CHARACTER_SLOTS: Nat = Nat::new(3);

/// 角色区的**时机键**（D185：由角色族自己定义，通用 `timeline` 不认识它）。
///
/// 依据：规则书·游戏区域 I(3)(4)(5) + 战斗行动【切换角色】。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CharacterZoneTiming {
    /// 角色**进入角色区**（开局放置 / 后续加入）。
    ///
    /// 依据：规则书·游戏区域 I「游戏开始后，牌组中所有『角色牌』都会被放置到角色区」。
    Entered,
    /// 角色**离开角色区**（被击倒 / 移出比赛）。
    ///
    /// 依据：规则书·其他规则 击倒「角色就会被『击倒』」；游戏区域 I(5)「如果『出战角色』被击倒…」。
    /// 注意：**「被击倒」本身**的时机键在 `character::trigger::CharacterTiming::Defeated`，
    /// 本时机是「这个角色不再占角色区槽位」的区域事实；两者的先后由调用方按结算顺序决定。
    Left,
    /// **切出**：原出战角色变回后台角色。
    ///
    /// 依据：规则书·游戏区域 I(4) + II(3)「出战角色被切换为后台角色时…」。
    SwitchOut,
    /// **切入**：后台角色成为新的出战角色。
    ///
    /// 依据：规则书·战斗行动【切换角色】「将一个我方的『后台角色』切换为『出战角色』」。
    SwitchIn,
    /// **出战角色变化**（切换完成 / 被击倒后的强制补位）。
    ///
    /// 依据：规则书·游戏区域 I(3)(5)。与 `SwitchIn` 的区别：`SwitchIn` 是「后台 -> 出战」这个动作，
    /// `ActiveChanged` 是「当前出战角色是谁」这个结果（强制补位没有切出方，也发它）。
    ActiveChanged,
}

/// 角色区效果函数指针：`fn(&mut 角色区, &Event, owner)`。
///
/// 与角色族 `CharacterEffect`（`fn(&mut CharacterState, &Event, EntityId)`，D185 的实例过滤约定）
/// 同形：第三个参数 = `Pending.owner`，效果函数自己用它判断「这条触发是不是属于本次主体」。
pub type CharacterZoneEffect = fn(&mut CharacterZone, &Event, EntityId);

/// 角色区的触发登记项（时机键 + 条件 + 效果 + 排序标识；D185 / D186）。
pub type CharacterZoneTrigger = Trigger<CharacterZoneTiming, CharacterZoneEffect>;

/// 角色区触发注册表（每个牌手一个具名实例，D34；同一张卡的多实例都登记在同一张表里）。
pub type CharacterZoneTriggers = TriggerRegistry<CharacterZoneTiming, CharacterZoneEffect>;

/// 一条**待执行**（owned；事件按值带走，便于从纯函数操作里返回）。由调用方执行。
pub type CharacterZonePending = Queued<CharacterZoneEffect>;

/// 一个角色槽位：角色实体号 + 它的运行期状态（生命 / 充能 / 状态 / 元素附着）。
///
/// `CharacterState` 的数值操作（`character.rs`）与触发（`trigger.rs`）完全复用，本层不做数值。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CharacterSlot {
    /// 角色实体号（本局唯一）。
    pub character: EntityId,
    /// 这个角色的运行期状态。
    pub state: CharacterState,
}

impl CharacterSlot {
    /// 用一个角色实体 + 运行期状态建槽位。
    pub const fn new(character: EntityId, state: CharacterState) -> CharacterSlot {
        CharacterSlot { character, state }
    }
}

/// **角色区**：最多 3 个槽位 + 当前出战角色指针 + 统计。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的角色区」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct CharacterZone {
    /// 槽位（顺序 = 进入角色区的先后；切换出战不重排）。
    slots: Vec<CharacterSlot>,
    /// 当前出战角色（没有角色 / 全部被击倒 -> None）。
    active: Option<EntityId>,
    /// 角色区统计（D187；feature 关闭时零大小、空操作）。
    stats: CharacterZoneStats,
}

impl PartialEq for CharacterZone {
    /// **统计不参与局面等价**（D187：统计不改变游戏语义）。
    fn eq(&self, other: &CharacterZone) -> bool {
        self.slots == other.slots && self.active == other.active
    }
}

impl Eq for CharacterZone {}

/// 一次角色区操作的结果：新状态 + 派发出来的待执行（由调用方执行）。
#[derive(Clone, Debug)]
pub struct CharacterZoneResolution {
    /// 操作后的角色区。
    pub zone: CharacterZone,
    /// 派发出来的待执行（本层不执行）。
    pub pending: Vec<CharacterZonePending>,
}

impl CharacterZone {
    /// 空角色区（没有槽位、没有出战角色）。
    pub const fn new() -> CharacterZone {
        CharacterZone {
            slots: Vec::new(),
            active: None,
            stats: CharacterZoneStats::EMPTY,
        }
    }

    /// 用一组槽位建角色区：**默认第一张为出战角色**（正式流程应随后用 `set_active` 显式选择，
    /// 见规则书·游戏准备「双方各自选择一张自己的『角色牌』作为『出战角色』」）。
    ///
    /// 失败（返回 `None`、不改任何东西）：超过 3 个槽位 / 角色实体重复。
    pub fn try_from_slots(slots: Vec<CharacterSlot>) -> Option<CharacterZone> {
        if Nat::new(slots.len() as _) > CHARACTER_SLOTS {
            return None;
        }
        for (index, slot) in slots.iter().enumerate() {
            if slots
                .iter()
                .take(index)
                .any(|earlier| earlier.character == slot.character)
            {
                return None;
            }
        }
        let active = slots.first().map(|slot| slot.character);
        Some(CharacterZone {
            slots,
            active,
            stats: CharacterZoneStats::EMPTY,
        })
    }

    /// 全部槽位（**只读指针**，D183；顺序 = 进入角色区先后）。
    pub fn slots(&self) -> &[CharacterSlot] {
        &self.slots
    }

    /// 角色数（`Nat`）。
    pub fn len(&self) -> Nat {
        Nat::new(self.slots.len() as _)
    }

    /// 角色区是否为空。
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// 角色区是否已满（3 个槽位）。
    pub fn is_full(&self) -> bool {
        self.len() >= CHARACTER_SLOTS
    }

    /// 还能放几个角色（已满 -> 0）。
    pub fn free_slots(&self) -> Nat {
        nsub!(CHARACTER_SLOTS, self.len())
    }

    /// 这个角色是否在角色区里。
    pub fn contains(&self, character: EntityId) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.character == character)
    }

    /// 某个角色的槽位（只读指针；不在角色区 -> None）。
    pub fn slot_of(&self, character: EntityId) -> Option<&CharacterSlot> {
        self.slots
            .iter()
            .find(|slot| slot.character == character)
    }

    /// 当前出战角色（只读指针；没有 -> None）。
    pub fn active(&self) -> Option<&CharacterSlot> {
        self.active.and_then(|entity| self.slot_of(entity))
    }

    /// 当前出战角色实体号（没有 -> None）。
    pub fn active_entity(&self) -> Option<EntityId> {
        self.active
    }

    /// 这个角色是不是当前出战角色。
    pub fn is_active(&self, character: EntityId) -> bool {
        self.active == Some(character)
    }

    /// 角色区统计（只读指针，D183 / D187；feature 关闭时读到的都是 0）。
    pub fn stats(&self) -> &CharacterZoneStats {
        &self.stats
    }

    /// 把这个角色的**运行期状态**整体回写（生命 / 充能 / 状态 / 附着变化后由调用方回写）。
    ///
    /// 纯函数：返回新角色区；角色不在区里 -> None（不改状态）。
    pub fn with_state(&self, character: EntityId, state: CharacterState) -> Option<CharacterZone> {
        let mut next = self.clone();
        let slot = next
            .slots
            .iter_mut()
            .find(|slot| slot.character == character)?;
        slot.state = state;
        Some(next)
    }

    /// **进入角色区**（开局放置 / 后续加入）+ 派发「进入角色区」。
    ///
    /// 失败（返回 None、不改状态）：角色区已满 / 该角色实体已在区里。
    /// 第一个进入的角色**自动成为出战角色**（正式开局仍应显式 `set_active`，见规则书·游戏准备）。
    pub fn add_with_triggers(
        &self,
        character: EntityId,
        state: CharacterState,
        triggers: &CharacterZoneTriggers,
    ) -> Option<CharacterZoneResolution> {
        if self.is_full() || self.contains(character) {
            return None;
        }
        let mut zone = self.clone();
        zone.slots.push(CharacterSlot::new(character, state));
        if zone.active.is_none() {
            zone.active = Some(character);
        }
        zone.stats.on_entered(Nat::new(1));
        let pending = collect(triggers, CharacterZoneTiming::Entered, entered_event(character));
        Some(CharacterZoneResolution { zone, pending })
    }

    /// **离开角色区**（被击倒 / 移出比赛）+ 派发「离开角色区」。
    ///
    /// 离开的是当前出战角色时，出战指针置空——按规则书·游戏区域 I(5)，牌手**必须**再选一个
    /// 后台角色成为新出战角色；选择本身走 `choose_active_after_defeat_with_triggers`。
    ///
    /// 失败（返回 None、不改状态）：该角色不在区里。
    pub fn remove_with_triggers(
        &self,
        character: EntityId,
        triggers: &CharacterZoneTriggers,
    ) -> Option<CharacterZoneResolution> {
        let position = self
            .slots
            .iter()
            .position(|slot| slot.character == character)?;
        let mut zone = self.clone();
        zone.slots.remove(position);
        if zone.active == Some(character) {
            zone.active = None;
        }
        zone.stats.on_left(Nat::new(1));
        let pending = collect(triggers, CharacterZoneTiming::Left, left_event(character));
        Some(CharacterZoneResolution { zone, pending })
    }

    /// **静默设置出战角色**（游戏准备「各自选择一张角色牌作为出战角色」用；不派发时机）。
    ///
    /// 失败（返回 None、不改状态）：该角色不在角色区 / 已经是当前出战角色 / 角色区为空。
    /// 需要派发时机的「切换角色」走 `switch_active_with_triggers`。
    pub fn set_active(&self, character: EntityId) -> Option<CharacterZone> {
        if !self.contains(character) || self.active == Some(character) {
            return None;
        }
        let mut next = self.clone();
        next.active = Some(character);
        Some(next)
    }

    /// **切换角色**（战斗行动）：把后台角色 `to` 切为出战角色，原出战角色变后台。
    ///
    /// 派发顺序（`结算顺序.md` 未规定，暂定）：`SwitchOut`（原出战）-> `SwitchIn`（新出战）
    /// -> `ActiveChanged`（新出战）。待复核。
    ///
    /// **不做**骰子费用支付（规则书·战斗行动【切换角色】「支付 1 个任意类型的元素骰」）：
    /// 支付归骰池（D121 / D133 / 分工计划 §4.4.15），本层只做区域与时机。
    ///
    /// 失败（返回 None、不改状态）：没有出战角色 / `to` 不在角色区 / `to` 已经是出战角色。
    pub fn switch_active_with_triggers(
        &self,
        to: EntityId,
        triggers: &CharacterZoneTriggers,
    ) -> Option<CharacterZoneResolution> {
        let from = self.active?;
        if from == to || !self.contains(to) {
            return None;
        }
        let mut zone = self.clone();
        zone.active = Some(to);
        zone.stats.on_switched(Nat::new(1));
        let mut pending = collect(
            triggers,
            CharacterZoneTiming::SwitchOut,
            switch_out_event(from, to),
        );
        pending.extend(collect(
            triggers,
            CharacterZoneTiming::SwitchIn,
            switch_in_event(to, from),
        ));
        pending.extend(collect(
            triggers,
            CharacterZoneTiming::ActiveChanged,
            active_changed_event(to),
        ));
        Some(CharacterZoneResolution { zone, pending })
    }

    /// **被击倒后的强制补位**（规则书·游戏区域 I(5)）：选一个后台角色成为新出战角色。
    ///
    /// 与 `switch_active_with_triggers` 的区别：没有「切出」这个动作（原出战角色已被击倒/离开），
    /// 所以只派发 `SwitchIn` + `ActiveChanged`。**待复核**（规则书只说「选择一个后台角色成为新的
    /// 出战角色」，没说它算不算一次「切换角色」行动、会不会触发「切换时」类效果）。
    ///
    /// 失败（返回 None、不改状态）：`to` 不在角色区 / `to` 已经是出战角色。
    pub fn choose_active_after_defeat_with_triggers(
        &self,
        to: EntityId,
        triggers: &CharacterZoneTriggers,
    ) -> Option<CharacterZoneResolution> {
        if !self.contains(to) || self.active == Some(to) {
            return None;
        }
        let mut zone = self.clone();
        zone.active = Some(to);
        let mut pending = collect(
            triggers,
            CharacterZoneTiming::SwitchIn,
            switch_in_event(to, to),
        );
        pending.extend(collect(
            triggers,
            CharacterZoneTiming::ActiveChanged,
            active_changed_event(to),
        ));
        Some(CharacterZoneResolution { zone, pending })
    }

}

/// 收集某个时机 + 事件的待执行（映射成 owned，事件按值带走）。
fn collect(
    triggers: &CharacterZoneTriggers,
    timing: CharacterZoneTiming,
    event: Event,
) -> Vec<CharacterZonePending> {
    dispatch(triggers, timing, &event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 执行一条待执行列表（**调用方显式调用**；派发永不执行）。
pub fn execute_character_zone(pending: &[CharacterZonePending], zone: &mut CharacterZone) {
    for item in pending {
        (item.effect)(zone, &item.event, item.owner);
    }
}

// ======================================================================
// 事件构造（统一口径，避免各卡自己拼字段）
// ======================================================================

/// 「进入角色区」事件：谁进入。
pub fn entered_event(character: EntityId) -> Event {
    Event::of(character)
}

/// 「离开角色区」事件：谁离开。
pub fn left_event(character: EntityId) -> Event {
    Event::of(character)
}

/// 「切出」事件：`who` = 原出战角色，`target` = 新出战角色。
pub fn switch_out_event(from: EntityId, to: EntityId) -> Event {
    Event::new(Some(from), Some(to), Nat::ZERO, None, None)
}

/// 「切入」事件：`who` = 新出战角色，`target` = 原出战角色。
pub fn switch_in_event(to: EntityId, from: EntityId) -> Event {
    Event::new(Some(to), Some(from), Nat::ZERO, None, None)
}

/// 「出战角色变化」事件：`who` = 新出战角色。
pub fn active_changed_event(active: EntityId) -> Event {
    Event::of(active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ops::order::EffectSource;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    fn character(hp: Nat, max_hp: Nat) -> CharacterState {
        CharacterState::new(hp, max_hp, Nat::ZERO, n(2))
    }

    fn slot(character_id: EntityId, hp: Nat) -> CharacterSlot {
        CharacterSlot::new(character_id, character(hp, n(10)))
    }

    /// 示例效果：给事件主体 +1 生命（用来观察「派发到了谁、执行了几次」）。
    fn bump_hp(zone: &mut CharacterZone, event: &Event, _owner: EntityId) {
        let Some(who) = event.who else {
            return;
        };
        let next = match zone.slot_of(who) {
            Some(slot) => slot.state.hp_add(Nat::new(1)),
            None => return,
        };
        if let Some(next_zone) = zone.with_state(who, next) {
            *zone = next_zone;
        }
    }

    fn registry_for(timing: CharacterZoneTiming, owner: EntityId) -> CharacterZoneTriggers {
        let mut triggers = CharacterZoneTriggers::new();
        triggers.register(Trigger::new(timing, owner, EffectSource::CharacterCard, n(1), bump_hp));
        triggers
    }

    /// 建一个「生命值 5 / 上限 10」的角色区（留出加血空间，便于观察触发生效）。
    fn zone_with(count: u32) -> CharacterZone {
        let slots: Vec<CharacterSlot> = (0..count)
            .map(|index| CharacterSlot::new(n(index), character(n(5), n(10))))
            .collect();
        CharacterZone::try_from_slots(slots).expect("合法槽位")
    }

    #[test]
    fn empty_zone_has_no_slots_and_no_active() {
        let zone = CharacterZone::new();
        assert!(zone.is_empty());
        assert_eq!(zone.len(), Nat::ZERO);
        assert_eq!(zone.free_slots(), CHARACTER_SLOTS);
        assert!(zone.active().is_none());
        assert_eq!(zone.active_entity(), None);
        assert!(zone.slots().is_empty());
        assert_eq!(zone.stats().entered().get(), Nat::ZERO);
    }

    #[test]
    fn zone_holds_three_slots_and_rejects_more() {
        let zone = zone_with(3);
        assert!(zone.is_full());
        assert_eq!(zone.len(), n(3));
        // 第 4 个角色进不来（不改状态）。
        let triggers = CharacterZoneTriggers::new();
        assert!(zone.add_with_triggers(n(9), character(n(10), n(10)), &triggers).is_none());
        assert_eq!(zone.len(), n(3));
    }

    #[test]
    fn try_from_slots_rejects_duplicates_and_overflow() {
        assert!(CharacterZone::try_from_slots(vec![
            slot(n(1), n(10)),
            slot(n(1), n(10))
        ])
        .is_none());
        assert!(CharacterZone::try_from_slots(vec![
            slot(n(1), n(10)),
            slot(n(2), n(10)),
            slot(n(3), n(10)),
            slot(n(4), n(10)),
        ])
        .is_none());
        let zone = CharacterZone::try_from_slots(vec![slot(n(1), n(10)), slot(n(2), n(10))])
            .expect("合法");
        // 默认第一张是出战角色。
        assert_eq!(zone.active_entity(), Some(n(1)));
        assert_eq!(zone.slots().len(), 2);
        assert_eq!(zone.slots().first().expect("第一槽").character, n(1));
    }

    #[test]
    fn add_dispatches_entered_and_first_added_becomes_active() {
        let triggers = registry_for(CharacterZoneTiming::Entered, n(1));
        let zone = CharacterZone::new();
        let first = zone
            .add_with_triggers(n(1), character(n(5), n(10)), &triggers)
            .expect("没满");
        assert_eq!(first.zone.active_entity(), Some(n(1)));
        assert_eq!(first.pending.len(), 1);
        assert_eq!(first.pending.first().expect("一条").event.who, Some(n(1)));
        // 执行：进入事件的效果给 1 号 +1 血（5 -> 6）。
        let mut after = first.zone.clone();
        execute_character_zone(&first.pending, &mut after);
        assert_eq!(after.slot_of(n(1)).expect("有 1 号").state.hp(), n(6));
        // 第二个进入不抢出战角色。
        let second = after
            .add_with_triggers(n(2), character(n(4), n(10)), &triggers)
            .expect("没满");
        assert_eq!(second.zone.active_entity(), Some(n(1)));
        assert_eq!(second.zone.len(), n(2));
    }

    #[test]
    fn add_rejects_duplicate_entity() {
        let triggers = CharacterZoneTriggers::new();
        let zone = CharacterZone::new()
            .add_with_triggers(n(1), character(n(5), n(10)), &triggers)
            .expect("第一个")
            .zone;
        assert!(zone.add_with_triggers(n(1), character(n(5), n(10)), &triggers).is_none());
    }

    #[test]
    fn switch_dispatches_out_then_in_then_active_changed() {
        let mut triggers = CharacterZoneTriggers::new();
        // 三条登记都指向同一个效果，靠时机区分：只有对应时机会被派发。
        triggers.register(Trigger::new(
            CharacterZoneTiming::SwitchOut,
            n(1),
            EffectSource::CharacterCard,
            n(0),
            bump_hp,
        ));
        triggers.register(Trigger::new(
            CharacterZoneTiming::SwitchIn,
            n(1),
            EffectSource::CharacterCard,
            n(0),
            bump_hp,
        ));
        triggers.register(Trigger::new(
            CharacterZoneTiming::ActiveChanged,
            n(1),
            EffectSource::CharacterCard,
            n(0),
            bump_hp,
        ));
        let zone = zone_with(3);
        let switched = zone.switch_active_with_triggers(n(2), &triggers).expect("可以切");
        assert_eq!(switched.zone.active_entity(), Some(n(2)));
        // 槽位顺序不变（切换不重排）。
        assert_eq!(
            switched.zone.slots().iter().map(|slot| slot.character).collect::<Vec<_>>(),
            vec![n(0), n(1), n(2)]
        );
        assert_eq!(switched.pending.len(), 3);
        assert_eq!(switched.pending.first().expect("切出").event.who, Some(n(0)));
        assert_eq!(switched.pending.get(1).expect("切入").event.who, Some(n(2)));
        assert_eq!(
            switched.pending.get(2).expect("出战变化").event.who,
            Some(n(2))
        );
        // 执行后：切出方 +1、新出战方 +2（切入 + 出战变化各一次）、无关角色不变。
        let mut after = switched.zone.clone();
        execute_character_zone(&switched.pending, &mut after);
        assert_eq!(after.slot_of(n(0)).expect("0 号").state.hp(), n(6));
        assert_eq!(after.slot_of(n(1)).expect("1 号").state.hp(), n(5));
        assert_eq!(after.slot_of(n(2)).expect("2 号").state.hp(), n(7));
    }

    #[test]
    fn switch_rejects_missing_same_or_no_active() {
        let triggers = CharacterZoneTriggers::new();
        let zone = zone_with(2);
        // 不在角色区
        assert!(zone.switch_active_with_triggers(n(9), &triggers).is_none());
        // 已经是出战角色
        assert!(zone.switch_active_with_triggers(n(0), &triggers).is_none());
        // 没有出战角色
        let empty = CharacterZone::new();
        assert!(empty.switch_active_with_triggers(n(0), &triggers).is_none());
    }

    #[test]
    fn removing_active_clears_active_and_dispatches_left() {
        let triggers = registry_for(CharacterZoneTiming::Left, n(1));
        let zone = zone_with(2);
        let removed = zone.remove_with_triggers(n(0), &triggers).expect("在区里");
        assert_eq!(removed.pending.len(), 1);
        assert_eq!(removed.zone.len(), n(1));
        assert_eq!(removed.zone.active_entity(), None);
        // 移除不存在的角色 -> None
        assert!(removed.zone.remove_with_triggers(n(9), &triggers).is_none());
    }

    #[test]
    fn choose_active_after_defeat_dispatches_in_and_changed() {
        let mut triggers = CharacterZoneTriggers::new();
        triggers.register(Trigger::new(
            CharacterZoneTiming::SwitchIn,
            n(1),
            EffectSource::CharacterCard,
            n(0),
            bump_hp,
        ));
        let zone = zone_with(2);
        let chosen = zone
            .choose_active_after_defeat_with_triggers(n(1), &triggers)
            .expect("补位");
        assert_eq!(chosen.zone.active_entity(), Some(n(1)));
        // 没有切出方：只派发切入 + 出战变化（后者没登记 -> 只有一条）。
        assert_eq!(chosen.pending.len(), 1);
        assert_eq!(chosen.pending.first().expect("切入").event.who, Some(n(1)));
    }

    #[test]
    fn set_active_is_silent_and_validates() {
        let zone = zone_with(2);
        let next = zone.set_active(n(1)).expect("可以设");
        assert_eq!(next.active_entity(), Some(n(1)));
        // 静默：不派发任何东西（返回的是纯状态，不带待执行列表）。
        assert_eq!(next.len(), zone.len());
        // 已经是出战 / 不在区里 -> None
        assert!(next.set_active(n(1)).is_none());
        assert!(zone.set_active(n(9)).is_none());
    }

    #[test]
    fn with_state_writes_back_and_is_pure() {
        let zone = zone_with(1);
        let before = zone.slot_of(n(0)).expect("有").state.clone();
        assert_eq!(before.hp(), n(5));
        let next = zone
            .with_state(n(0), before.hp_sub(Nat::new(3)))
            .expect("有 0 号");
        assert_eq!(next.slot_of(n(0)).expect("有").state.hp(), n(2));
        // 入参不改
        assert_eq!(zone.slot_of(n(0)).expect("有").state.hp(), n(5));
        assert!(zone.with_state(n(9), before).is_none());
    }

    #[test]
    fn stats_follow_feature_flag() {
        let triggers = CharacterZoneTriggers::new();
        let zone = zone_with(2);
        let zone = zone.switch_active_with_triggers(n(1), &triggers).expect("切").zone;
        if cfg!(feature = "stats") {
            assert_eq!(zone.stats().switches().get(), n(1));
        } else {
            assert!(zone.stats().switches().is_zero());
            assert_eq!(std::mem::size_of::<CharacterZoneStats>(), 0);
        }
    }
}
