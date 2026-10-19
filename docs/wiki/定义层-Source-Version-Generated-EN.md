# Definitions Layer: Source, Version, Generated

> [[定义层-Source-Version-Generated|中文]] · English

*Scope: source embedding (each file declares its own source + dependencies), the version declaration macro and the "old versions are not deleted" convention, and generated data with rich-text rendering.*

---

## 1. Responsibility

Three non-rule infrastructure pieces: ① each file declares its own source (full text + dependencies + ownership embedded at compile time, feature-trimmable); ② a game-version declaration macro (only "the version this card / module has been synced to"); ③ generated data and rich-text rendering (`generated/`, nothing read from files at runtime).

## 2. Key concepts and types

| Concept | File | Notes |
|---|---|---|
| `card_source!` / `module_source!` | `crates/definitions/src/source/mod.rs` | Declare "this file ↔ ownership" at the file top; the card macro binds the module's `pub const ID: CardId` and does not write the id |
| `SourceFile` | same | `owner` / `path` / `segments` / `content` (`include_str!` original text) / `dependencies`; `dependencies()` returns a zero-allocation view |
| `SOURCE_FILES` / `source_files` / `source_of` / `source_files_under` | same | Summary entry points; `SOURCE_FILES` is a hand-written list — missing entries compile but cannot be found |
| `GameVersion` / `card_game_version!` | `crates/definitions/src/version/mod.rs`, `version/macros.rs` | `major.minor`; the macro generates `GAME_VERSION`, meaning "synced to this game version" |
| Generated data | `crates/definitions/build.rs` + `generated/data/*.json` | JSON is read at compile time to build static tables; nothing is read at runtime |
| `Localized` / `Resolved` / `TagText` / `TAGS` | `generated/localized.rs`, `build.rs` | Three-state text and the single tag data (see [[定义层-Tags-EN]]) |
| Rich-text two-stage | `generated/text/mod.rs`, `generated/text/render.rs`, richtext crate | Parsing (strict, no game semantics) separated from rendering (name tables + damage values + missing policy) |

## 3. Public entry points

| Entry | File | Notes |
|---|---|---|
| `source_files()` / `source_of(path)` / `source_files_under(prefix)` | `source/mod.rs` | Read-only queries over the hand-written summary list |
| `SourceFile::dependencies()` / `Dependency::{kind, is_parsed, path, members, alias, raw}` | same | Dependencies store only byte spans of the original text; unparsable entries become `Unparsed` and keep their raw text |
| `GameVersion::{new, major, minor}` | `version/mod.rs` | `const`, `Ord`, `Display` prints `major.minor` |
| `card_game_version!(major, minor)` | `version/macros.rs` | Generates `pub const GAME_VERSION` |
| `keyword` / `keywords_all` / `action_card_type_text` / `status_type_text` | `generated/mod.rs` | Queries over generated tables |
| `render` / `render_segments` / `TextSources` / `MissingPolicy` | `generated/text/render.rs` | Render after strict parsing; the default missing policy keeps placeholders |

### Source (verbatim)

> [!NOTE]
> Real source excerpts for source embedding, the version macro, and generated data (macro definitions included; bodies omitted).

**`crates/definitions/src/source/mod.rs (SourceOwner / SourceFile)`**

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

**`crates/definitions/src/source/mod.rs (card_source! macro, embed-source enabled)`**

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

**`crates/definitions/src/source/mod.rs (module_source! macro, embed-source enabled)`**

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

**`crates/definitions/src/source/mod.rs (public query function signatures)`**

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

**`crates/definitions/src/version/mod.rs (GameVersion)`**

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

**`crates/definitions/src/version/macros.rs (card_game_version!)`**

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

**`crates/definitions/src/generated/mod.rs (generated-table query signatures)`**

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

**`crates/definitions/src/generated/text/render.rs (MissingPolicy / TextSources / render)`**

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

## 4. Invariants and prohibitions

> [!IMPORTANT]
> `SOURCE_FILES` is a hand-written list: a missing entry does not fail compilation, but the summary entry point cannot find it; reading a file's own `SOURCE` constant still works (D242).
> The version macro only states "the version this card / module has been synced to"; there is no repository-wide latest-version constant (D245).

- `SourceFile.content` is the original macro-call file text (macros are not expanded and nothing is parsed); the card macro binds `ID` automatically.
- Feature `embed-source` is on by default; off -> `SOURCE_FILES` is empty, each file's `content` is an empty string, dependency tables are empty, and accessors still work.
- Generated data is never read from files at runtime; tag / type text exists exactly once in `generated` (D67); adding a tag requires both `terms.json` and the tag file.
- Rich text keeps markers in the data layer; strict parsing failure returns `ParseError`, and rendering keeps missing text by default instead of silently dropping it.

## 5. Basis (rulebook and D-numbers)

> [!NOTE]
> This section covers infrastructure, not rulebook clauses; version / source policies come from user decisions and the decision log.

| Number | Conclusion |
|---|---|
| D213 / D214 / D215 / D216 | Each file declares its source, dependencies parsed at compile time, the card macro writes no id, two macro forms |
| D242 | Shared mechanisms stay in definitions; the card source table lives in the cards crate; `SOURCE_FILES` is hand-written |
| D244 / D245 / D246 | Version macro and "declare the synced version only"; D247 closed the phase |
| D248 / D249 / D250 / D251 | Card version intervals (design only, not implemented); per-card selection, latest by default, implemented old versions are kept |
| D62 / D75 / D76 | Rich-text two-stage: parsing in the richtext crate, rendering in definitions |
| D67 | Single tag data source, field access |
| D240 / D241 / D243 | Concrete cards live in the cards crate (optional feature); simulation excludes cards by default |

## 6. Common misuse

> [!WARNING]
> Treating the version macro as a "latest-version check" or expecting version intervals to exist both lead to wrong conclusions.

1. Using `module_source!` but forgetting the `SOURCE_FILES` line: summary queries cannot find it (tests only pin path order of registered entries).
2. Writing an id by hand in the card macro: it binds the module's `ID`; a missing `ID` or wrong type fails compilation.
3. Treating the version macro as a latest-version check: it only means "synced to"; there is no `is_synced_to` / latest constant (removed by D245).
4. Expecting version intervals to be implemented: D248 is design only; there is currently no multi-version definition / loader.
5. Bypassing `generated::TAGS` and writing text in tag files: data must have a single copy.
6. Treating parser fallback as the render missing policy: unmatched parsing is `ParseError`; `MissingPolicy` only handles missing name-table entries.

## 7. Tests

| File | Tests |
|---|---|
| `source/mod.rs` | `registry_lists_onboarded_module_files_in_path_order`, `prefix_helper_filters_by_path_prefix`, `module_owner_keeps_multilingual_name`, `disabled_embed_means_empty_registry`, `parses_single_alias_brace_and_keeps_unparsed_raw`, `parses_multi_line_brace_import`, `unsupported_forms_are_unparsed_with_raw_text` |
| `version/mod.rs` | `version_compares_and_displays`, `macro_generates_a_readable_constant`, `different_cards_can_declare_different_versions` |
| `generated/mod.rs` | `keyword_table_is_non_empty_sorted_and_complete`, `type_texts_cover_all_four_languages`, `damage_element_names_are_complete`, `tag_data_is_one_field_per_tag_and_pointers_are_distinct`, `descriptions_corpus_is_non_empty` |
| `generated/text/render.rs` | `strips_color_and_sprite_markers`, `replaces_names_by_language`, `replaces_keyword_and_ignores_variant`, `replaces_damage_and_element`, `missing_names_keep_placeholder_or_empty`, `runtime_placeholders_are_kept`, `definitions_sources_uses_generated_keyword_table` |
| `generated/text/tests.rs` | `every_generated_text_parses_strictly`, `keyword_descriptions_parse_strictly` |

## 8. Not yet wired and to be verified

- `留空.md#24`: card version intervals (D248) are not implemented; per-card selection / latest by default / old versions kept (D249–D251) are design conventions with no code path.
- `留空.md#11`: card-face text fills Chinese only; other languages are empty (rendered via `Resolved::Fallback`); filling all four is undecided.
- Source embedding / generated data have no rule semantics and no dedicated `留空` entry; the hand-written `SOURCE_FILES` list is a known limitation (documented in D242).

---

Last verified: 2026-10-19 (code: crates/definitions/src/source/mod.rs:SourceFile)