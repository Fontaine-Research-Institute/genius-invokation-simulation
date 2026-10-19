//! 通用统计原语（决策日志 D181）：**计数器** + **有序可重复序列**。
//!
//! 纯数据容器：**不含任何游戏语义**。引擎不知道「支援区弃置牌数」「打出过的非初始卡牌」
//! 这些概念是什么，只提供两种载体：能增能减的计数、能按插入顺序重复追加的序列。
//!
//! 定位（D182）：两者都是**普通值类型变量**（与「区是变量」D42 同一类），
//! 由**各个实体区自己当字段持有**；同一个实体区可以有**多个**统计属性，不同区各自独立、
//! 互不共享。引擎**只提供类型与操作**：
//! - **不做**「区 → 统计」的绑定；**没有** id、全局表 / 注册表、statics；
//! - 无内部可变性，可直接内嵌为区结构体字段：`Counter` 是 `Copy`，`Seq<T>` 是 `Clone`；
//!   两者的构造都是 `const fn`（可写常量 / 字段初始值）。
//!
//! 只读边界（D183）：字段**全部私有**，对外只提供 `&self` 的读方法（`get` / `len` / `iter` /
//! `as_slice` ...），调用方拿 `&Counter` / `&Seq<T>` 就能读、但**拿不到 `&mut`**、也拿不到内部字段；
//! 「修改」只能由**持有它的实体区自己**用 `&mut self` 调 `add` / `sub` / `clear` / `push` 来做。
//! 不提供可变字段 / 可变切片入口，也不为了外部方便返回拷贝（快照需要时由持有者显式 `clone`）。
//!
//! 数值口径（D181）：
//! - 数值一律 [`Nat`]（默认 `Nat32`，**饱和**语义，与 [`crate::nat`] 一致）；
//! - 不出现浮点；
//! - **不扩展 `Nat` 族的计算口径**（不加新的数值类型 / 运算宏）。

use crate::nat::Nat;

/// 计数器：一个 [`Nat`] 的**饱和**增 / 减 / 清零 / 读。
///
/// 普通值类型变量（D182）：由实体区自己当字段持有，可以一个区多个、区之间互不共享；
/// `Copy`、无内部可变性、`const fn` 构造。
///
/// 只读边界（D183）：字段私有，外部只能拿 `&Counter` 读（[`Counter::get`] / [`Counter::is_zero`]）；
/// 修改（`add` / `sub` / `clear`）都要 `&mut self`，只有持有它的实体区做得到。
///
/// 口径（与 `Nat` 族一致：**溢出是正常业务，不 panic**）：
/// - 加：溢出停在 [`Nat::MAX`]；
/// - 减：到 [`Nat::ZERO`] 就停（不变负数、不 panic）；
/// - 清零：直接回到 [`Nat::ZERO`]。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Counter {
    value: Nat,
}

impl Counter {
    /// 计数 0。
    pub const ZERO: Counter = Counter { value: Nat::ZERO };

    /// 用初始值建计数器。
    pub const fn new(value: Nat) -> Counter {
        Counter { value }
    }

    /// 当前值（读）。
    pub const fn get(self) -> Nat {
        self.value
    }

    /// 是否已经是 0。
    pub const fn is_zero(&self) -> bool {
        self.value.get() == 0
    }

    /// 加：**饱和**（溢出停在 [`Nat::MAX`]，不 panic）；返回加完之后的值。
    pub fn add(&mut self, amount: Nat) -> Nat {
        self.value = self.value.saturating_add(amount);
        self.value
    }

    /// 减：**饱和**（到 0 就停，不 panic）；返回减完之后的值。
    pub fn sub(&mut self, amount: Nat) -> Nat {
        self.value = self.value.saturating_sub(amount);
        self.value
    }

    /// 清零（回到 [`Nat::ZERO`]）。
    pub fn clear(&mut self) {
        self.value = Nat::ZERO;
    }
}

impl From<Nat> for Counter {
    /// 用初始值建计数器（等价于 [`Counter::new`]）。
    fn from(value: Nat) -> Counter {
        Counter::new(value)
    }
}

impl From<Counter> for Nat {
    /// 取出当前值（等价于 [`Counter::get`]）。
    fn from(counter: Counter) -> Nat {
        counter.get()
    }
}

/// 有序**可重复**序列：允许重复、保持插入顺序。
///
/// 普通值类型变量（D182）：由实体区自己当字段持有，可以一个区多个、区之间互不共享；
/// `Clone`、无内部可变性、`const fn` 构造。
///
/// 只读边界（D183）：字段私有，外部只能拿 `&Seq<T>` 读（`len` / `is_empty` / `get` / `iter` /
/// `as_slice`，都是只读指针）；没有可变切片 / 可变字段入口；`push` / `clear` 需要 `&mut self`。
///
/// 「卡牌 ID 列表（有序、可出现多次）」用它承载，例如 `Seq<CardId>`（D181）；
/// 引擎不知道元素语义，只保证**顺序**与**重复**。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seq<T> {
    items: Vec<T>,
}

impl<T: Clone> Seq<T> {
    /// 空序列。
    pub const fn new() -> Seq<T> {
        Seq { items: Vec::new() }
    }

    /// 追加一项到末尾（**允许重复**）。
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    /// 元素个数（`Nat`，与引擎数值口径一致）。
    pub fn len(&self) -> Nat {
        Nat::new(self.items.len() as _)
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 取第 `index` 项（0 开始；**越界 -> `None`**）。
    pub fn get(&self, index: Nat) -> Option<&T> {
        // bare-int-ok: 容器下标，非业务数值
        self.items.get(index.get() as usize)
    }

    /// 按**插入顺序**迭代。
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// 清空（长度归 0）。
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// 底层切片（只读；顺序即插入顺序）。
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }
}

impl<T: Clone> Default for Seq<T> {
    /// 空序列（不要求 `T: Default`）。
    fn default() -> Seq<T> {
        Seq::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: u32) -> Nat {
        Nat::new(v)
    }

    // ---------------------------------------------------------------- 计数器

    #[test]
    fn counter_starts_at_zero() {
        let counter = Counter::new(Nat::ZERO);
        assert_eq!(counter.get(), Nat::ZERO);
        assert!(counter.is_zero());
        assert_eq!(Counter::ZERO.get(), Nat::ZERO);
        assert_eq!(Counter::default().get(), Nat::ZERO);
    }

    #[test]
    fn counter_add_and_read() {
        let mut counter = Counter::new(n(3));
        assert_eq!(counter.add(n(2)), n(5));
        assert_eq!(counter.get(), n(5));
        assert!(!counter.is_zero());
        // 运算符不参与：只暴露 Saturating 语义的方法。
        assert_eq!(counter.add(Nat::ZERO), n(5));
    }

    #[test]
    fn counter_add_saturates_at_max() {
        let mut counter = Counter::new(n(u32::MAX - 1));
        assert_eq!(counter.add(n(1)), n(u32::MAX));
        // 再加会停在 MAX（溢出是正常业务，不 panic）。
        assert_eq!(counter.add(n(1)), Nat::MAX);
        assert_eq!(counter.get(), Nat::MAX);
    }

    #[test]
    fn counter_sub_stops_at_zero() {
        let mut counter = Counter::new(n(5));
        assert_eq!(counter.sub(n(3)), n(2));
        // 减到 0 为止（不会变负数、不 panic）。
        assert_eq!(counter.sub(n(5)), Nat::ZERO);
        assert_eq!(counter.sub(n(1)), Nat::ZERO);
        assert!(counter.is_zero());
    }

    #[test]
    fn counter_clear_resets_to_zero() {
        let mut counter = Counter::new(n(42));
        counter.clear();
        assert_eq!(counter.get(), Nat::ZERO);
        assert!(counter.is_zero());
    }

    #[test]
    fn counter_from_conversions_roundtrip() {
        let counter = Counter::from(n(7));
        assert_eq!(counter.get(), n(7));
        assert_eq!(Nat::from(counter), n(7));
    }

    // ------------------------------------------------------------ 有序可重复序列

    #[test]
    fn seq_starts_empty() {
        let seq: Seq<Nat> = Seq::new();
        assert_eq!(seq.len(), Nat::ZERO);
        assert!(seq.is_empty());
        assert_eq!(seq.get(Nat::ZERO), None);
        assert_eq!(seq.iter().count(), 0);
        assert!(seq.as_slice().is_empty());
        assert_eq!(seq, Seq::default());
    }

    #[test]
    fn seq_keeps_order_and_duplicates() {
        let mut seq: Seq<Nat> = Seq::new();
        seq.push(n(5));
        seq.push(n(5));
        seq.push(n(7));

        assert_eq!(seq.len(), n(3));
        assert!(!seq.is_empty());
        // 允许重复、保持插入顺序。
        assert_eq!(seq.get(Nat::ZERO), Some(&n(5)));
        assert_eq!(seq.get(n(1)), Some(&n(5)));
        assert_eq!(seq.get(n(2)), Some(&n(7)));
        assert_eq!(seq.as_slice(), [n(5), n(5), n(7)]);
        let dumped: Vec<Nat> = seq.iter().copied().collect();
        assert_eq!(dumped, vec![n(5), n(5), n(7)]);
    }

    #[test]
    fn seq_get_out_of_range_is_none() {
        let mut seq: Seq<Nat> = Seq::new();
        seq.push(n(1));
        assert_eq!(seq.get(n(1)), None);
        assert_eq!(seq.get(n(2)), None);
        assert_eq!(seq.get(Nat::MAX), None);
    }

    #[test]
    fn seq_clear_empties_it() {
        let mut seq: Seq<Nat> = Seq::new();
        seq.push(n(1));
        seq.push(n(2));
        seq.clear();
        assert_eq!(seq.len(), Nat::ZERO);
        assert!(seq.is_empty());
        assert_eq!(seq.get(Nat::ZERO), None);
        assert_eq!(seq.iter().count(), 0);
        // 清空后还能继续追加。
        seq.push(n(9));
        assert_eq!(seq.as_slice(), [n(9)]);
    }

    #[test]
    fn seq_is_generic_over_element_type() {
        // 元素只要求 `T: Clone`（这里用 String 证明不依赖引擎的数值类型）。
        let mut seq: Seq<String> = Seq::new();
        seq.push("a".to_string());
        seq.push("b".to_string());
        seq.push("a".to_string());
        let dumped: Vec<&str> = seq.iter().map(String::as_str).collect();
        assert_eq!(dumped, vec!["a", "b", "a"]);
    }

    // ------------------------------------------------- D182：区的普通字段 / 值语义

    /// 演示两种统计属性直接内嵌成「区结构体」的普通字段，且能写常量初始值。
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct AreaStats {
        discarded: Counter,
        played_extra_cards: Seq<Nat>,
    }

    const EMPTY_AREA_STATS: AreaStats = AreaStats {
        discarded: Counter::ZERO,
        played_extra_cards: Seq::new(),
    };

    #[test]
    fn stats_can_be_const_field_initializers() {
        assert_eq!(EMPTY_AREA_STATS.discarded.get(), Nat::ZERO);
        assert!(EMPTY_AREA_STATS.played_extra_cards.is_empty());
        assert_eq!(EMPTY_AREA_STATS, AreaStats::default());
    }

    #[test]
    fn stats_are_independent_plain_value_fields() {
        // 同一类区有两个实例 -> 各自独立、互不共享（没有全局表 / id / 注册表）。
        let mut my_area = EMPTY_AREA_STATS;
        let mut opp_area = EMPTY_AREA_STATS;
        my_area.discarded.add(n(2));
        my_area.played_extra_cards.push(n(7));
        opp_area.discarded.add(n(1));

        assert_eq!(my_area.discarded.get(), n(2));
        assert_eq!(opp_area.discarded.get(), n(1));
        assert_eq!(my_area.played_extra_cards.as_slice(), [n(7)]);
        assert!(opp_area.played_extra_cards.is_empty());

        // 普通值语义：拷贝出的一份改了，原件不受影响。
        let mut copy = my_area.clone();
        copy.discarded.clear();
        copy.played_extra_cards.push(n(8));
        assert_eq!(my_area.discarded.get(), n(2));
        assert_eq!(my_area.played_extra_cards.len(), n(1));
        assert_eq!(copy.played_extra_cards.len(), n(2));
    }

    // --------------------------------------------- D183：对外只给只读指针（&self）

    #[test]
    fn stats_are_readable_through_shared_references() {
        // 外部拿到的是 `&Counter` / `&Seq<T>`：所有读操作都只要 `&self`。
        let counter = Counter::new(n(3));
        let counter_ref: &Counter = &counter;
        assert_eq!(counter_ref.get(), n(3));
        assert!(!counter_ref.is_zero());

        let mut built: Seq<Nat> = Seq::new();
        built.push(n(1));
        built.push(n(1));
        let seq_ref: &Seq<Nat> = &built;
        assert_eq!(seq_ref.len(), n(2));
        assert!(!seq_ref.is_empty());
        assert_eq!(seq_ref.get(Nat::ZERO), Some(&n(1)));
        assert_eq!(seq_ref.as_slice(), [n(1), n(1)]);
        let dumped: Vec<Nat> = seq_ref.iter().copied().collect();
        assert_eq!(dumped, vec![n(1), n(1)]);

        // 只读引用不改变原值（没有内部可变性）。
        assert_eq!(counter.get(), n(3));
        assert_eq!(built.len(), n(2));
    }
}
