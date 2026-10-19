//! **统一记录器**（D223 / D224）：极薄的泛型容器，只提供「属于哪一步 + 任意内容」。
//!
//! ## 形状（用户口径）
//!
//! 用户原话：「不定义那么具体，就像引擎的实体区一样，提供属于哪一步，可填任意内容，
//! 内容的类型自己定义如何显示」。
//!
//! - [Record]：一条记录 = 步号 + 内容（内容类型由**各模块自己定义**）；
//! - [Recorder]：泛型容器，唯一写入入口是 [Recorder::push]；对外只读指针
//!   [Recorder::entries]（与区自持 / 只读指针同风格，D182 / D183）；
//! - [ShowRecord]：内容的显示由内容类型自己实现（记录器不预设任何种类枚举 / 字段）；
//! - **可整体裁剪**：关闭 feature 时 Recorder 是零大小类型，push 为空操作（零开销）。
//!
//! ## 各模块怎么用
//!
//! 各模块用**自己的内容类型**实例化记录器（D225 已落地）：
//! - keywords 用 [crate::keywords::record::KeywordRecord]（执行了哪个关键字 / 入参 / 结果）；
//! - flow 用 [crate::flow::record::FlowRecord]；
//! - damage_chain 用 [crate::families::character::damage_chain::record::DamageChainRecord]；
//! - character zones / summon zones / action_card areas / action_card zones（pile、hand）
//!   各自在自己的 `record.rs` 里定义内容类型并实现 [ShowRecord]；
//! - 新增模块照同一口径：内容类型放本模块的 `record.rs`，显示由该类型实现 [ShowRecord]，
//!   记录点写在动作处（只写观测、不改结算）。
//!
//! 记录器**不含业务逻辑**：它只存 + 按步读 + 导出。
//!
//! ## 关闭时
//!
//! Cargo feature records（默认开启）。关闭时 [Recorder] 是零大小类型、[Recorder::push] 丢弃内容，
//! 结算结果完全不受影响。

use gi_genius_invokation_engine::Nat;

/// 一条记录：**属于哪一步** + 任意内容（内容类型由使用方定义）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record<T> {
    /// 步号（0 起；由写入方给）。
    pub step: Nat,
    /// 记录内容（类型由写入方定义）。
    pub content: T,
}

impl<T> Record<T> {
    /// 建一条记录。
    pub const fn new(step: Nat, content: T) -> Record<T> {
        Record { step, content }
    }
}

/// **内容类型自己定义如何显示**（记录器不预设种类 / 字段）。
pub trait ShowRecord {
    /// 把这条内容写成文本（零依赖；由内容类型决定格式）。
    fn show(&self, out: &mut String);
}

/// **记录器**：泛型容器，按写入顺序保存 Record。
///
/// 唯一写入入口 [Recorder::push]；对外只读指针 [Recorder::entries]。
#[cfg(feature = "records")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorder<T> {
    entries: Vec<Record<T>>,
}

/// **关闭 feature 时的记录器**：零大小，push 为空操作（零开销）。
#[cfg(not(feature = "records"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recorder<T> {
    marker: std::marker::PhantomData<T>,
}

#[cfg(feature = "records")]
impl<T> Default for Recorder<T> {
    fn default() -> Recorder<T> {
        Recorder::new()
    }
}

#[cfg(feature = "records")]
impl<T> Recorder<T> {
    /// 空记录器。
    pub const fn new() -> Recorder<T> {
        Recorder {
            entries: Vec::new(),
        }
    }

    /// **唯一写入入口**：把一条内容记到某一步。
    pub fn push(&mut self, step: Nat, content: T) {
        self.entries.push(Record::new(step, content));
    }

    /// 只读遍历（顺序 = 写入顺序）。
    pub fn entries(&self) -> &[Record<T>] {
        &self.entries
    }

    /// 清空。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 记录条数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否一条都没有。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 某一「步」的全部内容（按写入顺序；只读迭代）。
    pub fn step_entries(&self, step: Nat) -> impl Iterator<Item = &T> + '_ {
        self.entries
            .iter()
            .filter(move |record| record.step == step)
            .map(|record| &record.content)
    }
}

#[cfg(not(feature = "records"))]
impl<T> Recorder<T> {
    /// 空记录器（零大小）。
    pub const fn new() -> Recorder<T> {
        Recorder {
            marker: std::marker::PhantomData,
        }
    }

    /// **空实现**：丢弃内容（关闭时零开销）。
    pub fn push(&mut self, _step: Nat, _content: T) {}

    /// 只读遍历（关闭时恒为空）。
    pub fn entries(&self) -> &[Record<T>] {
        &[]
    }

    /// 空实现。
    pub fn clear(&mut self) {}

    /// 记录条数（关闭时恒为 0）。
    pub fn len(&self) -> Nat {
        Nat::ZERO
    }

    /// 关闭时恒为空。
    pub fn is_empty(&self) -> bool {
        true
    }

    /// 关闭时恒为空迭代。
    pub fn step_entries(&self, _step: Nat) -> impl Iterator<Item = &T> + '_ {
        std::iter::empty()
    }
}

impl<T: ShowRecord> Recorder<T> {
    /// **导出为纯文本**（只读；每行 = 步号 + 内容自己的显示）。
    pub fn render(&self) -> String {
        let mut text = String::new();
        for record in self.entries() {
            text.push_str(&format!("[步 {}] ", record.step.get()));
            record.content.show(&mut text);
            text.push('\n');
        }
        text
    }
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;

    /// 内容类型 A（模拟一个模块自己的记录类型）。
    #[derive(Clone, PartialEq, Eq, Debug)]
    struct ModuleARecord {
        what: &'static str,
        amount: Nat,
    }

    impl ShowRecord for ModuleARecord {
        fn show(&self, out: &mut String) {
            out.push_str(&format!("A:{} x{}", self.what, self.amount.get()));
        }
    }

    /// 内容类型 B（模拟另一个模块自己的记录类型）。
    #[derive(Clone, PartialEq, Eq, Debug)]
    struct ModuleBRecord(String);

    impl ShowRecord for ModuleBRecord {
        fn show(&self, out: &mut String) {
            out.push_str("B:");
            out.push_str(&self.0);
        }
    }

    #[test]
    fn records_keep_push_order_and_step_numbers() {
        let mut recorder: Recorder<ModuleARecord> = Recorder::new();
        assert!(recorder.is_empty());
        recorder.push(Nat::ZERO, ModuleARecord { what: "伤害", amount: Nat::new(6) });
        recorder.push(Nat::ZERO, ModuleARecord { what: "治疗", amount: Nat::new(2) });
        recorder.push(Nat::new(1), ModuleARecord { what: "召唤", amount: Nat::new(1) });
        assert_eq!(recorder.len(), Nat::new(3));
        let entries = recorder.entries();
        assert_eq!(entries[0].step, Nat::ZERO);
        assert_eq!(entries[1].content.what, "治疗");
        assert_eq!(entries[2].step, Nat::new(1));
        // 按步读。
        let first_step: Vec<&'static str> = recorder
            .step_entries(Nat::ZERO)
            .map(|record| record.what)
            .collect();
        assert_eq!(first_step, vec!["伤害", "治疗"]);
    }

    #[test]
    fn display_is_defined_by_content_types() {
        let mut a: Recorder<ModuleARecord> = Recorder::new();
        a.push(Nat::ZERO, ModuleARecord { what: "伤害", amount: Nat::new(6) });
        let mut b: Recorder<ModuleBRecord> = Recorder::new();
        b.push(Nat::ZERO, ModuleBRecord("入场".to_string()));
        // 同一个记录器只装一种内容；不同模块各自实例化自己的 Recorder。
        assert_eq!(a.render(), "[步 0] A:伤害 x6\n");
        assert_eq!(b.render(), "[步 0] B:入场\n");
    }

    #[test]
    fn entries_are_read_only_and_clear_empties_the_container() {
        let mut recorder: Recorder<ModuleARecord> = Recorder::new();
        recorder.push(Nat::ZERO, ModuleARecord { what: "伤害", amount: Nat::new(3) });
        let snapshot: Vec<ModuleARecord> = recorder
            .entries()
            .iter()
            .map(|record| record.content.clone())
            .collect();
        assert_eq!(snapshot.len(), 1);
        recorder.clear();
        assert!(recorder.is_empty());
        assert_eq!(snapshot[0].amount, Nat::new(3));
        assert!(recorder.render().is_empty());
    }

    #[test]
    fn empty_recorder_renders_nothing_and_records_belong_to_explicit_steps() {
        let recorder: Recorder<ModuleBRecord> = Recorder::new();
        assert!(recorder.render().is_empty());
        let mut recorder: Recorder<ModuleBRecord> = Recorder::new();
        recorder.push(Nat::new(2), ModuleBRecord("第二步".to_string()));
        recorder.push(Nat::new(2), ModuleBRecord("第二步之二".to_string()));
        assert_eq!(recorder.render().lines().count(), 2);
        assert!(recorder.render().starts_with("[步 2] "));
    }
}
