//! `gi-genius-invokation-richtext`：富文本**解析** crate（D62 / D63 / D65 / D75 / D76）。
//!
//! 职责只有一个：**带标记的原始文本 → 中性的片段序列**。
//!
//! - 只认语法结构：颜色标签、`$[…]` 占位符、`${[…]}` token、`{SPRITE_PRESET#…}` 图标、普通文本。
//! - **严格（D63）**：任何未识别的内容都返回 [`ParseError`]，**没有兜底分支**，不当普通文本吞掉。
//! - **零游戏语义（D76）**：不认识语言（不引用 `Lang` / `Localized`）、不认识名字表、
//!   不认识卡牌 / 标签 / 关键字表。K / C / A / S 码在片段里只是数字；
//!   名字替换与语言选择由 definitions 侧的渲染器完成。
//! - **零依赖（D75）**：不依赖 `gi-genius-invokation-engine` / `gi-genius-invokation-definitions`，也不引任何第三方 crate。
//!   只做字符串处理；内部允许按位置切 `&str`（D75：这不属于「按编号取数据表」，见 D67）。
//!
//! 公开面：片段类型 [`Segment`]、解析函数 [`parse`]、错误类型 [`ParseError`]，
//! 外加两个结构枚举（[`DamageKey`] / [`AnyMatch`]）与三个**语法白名单**常量
//! （[`KEYWORD_VARIANTS`] / [`SPRITE_PRESETS`] / [`TOKEN_VALUES`]，来源与依据见各自注释）。
//!
//! ```
//! use gi_genius_invokation_richtext::{parse, Segment};
//!
//! let segments = parse("造成$[D__KEY__DAMAGE]点伤害").expect("应能解析");
//! assert_eq!(segments.len(), 3);
//! assert_eq!(segments.first(), Some(&Segment::Text(String::from("造成"))));
//! ```

#![forbid(unsafe_code)]

pub mod parse;

pub use parse::{
    parse, AnyMatch, DamageKey, ParseError, Segment, ALLOWED_ANY_MATCHES, KEYWORD_VARIANTS,
    SPRITE_PRESETS, TOKEN_VALUES,
};

#[cfg(test)]
mod tests;