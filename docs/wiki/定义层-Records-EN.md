# Definitions Layer: Records

> [[定义层-Records|中文]] · English

*Scope: the unified record API — the `Record<T>` / `Recorder<T>` / `ShowRecord` contract, per-module content types, and zero-overhead behavior when the feature is off.*

---


## 1. Responsibility

The unified record API: a very thin generic container of "which step + arbitrary content"; content types and display are defined by each module. When feature `records` is off, `Recorder` is zero-sized and `push` is a no-op, without affecting settlement.

## 2. Key concepts and types

- ---: ---; ---
- `Record<T>`: One record: `step: Nat` + `content: T` (文件: `crates/definitions/src/records/mod.rs`)
- `Recorder<T>`: same; Single writer `push`; read-only `entries`; `clear` / `len` / `is_empty` / `step_entries`
- `ShowRecord`: same; Content types implement `show(&self, out: &mut String)`; the recorder presets no kinds / fields
- feature `records`: On by default; off -> zero-sized `Recorder`, `push` discards, `entries` always empty (文件: `crates/definitions/Cargo.toml`)

Per-module content types (one `record.rs` per module, D225):

- ---: ---; ---
- Keywords: `KeywordRecord` (文件: `keywords/record.rs`)
- Damage chain: `DamageChainRecord` (文件: `families/character/damage_chain/record.rs`)
- Flow: `FlowRecord` (文件: `flow/record.rs`)
- Character zone / combat status / equipment: `CharacterZoneRecord` (文件: `families/character/zones/record.rs`)
- Summon zone: `SummonZoneRecord` (文件: `families/summon/zones/record.rs`)
- Support zone / hand-card states / current cost: `SupportRecord` / `HandCardStateRecord` / `CostRecord` (文件: `families/action_card/areas/record.rs`)
- Deck / hand: `PileRecord` / `HandRecord` (文件: `families/action_card/zones/{pile,hand}/record.rs`)

## 3. Public entry points

- ---: ---; ---
- `Record::new(step, content)`: `const` construction of one record (文件: `records/mod.rs`)
- `Recorder::{new, push, entries, clear, len, is_empty, step_entries}`: same; Write / read-only / clear / filter by step
- `impl<T> Default for Recorder<T>`: same; Equivalent to `new()`
- `trait ShowRecord`: same; Content types implement display themselves
- `type XRecorder = Recorder<XRecord>` and `*_recorded` entries: each module's `record.rs`; Module-specific recorders / recorded entry points

### Source

> [!NOTE]
> The core definitions of `records/mod.rs` are reproduced below (`Record` / `ShowRecord` / both `Recorder` variants and their impls, including `cfg` and `derive` lines).

The interfaces are listed per file; the file path is shown above each code block.

File: `crates/definitions/src/records/mod.rs`

```rust
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
```

The file's current public interface is shown above; bodies are omitted.
## 4. Invariants and prohibitions

> [!IMPORTANT]
> The step number is supplied by the writer: the recorder never infers the current step; multiple contents of one action may share a step.
> Records only observe; they never change settlement. When the feature is off, no behavior may depend on recording being enabled.

- Content types belong to their modules: do not add a unified record-kind enum or unified fields in `records`.
- Display belongs to content types: do not format / write localized text inside the recorder; `ShowRecord::show` is the only entry.
- Read-only: `entries()` returns a read-only slice; writes only via `push`; clearing only via `clear`.
- When the feature is off, do not construct content: wrap `push` in `#[cfg(feature = "records")]` at record points.
- The recorder takes no part in position equivalence: states / zones never include `Recorder` in `PartialEq`.

## 5. Basis (rulebook and D-numbers)

> [!NOTE]
> The recorder itself interprets no rules; it only serves the observation need "inserted settlements can be replayed / compared".

- D221: Recording need for replaying inserted settlements
- D223 / D224: Minimal record API: step + arbitrary content; content types and display are self-defined
- D225: Content types in a separate file (each module's `record.rs`)
- D190 / D191: Randomized observation expansion can be recorded but is not written back to state
- D187: Statistics (counts) and records (what happened at which step) are separate tracks

## 6. Common misuse

> [!WARNING]
> Treating records as settlement input creates circular dependencies; putting recorders into position equality breaks state comparison.

1. Adding `enum RecordKind` in `records/mod.rs`: violates "content type belongs to the module".
2. Writing localized formatting outside content types: `ShowRecord` is the display entry.
3. Inferring / rewriting settlement from record results: records only observe.
4. Constructing large objects in `push` arguments when the feature is off: keep construction inside `#[cfg(feature = "records")]`.
5. Putting the recorder into state `PartialEq/Eq`: breaks position equivalence.

## 7. Tests

- `records/mod.rs`: `records_keep_push_order_and_step_numbers`, `display_is_defined_by_content_types`, `entries_are_read_only_and_clear_empties_the_container`, `empty_recorder_renders_nothing_and_records_belong_to_explicit_steps`
- Each module's `record.rs`: record tests for damage chain / keywords / character zones / summon zones / support zone / pile / hand / flow (feature-gated)

## 8. Not yet wired and to be verified

- `留空.md#56`: trace / record paths do not settle reactions (trace types have no reaction entries), diverging from `execute_step_me`.
- The recorder itself has no rule semantics and no dedicated `留空` entry; each module's "which actions are recorded" grows with block wiring.

## Summary

Record only answers which step plus arbitrary content, while content types and ShowRecord display belong to each module. Recorder has a single writer, push, and a read-only entries view; with the feature off it is zero-sized and push is a no-op. Records observe only: they take no part in position equivalence and must never feed back into settlement.

---



Last verified: 2026-10-19 (code: crates/definitions/src/records/mod.rs:Recorder)