# Crate-Richtext (富文本解析) 

> 中文 · [[Crate-Richtext-EN|English]]

*本页范围: `richtext` crate 的职责, 片段模型与白名单, 公开入口, 不变量, 依据, 测试与待接入项.*

---

## 1. 职责

原始标记文本 → 中性片段序列.只认语法结构 (颜色标签 / `$[` 占位符 / `${[` token / `{SPRITE_PRESET#…}` 图标 / 普通文本) , 不认识语言, 名字表, 卡牌, 标签, 关键字; 严格解析, 任何未识别内容都返回错误, 没有兜底分支.

- 目录: `new/crates/richtext/`
- package / lib: `gi-genius-invokation-richtext` / `gi_genius_invokation_richtext`
- 依赖: 无 (不依赖 engine / definitions, 也不引第三方 crate; D75 / D76) 

## 2. 关键概念与类型

| 概念 | 位置 | 说明 |
|---|---|---|
| 片段 | `src/parse.rs`: `Segment` | `Text` / `ColorStart` / `ColorEnd` / `Keyword { code, variant }` / `Card` / `Character` / `Skill` / `Damage` / `Variable` / `TokenValue` / `Icon` |
| 伤害占位 | `src/parse.rs`: `DamageKey` | `Element` / `Damage` / `Numbered(u32)`; `marker()` 还原原始标记 |
| 解析错误 | `src/parse.rs`: `ParseError` | `position` (字节下标) + `found` + `expected`; `line_col()` / `snippet()` / `Display` / `Error` |
| any 白名单 | `src/parse.rs`: `AnyMatch` / `ALLOWED_ANY_MATCHES` | 唯一允许'中间任意'的匹配点 = 颜色值 (`ColorDigits`, D65)  |
| 固定允许集 | `src/parse.rs` | `KEYWORD_VARIANTS = ["s1"]`, `SPRITE_PRESETS` (43 个) , `TOKEN_VALUES` (1 个) ——逐个枚举, 固定匹配, 未登记即失败 (D65 / D66)  |
| 解析入口 | `src/parse.rs`: `parse()` | 整段文本 → `Result<Vec<Segment>, ParseError>` |
| crate 导出 | `src/lib.rs` | 再导出 `Segment` / `parse` / `ParseError` / `AnyMatch` / `DamageKey` / 三个白名单常量; `#![forbid(unsafe_code)]` |
| 测试 | `src/tests.rs` | 17 条单元测试 + 1 条 doctest |

**接受的标记形态**

| 形态 | 例子 | 产物 |
|---|---|---|
| 颜色 | `<color=#FFFFFFFF>` / `</color>` | `ColorStart("#FFFFFFFF")` / `ColorEnd` (颜色值是唯一 any)  |
| 关键字 | `$[K<标识>]`, `$[K<标识>|s1]` | `Keyword { code, variant }` (变体只允许 `s1`)  |
| 名字引用 | `$[C114181]` / `$[A1418]` / `$[S14181]` | `Card` / `Character` / `Skill` |
| 伤害占位 | `$[D__KEY__ELEMENT]` / `$[D__KEY__DAMAGE_1]` | `Damage(DamageKey)` |
| 运行期变量 | `$[G16121]` | `Variable` (原样保留)  |
| token 数值 | `${[GCG_TOKEN_SHIELD]}` | `TokenValue` (名字必须在 `TOKEN_VALUES`)  |
| 图标 | `{SPRITE_PRESET#3503}` | `Icon` (预设名必须在 `SPRITE_PRESETS`)  |

## 3. 公开入口

| 入口 | 签名 |
|---|---|
| `parse` | `pub fn parse(raw: &str) -> Result<Vec<Segment>, ParseError>` |
| `Segment::any_match` | `pub fn any_match(&self) -> Option<AnyMatch>` (穷尽 match)  |
| `Segment::raw_marker` | `pub fn raw_marker(&self) -> Option<String>` (占位符还原原始标记)  |
| `DamageKey::marker` | `pub fn marker(self) -> String` |
| `ParseError::line_col` / `snippet` | `(&self, raw: &str) -> (usize, usize)` / `-> String` |
| 白名单常量 | `ALLOWED_ANY_MATCHES` / `KEYWORD_VARIANTS` / `SPRITE_PRESETS` / `TOKEN_VALUES` |

> [!NOTE]
> definitions 侧只 `pub use` 本 crate 的解析器 (`generated::text`) , 不留第二份 (D75 / D76) ; 渲染在 `definitions/src/generated/text/render.rs`.

### 3.1 真实源码
**伤害占位 `DamageKey`**

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

**any 匹配点 `AnyMatch`**

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

**any 白名单与固定允许集常量**

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

**解析错误 `ParseError`**

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

**解析入口 `parse`**

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


**片段 `Segment`**

`new/crates/richtext/src/parse.rs:80-112`

注释中的关键字代码已替换为名称, 代码行未改

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

## 4. 不变量与禁止

> [!IMPORTANT]
> 严格解析 (D63) : 没有兜底分支; 未知 `$[…]`, 畸形颜色, 未登记图标 / 变体 / token, 孤立的 `$` / `{` / `<` 一律 `ParseError`, 不当普通文本吞掉.

- 头尾固定匹配 (D65) : 每条模式前缀 / 后缀都锚定; 中间 any 只允许白名单 (当前只有颜色值) .
- 整段消化: 解析成功时把整段文本全部片段化; 普通文本 + 颜色结构 + 占位符结构可拼回原始文本.
- 零游戏语义 (D76) : 不引用 `Lang` / `Localized`, 不查名字表 / 卡牌 / 标签 / 关键字表; 关键字 / 卡牌 / 角色 / 技能占位符在这里只是裸数字.
- 零依赖 (D75) ; 内部按 `usize` 位置切 `&str` (D75 允许) .
- 禁止 `unsafe` (`#![forbid(unsafe_code)]`) .
- 白名单只能靠证明扩充: 新增 any 类别必须在 `AnyMatch` 里写清证明; 新增图标 / token / 变体只改常量清单.

> [!WARNING]
> 颜色是唯一 any: 因为渲染层会删除颜色标签, 色值不影响最终文本.图标 / token / 关键字变体都未证明, 必须固定匹配 (D66) .

## 5. 依据 (规则书与 D 编号) 

| 主题 | 依据 |
|---|---|
| 文本拆'解析 / 渲染'两段; 全量文本解析测试 | D62 |
| 解析必须严格: 未匹配 = 失败 | D63 |
| 头尾固定; 中间 any 只允许已证明不影响结果的类别 | D65 / D66 |
| 富文本解析器独立成 crate, 与 engine 同级; 不依赖业务 crate | D75 / D76 |
| crate 命名 (包名不许出现 TCG)  | D77 / D78 |
| 本 crate 无规则书映射 (纯字符串结构)  | — |

## 6. 常见误用

> [!WARNING]
> 不要把解析器当渲染器: 它不取名字, 不选语言, 不删颜色 / 图标; 遇到未知标记也不能当普通文本吞掉 (D63) .

- 给图标 / token / 关键字变体加'any 匹配': 未证明就不许 (D66) .
- 以为 `{SPRITE_PRESET#…}` 在解析阶段就被删掉: 它是 `Segment::Icon`, 删除发生在渲染层.
- 以为 `Variable` / `TokenValue` 有静态值可填: 它们原样保留.
- 在 definitions 里再写一份解析器: 只 `pub use` 本 crate.
- 新增数据出现新占位符 / 图标时放宽匹配: 应先取证再登记, 全量测试会报警.

## 7. 测试

- `cargo test --offline -p gi-genius-invokation-richtext`: 当前 18 条 (17 单元 + 1 doctest) .
- 代表用例: 

| 用例 | 覆盖点 |
|---|---|
| `richtext/src/tests.rs::tests::plain_text_is_one_segment` | 普通文本 |
| `richtext/src/tests.rs::tests::unknown_placeholder_is_error` | 未知占位符报错 |
| `richtext/src/tests.rs::tests::malformed_color_is_error` | 畸形颜色 |
| `richtext/src/tests.rs::tests::lone_marker_characters_are_errors` | 孤立标记字符 |
| `richtext/src/tests.rs::tests::unregistered_sprite_preset_is_error` | 未登记图标 |
| `richtext/src/tests.rs::tests::unregistered_keyword_variant_and_token_are_errors` | 未登记变体 / token |
| `richtext/src/tests.rs::tests::color_is_the_only_any_match` | any 白名单 |
| `richtext/src/tests.rs::tests::raw_marker_rebuilds_placeholders` | 标记还原 |
| `richtext/src/tests.rs::tests::line_col_points_at_error` | 错误定位 |

> [!NOTE]
> 全量文本解析测试在 definitions 侧 (需要生成数据) : `definitions/src/generated/text/tests.rs::every_generated_text_parses_strictly`, `keyword_descriptions_parse_strictly`.

## 8. 待接入与待实测

| 编号 | 事项 |
|---|---|
| #3 | 关键字名字有低置信反推样本 (约 182 个'语言 × 关键字'票数 < 3) , 靠换权威数据覆盖 JSON; 与解析器无关 |
| 无编号 | 渲染侧'结构认识但名字表没有'按 `MissingPolicy` 处理 (`definitions/src/generated/text/render.rs`) ; 不属本 crate |
| 无编号 | 数据出现新图标 / token / 关键字变体: 先取证再登记常量 (D65 / D66)  |

---

最后核对: 2026-10-19 (代码: `new/crates/richtext/src/parse.rs:parse`) 
