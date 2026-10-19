//! **一方的全部区**：`PlayerDefs`（我方 / 敌方各一个具名实例，D34 / D42）。
//!
//! ## 依据（各区自己的文件里有逐条规则书依据；本层只负责「装在一起」）
//!
//! | 区 | 类型 | 所在文件 |
//! |---|---|---|
//! | 骰池 | `DicePool` | `resources/dice/mod.rs`（纯计数，上限 16，D47/D48） |
//! | 手牌 | `HandState` | `families/action_card/zones/hand/mod.rs`（上限 10，D90 起手） |
//! | 牌库 | `PileState` | `families/action_card/zones/pile/mod.rs`（位置为键 + 段 + 公共状态列表） |
//! | 手牌卡状态 | `HandCardStates` | `families/action_card/areas/hand_state.rs`（减费 / 赋能…） |
//! | 每张手牌的当前费用 | `HandCosts` | 本文件（承载 `areas/cost.rs` 的 `CurrentCost`） |
//! | 角色区 | `CharacterZone` | `families/character/zones/field.rs`（3 槽位 + 出战指针 + 切换） |
//! | 出战状态区 | `CombatStatusZone` | `families/character/zones/combat_status.rs`（挂出战角色、切换时转移） |
//! | 每角色装备栏 | `Vec<EquipmentZone>` | `families/character/zones/equipment.rs`（每角色一个实例） |
//! | 召唤物区 | `SummonZone` | `families/summon/zones/mod.rs`（上限 4、同名合并、可用次数） |
//! | 支援区 | `SupportZone` | `families/action_card/areas/support.rs`（上限 4、满 4 选择弃置） |
//! | 每回合使用计数 | `SkillUses` | `families/character/skill_flow/usage.rs`（回合开始 `reset_round`） |
//! | 下落攻击标记 | `PlungingMark` | `families/character/skill_flow/attack_form.rs`（切换后待消耗；回合边界 / 战斗行动结束清除） |
//! | 击倒区 | `Vec<CharacterSlot>` | 本文件（**内部记录**：被击倒角色的身份 + 清理后状态，供复苏放回角色区；不是规则书里的可见区域，TODO 待实测） |
//!
//! ## 为什么没有「区号 / 区表」（D42）
//!
//! 区就是本结构的**字段**；要哪个区就写哪个字段，编译后是直接内存寻址，不查表、不登记。
//! 「这是谁的手牌」由持有它的**实例**表达：`DefState` 的 `me` / `opp` 两个字段（D34）。
//!
//! ## 只读指针（D183）
//!
//! 字段私有；对外一律 `&self` + `&T`（不返回可变引用、不拷贝）。例：
//! `player.hand()` -> `&HandState`、`player.equipment_of(c)` -> `Option<&EquipmentZone>`。
//! 可变入口只有 `pub(crate) fn parts_mut`（只给本 crate 的 `state::flows` 用）。
//!
//! ## 每张手牌的当前费用（`HandCosts`）
//!
//! 手牌区（`HandState`）只存「实体号 + 卡 id」；**费用是这张牌的另一个属性**，
//! 由本文件的 `HandCosts` 用实体号做键另存（与 `HandCardStates` 同一套做法，不改手牌容器）。
//! 基础费用来自卡牌定义（上层给），加费 / 减费在 `CurrentCost` 里叠 delta（D170/D172/D178）。
//! 手牌一进一出后调用 [`PlayerDefs::with_synced_hand_costs`] 同步：新牌按基础费用建、离手的牌删掉、已有 delta 保留。

use crate::base::CardId;
use crate::families::action_card::areas::cost::CurrentCost;
use crate::families::action_card::areas::hand_state::HandCardStates;
use crate::families::action_card::areas::support::SupportZone;
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::pile::PileState;
use crate::families::character::skill_flow::{PlungingMark, SkillUses};
use crate::families::character::zones::combat_status::CombatStatusZone;
use crate::families::character::zones::equipment::EquipmentZone;
use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
use crate::families::summon::zones::SummonZone;
use crate::resources::dice::{CostComponent, DicePool};
use gi_genius_invokation_engine::{EntityId, Nat};

/// **每张手牌的当前元素骰费用表**（手牌实体号 -> 当前费用）。
///
/// - 手牌无序：本表只按实体号存，**不承诺顺序**（顺序 = 手牌容器当前顺序，仅供复现）；
/// - 一张牌离开手牌 -> 这一项删掉（费用跟着这张牌，牌不在手牌就没有「当前费用」可言）；
/// - 纯函数：`with_cost` / `without` / `synced` 都返回新表，不改入参；
/// - 对外只读指针（D183）：`entries()` / `cost_of()`。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HandCosts {
    /// (手牌实体号, 当前费用)。没有用 `HashMap`（全仓禁用）。
    entries: Vec<(EntityId, CurrentCost)>,
}

impl HandCosts {
    /// 空费用表。
    pub const fn new() -> HandCosts {
        HandCosts {
            entries: Vec::new(),
        }
    }

    /// 有费用的手牌张数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否一张都没有。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 全部条目（**只读指针**，D183）。
    pub fn entries(&self) -> &[(EntityId, CurrentCost)] {
        &self.entries
    }

    /// 这张手牌有没有费用记录。
    pub fn contains(&self, entity: EntityId) -> bool {
        self.entries.iter().any(|entry| entry.0 == entity)
    }

    /// 这张手牌的当前费用（**只读指针**；不在表里 -> None）。
    pub fn cost_of(&self, entity: EntityId) -> Option<&CurrentCost> {
        self.entries
            .iter()
            .find(|entry| entry.0 == entity)
            .map(|entry| &entry.1)
    }

    /// 写入 / 覆盖一张手牌的费用（纯函数）。
    pub fn with_cost(&self, entity: EntityId, cost: CurrentCost) -> HandCosts {
        let mut next = self.clone();
        match next.entries.iter_mut().find(|entry| entry.0 == entity) {
            Some(entry) => entry.1 = cost,
            None => next.entries.push((entity, cost)),
        }
        next
    }

    /// 删掉一张手牌的费用（纯函数；本来没有也返回等价的表）。
    pub fn without(&self, entity: EntityId) -> HandCosts {
        let mut next = self.clone();
        next.entries.retain(|entry| entry.0 != entity);
        next
    }

    /// **跟当前手牌同步**（纯函数）：
    ///
    /// - 新手牌里的每张牌：已有费用记录 -> **保留**（含已叠加的加减费 delta）；
    ///   没有 -> 用 `base_cost(card)` 建一个 `CurrentCost`；
    /// - 已经不在手牌里的牌 -> 丢掉；
    /// - 顺序 = 当前手牌容器顺序（可复现）。
    pub fn synced(
        &self,
        hand: &HandState,
        base_cost: &dyn Fn(CardId) -> Vec<CostComponent>,
    ) -> HandCosts {
        let mut next = HandCosts::new();
        for instance in hand.entries() {
            let cost = match self.cost_of(instance.entity) {
                Some(existing) => existing.clone(),
                None => CurrentCost::new(base_cost(instance.card)),
            };
            next.entries.push((instance.entity, cost));
        }
        next
    }
}

/// **一方的全部区**（我方 / 敌方各一个实例，D34）。
///
/// 容器没有身份（D42 / D182）：它就是 `DefState` 里的一个字段；「这是谁的全部区」由
/// `DefState.me` / `DefState.opp` 两个具名实例表达，本类型不出现敌我参数、不出现区号。
///
/// 相等语义：`PartialEq` 按字段比较，**统计不参与**（各容器的 `PartialEq` 已忽略 stats，D187）。
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerDefs {
    /// 骰池（纯计数，上限 16）。
    dice: DicePool,
    /// 手牌（上限 10）。
    hand: HandState,
    /// 牌库（位置为键 + 段 + 公共状态列表）。
    pile: PileState,
    /// 每张手牌的当前费用。
    costs: HandCosts,
    /// 角色区（3 槽位 + 出战指针）。
    characters: CharacterZone,
    /// 出战状态区（挂在出战角色上）。
    combat_status: CombatStatusZone,
    /// 每角色一个装备栏（用 `EquipmentZone::owner()` 对上是哪个角色）。
    equipment: Vec<EquipmentZone>,
    /// 召唤物区（上限 4）。
    summons: SummonZone,
    /// 支援区（上限 4）。
    supports: SupportZone,
    /// 手牌卡状态（减费 / 赋能…，按手牌实体号）。
    hand_states: HandCardStates,
    /// 每回合技能使用计数。
    skill_uses: SkillUses,
    /// **下落攻击**待消耗标记（S8.10：切换为出战角色后，本回合下一个战斗行动若为普通攻击
    /// 则视为下落攻击）。
    ///
    /// 写入点 / 清除点见 `state::flows::plunging`；回合边界由
    /// `state::flows::round_begin::begin_round` 自动清除。
    plunging: PlungingMark,
    /// **击倒区**：被击倒清理移出角色区的角色（身份 + 清理后的运行期状态）。
    ///
    /// - **规则书没有这个可见区域**：其他规则 击倒只说「角色就会被击倒」，
    ///   没说击倒后的角色牌放在哪（`defeat.rs` 的待复核同款问题）；
    /// - 这里只是**内部记录**：不参与结算、不派发时机，唯一用途是让复苏
    ///   （`state::flows::revive`）把角色放回角色区（需要角色身份 + 生命上限等数据）；
    /// - 顺序 = 被击倒的先后（可复现）；TODO：规则书 / 实测若给出正式「击倒区」规则，按它改。
    defeated: Vec<CharacterSlot>,
}

impl Default for PlayerDefs {
    fn default() -> PlayerDefs {
        PlayerDefs::new()
    }
}

impl PlayerDefs {
    /// 完全空的一方的区（骰池空、手牌空、牌库空、没有角色 / 召唤物 / 支援 / 装备）。
    pub const fn new() -> PlayerDefs {
        PlayerDefs {
            dice: DicePool::empty(),
            hand: HandState::new(),
            pile: PileState::new(),
            costs: HandCosts::new(),
            characters: CharacterZone::new(),
            combat_status: CombatStatusZone::new(),
            equipment: Vec::new(),
            summons: SummonZone::new(),
            supports: SupportZone::new(),
            hand_states: HandCardStates::new(),
            skill_uses: SkillUses::new(),
            plunging: PlungingMark::NONE,
            defeated: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // 只读指针（D183；对外唯一的读取形态）
    // ------------------------------------------------------------------

    /// 骰池（只读指针）。
    pub fn dice(&self) -> &DicePool {
        &self.dice
    }

    /// 手牌（只读指针）。
    pub fn hand(&self) -> &HandState {
        &self.hand
    }

    /// 牌库（只读指针）。
    pub fn pile(&self) -> &PileState {
        &self.pile
    }

    /// 每张手牌的当前费用表（只读指针）。
    pub fn costs(&self) -> &HandCosts {
        &self.costs
    }

    /// 某张手牌的当前费用（**只读指针**；不在手牌 / 没有记录 -> None）。
    pub fn cost_of(&self, entity: EntityId) -> Option<&CurrentCost> {
        self.costs.cost_of(entity)
    }

    /// 角色区（只读指针）。
    pub fn characters(&self) -> &CharacterZone {
        &self.characters
    }

    /// 出战状态区（只读指针）。
    pub fn combat_status(&self) -> &CombatStatusZone {
        &self.combat_status
    }

    /// 全部角色的装备栏（只读指针；顺序 = 建栏先后）。
    pub fn equipment(&self) -> &[EquipmentZone] {
        &self.equipment
    }

    /// 某个角色的装备栏（**只读指针**；这个角色没有装备栏 -> None）。
    pub fn equipment_of(&self, character: EntityId) -> Option<&EquipmentZone> {
        self.equipment
            .iter()
            .find(|zone| zone.owner() == character)
    }

    /// 召唤物区（只读指针）。
    pub fn summons(&self) -> &SummonZone {
        &self.summons
    }

    /// 支援区（只读指针）。
    pub fn supports(&self) -> &SupportZone {
        &self.supports
    }

    /// 手牌卡状态表（只读指针）。
    pub fn hand_states(&self) -> &HandCardStates {
        &self.hand_states
    }

    /// 每回合技能使用计数（只读指针）。
    pub fn skill_uses(&self) -> &SkillUses {
        &self.skill_uses
    }

    /// **下落攻击待消耗标记**（S8.10；纯值拷贝，不是指针）。
    pub fn plunging(&self) -> PlungingMark {
        self.plunging
    }

    /// **击倒区**（只读指针；顺序 = 被击倒先后；内部记录，不是规则书可见区域）。
    pub fn defeated(&self) -> &[CharacterSlot] {
        &self.defeated
    }

    /// 击倒区里某个角色（**只读指针**；不在击倒区 -> None）。
    pub fn defeated_character(&self, character: EntityId) -> Option<&CharacterSlot> {
        self.defeated
            .iter()
            .find(|slot| slot.character == character)
    }

    // ------------------------------------------------------------------
    // 纯函数派生（对外：吃 &self、返回新值，不改入参）
    // ------------------------------------------------------------------

    /// 跟当前手牌同步费用表（见 [`HandCosts::synced`]）。
    pub fn with_synced_hand_costs(
        &self,
        base_cost: &dyn Fn(CardId) -> Vec<CostComponent>,
    ) -> PlayerDefs {
        let mut next = self.clone();
        next.costs = self.costs.synced(&self.hand, base_cost);
        next
    }

    /// 回合开始：所有技能的「本回合已用次数」清零（上限保留；规则书·回合流程「开始新的回合」）。
    ///
    /// 复用 `skill_flow::usage::SkillUses::reset_round`（纯函数）。
    pub fn with_skill_uses_reset(&self) -> PlayerDefs {
        let mut next = self.clone();
        next.skill_uses = self.skill_uses.reset_round();
        next
    }

    /// 写入**下落攻击待消耗标记**（纯函数；切换成功后把 flow 返回的
    /// `SwitchCharacterOutcome::plunging` 写回这里）。
    pub fn with_plunging(&self, plunging: PlungingMark) -> PlayerDefs {
        PlayerDefs {
            plunging,
            ..self.clone()
        }
    }

    /// **一次战斗行动结束**：清除下落攻击标记（S8.10；纯函数）。
    ///
    /// 调用点见 `state::flows::plunging::battle_action_end_me/opp`。
    pub fn with_plunging_after_combat_action(&self) -> PlayerDefs {
        self.with_plunging(self.plunging.after_combat_action())
    }

    /// **回合边界**：清除下落攻击标记（S8.10「本回合内」；纯函数）。
    ///
    /// `state::flows::round_begin::begin_round` 已自动调用，不需要调用方再做。
    pub fn with_plunging_cleared_round(&self) -> PlayerDefs {
        self.with_plunging(self.plunging.cleared_round())
    }

    // ------------------------------------------------------------------
    // 构造用（对外：建局时把各区的初始值填进来；纯函数，返回新值）
    // ------------------------------------------------------------------
    //
    // 字段私有（D183），所以外部要"造一个非空的 PlayerDefs"只能用这些 with_*。
    // 它们都是纯函数：吃 &self、返回新值，不改入参；可以先 Default::default() 再逐个填。

    /// 换掉骰池（纯函数）。
    pub fn with_dice(&self, dice: DicePool) -> PlayerDefs {
        PlayerDefs {
            dice,
            ..self.clone()
        }
    }

    /// 换掉手牌（纯函数；**不**同步费用表，需要时再调 [`PlayerDefs::with_synced_hand_costs`]）。
    pub fn with_hand(&self, hand: HandState) -> PlayerDefs {
        PlayerDefs {
            hand,
            ..self.clone()
        }
    }

    /// 换掉牌库（纯函数）。
    pub fn with_pile(&self, pile: PileState) -> PlayerDefs {
        PlayerDefs {
            pile,
            ..self.clone()
        }
    }

    /// 换掉每张手牌的当前费用表（纯函数）。
    pub fn with_costs(&self, costs: HandCosts) -> PlayerDefs {
        PlayerDefs {
            costs,
            ..self.clone()
        }
    }

    /// 换掉角色区（纯函数）。
    pub fn with_characters(&self, characters: CharacterZone) -> PlayerDefs {
        PlayerDefs {
            characters,
            ..self.clone()
        }
    }

    /// 换掉出战状态区（纯函数）。
    pub fn with_combat_status(&self, combat_status: CombatStatusZone) -> PlayerDefs {
        PlayerDefs {
            combat_status,
            ..self.clone()
        }
    }

    /// 放 / 换一个角色的装备栏（纯函数；按 `EquipmentZone::owner()` 覆盖同一个角色的旧装备栏）。
    pub fn with_equipment_zone(&self, zone: EquipmentZone) -> PlayerDefs {
        let owner = zone.owner();
        let mut next = self.clone();
        match next
            .equipment
            .iter_mut()
            .find(|existing| existing.owner() == owner)
        {
            Some(existing) => *existing = zone,
            None => next.equipment.push(zone),
        }
        next
    }

    /// 换掉召唤物区（纯函数）。
    pub fn with_summons(&self, summons: SummonZone) -> PlayerDefs {
        PlayerDefs {
            summons,
            ..self.clone()
        }
    }

    /// 换掉支援区（纯函数）。
    pub fn with_supports(&self, supports: SupportZone) -> PlayerDefs {
        PlayerDefs {
            supports,
            ..self.clone()
        }
    }

    /// 换掉手牌卡状态表（纯函数）。
    pub fn with_hand_states(&self, hand_states: HandCardStates) -> PlayerDefs {
        PlayerDefs {
            hand_states,
            ..self.clone()
        }
    }

    /// 换掉每回合技能使用计数（纯函数）。
    pub fn with_skill_uses(&self, skill_uses: SkillUses) -> PlayerDefs {
        PlayerDefs {
            skill_uses,
            ..self.clone()
        }
    }

    /// 换掉**击倒区**（纯函数；建局 / 测试注入用）。
    ///
    /// 正式流程不要直接调它：击倒清理写它（`state::flows::defeat`）、
    /// 复苏从它移除（`state::flows::revive`）。
    pub fn with_defeated(&self, defeated: Vec<CharacterSlot>) -> PlayerDefs {
        PlayerDefs {
            defeated,
            ..self.clone()
        }
    }

    // ------------------------------------------------------------------
    // 可变入口（**只给本 crate 的 state::flows 用**；D183：crate 外拿不到）
    // ------------------------------------------------------------------

    /// 流程要用的区的可变引用（`pub(crate)`；只有 `state::flows` 这一层用）。
    ///
    /// 为什么不是一堆 `set_xxx`：流程要同时读一个区、写另一个区（切换角色 -> 出战状态转移），
    /// 一次借出多个字段最省事；**对外仍然只有只读指针**（本方法 crate 外不可见）。
    pub(crate) fn parts_mut(&mut self) -> PlayerParts<'_> {
        PlayerParts {
            hand: &mut self.hand,
            pile: &mut self.pile,
            characters: &mut self.characters,
            combat_status: &mut self.combat_status,
            equipment: &mut self.equipment,
            summons: &mut self.summons,
            defeated: &mut self.defeated,
        }
    }
}

/// [`PlayerDefs`] 里**流程会用到的区**的可变引用（**仅 crate 内可用**）。
///
/// 字段按 `state::flows` 的实际需要逐步增加（没被流程写的区就不借出来，避免「借了不用」）。
pub(crate) struct PlayerParts<'a> {
    /// 手牌。
    pub(crate) hand: &'a mut HandState,
    /// 牌库。
    pub(crate) pile: &'a mut PileState,
    /// 角色区。
    pub(crate) characters: &'a mut CharacterZone,
    /// 出战状态区。
    pub(crate) combat_status: &'a mut CombatStatusZone,
    /// 每角色装备栏。
    pub(crate) equipment: &'a mut Vec<EquipmentZone>,
    /// 召唤物区。
    pub(crate) summons: &'a mut SummonZone,
    /// 击倒区（击倒清理写入 / 复苏移除）。
    pub(crate) defeated: &'a mut Vec<CharacterSlot>,
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::testing::{characters, hand, n, pile, CARD_A};
    use crate::base::CardId;
    use crate::resources::dice::CostComponent;

    fn base_three(_card: CardId) -> Vec<CostComponent> {
        vec![CostComponent::AnyColor(n(3))]
    }

    #[test]
    fn new_player_is_completely_empty() {
        let player = PlayerDefs::new();
        assert_eq!(player.dice().total(), Nat::ZERO);
        assert!(player.hand().is_empty());
        assert!(player.pile().is_empty());
        assert!(player.costs().is_empty());
        assert!(player.characters().is_empty());
        assert!(player.combat_status().is_empty());
        assert!(player.equipment().is_empty());
        assert!(player.summons().is_empty());
        assert!(player.supports().is_empty());
        assert!(player.hand_states().is_empty());
        assert!(player.skill_uses().is_empty());
        assert!(player.defeated().is_empty());
        assert!(!player.plunging().is_pending());
        // Default = new
        assert_eq!(PlayerDefs::default(), PlayerDefs::new());
    }

    #[test]
    fn builders_are_pure_and_replace_one_zone_at_a_time() {
        let base = PlayerDefs::new();
        let built = base
            .with_pile(pile(&[101, 102]))
            .with_hand(hand(&[201]))
            .with_characters(characters(&[301, 302]));
        assert_eq!(built.pile().len(), Nat::new(2));
        assert_eq!(built.hand().len(), Nat::new(1));
        assert_eq!(built.characters().len(), Nat::new(2));
        // 纯函数：base 一个字节都没动
        assert_eq!(base, PlayerDefs::new());
    }

    #[test]
    fn plunging_mark_is_pure_with_two_clear_points() {
        let base = PlayerDefs::new();
        // 建局：没有标记
        assert!(!base.plunging().is_pending());
        // 切换成功后置位（S8.10）
        let armed = base.with_plunging(PlungingMark::new().armed());
        assert!(armed.plunging().is_pending());
        // 纯函数：入参不变
        assert!(!base.plunging().is_pending());
        // 清除点 1：一次战斗行动结束
        assert!(!armed
            .with_plunging_after_combat_action()
            .plunging()
            .is_pending());
        // 清除点 2：回合边界
        assert!(!armed.with_plunging_cleared_round().plunging().is_pending());
        // 布尔标记：重复置位与一次置位等价（规则书未写「刷新」，留空 #60）
        assert_eq!(base.with_plunging(PlungingMark::new().armed().armed()), armed);
    }

    #[test]
    fn defeated_zone_is_pure_and_queryable() {
        // 击倒区的角色：HP 已 0（击倒清理后的状态）
        let dead = CharacterSlot::new(
            n(901),
            crate::state::testing::character().hp_sub(n(10)),
        );
        let player = PlayerDefs::new().with_defeated(vec![dead]);
        assert_eq!(player.defeated().len(), 1);
        assert_eq!(
            player.defeated_character(n(901)).expect("在击倒区").state.hp(),
            Nat::ZERO
        );
        assert!(player.defeated_character(n(999)).is_none());
        // 纯函数：新建的一方不受影响
        assert!(PlayerDefs::new().defeated().is_empty());
        // 角色区不被影响（击倒区是独立记录）
        assert!(player.characters().is_empty());
    }

    #[test]
    fn equipment_zone_is_upserted_by_owner() {
        let first = crate::state::testing::equipment(n(301), &[(11, crate::rules::ops::equip::SlotKind::Artifact, 312001)]);
        let second = crate::state::testing::equipment(n(302), &[(21, crate::rules::ops::equip::SlotKind::Artifact, 312002)]);
        let replacement = crate::state::testing::equipment(n(301), &[(31, crate::rules::ops::equip::SlotKind::Artifact, 312003)]);
        let player = PlayerDefs::new()
            .with_equipment_zone(first)
            .with_equipment_zone(second);
        assert_eq!(player.equipment().len(), 2);
        assert_eq!(
            player.equipment_of(n(301)).expect("有").entries().first().expect("一件").equip,
            n(11)
        );
        // 同一个角色再放一次 -> 覆盖，不新增
        let replaced = player.with_equipment_zone(replacement);
        assert_eq!(replaced.equipment().len(), 2);
        assert_eq!(
            replaced.equipment_of(n(301)).expect("有").entries().first().expect("一件").equip,
            n(31)
        );
    }

    #[test]
    fn synced_hand_costs_adds_new_keeps_deltas_and_drops_stale_entries() {
        // 手牌 2 张：先按基础 3 费同步，再给一张减 1 费，最后手牌换人。
        let player = PlayerDefs::new().with_hand(hand(&[101, 102]));
        let synced = player.with_synced_hand_costs(&base_three);
        assert_eq!(synced.costs().len(), Nat::new(2));
        assert_eq!(
            synced.cost_of(n(101)).expect("有费用").total(),
            Nat::new(3)
        );

        // 减 1 费（等价于「本回合打出的下一张费用 -1」这类效果）
        let updated = synced
            .cost_of(n(101))
            .expect("有费用")
            .with_decrease(Nat::new(1));
        let discounted = synced.with_costs(synced.costs().with_cost(n(101), updated));
        assert_eq!(
            discounted.cost_of(n(101)).expect("有费用").total(),
            Nat::new(2)
        );

        // 手牌换人：101 离手、103 上手 -> 同步后只剩 102（保留基础）/ 103（新加），101 的减费记录消失
        let next_hand = hand(&[102, 103]);
        let after = discounted
            .with_hand(next_hand)
            .with_synced_hand_costs(&base_three);
        assert_eq!(after.costs().len(), Nat::new(2));
        assert!(after.cost_of(n(101)).is_none());
        assert_eq!(after.cost_of(n(102)).expect("有").total(), Nat::new(3));
        assert_eq!(after.cost_of(n(103)).expect("有").total(), Nat::new(3));
    }

    #[test]
    fn hand_costs_entries_are_read_only_pointers() {
        let costs = HandCosts::new().with_cost(n(101), CurrentCost::new(Vec::new()));
        let first: &CurrentCost = costs.cost_of(n(101)).expect("有");
        let second: &CurrentCost = costs.cost_of(n(101)).expect("有");
        // 只读指针：两次拿到的指向同一份数据（不是拷贝）
        assert!(std::ptr::addr_eq(first, second));
        // 纯函数：with_cost / without 返回新表，入参不变
        let removed = costs.without(n(101));
        assert!(removed.cost_of(n(101)).is_none());
        assert!(costs.cost_of(n(101)).is_some());
        assert_eq!(costs.len(), Nat::new(1));
    }

    #[test]
    fn read_only_pointers_point_at_internal_data() {
        let player = PlayerDefs::new()
            .with_pile(pile(&[101, 102]))
            .with_hand(hand(&[201]))
            .with_dice(DicePool::new(Nat::new(2), Vec::new(), Vec::new()));
        // 类型层面：拿到的就是 &T（不是拷贝、不是可变引用）
        let dice: &DicePool = player.dice();
        let hand: &HandState = player.hand();
        let pile: &PileState = player.pile();
        let characters: &CharacterZone = player.characters();
        let summons: &SummonZone = player.summons();
        // 两次调用指向同一份数据（内部字段的地址）
        assert!(std::ptr::addr_eq(dice, player.dice()));
        assert!(std::ptr::addr_eq(hand, player.hand()));
        assert!(std::ptr::addr_eq(pile, player.pile()));
        assert!(std::ptr::addr_eq(characters, player.characters()));
        assert!(std::ptr::addr_eq(summons, player.summons()));
        // 读出来的值正确
        assert_eq!(dice.total(), Nat::new(2));
        assert_eq!(hand.len(), Nat::new(1));
        assert_eq!(pile.len(), Nat::new(2));
        assert_eq!(CARD_A, pile.entries().first().expect("有").card);
    }

    #[test]
    fn with_skill_uses_reset_is_pure_and_keeps_limits() {
        let skill = crate::base::SkillId::new(12161);
        let uses = SkillUses::new()
            .with_limit(skill, Nat::new(1))
            .recorded(skill);
        let player = PlayerDefs::new().with_skill_uses(uses);
        let next = player.with_skill_uses_reset();
        assert_eq!(next.skill_uses().used_count(skill), Nat::ZERO);
        assert_eq!(next.skill_uses().limit(skill), Some(Nat::new(1)));
        // 入参不变
        assert_eq!(player.skill_uses().used_count(skill), Nat::new(1));
    }
}