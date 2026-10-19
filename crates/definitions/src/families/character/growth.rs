//! **角色成长**（覆盖报告 R4）：通用成长条目（属性 + 增量）+ 叠加 / 读取。
//!
//! ## 依据（唯一来源）
//!
//! 权威关键字（`generated/data/keywords.json`）：
//! - **K64 角色等级**：「重复获取角色可以提高其角色等级，提高角色等级可以强化角色属性。」
//! - **K65 角色等阶**：「角色等级达到 2，4，7 时，会突破等阶。突破后可以获得天赋选择或是
//!   技能效果提升。」
//!
//! 卡面数据（只读依据，**本模块不导入卡牌数据**）：「攻击力+2」「最大生命值+10」
//! 「充能上限-1」「生命上限+…」这类**持久**属性增量（装备 / 天赋 / 场地效果给）。
//!
//! ## 口径
//!
//! - **成长条目** = 属性 + 增量；增量带方向（增 / 减），数量是 `Nat`（零浮点、禁裸算术）；
//!   ≤0 的增量不用负数表示（全仓禁用有符号裸整数）：用「方向 + 数量」两个字段；
//! - **叠加** = 同一属性的多条条目分别合计（容器是 `Vec`；**不用 `HashMap`**、不建属性 -> 下标表）；
//! - **读取** = `totals(attribute)` 给出增 / 减两侧合计；`effective(base, attribute)` 把合计作用到
//!   「基础值」上（`Nat` 饱和：减少量不会把基础值减成负数）；
//! - **纯函数**：`with` / `merged` / `apply_to_character` 都返回新值、不改入参；
//! - **基值不属于本层**：攻击力怎么进伤害公式、生命上限提升是否同时回血，由伤害 / 治疗层决定。
//!
//! ## 定义值 vs 实际值（D238）
//!
//! 生命上限有**两个值**，不要混用：
//!
//! - **定义值**：`CharacterDef::max_hp` / `CharacterDef::max_energy`（卡牌定义里的固定上限）——
//!   **不可变**；查询 / 选择器**禁用定义值**（定义值不随成长变，直接用它就丢了成长）；
//! - **实际值**：定义值 + 成长增益（**可变**）—— 读 `actual_max_hp(定义值)` / `effective(...)`；
//!   选择器 / 出现值**只能用实际值**；
//! - **角色被击倒时**：**实际最大生命值上限重置为定义值**（D238，用户口径「最大生命值上限
//!   （实际可变那个），角色被击倒后变回定义值」）—— 调 `reset_max_hp()`：清掉**全部**「生命上限」
//!   成长条目（增加 / 减少都清），其它属性成长保留；
//!   要**整体**回到定义值（所有属性都清）用 `cleared()`；
//! - 不变量：本容器**只存增量**，定义值不在成长里 —— 所以「重置到定义值」= 清空对应条目，
//!   不需要（也不允许）把定义值抄一份进来。
//!
//! ## 不做 / 待复核
//!
//! - 不写具体卡牌 / 不做 K64 的经验曲线（升级所需经验不在已入库数据里）；
//! - 不把成长直接写进 `CharacterState`（`character.rs` 本轮不在可写范围）：用
//!   `apply_to_character` 得到一个「上限已成长」的新状态；
//! - **待复核**：生命上限 +N 时当前生命是否也 +N —— 规则书 / 关键字未单列，本层**不回血**，
//!   `apply_to_character` 只在**上限下降**时把当前值钳到新上限（待实测）；
//! - **待复核**：元素精通（K62）在卡面上是「我方元素精通 +N」（队伍级，不是角色属性），本层不收。

use crate::families::character::character::CharacterState;
use gi_genius_invokation_engine::{nadd, nsub, Nat};

/// 一个**可成长的角色属性**。
///
/// 只收卡面按「角色属性」给的三种；元素精通（K62）是队伍级属性，不在角色成长里（待复核）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum GrowthAttribute {
    /// 攻击力（卡面「攻击力 +N」；具体怎么进伤害由伤害层决定）。
    Attack,
    /// 生命上限（卡面「生命上限 +N」「最大生命值 +N」）。
    MaxHp,
    /// 充能上限（卡面「充能上限 +N」「充能上限 -1」）。
    MaxEnergy,
}

impl GrowthAttribute {
    /// 全部属性（遍历 / 测试用；顺序不表示语义）。
    pub const ALL: [GrowthAttribute; 3] = [
        GrowthAttribute::Attack,
        GrowthAttribute::MaxHp,
        GrowthAttribute::MaxEnergy,
    ];
}

/// 增量方向：增 / 减（不用有符号数，符合「数值一律 `Nat`」）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum GrowthDirection {
    /// 增加（「攻击力 +1」）。
    Increase,
    /// 减少（「充能上限 -1」）。
    Decrease,
}

/// 一条**成长条目**：属性 + 增量（方向 + 数量）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Growth {
    /// 成长哪个属性。
    pub attribute: GrowthAttribute,
    /// 增量方向。
    pub direction: GrowthDirection,
    /// 增量数量（`Nat`；0 表示这条不改变任何值）。
    pub amount: Nat,
}

impl Growth {
    /// 建一条成长条目。
    pub const fn new(
        attribute: GrowthAttribute,
        direction: GrowthDirection,
        amount: Nat,
    ) -> Growth {
        Growth {
            attribute,
            direction,
            amount,
        }
    }

    /// 一条「属性 +N」的成长条目（卡面最常见的写法）。
    pub const fn increase(attribute: GrowthAttribute, amount: Nat) -> Growth {
        Growth::new(attribute, GrowthDirection::Increase, amount)
    }

    /// 一条「属性 -N」的成长条目（如「充能上限 -1」）。
    pub const fn decrease(attribute: GrowthAttribute, amount: Nat) -> Growth {
        Growth::new(attribute, GrowthDirection::Decrease, amount)
    }

    /// 是不是「+」。
    pub const fn is_increase(self) -> bool {
        matches!(self.direction, GrowthDirection::Increase)
    }

    /// 这条条目是否真的会改变值（数量非 0）。
    pub fn is_effective(self) -> bool {
        self.amount != Nat::ZERO
    }
}

/// 某个属性的**成长合计**：增加侧合计 + 减少侧合计（两侧都是 `Nat`，不做有符号中间值）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GrowthTotals {
    /// 增加侧合计。
    pub gain: Nat,
    /// 减少侧合计。
    pub loss: Nat,
}

impl GrowthTotals {
    /// 空合计（什么都不加、不减）。
    pub const ZERO: GrowthTotals = GrowthTotals {
        gain: Nat::ZERO,
        loss: Nat::ZERO,
    };

    /// 是否没有任何增减。
    pub fn is_empty(self) -> bool {
        self.gain == Nat::ZERO && self.loss == Nat::ZERO
    }

    /// 把合计作用到**基础值**上（`Nat` 饱和：减少超过基础值时停在 0）。
    ///
    /// 例：基础生命上限 10、增 3、减 1 -> 12；基础充能上限 2、减 1 -> 1；
    /// 基础充能上限 0、减 1 -> 0（不为负）。
    pub fn apply_to(self, base: Nat) -> Nat {
        if self.gain >= self.loss {
            nadd!(base, nsub!(self.gain, self.loss))
        } else {
            nsub!(base, nsub!(self.loss, self.gain))
        }
    }
}

/// 一组**成长条目**（叠加容器；纯函数风格）。
///
/// 容器没有身份（D42）：它就是某个角色 / 某个状态里的一个字段；「这段成长是谁给的」
/// 由持有它的东西（装备 / 天赋 / 场地）表达，这里不登记来源。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrowthList {
    /// 成长条目（顺序 = 加入顺序；合计与顺序无关）。
    entries: Vec<Growth>,
}

impl GrowthList {
    /// 空成长列表。
    pub const fn new() -> GrowthList {
        GrowthList {
            entries: Vec::new(),
        }
    }

    /// 全部条目（**只读指针**）。
    pub fn entries(&self) -> &[Growth] {
        &self.entries
    }

    /// 条目数（`Nat`，不是裸整型）。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何条目。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// **叠加**一条成长（纯函数：返回新列表，不改入参）。
    pub fn with(&self, growth: Growth) -> GrowthList {
        let mut next = self.clone();
        next.entries.push(growth);
        next
    }

    /// **叠加**另一组成长（纯函数：按顺序接在后面，不改两侧入参）。
    pub fn merged(&self, other: &GrowthList) -> GrowthList {
        let mut next = self.clone();
        next.entries.extend_from_slice(&other.entries);
        next
    }

    /// **重置某个属性的成长**（纯函数：返回新列表，不改入参）。
    ///
    /// 语义 = 该属性的实际值回到**定义值**：把该属性的全部成长条目（增加与减少）都丢掉，
    /// 其它属性一条不动。
    pub fn without(&self, attribute: GrowthAttribute) -> GrowthList {
        GrowthList {
            entries: self
                .entries
                .iter()
                .copied()
                .filter(|entry| entry.attribute != attribute)
                .collect(),
        }
    }

    /// **重置实际最大生命值上限**（D238）：角色被击倒后调用 —— 实际上限回到**卡牌定义值**。
    ///
    /// 语义 = 清掉全部「生命上限」成长条目（增加 / 减少都清），其它属性成长保留。
    /// 定义值本身不在成长里（`CharacterDef::max_hp`，不可变），所以清空条目后
    /// `actual_max_hp(定义值)` 自然等于定义值。
    ///
    /// 纯函数：返回新列表、不改入参；由角色区的击倒流程决定**何时**调用（本层不派发时机）。
    pub fn reset_max_hp(&self) -> GrowthList {
        self.without(GrowthAttribute::MaxHp)
    }

    /// **整体重置到定义值**（纯函数：返回空列表，不改入参）。
    ///
    /// 全部属性的成长条目都清掉（实际值 = 定义值）；只重置生命上限用 `reset_max_hp()`。
    pub fn cleared(&self) -> GrowthList {
        GrowthList::new()
    }

    /// 某个属性的**成长合计**（增 / 减两侧分别相加；顺序无关、不用 `HashMap`）。
    pub fn totals(&self, attribute: GrowthAttribute) -> GrowthTotals {
        let mut totals = GrowthTotals::ZERO;
        for entry in &self.entries {
            if entry.attribute != attribute {
                continue;
            }
            match entry.direction {
                GrowthDirection::Increase => {
                    totals.gain = nadd!(totals.gain, entry.amount);
                }
                GrowthDirection::Decrease => {
                    totals.loss = nadd!(totals.loss, entry.amount);
                }
            }
        }
        totals
    }

    /// 读**成长后**的属性值：基础值 + 该属性的增加合计 - 减少合计（`Nat` 饱和）。
    pub fn effective(&self, base: Nat, attribute: GrowthAttribute) -> Nat {
        self.totals(attribute).apply_to(base)
    }

    /// 读**成长后的生命上限**（基础生命上限 + 生命上限成长）。
    pub fn effective_max_hp(&self, base: Nat) -> Nat {
        self.effective(base, GrowthAttribute::MaxHp)
    }

    /// 读**成长后的充能上限**（基础充能上限 + 充能上限成长）。
    pub fn effective_max_energy(&self, base: Nat) -> Nat {
        self.effective(base, GrowthAttribute::MaxEnergy)
    }

    /// 读**实际最大生命值上限**（= 定义值 + 生命上限成长；D238）。
    ///
    /// `definition_max_hp` 传**定义值**（`CharacterDef::max_hp`，不可变）；
    /// 结果 = **实际值**（可变）。**选择器 / 出现值只能用实际值**，定义值选择器禁用。
    /// 角色被击倒后由 `reset_max_hp()` 把成长清掉，本函数自然回到定义值。
    pub fn actual_max_hp(&self, definition_max_hp: Nat) -> Nat {
        self.effective_max_hp(definition_max_hp)
    }

    /// **把成长作用到角色状态**（纯函数：返回新状态，不改入参）。
    ///
    /// 只改两个上限 + 把当前值钳到新上限（`CharacterState` 的不变式：当前值不超过上限）：
    /// - 生命上限提升**不回血**（规则书未单列，待实测）；
    /// - 充能上限下降时，当前充能钳到新上限；
    /// - 攻击力不写进 `CharacterState`（本状态没有攻击力字段）：用 `effective` 现算。
    ///
    /// 返回状态里的 `max_hp` / `max_energy` 是**实际值**（定义值 + 成长），**不是定义值**
    /// （D238：定义值固定、选择器禁用；选择器只用实际值）。角色被击倒时先
    /// `reset_max_hp()` 再问实际值，实际上限就回到定义值。
    pub fn apply_to_character(&self, state: &CharacterState) -> CharacterState {
        let mut next = state.clone();
        next.max_hp = self.effective_max_hp(state.max_hp);
        if next.hp > next.max_hp {
            next.hp = next.max_hp;
        }
        next.max_energy = self.effective_max_energy(state.max_energy);
        if next.energy > next.max_energy {
            next.energy = next.max_energy;
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gi_genius_invokation_engine::Nat;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 角色状态：生命 / 充能（当前值 + 上限）。
    fn character(hp: Nat, max_hp: Nat, energy: Nat, max_energy: Nat) -> CharacterState {
        CharacterState::new(hp, max_hp, energy, max_energy)
    }

    #[test]
    fn empty_growth_changes_nothing() {
        let growth = GrowthList::new();
        assert!(growth.is_empty());
        assert_eq!(growth.len(), Nat::ZERO);
        assert!(growth.entries().is_empty());
        for attribute in GrowthAttribute::ALL {
            assert_eq!(growth.totals(attribute), GrowthTotals::ZERO);
            assert!(growth.totals(attribute).is_empty());
            assert_eq!(growth.effective(n(7), attribute), n(7));
        }
        assert_eq!(GrowthTotals::ZERO.apply_to(n(5)), n(5));
    }

    #[test]
    fn single_increase_adds_to_the_base() {
        let growth = GrowthList::new().with(Growth::increase(GrowthAttribute::MaxHp, n(3)));
        assert_eq!(growth.len(), n(1));
        let totals = growth.totals(GrowthAttribute::MaxHp);
        assert_eq!(totals.gain, n(3));
        assert_eq!(totals.loss, Nat::ZERO);
        assert_eq!(growth.effective(n(10), GrowthAttribute::MaxHp), n(13));
        assert_eq!(growth.effective_max_hp(n(10)), n(13));
        // 别的属性不受影响。
        assert_eq!(growth.effective(n(10), GrowthAttribute::Attack), n(10));
    }

    #[test]
    fn stacking_sums_entries_and_is_order_independent() {
        let a = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::Attack, n(1)))
            .with(Growth::increase(GrowthAttribute::Attack, n(2)))
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)));
        let b = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::increase(GrowthAttribute::Attack, n(2)))
            .with(Growth::increase(GrowthAttribute::Attack, n(1)));
        // 叠加 = 合计；顺序不影响结果。
        assert_eq!(
            a.totals(GrowthAttribute::Attack),
            b.totals(GrowthAttribute::Attack)
        );
        assert_eq!(a.totals(GrowthAttribute::Attack).gain, n(3));
        assert_eq!(a.effective(n(0), GrowthAttribute::Attack), n(3));
        assert_eq!(a.effective_max_hp(n(10)), n(15));
        // merged 把两组叠在一起。
        let merged = a.merged(&b);
        assert_eq!(merged.len(), n(6));
        assert_eq!(merged.totals(GrowthAttribute::Attack).gain, n(6));
        assert_eq!(merged.effective_max_hp(n(10)), n(20));
    }

    #[test]
    fn decrease_is_supported_and_saturates_at_zero() {
        // 卡面真实写法：「充能上限 -1」。
        let growth = GrowthList::new().with(Growth::decrease(GrowthAttribute::MaxEnergy, n(1)));
        assert_eq!(growth.totals(GrowthAttribute::MaxEnergy).loss, n(1));
        assert_eq!(growth.effective_max_energy(n(2)), n(1));
        // 基础值不足时停在 0（Nat 饱和，不为负）。
        assert_eq!(growth.effective_max_energy(Nat::ZERO), Nat::ZERO);
        assert_eq!(growth.totals(GrowthAttribute::MaxEnergy).apply_to(Nat::ZERO), Nat::ZERO);
    }

    #[test]
    fn increase_and_decrease_cancel_out() {
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxEnergy, n(3)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(1)));
        let totals = growth.totals(GrowthAttribute::MaxEnergy);
        assert_eq!(totals.gain, n(3));
        assert_eq!(totals.loss, n(1));
        assert_eq!(growth.effective(n(2), GrowthAttribute::MaxEnergy), n(4));
        // 减多于增：作用到基础值上（饱和到 0）。
        let net_loss = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxEnergy, n(1)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(4)));
        assert_eq!(net_loss.effective(n(2), GrowthAttribute::MaxEnergy), Nat::ZERO);
        assert_eq!(net_loss.effective(n(5), GrowthAttribute::MaxEnergy), n(2));
    }

    #[test]
    fn attributes_are_isolated_from_each_other() {
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::Attack, n(2)))
            .with(Growth::increase(GrowthAttribute::MaxHp, n(10)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(1)));
        assert_eq!(growth.totals(GrowthAttribute::Attack).gain, n(2));
        assert_eq!(growth.totals(GrowthAttribute::MaxHp).gain, n(10));
        assert_eq!(growth.totals(GrowthAttribute::MaxEnergy).loss, n(1));
        assert_eq!(growth.effective(n(0), GrowthAttribute::Attack), n(2));
        assert_eq!(growth.effective(n(10), GrowthAttribute::MaxHp), n(20));
        assert_eq!(growth.effective(n(2), GrowthAttribute::MaxEnergy), n(1));
    }

    #[test]
    fn growth_is_pure_and_does_not_touch_its_inputs() {
        let base = GrowthList::new().with(Growth::increase(GrowthAttribute::Attack, n(1)));
        let _bigger = base.with(Growth::increase(GrowthAttribute::Attack, n(1)));
        assert_eq!(base.len(), n(1));
        assert_eq!(base.totals(GrowthAttribute::Attack).gain, n(1));
        let other = GrowthList::new().with(Growth::increase(GrowthAttribute::MaxHp, n(2)));
        let _merged = base.merged(&other);
        assert_eq!(base.len(), n(1));
        assert_eq!(other.len(), n(1));
    }

    #[test]
    fn zero_amount_entry_is_ineffective() {
        let a = Growth::new(GrowthAttribute::Attack, GrowthDirection::Increase, Nat::ZERO);
        assert!(!a.is_effective());
        assert!(a.is_increase());
        let b = Growth::increase(GrowthAttribute::Attack, n(1));
        assert!(b.is_effective());
        let c = Growth::decrease(GrowthAttribute::MaxEnergy, n(1));
        assert!(!c.is_increase());
        assert!(c.is_effective());
    }

    #[test]
    fn apply_to_character_grows_caps_and_keeps_invariants() {
        let state = character(n(7), n(10), n(1), n(2));
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(1)));
        let next = growth.apply_to_character(&state);
        assert_eq!(next.max_hp(), n(15));
        // 上限提升不回血（待实测）；当前生命保持。
        assert_eq!(next.hp(), n(7));
        assert_eq!(next.max_energy(), n(1));
        // 当前充能钳到新上限（不变式：当前值 <= 上限）。
        assert_eq!(next.energy(), n(1));
        // 入参不变（纯函数）。
        assert_eq!(state.max_hp(), n(10));
        assert_eq!(state.max_energy(), n(2));
        assert_eq!(state.energy(), n(1));
    }

    #[test]
    fn apply_to_character_clamps_current_values_when_caps_drop() {
        // 充能上限降到 0 -> 当前充能也归 0；生命上限下降 -> 当前生命钳到新上限。
        let state = character(n(9), n(10), n(2), n(2));
        let growth = GrowthList::new()
            .with(Growth::decrease(GrowthAttribute::MaxHp, n(4)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(2)));
        let next = growth.apply_to_character(&state);
        assert_eq!(next.max_hp(), n(6));
        assert_eq!(next.hp(), n(6));
        assert_eq!(next.max_energy(), Nat::ZERO);
        assert_eq!(next.energy(), Nat::ZERO);
    }

    #[test]
    fn empty_growth_apply_to_character_returns_equivalent_state() {
        let state = character(n(5), n(10), n(1), n(2));
        let next = GrowthList::new().apply_to_character(&state);
        assert_eq!(next.hp(), state.hp());
        assert_eq!(next.max_hp(), state.max_hp());
        assert_eq!(next.energy(), state.energy());
        assert_eq!(next.max_energy(), state.max_energy());
    }

    #[test]
    fn attribute_list_is_complete_and_distinct() {
        assert_eq!(GrowthAttribute::ALL.len(), 3);
        let mut names: Vec<&'static str> = GrowthAttribute::ALL
            .iter()
            .map(|attribute| match attribute {
                GrowthAttribute::Attack => "Attack",
                GrowthAttribute::MaxHp => "MaxHp",
                GrowthAttribute::MaxEnergy => "MaxEnergy",
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 3);
    }

    // ==================================================================
    // D238：定义值 vs 实际值 + 击倒后「实际上限重置为定义值」
    // ==================================================================

    /// 卡牌**定义值**（不可变；选择器禁用）：生命上限 10 / 充能上限 2。
    const DEFINITION_MAX_HP: Nat = Nat::new(10);
    const DEFINITION_MAX_ENERGY: Nat = Nat::new(2);

    #[test]
    fn actual_max_hp_is_definition_plus_growth() {
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::increase(GrowthAttribute::MaxHp, n(2)));
        // 实际值 = 定义值 + 成长合计。
        assert_eq!(growth.actual_max_hp(DEFINITION_MAX_HP), n(17));
        assert_eq!(growth.effective_max_hp(DEFINITION_MAX_HP), n(17));
        // 定义值本身不被改写（常量 10 不受影响）。
        assert_eq!(DEFINITION_MAX_HP, n(10));
        // 状态上的上限也是实际值（定义值 + 5）。
        let single = GrowthList::new().with(Growth::increase(GrowthAttribute::MaxHp, n(5)));
        let state = character(n(10), DEFINITION_MAX_HP, Nat::ZERO, DEFINITION_MAX_ENERGY);
        let next = single.apply_to_character(&state);
        assert_eq!(next.max_hp(), n(15));
        assert_eq!(next.max_hp(), single.actual_max_hp(DEFINITION_MAX_HP));
    }

    #[test]
    fn reset_max_hp_returns_actual_cap_to_definition() {
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::increase(GrowthAttribute::Attack, n(2)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(1)));
        assert_eq!(growth.actual_max_hp(DEFINITION_MAX_HP), n(15));
        let reset = growth.reset_max_hp();
        // 实际上限回到定义值（D238：角色被击倒后变回定义值）。
        assert_eq!(reset.actual_max_hp(DEFINITION_MAX_HP), DEFINITION_MAX_HP);
        assert_eq!(reset.totals(GrowthAttribute::MaxHp), GrowthTotals::ZERO);
        // 其它属性的成长保留（只重置生命上限）。
        assert_eq!(reset.totals(GrowthAttribute::Attack).gain, n(2));
        assert_eq!(reset.totals(GrowthAttribute::MaxEnergy).loss, n(1));
        assert_eq!(reset.len(), n(2));
    }

    #[test]
    fn reset_max_hp_is_pure_and_does_not_touch_the_input() {
        let growth = GrowthList::new().with(Growth::increase(GrowthAttribute::MaxHp, n(5)));
        let _reset = growth.reset_max_hp();
        // 入参一个字节都不改（纯函数口径）。
        assert_eq!(growth.len(), n(1));
        assert_eq!(growth.actual_max_hp(DEFINITION_MAX_HP), n(15));
        assert_eq!(growth.totals(GrowthAttribute::MaxHp).gain, n(5));
        // 幂等：重置后的列表再重置还是空成长。
        let reset = growth.reset_max_hp();
        assert!(reset.reset_max_hp().is_empty());
        // 空列表重置 = 空列表。
        assert!(GrowthList::new().reset_max_hp().is_empty());
    }

    #[test]
    fn reset_max_hp_also_drops_decrease_entries() {
        // 「实际可变那个」的重置 = 增 / 减两条都清（净额回到定义值）。
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::decrease(GrowthAttribute::MaxHp, n(2)));
        assert_eq!(growth.actual_max_hp(DEFINITION_MAX_HP), n(13));
        let reset = growth.reset_max_hp();
        assert_eq!(reset.actual_max_hp(DEFINITION_MAX_HP), DEFINITION_MAX_HP);
        assert!(reset.is_empty());
    }

    #[test]
    fn without_removes_only_the_requested_attribute() {
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::decrease(GrowthAttribute::MaxHp, n(2)))
            .with(Growth::increase(GrowthAttribute::MaxEnergy, n(1)));
        let without_hp = growth.without(GrowthAttribute::MaxHp);
        assert_eq!(without_hp.len(), n(1));
        assert_eq!(without_hp.totals(GrowthAttribute::MaxHp), GrowthTotals::ZERO);
        assert_eq!(without_hp.totals(GrowthAttribute::MaxEnergy).gain, n(1));
        // without 也是纯函数。
        assert_eq!(growth.len(), n(3));
    }

    #[test]
    fn cleared_resets_every_attribute_to_definition() {
        let growth = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::Attack, n(2)))
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .with(Growth::decrease(GrowthAttribute::MaxEnergy, n(1)));
        let cleared = growth.cleared();
        assert!(cleared.is_empty());
        for attribute in GrowthAttribute::ALL {
            let definition = match attribute {
                GrowthAttribute::Attack => Nat::ZERO,
                GrowthAttribute::MaxHp => DEFINITION_MAX_HP,
                GrowthAttribute::MaxEnergy => DEFINITION_MAX_ENERGY,
            };
            assert_eq!(cleared.effective(definition, attribute), definition);
        }
        // 入参不变。
        assert_eq!(growth.len(), n(3));
        assert_eq!(growth.actual_max_hp(DEFINITION_MAX_HP), n(15));
    }

    #[test]
    fn defeat_reset_then_apply_returns_definition_cap() {
        // 击倒流程的推荐组合：reset_max_hp -> apply_to_character；上限回到定义值。
        let growth = GrowthList::new().with(Growth::increase(GrowthAttribute::MaxHp, n(5)));
        let definition_state = character(n(10), DEFINITION_MAX_HP, Nat::ZERO, DEFINITION_MAX_ENERGY);
        let grown = growth.apply_to_character(&definition_state);
        assert_eq!(grown.max_hp(), n(15));
        let after_defeat = growth.reset_max_hp().apply_to_character(&definition_state);
        assert_eq!(after_defeat.max_hp(), DEFINITION_MAX_HP);
        assert!(after_defeat.max_hp() < grown.max_hp());
    }
}
