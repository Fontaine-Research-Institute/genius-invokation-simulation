# 定义层-Tags

> 中文 · [[定义层-Tags-EN|English]]

*本页范围: 标签体系——一标签一文件一零大小类型, `TagText`, `Localized` / `Resolved`, 以及生成的唯一标签数据 `TAGS`.*

---

## 1. 职责

标签是独立顶层目录 `tags/` 下的'一标签一文件一零大小类型' (D53 / D54) ; 文本数据只有一份, 编译期生成到 `generated::TAGS`, 运行期身份 = 字段地址 (D67) .没有标签枚举, 数字 / 字符串编号或查表.

## 2. 关键概念与类型

| 概念 | 文件 | 说明 |
|---|---|---|
| 标签类型 | `crates/definitions/src/tags/<family>/<type>/<tag>.rs` | 每个标签一个 `pub struct Xxx;` (ZST) , 并提供 `data() -> &'static TagText` 返回 `&TAGS.<field>` |
| 目录分组 | `tags/action_card/{support,equipment,event}`, `tags/action_card/common`, `tags/character/{element,nation,arkhe}`, `tags/summon`, `tags/enemy`, `tags/status` | 按牌族 / 类型分组; 跨牌族共用标签只导入唯一定义处 (D55)  |
| `TagText` | `crates/definitions/build.rs` 生成到 `OUT_DIR/generated_data.rs` | `{ name: Localized, description: Localized }`; description 本轮无数据, 恒为空 |
| `TAGS` / `Tags` | 同上 | 一个标签一个字段; 字段名 = `terms.json` 的 `rust` 标识转 snake_case; 字段访问无查表 |
| `Localized` / `Resolved` / `Lang` | `crates/definitions/src/generated/localized.rs` | 4 语文本; `get(lang)` 返回 `Exact / Fallback / Missing`, 缺语言显式报告回退 |

## 3. 公开入口

| 入口 | 文件 | 说明 |
|---|---|---|
| `pub struct Xxx;` | 各标签文件 | 具体标签类型 (ZST) , 导入到最终层级使用 |
| `data() -> &'static TagText` | 各标签文件 | 返回 `&TAGS.<field>`, 不写文案字面量 |
| `Localized::{EMPTY, from_parts, from_all, slot, has, get}` | `generated/localized.rs` | 构造 / 取语言槽 / 带三态回退的取值 |
| `Resolved::{text, is_exact, is_fallback, is_missing, fallback_from}` | 同上 | 缺语言的三态访问器 |
| `generated::TAGS.<field>` | 生成代码 | 标签文本唯一数据源 |

### 源码 

> [!NOTE]
> 标签类型与唯一数据: `TagText` 由 `build.rs` 在编译期生成 (下面同时贴出生成代码与一个真实标签文件) .

**`crates/definitions/src/tags/action_card/support/location.rs`**

```rust
//! 「场地」标签（行动牌 · 支援）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::action_card::support::location::Location;`
//!
//! 支援牌中的场地牌。

/// 「场地」：支援牌中的场地牌。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Location;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.location
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
```

**`crates/definitions/build.rs(TagText / TAGS 生成节选)`**

```rust
    // D67：标签数据**唯一一份**，一个标签一个字段；运行期身份 = 字段地址（&TAGS.field）。
    out.push_str("/// 单个标签的静态文本（D67：字段访问，运行期身份 = &TAGS.<字段>）。\n");
    out.push_str("#[derive(Clone, Copy, Debug)]\npub struct TagText {\n    /// 4 语名字。\npub name: Localized,\n    /// 4 语描述（本轮无数据，全部为空）。\npub description: Localized,\n}\n\n");
    out.push_str("/// 全部标签的静态文本（唯一一份数据；字段访问 = 基址 + 固定偏移，没有查表）。\n");
    out.push_str("#[derive(Clone, Copy, Debug)]\npub struct Tags {\n");
    for (field, _) in &tags {
        writeln!(out, "    /// 见 crate::tags 下对应标签。\npub {field}: TagText,").expect("写字符串不会失败");
    }
    out.push_str("}\n\n");
    out.push_str("/// 标签数据的唯一一份（D67：一个标签一个字段）。\npub static TAGS: Tags = Tags {\n");
    for (field, texts) in &tags {
        writeln!(
            out,
            "    {field}: TagText {{ name: {texts}, description: Localized::EMPTY }},"
        )
        .expect("写字符串不会失败");
    }
    out.push_str("};\n\n");

```

**`crates/definitions/src/generated/localized.rs`**

```rust

/// 语言（4 语）。D56/D68：语言只能用它表示，不许写成字符串。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Lang {
    /// 简体中文（zh_cn）。
    ZhCn,
    /// English（en_us）。
    EnUs,
    /// 日本語（ja_jp）。
    JaJp,
    /// 한국어（ko_kr）。
    KoKr,
}
```

**`crates/definitions/src/generated/localized.rs`**

*注释中的关键字代码已替换为名称, 代码行未改*

```rust
/// 底层是若干条 UTF-8 静态字符串；**保留富文本标记的原文**（<color>、关键字占位符 等），
/// 渲染在 generated::text 里做。字段直接按语言命名，取值走 get()（会报告回退）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Localized {
    /// 简体中文。
    pub zh_cn: Option<&'static str>,
    /// English。
    pub en_us: Option<&'static str>,
    /// 日本語。
    pub ja_jp: Option<&'static str>,
    /// 한국어。
    pub ko_kr: Option<&'static str>,
}
impl Localized {
    /// 一条文本都没有。
    pub const EMPTY: Localized = Localized {
        zh_cn: None,
        en_us: None,
        ja_jp: None,
        ko_kr: None,
    };

    /// 由 4 个语言槽位构造（生成代码 / 常量用）。
    pub const fn from_parts(
        zh_cn: Option<&'static str>,
        en_us: Option<&'static str>,
        ja_jp: Option<&'static str>,
        ko_kr: Option<&'static str>,
    ) -> Localized {
        Localized {
            zh_cn,
            en_us,
            ja_jp,
            ko_kr,
        }
    }

    /// 4 语齐全时构造。
    pub const fn from_all(
        zh_cn: &'static str,
        en_us: &'static str,
        ja_jp: &'static str,
        ko_kr: &'static str,
    ) -> Localized {
        Localized {
            zh_cn: Some(zh_cn),
            en_us: Some(en_us),
            ja_jp: Some(ja_jp),
            ko_kr: Some(ko_kr),
        }
    }

    /// 取该语言槽位的值（match，不用下标）。
    pub const fn slot(self, lang: Lang) -> Option<&'static str> {
        match lang {
            Lang::ZhCn => self.zh_cn,
            Lang::EnUs => self.en_us,
            Lang::JaJp => self.ja_jp,
            Lang::KoKr => self.ko_kr,
        }
    }

    /// 该语言是否有文本。
    pub const fn has(self, lang: Lang) -> bool {
        self.slot(lang).is_some()
    }

    /// 取该语言的**渲染前原文**；缺语言时**显式**报告回退。
    ///
    /// 回退顺序固定 ZhCn → EnUs → JaJp → KoKr 中第一个有值的语言；一个都没有时 Missing。
    pub const fn get(self, lang: Lang) -> Resolved {
        if let Some(text) = self.slot(lang) {
            return Resolved::Exact(text);
        }
        if let Some(text) = self.zh_cn {
            return Resolved::Fallback {
                text,
                from: Lang::ZhCn,
            };
        }
        if let Some(text) = self.en_us {
            return Resolved::Fallback {
                text,
                from: Lang::EnUs,
            };
        }
        if let Some(text) = self.ja_jp {
            return Resolved::Fallback {
                text,
                from: Lang::JaJp,
            };
        }
        if let Some(text) = self.ko_kr {
            return Resolved::Fallback {
                text,
                from: Lang::KoKr,
            };
        }
        Resolved::Missing
    }
}
```

**`crates/definitions/src/generated/localized.rs`**

```rust

/// 取用结果：**缺语言显式报告回退**（D56）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolved {
    /// 正好是请求的语言。
    Exact(&'static str),
    /// 请求的语言缺失，回退到 from 那个语言（结果不是请求语言，调用方必须能看见）。
    Fallback {
        /// 回退后拿到的原文。
        text: &'static str,
        /// 实际来自哪个语言。
        from: Lang,
    },
    /// 一个语言都没有。
    Missing,
}
impl Resolved {
    /// 取文本（Missing 时为 None）；**不吞掉**「这是回退」这件事。
    pub const fn text(self) -> Option<&'static str> {
        match self {
            Resolved::Exact(text) | Resolved::Fallback { text, .. } => Some(text),
            Resolved::Missing => None,
        }
    }

    /// 是否正好命中请求语言。
    pub const fn is_exact(self) -> bool {
        matches!(self, Resolved::Exact(_))
    }

    /// 是否是回退（请求语言缺失，用了别语言）。
    pub const fn is_fallback(self) -> bool {
        matches!(self, Resolved::Fallback { .. })
    }

    /// 是否一个语言都没有。
    pub const fn is_missing(self) -> bool {
        matches!(self, Resolved::Missing)
    }

    /// 回退来源（Exact / Missing 时为 None）。
    pub const fn fallback_from(self) -> Option<Lang> {
        match self {
            Resolved::Fallback { from, .. } => Some(from),
            _ => None,
        }
    }
}
```
## 4. 不变量与禁止

> [!IMPORTANT]
> 一个标签一个文件, 一个零大小类型; 没有'总标签枚举', 也没有数字 / 字符串编号 (D54) .
> 标签文本只有一份 (`TAGS`) : 标签文件里不写文案字面量; 运行期身份 = 字段地址 (D67) .

- 只能导入最终层级的具体标签, 例如 `use crate::tags::action_card::support::location::Location;`.
- 跨牌族共用标签只导入定义处那一份 (D55) , 不许复制第二份类型 / 数据.
- 缺语言必须走 `Resolved` 显式回退, 不要自己用 `slot()` 静默拿别语言顶上.
- 标签集合形态 (元组 / Vec / 位图) 本轮未定, 不要擅自引入全局位图.

## 5. 依据 (规则书与 D 编号) 

> [!NOTE]
> 数据来源: `generated/data/terms.json` -> `build.rs` 生成; 运行期不读文件.
> 标签全集来自游戏内文本描述, 当前 52 个; 规则表实现其中 51 个 (排除自走牌, 见 [[定义层-Rules]]) .

| 编号 | 结论 |
|---|---|
| D53 / D54 | 标签是零大小类型, 一标签一文件, 不是枚举 / 数字 / 字符串 |
| D55 | 每个标签唯一定义, 全建 (52 个) ; 跨牌族共用只导入定义处 |
| D67 | 运行期身份 = `&TAGS.<field>` 的地址, 字段访问不查表 |
| D56 / D68 | 语言用枚举, 缺语言显式回退 (`Resolved`) , 字段命名固定 |
| D62 | 富文本保留标记, 渲染在 `generated::text` |

## 6. 常见误用

> [!WARNING]
> 以下写法会让标签数据出现第二份或让缺语言变成静默错误; 相关测试会直接失败.

1. 新加标签只在 `tags/` 建类型, 忘了在 `terms.json` 加条目: `TAGS` 没有字段, `data()` 编译失败.
2. 在标签文件里写名字字符串: 违反'数据唯一一份'.
3. 用 `tag as usize` / `TypeId` 当身份: 身份是数据地址.
4. 请求语言缺失时直接取 `slot()` 当有效文本: 必须看 `Resolved`.
5. 把 `tags/status/` 当已有标签全集: 数据里状态没有标签, 本轮为空.

## 7. 测试

| 文件 | 测试 |
|---|---|
| `tags/mod.rs` | `every_tag_is_a_distinct_zero_sized_type` (52 个, 类型名互异)  |
| `generated/mod.rs` | `tag_data_is_one_field_per_tag_and_pointers_are_distinct`, `tag_files_have_no_string_literals_and_all_expose_data`, `tags_struct_fields_match_tag_files_one_to_one`, `no_index_access_to_tag_data_in_sources` |
| `generated/localized.rs` | `lang_all_order_is_fixed`, `resolved_exact_when_language_present`, `resolved_fallback_reports_source_language`, `resolved_missing_is_not_silent`, `only_missing_slots_are_none` |

## 8. 待接入与待实测

- `留空.md#11`: 卡面 / 标签文案只填中文 (其他语言为 `None`, 取用时走 `Fallback`) ; 是否补齐 4 语未定.
- `留空.md#10`: 手牌状态的描述在关键字数据里缺失 (不编) .
- 标签集合形态未定 (元组 / Vec / 位图) , 设计待定, 无对应固定编号.

---

最后核对: 2026-10-19 (代码: crates/definitions/src/tags/action_card/support/location.rs:Location) 