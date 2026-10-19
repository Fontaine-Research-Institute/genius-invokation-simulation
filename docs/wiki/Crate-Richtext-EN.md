# Crate-Richtext (Rich-Text Parsing)

> [[Crate-Richtext|中文]] · English

*Scope: the `richtext` crate's responsibility, segment model and whitelists, public entry points, invariants, basis, tests, and pending work.*

---

## 1. Responsibility

Raw marked-up text -> neutral segment sequence. It recognizes syntax structure only (color tags, `$[` placeholders, `${[` tokens, `{SPRITE_PRESET#...}` icons, plain text); it knows nothing about languages, name tables, cards, tags, or keywords. Parsing is strict: any unrecognized content returns an error, with no fallback branch.

- Directory: `new/crates/richtext/`
- package / lib: `gi-genius-invokation-richtext` / `gi_genius_invokation_richtext`
- Dependencies: none (not engine, not definitions, no third-party crate; D75 / D76)

## 2. Key Concepts and Types

| Concept | Location | Notes |
|---|---|---|
| Segment | `src/parse.rs`: `Segment` | `Text` / `ColorStart` / `ColorEnd` / `Keyword { code, variant }` / `Card` / `Character` / `Skill` / `Damage` / `Variable` / `TokenValue` / `Icon` |
| Damage placeholder | `src/parse.rs`: `DamageKey` | `Element` / `Damage` / `Numbered(u32)`; `marker()` rebuilds the raw marker |
| Parse error | `src/parse.rs`: `ParseError` | `position` (byte index) + `found` + `expected`; `line_col()` / `snippet()` / `Display` / `Error` |
| Any-match whitelist | `src/parse.rs`: `AnyMatch` / `ALLOWED_ANY_MATCHES` | the only wildcard point is the color value (`ColorDigits`, D65) |
| Fixed allow-lists | `src/parse.rs` | `KEYWORD_VARIANTS = ["s1"]`, `SPRITE_PRESETS` (43), `TOKEN_VALUES` (1): enumerated, fixed matching, unregistered means failure (D65 / D66) |
| Parse entry | `src/parse.rs`: `parse()` | whole text -> `Result<Vec<Segment>, ParseError>` |
| Crate exports | `src/lib.rs` | re-exports `Segment` / `parse` / `ParseError` / `AnyMatch` / `DamageKey` / the three allow-lists; `#![forbid(unsafe_code)]` |
| Tests | `src/tests.rs` | 17 unit tests + 1 doctest |

**Accepted marker forms**

| Form | Example | Output |
|---|---|---|
| Color | `<color=#FFFFFFFF>` / `</color>` | `ColorStart("#FFFFFFFF")` / `ColorEnd` (color value is the only any-match) |
| Keyword | `$[K<id>]`, `$[K<id>|s1]` | `Keyword { code, variant }` (variant may only be `s1`) |
| Name reference | `$[C114181]` / `$[A1418]` / `$[S14181]` | `Card` / `Character` / `Skill` |
| Damage placeholder | `$[D__KEY__ELEMENT]` / `$[D__KEY__DAMAGE_1]` | `Damage(DamageKey)` |
| Runtime variable | `$[G16121]` | `Variable` (kept as-is) |
| Token value | `${[GCG_TOKEN_SHIELD]}` | `TokenValue` (name must be in `TOKEN_VALUES`) |
| Icon | `{SPRITE_PRESET#3503}` | `Icon` (preset must be in `SPRITE_PRESETS`) |

## 3. Public Entry Points

| Entry point | Signature |
|---|---|
| `parse` | `pub fn parse(raw: &str) -> Result<Vec<Segment>, ParseError>` |
| `Segment::any_match` | `pub fn any_match(&self) -> Option<AnyMatch>` (exhaustive match) |
| `Segment::raw_marker` | `pub fn raw_marker(&self) -> Option<String>` (rebuild placeholder text) |
| `DamageKey::marker` | `pub fn marker(self) -> String` |
| `ParseError::line_col` / `snippet` | `(&self, raw: &str) -> (usize, usize)` / `-> String` |
| Allow-list consts | `ALLOWED_ANY_MATCHES` / `KEYWORD_VARIANTS` / `SPRITE_PRESETS` / `TOKEN_VALUES` |

> [!NOTE]
> The definitions side only `pub use`s this crate's parser (`generated::text`) and keeps no second parser (D75 / D76); rendering lives in `definitions/src/generated/text/render.rs`.

### 3.1 Real Source Excerpts
**Damage placeholder `DamageKey`**

`new/crates/richtext/src/parse.rs:58-67`

```rust
/// `$[D__KEY__…]` 占位符的三种形态。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DamageKey {
    /// `$[D__KEY__ELEMENT]`：伤害元素（由渲染上下文给）。
    Element,
    /// `$[D__KEY__DAMAGE]`：基础伤害值。
    Damage,
    /// `$[D__KEY__DAMAGE_n]`：第 n 个伤害值（n 从 1 起，由渲染上下文给）。
    Numbered(u32),
}
```

**Any-match class `AnyMatch`**

`new/crates/richtext/src/parse.rs:24-35`

```rust
/// D65 any 白名单里的匹配点。
///
/// 只有「中间那段取任何值都**不影响最终文本结果**」的类别才允许进来；
/// 每新增一项都要在这里写清证明。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnyMatch {
    /// `<color=#RRGGBBAA>` 里的颜色值。
    ///
    /// 证明：颜色只影响显示样式；渲染阶段按分工计划 §4.4.16 规则 1 **删除颜色标签**，
    /// 所以颜色值取什么都不会改变渲染后的文本。
    ColorDigits,
}
```

**Any-match whitelist and fixed allow-lists**

`new/crates/richtext/src/parse.rs:37-47`

```rust
/// D65：允许「中间任意」的匹配点**白名单**（目前只有颜色值）。
///
/// - **内容**：`<color=#RRGGBBAA>` 里的颜色值（`AnyMatch::ColorDigits`）。这是**唯一**
///   经证明「中间取任何值都不影响最终文本结果」的类别；其余（图标预设名 / 关键字变体 /
///   token 名 / 各类数字参数）一律**固定匹配**（D66）。
/// - **依据**：D65 / D66；全量解析测试会断言：解析结果里出现的 any 匹配点都在本表内，
///   且 any 出现次数 = 颜色片段数。
/// - **未登记即失败**：解析器没有 any 兜底分支，白名单外的中间值直接返回 ParseError。
/// - **维护**：将来数据变化要增删 any 类别时，**只改这份清单**（以及 `AnyMatch` 的证明），
///   不改解析逻辑。
pub const ALLOWED_ANY_MATCHES: [AnyMatch; 1] = [AnyMatch::ColorDigits];
```

**Parse error `ParseError`**

`new/crates/richtext/src/parse.rs:155-164`

```rust
/// 解析失败的**位置与卡住的内容**（D63：失败必须能逐条打印出来）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    /// 出错处的**字节**下标（0 起）。
    pub position: usize,
    /// 卡住的内容（从出错处起的原文片段，最多 24 个字符）。
    pub found: String,
    /// 这里期望什么（人读的说明）。
    pub expected: &'static str,
}
```

**Parse entry `parse`**

`new/crates/richtext/src/parse.rs:195-201`

```rust
/// 解析整段原文：原始标记 → 片段序列。
///
/// **严格（D63）**：任何未识别的内容都返回 ParseError，不兜底、不当普通文本吞掉。
/// 解析器保证把整段原文消化完（片段拼回去等于原文）。
pub fn parse(raw: &str) -> Result<Vec<Segment>, ParseError> {
    Parser { text: raw, pos: 0 }.parse_all()
}
```


**Segment `Segment`**

`new/crates/richtext/src/parse.rs:80-112`

Keyword codes in comments were replaced with names; code lines are unchanged.

```rust
/// 解析后的一段（渲染器只吃这些，不再看原文）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Segment {
    /// 普通文本。
    Text(String),
    /// `<color=#RRGGBBAA>`：颜色开始（色值原样，含 #）。**唯一的 any 匹配点**。
    ColorStart(String),
    /// `</color>`：颜色结束。
    ColorEnd,
    /// 关键字占位符（如 `准备技能` / `免于被击倒`，可带变体后缀；变体是固定允许集）。
    Keyword {
        /// 关键字标识（数值；这是参数不是 any：不同 关键字标识取不同名字）。
        ///
        /// 本 crate 不认识关键字表，所以这里只是裸数值；definitions 侧再映射。
        code: u32,
        /// `|s1` 这类变体后缀（去掉竖线后原样保留；渲染时由 definitions 侧忽略，按同一 关键字标识取名字）。
        variant: Option<String>,
    },
    /// `$[C114181]`：卡牌名引用（C 号；参数不是 any）。
    Card(u32),
    /// `$[A1418]`：角色名引用（A 号）。
    Character(u32),
    /// `$[S14181]`：技能名引用（S 号）。
    Skill(u32),
    /// `$[D__KEY__…]`：伤害/元素占位。
    Damage(DamageKey),
    /// `$[G16121]`：运行期数值变量；结构认识，但没有静态值可填（渲染时原样保留）。
    Variable(String),
    /// `${[GCG_TOKEN_SHIELD]}`：运行期 token 数值；同上，原样保留。
    TokenValue(String),
    /// `{SPRITE_PRESET#3503}`：图标（预设名按**固定允许集**精确匹配）。
    Icon(String),
}
```

## 4. Invariants and Prohibitions

> [!IMPORTANT]
> Strict parsing (D63): no fallback branch. Unknown `$[...]`, malformed colors, unregistered icons/variants/tokens, and stray `$` / `{` / `<` all return `ParseError` and are never swallowed as plain text.

- Head/tail anchored matching (D65): every pattern anchors its prefix and suffix; the only wildcard is the color value.
- Whole-input consumption: on success the input is fully segmented; plain text, color structure, and placeholders can be rebuilt into the original text.
- Zero game semantics (D76): no `Lang` / `Localized`, no name/card/tag/keyword tables; keyword / card / character / skill placeholders are bare numbers here.
- Zero dependencies (D75); internal `usize` slicing of `&str` is allowed.
- No `unsafe` (`#![forbid(unsafe_code)]`).
- Whitelists expand only with proof: a new any-match class must document its proof in `AnyMatch`; new icons/tokens/variants only extend the constant lists.

> [!WARNING]
> Color is the only any-match because the render layer deletes color tags, so the value cannot affect the final text. Icons, tokens, and keyword variants are unproven and must stay fixed-match (D66).

## 5. Basis (Rulebook and D Numbers)

| Topic | Basis |
|---|---|
| Split text handling into parse and render; parse all texts in tests | D62 |
| Parsing must be strict: no match means failure | D63 |
| Head/tail fixed; any-match only for proven-irrelevant classes | D65 / D66 |
| Rich-text parser is a separate crate next to engine; no business deps | D75 / D76 |
| Crate naming (the package name must not contain TCG) | D77 / D78 |
| This crate has no rulebook mapping (pure string structure) | - |

## 6. Common Misuse

> [!WARNING]
> Do not treat the parser as a renderer: it does not resolve names, choose languages, or strip colors/icons; never swallow unknown markup as text (D63).

- Adding "any-match" for icons/tokens/keyword variants: unproven, forbidden (D66).
- Expecting `{SPRITE_PRESET#...}` to be stripped at parse time: it is a `Segment::Icon`; stripping happens in the renderer.
- Expecting `Variable` / `TokenValue` to carry static values: they are kept as-is.
- Writing a second parser inside definitions: only `pub use` this crate.
- Loosening matching when new data appears: gather evidence and register first; the full-text test will flag it.

## 7. Tests

- `cargo test --offline -p gi-genius-invokation-richtext`: currently 18 (17 unit tests + 1 doctest).
- Representative cases:

| Test | Coverage |
|---|---|
| `richtext/src/tests.rs::tests::plain_text_is_one_segment` | plain text |
| `richtext/src/tests.rs::tests::unknown_placeholder_is_error` | unknown placeholder errors |
| `richtext/src/tests.rs::tests::malformed_color_is_error` | malformed color |
| `richtext/src/tests.rs::tests::lone_marker_characters_are_errors` | stray marker characters |
| `richtext/src/tests.rs::tests::unregistered_sprite_preset_is_error` | unregistered icon |
| `richtext/src/tests.rs::tests::unregistered_keyword_variant_and_token_are_errors` | unregistered variant/token |
| `richtext/src/tests.rs::tests::color_is_the_only_any_match` | any-match whitelist |
| `richtext/src/tests.rs::tests::raw_marker_rebuilds_placeholders` | marker rebuilding |
| `richtext/src/tests.rs::tests::line_col_points_at_error` | error location |

> [!NOTE]
> The full-text parse tests live on the definitions side (they need generated data): `definitions/src/generated/text/tests.rs::every_generated_text_parses_strictly` and `keyword_descriptions_parse_strictly`.

## 8. Pending Work and Field Verification

| No. | Item |
|---|---|
| #3 | Keyword names include low-confidence reconstructed samples (about 182 language x K-code votes below 3); replacing the authoritative JSON fixes it; unrelated to the parser |
| no number | Render-side "structure known but missing from the name table" is handled by `MissingPolicy` (`definitions/src/generated/text/render.rs`); not this crate |
| no number | New icons / tokens / keyword variants in data: gather evidence and register constants (D65 / D66) |

---

Last reviewed: 2026-10-19 (code: `new/crates/richtext/src/parse.rs:parse`)
