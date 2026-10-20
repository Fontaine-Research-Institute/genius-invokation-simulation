# 定义层-Records

> 中文 · [[定义层-Records-EN|English]]

*本页范围: 统一记录 API——`Record<T>` / `Recorder<T>` / `ShowRecord` 契约, 各模块内容类型, 以及 feature 关闭时的零开销行为.*

---


## 1. 职责

统一的记录 API: 极薄的泛型容器"属于哪一步 + 任意内容", 内容类型与显示由各模块自己定义; feature `records` 关闭时零大小, `push` 空操作, 不影响结算.

## 2. 关键概念与类型

- `Record<T>`: 一条记录: `step: Nat` + `content: T` (文件: `crates/definitions/src/records/mod.rs`)
- `Recorder<T>`: 唯一写入 `push`; 只读 `entries`; `clear` / `len` / `is_empty` / `step_entries` (文件: 同上)
- `ShowRecord`: 内容类型实现 `show(&self, out: &mut String)`; 记录器不预设种类 / 字段 (文件: 同上)
- feature `records`: 默认开启; 关闭时 `Recorder` 零大小, `push` 丢弃内容, `entries` 恒空 (文件: `crates/definitions/Cargo.toml`)

各模块内容类型 (一模块一个 `record.rs`, D225) :

- 关键字: `KeywordRecord` (文件: `keywords/record.rs`)
- 伤害链: `DamageChainRecord` (文件: `families/character/damage_chain/record.rs`)
- 流程: `FlowRecord` (文件: `flow/record.rs`)
- 角色区 / 出战状态 / 装备栏: `CharacterZoneRecord` (文件: `families/character/zones/record.rs`)
- 召唤物区: `SummonZoneRecord` (文件: `families/summon/zones/record.rs`)
- 支援区 / 手牌卡状态 / 当前费用: `SupportRecord` / `HandCardStateRecord` / `CostRecord` (文件: `families/action_card/areas/record.rs`)
- 牌库 / 手牌: `PileRecord` / `HandRecord` (文件: `families/action_card/zones/{pile,hand}/record.rs`)

## 3. 公开入口

- `Record::new(step, content)`: `const` 构造一条记录 (文件: `records/mod.rs`)
- `Recorder::{new, push, entries, clear, len, is_empty, step_entries}`: 写入 / 只读 / 清空 / 按步筛选 (文件: 同上)
- `impl<T> Default for Recorder<T>`: 等价于 `new()` (文件: 同上)
- `trait ShowRecord`: 内容类型自己实现显示 (文件: 同上)
- `type XRecorder = Recorder<XRecord>` 与 `*_recorded` 入口: 各模块专用记录器 / 带记录的入口 (文件: 各模块 `record.rs`)

### 源码

> [!NOTE]
> 下面贴出 `records/mod.rs` 的核心定义 (`Record` / `ShowRecord` / 两种 `Recorder` 及其 impl, 含 `cfg` 与 `derive` 行) .

下面按文件给出接口, 每个代码块上方标出文件位置.

文件: `crates/definitions/src/records/mod.rs`

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

以上是该文件当前的公开接口, 函数体从略.
## 4. 不变量与禁止

> [!IMPORTANT]
> 步号由写入方给: 记录器不推断当前步; 同一动作的多条内容可以同 step.
> 记录只写观测, 不改结算; 关闭 feature 时不允许留下"只有开记录才发生"的行为.

- 内容类型归模块自己: 不要在 `records` 里加统一记录种类大枚举或统一字段.
- 显示归内容类型: 不要在记录器里写格式化 / 中文文案; `ShowRecord::show` 是唯一入口.
- 只读: `entries()` 返回只读切片; 写入只能 `push`; 清空只能 `clear`.
- 关闭 feature 时不得构造内容: 模块内记录点用 `#[cfg(feature = "records")]` 包住 `push`.
- 记录器不参与局面等价: 各状态 / 区不把 `Recorder` 放进 `PartialEq`.

## 5. 依据 (规则书与 D 编号)

> [!NOTE]
> 记录器本身不解释规则; 它只服务"插入式结算可回放 / 可对照"这一观测需求.

- D221: 插入式结算回放的记录需求
- D223 / D224: 记录 API 极薄: 属于哪一步 + 任意内容, 内容类型与显示自定
- D225: 内容类型单独一个文件 (各模块 `record.rs`)
- D190 / D191: 随机效果的观测展开可记录, 但不写回状态
- D187: 统计 (计数) 与记录 (哪一步发生了什么) 是两条线

## 6. 常见误用

> [!WARNING]
> 把记录当结算输入会形成循环依赖; 把记录器放进局面等价会因记录差异破坏状态比较.

1. 在 `records/mod.rs` 加 `enum RecordKind`: 违反"内容类型由模块自己定".
2. 在内容类型之外写中文格式化: `ShowRecord` 才是显示入口.
3. 用记录结果反推 / 改写结算: 记录只写观测.
4. 关闭 feature 时在 `push` 参数里构造大对象: 构造应放进 `#[cfg(feature = "records")]` 内.
5. 把记录器放进状态 `PartialEq/Eq`: 破坏局面等价.

## 7. 测试

- `records/mod.rs`: `records_keep_push_order_and_step_numbers`, `display_is_defined_by_content_types`, `entries_are_read_only_and_clear_empties_the_container`, `empty_recorder_renders_nothing_and_records_belong_to_explicit_steps`
- 各模块 `record.rs`: 伤害链 / 关键字 / 角色区 / 召唤物区 / 支援区 / 牌库 / 手牌 / 流程的记录测试 (feature 门控)

## 8. 待接入与待实测

- `留空.md#56`: trace / record 路径不结算反应 (轨迹类型没有反应条目) , 与 `execute_step_me` 不一致.
- 记录器本身无规则语义, 没有专属 `留空` 条目; 各模块"记录哪些动作"的覆盖范围随区块接线扩展.

## 小结

Record 只回答哪一步加任意内容, 内容类型与 ShowRecord 显示归各模块. Recorder 唯一写入口是 push, entries 只读; feature 关闭时零大小且 push 无操作. 记录只写观测, 不参与局面等价, 也不能反过来影响结算.

---



最后核对: 2026-10-19 (代码: crates/definitions/src/records/mod.rs:Recorder)