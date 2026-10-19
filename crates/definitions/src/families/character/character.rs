//! 角色运行期状态与**底层数值操作**（D121 / D133 / D138）。
//!
//! 本文件只做数值：**生命** 加 / 减 / 设 / 读、**充能** 加 / 减 / 设 / 读，
//! 以及两个上限的读。**不做**任何业务语义封装（D133）：
//! - 不写「受到伤害 / 治疗 / 倒下 / 免于击倒 / 强制选人」这类高层入口
//!   （伤害入口在 damage.rs，仍然只产记录 + 只做 `hp_sub`；倒下等属于后续统一包装层，D8 / D133）；
//! - **不写「获取充能（布尔值）」**（D138：用户原话「布尔获取充能也不做，自己操作」）；
//! - **不做骰子消耗 / 充能支付**（D121 / D133：技能消耗格式在 skill.rs；
//!   支付 / 扣减在「被花的东西」那边 —— 骰池、角色充能；本文件只到 `energy_sub` 这一层数值）。
//!
//! **纯函数风格**（与 zones 一致）：操作吃 `&self`、返回新状态，不改入参；
//! 边界一律用 `Nat` 的饱和运算（加 / 减 / 乘饱和，§8.3 禁止裸 `+ - * /`）。
//!
//! 字段说明：
//! - 生命 / 充能的**当前值**与**上限**分开存；当前值永远不超过上限（构造与所有写操作都钳住）；
//! - `states`：这个角色身上的**状态**（D105 / D112：所有状态共用一张通用列表；
//!   本块只持有容器字段，状态的增删查归状态块，D133）；
//! - `attachment`：元素附着（D130；**不是状态**，不放进 `StateList`），见 attachment.rs。

use crate::families::action_card::status::StateList;
use crate::families::character::attachment::Attachment;
use crate::families::character::definition::CharacterDef;
use gi_genius_invokation_engine::{nadd, nsub, Nat};

/// 一个角色的状态：生命 / 充能（当前值 + 上限）+ 状态列表 + 元素附着。
///
/// 容器没有身份（D42）：它就是状态里的一个字段；「这是谁」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34），这里不出现敌我枚举或区号。
///
/// 不变式（本类型维护）：
/// 1. `hp <= max_hp`；`energy <= max_energy`（构造与写操作都钳住）；
/// 2. 生命 / 充能的**上限本身**只能通过构造给出（本块不提供「改上限」操作，
///    上限变更属于卡牌效果 / 后续层）。
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CharacterState {
    /// 当前生命。
    pub hp: Nat,
    /// 生命上限。
    pub max_hp: Nat,
    /// 当前充能。
    pub energy: Nat,
    /// 充能上限。
    pub max_energy: Nat,
    /// 这个角色身上的**状态**（D105 / D112 的通用列表；本块只放字段，不解释状态语义）。
    pub states: StateList,
    /// 元素附着（D130；不是状态，见 attachment.rs）。
    pub attachment: Attachment,
}

impl CharacterState {
    /// 建一个角色状态：生命 / 充能先各自钳到自己的上限，状态列表与附着为空。
    ///
    /// 参数顺序 = (当前生命, 生命上限, 当前充能, 充能上限)。
    pub fn new(hp: Nat, max_hp: Nat, energy: Nat, max_energy: Nat) -> CharacterState {
        CharacterState {
            hp: at_most(hp, max_hp),
            max_hp,
            energy: at_most(energy, max_energy),
            max_energy,
            states: StateList::new(),
            attachment: Attachment::none(),
        }
    }

    /// 按角色牌定义建**初始状态**（用户裁决的上限口径）：
    ///
    /// 生命 = 生命上限、充能 = 0；状态列表 / 元素附着 / 准备技能为空。
    /// 上限是**定义数据**（~CharacterDef::max_hp~ / ~max_energy~），运行期当前值在这里。
    pub fn from_def<Tags>(def: &CharacterDef<Tags>) -> CharacterState {
        CharacterState {
            hp: def.max_hp,
            max_hp: def.max_hp,
            energy: Nat::ZERO,
            max_energy: def.max_energy,
            states: StateList::new(),
            attachment: Attachment::none(),
        }
    }

    /// 读当前生命。
    pub fn hp(&self) -> Nat {
        self.hp
    }

    /// 读生命上限。
    pub fn max_hp(&self) -> Nat {
        self.max_hp
    }

    /// 读当前充能。
    pub fn energy(&self) -> Nat {
        self.energy
    }

    /// 读充能上限。
    pub fn max_energy(&self) -> Nat {
        self.max_energy
    }

    /// 生命**加**：加到上限为止（不会超过 `max_hp`）。返回新状态，不改入参。
    ///
    /// 只做数值：本方法**不代表**治疗 / 回复这类语义，调用方自己组合。
    pub fn hp_add(&self, amount: Nat) -> CharacterState {
        let mut next = self.clone();
        next.hp = at_most(nadd!(self.hp, amount), self.max_hp);
        next
    }

    /// 生命**减**：减到 0 为止（饱和减法，不为负）。返回新状态，不改入参。
    ///
    /// 只做数值：本方法**不代表**受到伤害；生命到 0 也**不代表**倒下
    /// （倒下 / 免于击倒属于后续统一包装层，D8 / D133）。
    pub fn hp_sub(&self, amount: Nat) -> CharacterState {
        let mut next = self.clone();
        next.hp = nsub!(self.hp, amount);
        next
    }

    /// 生命**设**：钳到 `[0, max_hp]` 后写入。返回新状态，不改入参。
    pub fn hp_set(&self, value: Nat) -> CharacterState {
        let mut next = self.clone();
        next.hp = at_most(value, self.max_hp);
        next
    }

    /// 充能**加**：加到上限为止（不会超过 `max_energy`）。返回新状态，不改入参。
    ///
    /// 只做数值：**不提供**「本次是否获得充能」这类布尔封装（D138），
    /// 是否真的获得 / 溢出多少由调用方自己读前后值判断。
    pub fn energy_add(&self, amount: Nat) -> CharacterState {
        let mut next = self.clone();
        next.energy = at_most(nadd!(self.energy, amount), self.max_energy);
        next
    }

    /// 充能**减**：减到 0 为止（饱和减法，不为负）。返回新状态，不改入参。
    ///
    /// 只做数值：本方法**不代表**「消耗充能打元素爆发」这类语义，调用方自己组合。
    pub fn energy_sub(&self, amount: Nat) -> CharacterState {
        let mut next = self.clone();
        next.energy = nsub!(self.energy, amount);
        next
    }

    /// 充能**设**：钳到 `[0, max_energy]` 后写入。返回新状态，不改入参。
    pub fn energy_set(&self, value: Nat) -> CharacterState {
        let mut next = self.clone();
        next.energy = at_most(value, self.max_energy);
        next
    }
}

/// 把 `value` 钳到 `max`：超过上限就取上限（不 panic，也不写裸 min 运算）。
fn at_most(value: Nat, max: Nat) -> Nat {
    if value > max {
        max
    } else {
        value
    }
}

// TODO(D121 / D133)：**消耗骰子**（元素骰费用 / 消耗目标）归骰子模块
// （`resources::dice::CostComponent` 已建）；技能的消耗格式在 `skill.rs`（D162 迁移）；
// 角色块只到「减数值」这一层（`energy_sub`），不写「能不能付 / 付几个」的支付策略。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::generated::Localized;

    /// 测试用角色定义：生命上限 10 / 充能上限 2、无技能。
    static SAMPLE_DEF: CharacterDef<()> = CharacterDef {
        id: CardId::new(1216),
        name: Localized::EMPTY,
        tags: (),
        max_hp: Nat::new(10),
        max_energy: Nat::new(2),
        skills: &[],
    };

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    #[test]
    fn new_clamps_current_values_to_their_maxima() {
        // 当前值超过上限 -> 钳到上限；不超过 -> 原样。
        let state = CharacterState::new(n(9), n(5), n(7), n(2));
        assert_eq!(state.hp(), n(5));
        assert_eq!(state.max_hp(), n(5));
        assert_eq!(state.energy(), n(2));
        assert_eq!(state.max_energy(), n(2));
        // 状态列表 / 附着初始为空。
        assert!(state.states.is_empty());
        assert!(state.attachment.is_empty());
    }

    #[test]
    fn from_def_builds_initial_state_from_definition_limits() {
        // 用户裁决：CharacterDef 的 max_hp / max_energy 是定义数据（上限来源）；
        // 初始状态 hp = max_hp、energy = 0。
        let state = CharacterState::from_def(&SAMPLE_DEF);
        assert_eq!(state.hp(), n(10));
        assert_eq!(state.max_hp(), n(10));
        assert_eq!(state.energy(), Nat::ZERO);
        assert_eq!(state.max_energy(), n(2));
        assert!(state.states.is_empty());
        assert!(state.attachment.is_empty());
    }

    #[test]
    fn hp_add_stops_at_max_hp() {
        let state = CharacterState::new(n(3), n(5), n(0), n(2));
        // 正常加
        assert_eq!(state.hp_add(n(1)).hp(), n(4));
        // 加爆上限：停在上限，不溢出
        assert_eq!(state.hp_add(n(10)).hp(), n(5));
        assert_eq!(state.hp_add(Nat::MAX).hp(), n(5));
    }

    #[test]
    fn hp_sub_saturates_at_zero() {
        let state = CharacterState::new(n(3), n(5), n(0), n(2));
        assert_eq!(state.hp_sub(n(1)).hp(), n(2));
        // 减过头：停在 0，不为负
        assert_eq!(state.hp_sub(n(10)).hp(), n(0));
        assert_eq!(state.hp_sub(Nat::MAX).hp(), Nat::ZERO);
    }

    #[test]
    fn hp_set_clamps_to_range() {
        let state = CharacterState::new(n(3), n(5), n(0), n(2));
        assert_eq!(state.hp_set(n(4)).hp(), n(4));
        assert_eq!(state.hp_set(n(99)).hp(), n(5));
        assert_eq!(state.hp_set(Nat::ZERO).hp(), Nat::ZERO);
    }

    #[test]
    fn energy_add_stops_at_max_energy() {
        // 充能上限 2（元素爆发常见的 2 点充能）。
        let state = CharacterState::new(n(5), n(5), n(1), n(2));
        assert_eq!(state.energy_add(n(1)).energy(), n(2));
        // 加爆上限：停在上限
        assert_eq!(state.energy_add(n(9)).energy(), n(2));
        assert_eq!(state.energy_add(Nat::MAX).energy(), n(2));
        // 上限可读，且不变
        assert_eq!(state.energy_add(n(9)).max_energy(), n(2));
    }

    #[test]
    fn energy_sub_saturates_at_zero() {
        let state = CharacterState::new(n(5), n(5), n(2), n(2));
        assert_eq!(state.energy_sub(n(1)).energy(), n(1));
        assert_eq!(state.energy_sub(n(2)).energy(), n(0));
        // 减过头：停在 0，不为负
        assert_eq!(state.energy_sub(n(99)).energy(), Nat::ZERO);
    }

    #[test]
    fn energy_set_clamps_to_range() {
        let state = CharacterState::new(n(5), n(5), n(1), n(2));
        assert_eq!(state.energy_set(n(1)).energy(), n(1));
        assert_eq!(state.energy_set(n(99)).energy(), n(2));
        assert_eq!(state.energy_set(Nat::ZERO).energy(), Nat::ZERO);
    }

    #[test]
    fn numeric_operations_are_pure() {
        // 纯函数：所有操作返回新状态，入参一个字节都不改。
        let state = CharacterState::new(n(4), n(5), n(1), n(2));
        let _ = state.hp_add(n(1));
        let _ = state.hp_sub(n(2));
        let _ = state.hp_set(n(0));
        let _ = state.energy_add(n(1));
        let _ = state.energy_sub(n(1));
        let _ = state.energy_set(n(0));
        assert_eq!(state.hp(), n(4));
        assert_eq!(state.energy(), n(1));
        assert_eq!(state.max_hp(), n(5));
        assert_eq!(state.max_energy(), n(2));
    }

    #[test]
    fn energy_zero_is_valid_and_capped_by_upper_bound_only() {
        // 充能 0 是合法值（还没攒够）；上限为 0 的角色也不能加出充能。
        let no_energy = CharacterState::new(n(5), n(5), n(0), n(0));
        assert_eq!(no_energy.energy(), Nat::ZERO);
        assert_eq!(no_energy.energy_add(n(3)).energy(), Nat::ZERO);
    }
}
