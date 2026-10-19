//! 富文本的两段式处理（D62）。
//!
//! - 解析：由独立的 **gi-genius-invokation-richtext** crate 提供（D75/D76，零游戏语义）；
//!   本模块只 re-export 它，**不留第二份解析器**。
//! - render：吃片段序列 + 一个很小的取值来源，拼出最终字符串（留在 definitions，因为要名字表/语言）。
//!
//! 数据层保留带标记的原文；对外取文本一律经过这里，不把原文漏出去。

pub mod render;

pub use gi_genius_invokation_richtext::{
    parse, AnyMatch, DamageKey, ParseError, Segment, ALLOWED_ANY_MATCHES, KEYWORD_VARIANTS,
    SPRITE_PRESETS, TOKEN_VALUES,
};
pub use render::{
    render, render_segments, DamageElement, DefinitionsSources, MissingPolicy, TextSources,
};

#[cfg(test)]
mod tests;
