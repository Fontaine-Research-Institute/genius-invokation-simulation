# 定义层-Source-Version-Generated

> 中文 · [[定义层-Source-Version-Generated-EN|English]]

*本页范围：源码嵌入（每文件自声明源码 + 依赖）、版本声明宏、「不删旧版本」口径，以及生成数据与富文本渲染。*

---

## 1. 职责

三件非规则基础设施：① 每文件自声明源码（编译期嵌入全文 + 依赖 + 归属，可裁剪）；② 游戏版本声明宏（只写「本卡 / 本模块已同步到的版本」）；③ 生成数据与富文本渲染（`generated/`，运行期不读文件）。

## 2. 关键概念与类型

| 概念 | 文件 | 说明 |
|---|---|---|
| `card_source!` / `module_source!` | `crates/definitions/src/source/mod.rs` | 文件头声明「本文件 ↔ 归属」；卡牌宏绑定本模块 `pub const ID: CardId`，不写 id |
| `SourceFile` | 同上 | `owner` / `path` / `segments` / `content`（`include_str!` 原文）/ `dependencies`；`dependencies()` 零分配视图 |
| `SOURCE_FILES` / `source_files` / `source_of` / `source_files_under` | 同上 | 汇总入口；`SOURCE_FILES` 是手写清单，漏加不报错但查不到 |
| `GameVersion` / `card_game_version!` | `crates/definitions/src/version/mod.rs`、`version/macros.rs` | `major.minor`；宏生成 `GAME_VERSION`，语义 = 已同步到的最新游戏版本 |
| 生成数据 | `crates/definitions/build.rs` + `generated/data/*.json` | 编译期读 JSON 生成静态表，运行期不读文件 |
| `Localized` / `Resolved` / `TagText` / `TAGS` | `generated/localized.rs`、`build.rs` | 文本三态与标签唯一数据（见 [[定义层-Tags]]） |
| 富文本两段式 | `generated/text/mod.rs`、`generated/text/render.rs`、richtext crate | 解析（严格、无游戏语义）与渲染（名字表 + 伤害值 + 缺失策略）分离 |

## 3. 公开入口

| 入口 | 文件 | 说明 |
|---|---|---|
| `source_files()` / `source_of(path)` / `source_files_under(prefix)` | `source/mod.rs` | 汇总表的只读查询（手写清单） |
| `SourceFile::dependencies()` / `Dependency::{kind, is_parsed, path, members, alias, raw}` | 同上 | 依赖只存原文字节区间；解析失败记 `Unparsed` 并保留原文 |
| `GameVersion::{new, major, minor}` | `version/mod.rs` | `const`，`Ord`，`Display` 输出 `major.minor` |
| `card_game_version!(major, minor)` | `version/macros.rs` | 生成 `pub const GAME_VERSION` |
| `keyword` / `keywords_all` / `action_card_type_text` / `status_type_text` | `generated/mod.rs` | 生成表查询 |
| `render` / `render_segments` / `TextSources` / `MissingPolicy` | `generated/text/render.rs` | 严格解析后渲染；缺失策略默认保留占位符 |

### 源码（逐字）

> [!NOTE]
> 以下为源码嵌入、版本宏与生成数据的真实源码摘录（含宏定义；函数体从略）。

**`crates/definitions/src/source/mod.rs（SourceOwner / SourceFile）`**

```rust

/// 一份源码文件的**归属**（D216）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceOwner {
    /// 卡牌：归属到卡 id（宏自动绑定本模块的 ID）。
    Card(CardId),
    /// 非卡牌模块：归属到模块自己的多语名字。
    Module(&'static Localized),
}


/// 一份**编译期嵌入的完整源代码**（D213 / D214 / D215 / D216）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// 归属（卡牌 id / 模块多语名字）。
    pub owner: SourceOwner,
    /// 相对 crate 根的路径（段用 / 拼接，层级原样保留）。
    pub path: &'static str,
    /// 宏里写的路径分段（原样保留）。
    pub segments: &'static [&'static str],
    /// 本文件全文（include_str! 原文嵌入；不做宏展开 / 解析）。
    pub content: &'static str,
    /// 编译期解析出的依赖表（D214 / D216；用 [SourceFile::dependencies] 取结构化视图）。
    pub dependencies: ParsedDependencies,
}
```

**`crates/definitions/src/source/mod.rs（card_source! 宏，embed-source 开启版）`**

```rust
#[cfg(feature = "embed-source")]
#[macro_export]
macro_rules! card_source {
    (path: [$($segment:literal),+ $(,)?] $(,)?) => {
        /// 本文件的归属 + 编译期嵌入的原文 + 解析出的依赖（D213 / D214 / D215 / D216）。
        pub static SOURCE: $crate::source::SourceFile = {
            // D215：宏里不写 id —— 绑定本模块的 pub const ID: CardId；
            // 本模块缺 ID 或类型不对时这里编译报错（不静默）。
            // 本模块需定义 pub const ID: CardId。
            const _: $crate::base::CardId = ID;
            const CONTENT: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/",
                concat!("src/cards", $("/", $segment),+, "/mod.rs")
            ));
            $crate::source::SourceFile {
                // D215：id 自动绑定到本模块的 ID（调用处解析，不重复写卡 id）。
                owner: $crate::source::SourceOwner::Card(ID),
                path: concat!("src/cards", $("/", $segment),+, "/mod.rs"),
                segments: &[$($segment),+],
                content: CONTENT,
                dependencies: $crate::source::parse_dependencies(CONTENT),
            }
        };
    };
}
```

**`crates/definitions/src/source/mod.rs（module_source! 宏，embed-source 开启版）`**

```rust
#[cfg(feature = "embed-source")]
#[macro_export]
macro_rules! module_source {
    (path: [$($segment:literal),+ $(,)?], name: $name:expr $(,)?) => {
        /// 本模块的归属 + 编译期嵌入的原文 + 解析出的依赖（D216）。
        pub static SOURCE: $crate::source::SourceFile = {
            const CONTENT: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/",
                concat!("src", $("/", $segment),+, ".rs")
            ));
            $crate::source::SourceFile {
                owner: $crate::source::SourceOwner::Module(&$name),
                path: concat!("src", $("/", $segment),+, ".rs"),
                segments: &[$($segment),+],
                content: CONTENT,
                dependencies: $crate::source::parse_dependencies(CONTENT),
            }
        };
    };
}
```

**`crates/definitions/src/source/mod.rs（公开查询函数签名）`**

```rust
    /// 从原文切出这一段（区间来自编译期解析，安全；span 越界会 panic 而不是返回错数据）。
    pub fn of<'a>(&self, content: &'a str) -> &'a str

    /// 本文件解析出的依赖列表（只读视图；路径分段按需切原文，零分配）。
    pub fn dependencies(&self) -> DependencyList<'_>

    /// 直接解析一段源码（测试 / 工具用；正式使用走 [SourceFile::dependencies]）。
    pub const fn parse(content: &'a str) -> DependencyList<'a>

    /// 依赖条数。
    pub const fn len(&self) -> usize

    /// 是否没有解析出任何依赖。
    pub const fn is_empty(&self) -> bool

    /// 逐条遍历（零分配）。
    pub fn iter(&self) -> impl Iterator<Item = Dependency<'a>> + '_

    /// 依赖种类。
    pub const fn kind(self) -> DependencyKind

    /// 是否解析成功（Unparsed = false，此时用 [Dependency::raw] 看原文）。
    pub const fn is_parsed(self) -> bool

    /// 路径分段（**token 列表**：每段一个 &str；Brace 时是花括号前的公共前缀）。
    pub fn path(self) -> impl Iterator<Item = &'a str> + 'a

    /// 花括号成员（仅 Brace；每段一个 &str）。
    pub fn members(self) -> impl Iterator<Item = &'a str> + 'a

    /// use ... as X 的别名（无别名 = None）。
    pub fn alias(self) -> Option<&'a str>

    /// Unparsed 时保留的整条原文（成功解析 = None）。
    pub fn raw(self) -> Option<&'a str>

/// 全部已登记的源码文件（只读；顺序 = [SOURCE_FILES]）。
pub fn source_files() -> &'static [&'static SourceFile]

/// 按 path 精确查找（path 口径 = 宏路径分段拼出的相对 crate 根路径）。
pub fn source_of(path: &str) -> Option<&'static SourceFile>

/// 按路径前缀筛出某目录下的文件（path 口径同上；只读迭代器）。
pub fn source_files_under(prefix: &str) -> impl Iterator<Item = &'static SourceFile> + '_

pub const fn parse_dependencies(source: &str) -> ParsedDependencies
```

**`crates/definitions/src/version/mod.rs（GameVersion）`**

```rust
///
/// 派生 Ord / PartialOrd 用于普通比较；构造与访问器是 const。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct GameVersion {
    /// 主版本号，如 6。
    major: Nat,
    /// 次版本号，如 2。
    minor: Nat,
}

impl GameVersion {
    /// const 构造（写常量用）。
    pub const fn new(major: Nat, minor: Nat) -> GameVersion {
        GameVersion { major, minor }
    }

    /// 主版本号。
    pub const fn major(self) -> Nat {
        self.major
    }

    /// 次版本号。
    pub const fn minor(self) -> Nat {
        self.minor
    }
}

impl fmt::Display for GameVersion {
    /// 输出 major.minor（如 6.2）。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.major, f)?;
        f.write_str(".")?;
        fmt::Display::fmt(&self.minor, f)
    }
}
```

**`crates/definitions/src/version/macros.rs（card_game_version!）`**

```rust
/// 声明「本卡 / 本模块已同步（核对）到的最新游戏版本」（D245 / D246）。
///
/// 用法：crate::card_game_version!(6, 2); 生成
/// pub const GAME_VERSION: GameVersion = GameVersion::new(Nat::new(6), Nat::new(2));
///
/// 语义（D245 / D246）：
/// - 这个值 = 本卡已同步（核对）到的最新游戏版本；
/// - 「已同步到最新版本」的唯一表示方式 = 把宏里的版本号改成最新版本号；
/// - 不要求卡牌内容有任何改动：新版本没调整这张卡，同样算同步（未调整也算同步）；
/// - 卡牌实现与版本数值解耦：数值只表示「核对过，且该卡在该版本未调整」。
///
/// 升级流程（D246）：**改数字 -> 不动卡牌内容**。
/// 例：最新版变成 6.3 时，把 crate::card_game_version!(6, 2); 改成
/// crate::card_game_version!(6, 3);，卡牌实现保持原样即算已同步。
#[macro_export]
macro_rules! card_game_version {
    ($major:expr, $minor:expr $(,)?) => {
        /// 本卡 / 本模块已同步（核对）到的最新游戏版本（D245 / D246；由 card_game_version! 生成）。
        pub const GAME_VERSION: $crate::version::GameVersion = $crate::version::GameVersion::new(
            $crate::version::Nat::new($major),
            $crate::version::Nat::new($minor),
        );
    };
}
```

**`crates/definitions/src/generated/mod.rs（生成表查询函数签名）`**

```rust
    /// const 构造（生成表 / 代码里的常量用）。
    pub const fn new(value: Nat) -> KeywordCode

    /// 取数值。
    pub const fn get(self) -> Nat

/// 关键字查询：按 K 码二分查找（生成表按 code 升序）。
///
/// **数据来源**：generated/data/keywords.json（权威七圣召唤 keyword 数据，142 码 × 4 语名字 +
/// 说明；129 码有 4 语说明，13 码无说明——含手牌状态 3003~3008，数据没有就保持空）。
/// 名字/说明都**不是反推**的。将来换数据时**只覆盖 keywords.json 再 rebuild**，本函数与生成表结构不变。
pub fn keyword(code: KeywordCode) -> Option<&'static KeywordDef>

/// 全部关键字（按 K 码升序）。
pub fn keywords_all() -> &'static [KeywordDef]

/// 行动牌类型文本（4 语名字；描述本轮为空）。
pub fn action_card_type_text(card_type: ActionCardType) -> Option<&'static TextEntry>

/// 状态类型文本（4 语名字；描述本轮为空）。
pub fn status_type_text(status_type: StatusType) -> Option<&'static TextEntry>
```

**`crates/definitions/src/generated/text/render.rs（MissingPolicy / TextSources / render）`**

```rust
/// 注意：这不是解析兜底 —— 解析阶段的未匹配一律 ParseError（D63）；
/// 这里只决定**结构认识、但名字表里没有**的片段怎么输出。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MissingPolicy {
    /// 原样保留占位符（默认；不静默丢字）。
    #[default]
    KeepPlaceholder,
    /// 输出空串（由调用方显式选择）。
    Empty,
}

/// 渲染取值来源：只放名字表 + 伤害数值 + 缺失策略。
///
/// 入参一律用 base 的 id 新类型（`CardId` / `SkillId`，都是包 Nat 的），不写裸整数。
/// 卡牌 / 角色都按 `CardId`（角色也是牌，属于 `CardFamily::Character`）；技能按 `SkillId`。
pub trait TextSources {
    /// 关键字名（K 码）。
    fn keyword_name(&self, code: KeywordCode) -> Option<&'static Localized>;

    /// 卡牌名（`$[C…]`）。
    fn card_name(&self, id: CardId) -> Option<&'static Localized>;

    /// 角色名（`$[A…]`）；角色也是牌，用 `CardId`。
    fn character_name(&self, id: CardId) -> Option<&'static Localized>;

    /// 技能名（`$[S…]`）。
    fn skill_name(&self, id: SkillId) -> Option<&'static Localized>;

    /// 当前伤害元素（${D__KEY__ELEMENT${ 用）。
    fn base_element(&self) -> Option<DamageElement> {
        None
    }

    /// 伤害元素名；默认直接查生成表（DamageElement::name）。
    fn element_name(&self, element: DamageElement) -> Option<&'static Localized> {
        Some(element.name())
    }

    /// ${D__KEY__DAMAGE${ / ${D__KEY__DAMAGE_n${ 的数值。
    fn damage_value(&self, key: DamageKey) -> Option<Nat> {
        let _ = key;
        None
    }

    /// 找不到文本时的策略。
    fn missing_policy(&self) -> MissingPolicy {
        MissingPolicy::KeepPlaceholder
    }
}

/// 渲染片段序列。
pub fn render_segments(segments: &[Segment], lang: Lang, sources: &dyn TextSources) -> String

/// 渲染整段原文：**先严格解析**（解析失败返回错误），再渲染。
pub fn render(raw: &str, lang: Lang, sources: &dyn TextSources) -> Result<String, ParseError>
```

## 4. 不变量与禁止

> [!IMPORTANT]
> `SOURCE_FILES` 是手写清单：漏加不会编译失败，但汇总入口找不到；直接读文件自己的 `SOURCE` 常量不受影响（D242）。
> 版本宏只表示「本卡 / 本模块已同步到的版本」，没有全仓最新版本常量（D245）。

- `SourceFile.content` 是宏调用文件原文（不展开宏、不解析）；卡牌宏自动绑定 `ID`，不写 id。
- feature `embed-source` 默认开启；关闭时 `SOURCE_FILES` 为空、各文件 `content` 为空串、依赖表为空，访问器照常可调用。
- 生成数据运行期不读文件；标签 / 类型文本只有一份在 `generated`（D67）；新增标签必须同时改 `terms.json` 与标签文件。
- 富文本：数据层保留原文标记；解析严格失败返回 `ParseError`，渲染默认不静默丢字。

## 5. 依据（规则书与 D 编号）

> [!NOTE]
> 本节是基础设施依据，不直接对应规则书条文；版本 / 源码策略来自用户口径与决策日志。

| 编号 | 结论 |
|---|---|
| D213 / D214 / D215 / D216 | 每文件自声明源码、编译期解析依赖、卡牌宏不写 id、两种宏 |
| D242 | 通用机制留 definitions，卡牌源码表在 cards crate；`SOURCE_FILES` 手写清单 |
| D244 / D245 / D246 | 版本宏与「只写同步版本」；D247 阶段收口 |
| D248 / D249 / D250 / D251 | 卡牌多版本区间（仅设计、未实现）；按卡独立选择、默认最新、已实现旧版本不删除 |
| D62 / D75 / D76 | 富文本两段式：解析在 richtext crate、渲染在 definitions |
| D67 | 标签数据唯一一份、字段访问 |
| D240 / D241 / D243 | 具体卡牌在 cards crate（可选 feature）；simulation 默认不含卡牌 |

## 6. 常见误用

> [!WARNING]
> 把版本宏当「最新版本判断」或期望旧版本区间已实现，都会得到错误结论。

1. 用了 `module_source!` 但忘了在 `SOURCE_FILES` 加行：汇总查询不到（测试只保证已登记清单的路径顺序）。
2. 在卡牌宏里手写 id：宏自动绑定本模块 `ID`；缺 `ID` 或类型不对会编译报错。
3. 把版本宏当最新版本判断：它只表示「已同步到」；没有 `is_synced_to` / 最新常量（D245 已删）。
4. 期望旧版本区间已实现：D248 只是设计；当前没有多版本定义 / 加载器。
5. 绕过 `generated::TAGS` 写文案：数据必须唯一一份。
6. 把解析器兜底当渲染缺失策略：解析未匹配 = `ParseError`；`MissingPolicy` 只管名字表缺失。

## 7. 测试

| 文件 | 测试 |
|---|---|
| `source/mod.rs` | `registry_lists_onboarded_module_files_in_path_order`、`prefix_helper_filters_by_path_prefix`、`module_owner_keeps_multilingual_name`、`disabled_embed_means_empty_registry`、`parses_single_alias_brace_and_keeps_unparsed_raw`、`parses_multi_line_brace_import`、`unsupported_forms_are_unparsed_with_raw_text` |
| `version/mod.rs` | `version_compares_and_displays`、`macro_generates_a_readable_constant`、`different_cards_can_declare_different_versions` |
| `generated/mod.rs` | `keyword_table_is_non_empty_sorted_and_complete`、`type_texts_cover_all_four_languages`、`damage_element_names_are_complete`、`tag_data_is_one_field_per_tag_and_pointers_are_distinct`、`descriptions_corpus_is_non_empty` |
| `generated/text/render.rs` | `strips_color_and_sprite_markers`、`replaces_names_by_language`、`replaces_keyword_and_ignores_variant`、`replaces_damage_and_element`、`missing_names_keep_placeholder_or_empty`、`runtime_placeholders_are_kept`、`definitions_sources_uses_generated_keyword_table` |
| `generated/text/tests.rs` | `every_generated_text_parses_strictly`、`keyword_descriptions_parse_strictly` |

## 8. 待接入与待实测

- `留空.md#24`：卡牌多版本区间（D248）未实现；按卡独立选择 / 默认最新 / 旧版本不删除（D249–D251）目前只是设计约定，没有代码路径。
- `留空.md#11`：卡面文案只填中文，其它语言留空（渲染走 `Resolved::Fallback`）；补 4 语未定。
- 源码嵌入 / 生成数据本身无规则语义，没有专属 `留空` 条目；`SOURCE_FILES` 手写清单是已知限制（D242 已写明）。

---

最后核对：2026-10-19（代码：crates/definitions/src/source/mod.rs:SourceFile）