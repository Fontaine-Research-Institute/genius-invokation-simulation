//! 本地化文本（D56 / D68）：**语言用枚举**、底层 UTF-8、缺语言**显式报告回退**。
//!
//! - 语言**不许用字符串表示**：只用 Lang（变体按 Rust 驼峰：ZhCn / EnUs / JaJp / KoKr）。
//! - 名字与描述共用同一个文本类型 Localized；字段名固定为
//!   zh_cn / en_us / ja_jp / ko_kr（Rust 蛇形，D68）。
//! - 取值一律 match，**不用数组 + 下标**；返回值 Resolved 三态：
//!   Exact / Fallback { text, from } / Missing —— **绝不静默拿别的语言顶上**。
//! - 富文本的渲染不在这里：原文保留标记，渲染由 generated::text 的解析/渲染两段做（D62）。

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

impl Lang {
    /// 全部语言，顺序固定（也是回退顺序：ZhCn → EnUs → JaJp → KoKr）。
    pub const ALL: [Lang; 4] = [Lang::ZhCn, Lang::EnUs, Lang::JaJp, Lang::KoKr];
}

/// 统一的本地化文本类型（名字 / 描述共用，D56/D68）。
///
/// 底层是若干条 UTF-8 静态字符串；**保留富文本标记的原文**（<color>、$[K53] 等），
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

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: Localized = Localized::from_all("中", "EN", "日", "한");
    /// 只有中文 + 韩文：英文/日文缺失（用来验证回退来源）。
    const PARTIAL: Localized =
        Localized::from_parts(Some("中"), None, None, Some("한"));

    #[test]
    fn lang_all_order_is_fixed() {
        assert_eq!(Lang::ALL, [Lang::ZhCn, Lang::EnUs, Lang::JaJp, Lang::KoKr]);
    }

    #[test]
    fn resolved_exact_when_language_present() {
        for lang in Lang::ALL {
            let resolved = FULL.get(lang);
            assert!(resolved.is_exact(), "4 语齐全时必须是 Exact：{lang:?}");
            assert_eq!(resolved.fallback_from(), None);
            assert!(resolved.text().is_some());
        }
        assert_eq!(FULL.get(Lang::JaJp), Resolved::Exact("日"));
    }

    #[test]
    fn resolved_fallback_reports_source_language() {
        // 请求英文：缺失 -> 回退到顺序上第一个有值的语言（中文），并且**报告** from = ZhCn。
        assert_eq!(
            PARTIAL.get(Lang::EnUs),
            Resolved::Fallback {
                text: "中",
                from: Lang::ZhCn
            }
        );
        assert_eq!(
            PARTIAL.get(Lang::JaJp),
            Resolved::Fallback {
                text: "中",
                from: Lang::ZhCn
            }
        );
        assert!(PARTIAL.get(Lang::EnUs).is_fallback());
        assert_eq!(PARTIAL.get(Lang::EnUs).fallback_from(), Some(Lang::ZhCn));
        // 自己有值的语言不触发回退
        assert_eq!(PARTIAL.get(Lang::KoKr), Resolved::Exact("한"));
        // 只有韩文时，回退来源必须是 KoKr（顺序兜住 4 个槽位）
        let korean_only: Localized =
            Localized::from_parts(None, None, None, Some("한"));
        assert_eq!(
            korean_only.get(Lang::ZhCn),
            Resolved::Fallback {
                text: "한",
                from: Lang::KoKr
            }
        );
    }

    #[test]
    fn resolved_missing_is_not_silent() {
        assert_eq!(Localized::EMPTY.get(Lang::ZhCn), Resolved::Missing);
        assert!(!Localized::EMPTY.has(Lang::KoKr));
        assert_eq!(Localized::EMPTY.get(Lang::KoKr).text(), None);
        assert!(Localized::EMPTY.get(Lang::EnUs).is_missing());
        assert!(!Localized::EMPTY.get(Lang::EnUs).is_fallback());
    }

    #[test]
    fn only_missing_slots_are_none() {
        // None 表示「缺失」，不是空串：空串也必须被当成有值。
        let empty_string: Localized = Localized::from_parts(Some(""), None, None, None);
        assert!(empty_string.has(Lang::ZhCn));
        assert_eq!(empty_string.get(Lang::ZhCn), Resolved::Exact(""));
    }
}
