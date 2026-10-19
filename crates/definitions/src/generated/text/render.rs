//! 富文本**渲染**（D62）：吃解析好的片段序列 + 很小的取值来源，拼出最终字符串。
//!
//! - 输入：gi-genius-invokation-richtext 的 Segment（解析阶段已严格校验，见 D63 / D65）；本模块**不再解析原文**。
//! - 取值来源 [TextSources] 只放：关键字/卡牌/角色/技能名表、伤害数值、元素名、缺失策略。
//!   **不塞**卡牌定义、状态、局面之类东西。
//! - 颜色标签、图标按规则删除（D65 也明确：图标在渲染层删掉，和匹配无关）；
//!   `$[G…]` / `${[GCG_TOKEN_…]}` 这类运行期值原样保留。

use gi_genius_invokation_engine::Nat;

use crate::base::{CardId, Element, SkillId};
use crate::generated::localized::{Lang, Localized, Resolved};
use crate::generated::KeywordCode;

use gi_genius_invokation_richtext::{parse, DamageKey, ParseError, Segment};

/// 渲染用的伤害元素：**物理 + 7 元素**。
///
/// 说明：base::Element 只有 7 种元素、没有物理，而 ${D__KEY__ELEMENT${ 的
/// baseelement 实际会出现 GCG_ELEMENT_PHYSIC，所以渲染上下文需要一个能表达物理的枚举。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DamageElement {
    /// 物理。
    Physical,
    /// 火。
    Pyro,
    /// 水。
    Hydro,
    /// 雷。
    Electro,
    /// 草。
    Dendro,
    /// 冰。
    Cryo,
    /// 风。
    Anemo,
    /// 岩。
    Geo,
}

impl DamageElement {
    /// 全部 8 种（顺序固定，只用于遍历）。
    pub const ALL: [DamageElement; 8] = [
        DamageElement::Physical,
        DamageElement::Pyro,
        DamageElement::Hydro,
        DamageElement::Electro,
        DamageElement::Dendro,
        DamageElement::Cryo,
        DamageElement::Anemo,
        DamageElement::Geo,
    ];

    /// 该伤害元素的 4 语名字（生成表里来的静态数据）。
    pub fn name(self) -> &'static Localized {
        match self {
            DamageElement::Physical => &crate::generated::DAMAGE_PHYSIC,
            DamageElement::Pyro => &crate::generated::DAMAGE_PYRO,
            DamageElement::Hydro => &crate::generated::DAMAGE_HYDRO,
            DamageElement::Electro => &crate::generated::DAMAGE_ELECTRO,
            DamageElement::Dendro => &crate::generated::DAMAGE_DENDRO,
            DamageElement::Cryo => &crate::generated::DAMAGE_CRYO,
            DamageElement::Anemo => &crate::generated::DAMAGE_ANEMO,
            DamageElement::Geo => &crate::generated::DAMAGE_GEO,
        }
    }
}

impl From<Element> for DamageElement {
    fn from(element: Element) -> Self {
        match element {
            Element::Pyro => DamageElement::Pyro,
            Element::Hydro => DamageElement::Hydro,
            Element::Electro => DamageElement::Electro,
            Element::Dendro => DamageElement::Dendro,
            Element::Cryo => DamageElement::Cryo,
            Element::Anemo => DamageElement::Anemo,
            Element::Geo => DamageElement::Geo,
        }
    }
}

/// 取值来源里「找不到」时怎么办。
///
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

/// 最小取值来源：关键字名与元素名走生成表，其余（卡牌/角色/技能名、伤害值）暂缺。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DefinitionsSources;

impl TextSources for DefinitionsSources {
    fn keyword_name(&self, code: KeywordCode) -> Option<&'static Localized> {
        crate::generated::keyword(code).map(|keyword| &keyword.name)
    }

    fn card_name(&self, _id: CardId) -> Option<&'static Localized> {
        None
    }

    fn character_name(&self, _id: CardId) -> Option<&'static Localized> {
        None
    }

    fn skill_name(&self, _id: SkillId) -> Option<&'static Localized> {
        None
    }
}

/// 渲染片段序列。
pub fn render_segments(segments: &[Segment], lang: Lang, sources: &dyn TextSources) -> String {
    let mut out = String::new();
    for segment in segments {
        render_segment(segment, lang, sources, &mut out);
    }
    out
}

/// 渲染整段原文：**先严格解析**（解析失败返回错误），再渲染。
pub fn render(raw: &str, lang: Lang, sources: &dyn TextSources) -> Result<String, ParseError> {
    let segments = parse(raw)?;
    Ok(render_segments(&segments, lang, sources))
}

fn render_segment(segment: &Segment, lang: Lang, sources: &dyn TextSources, out: &mut String) {
    match segment {
        Segment::Text(text) => out.push_str(text),
        // 规则 1：颜色标签删掉（颜色值是唯一 any，本来就不影响结果）。
        Segment::ColorStart(_) | Segment::ColorEnd => {}
        // 规则 5 / D65：图标删掉。
        Segment::Icon(_) => {}
        Segment::Keyword { code, .. } => {
            let resolved = sources
                .keyword_name(KeywordCode::new(Nat::new(*code)))
                .and_then(|name| resolve(name, lang));
            match resolved {
                Some(text) => out.push_str(text),
                None => render_missing(segment, sources, out),
            }
        }
        Segment::Card(id) => {
            let resolved = sources
                .card_name(CardId::new(*id))
                .and_then(|name| resolve(name, lang));
            match resolved {
                Some(text) => out.push_str(text),
                None => render_missing(segment, sources, out),
            }
        }
        Segment::Character(id) => {
            let resolved = sources
                .character_name(CardId::new(*id))
                .and_then(|name| resolve(name, lang));
            match resolved {
                Some(text) => out.push_str(text),
                None => render_missing(segment, sources, out),
            }
        }
        Segment::Skill(id) => {
            let resolved = sources
                .skill_name(SkillId::new(*id))
                .and_then(|name| resolve(name, lang));
            match resolved {
                Some(text) => out.push_str(text),
                None => render_missing(segment, sources, out),
            }
        }
        Segment::Damage(DamageKey::Element) => {
            let resolved = sources
                .base_element()
                .and_then(|element| sources.element_name(element))
                .and_then(|name| resolve(name, lang));
            match resolved {
                Some(text) => out.push_str(text),
                None => render_missing(segment, sources, out),
            }
        }
        Segment::Damage(key) => match sources.damage_value(*key) {
            Some(value) => out.push_str(&value.to_string()),
            None => render_missing(segment, sources, out),
        },
        // 运行期数值：没有静态值可填，原样保留。
        Segment::Variable(raw) | Segment::TokenValue(raw) => out.push_str(raw),
    }
}

/// Localized::get 的结果取文本（回退也接受；Missing 才是没有）。
fn resolve(localized: &'static Localized, lang: Lang) -> Option<&'static str> {
    match localized.get(lang) {
        Resolved::Exact(text) => Some(text),
        Resolved::Fallback { text, .. } => Some(text),
        Resolved::Missing => None,
    }
}

fn render_missing(segment: &Segment, sources: &dyn TextSources, out: &mut String) {
    if sources.missing_policy() == MissingPolicy::KeepPlaceholder {
        if let Some(marker) = segment.raw_marker() {
            out.push_str(&marker);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::localized::Localized;
    use crate::generated::KeywordCode;

    const ZH: Localized = Localized::from_all("准备技能", "Prepare Skill", "スキル準備", "스킬 준비");
    const CARD: Localized = Localized::from_all("幽焰显迹", "Flame", "幽炎", "유염");
    /// 只有中文：验证渲染时缺语言会**显式回退**到中文，而不是空。
    const CHARACTER: Localized = Localized::from_parts(Some("菲林斯"), None, None, None);
    const SKILL: Localized = Localized::from_all("普通攻击", "Normal Attack", "通常攻撃", "일반 공격");
    const HYDRO: Localized = Localized::from_all("水元素伤害", "Hydro DMG", "水元素ダメージ", "물 원소 피해");

    struct Fake {
        damage: Option<Nat>,
        numbered: Option<Nat>,
        element: Option<DamageElement>,
        missing: MissingPolicy,
    }

    impl Fake {
        fn full() -> Fake {
            Fake {
                damage: Some(Nat::new(2)),
                numbered: Some(Nat::new(4)),
                element: Some(DamageElement::Hydro),
                missing: MissingPolicy::KeepPlaceholder,
            }
        }
    }

    impl TextSources for Fake {
        fn keyword_name(&self, code: KeywordCode) -> Option<&'static Localized> {
            if code == KeywordCode::new(Nat::new(53)) {
                Some(&ZH)
            } else {
                None
            }
        }

        fn card_name(&self, id: CardId) -> Option<&'static Localized> {
            if id == CardId::new(114181) {
                Some(&CARD)
            } else {
                None
            }
        }

        fn character_name(&self, id: CardId) -> Option<&'static Localized> {
            if id == CardId::new(1418) {
                Some(&CHARACTER)
            } else {
                None
            }
        }

        fn skill_name(&self, id: SkillId) -> Option<&'static Localized> {
            if id == SkillId::new(14181) {
                Some(&SKILL)
            } else {
                None
            }
        }

        fn base_element(&self) -> Option<DamageElement> {
            self.element
        }

        fn element_name(&self, element: DamageElement) -> Option<&'static Localized> {
            if element == DamageElement::Hydro {
                Some(&HYDRO)
            } else {
                None
            }
        }

        fn damage_value(&self, key: DamageKey) -> Option<Nat> {
            match key {
                DamageKey::Damage => self.damage,
                DamageKey::Numbered(3) => self.numbered,
                _ => None,
            }
        }

        fn missing_policy(&self) -> MissingPolicy {
            self.missing
        }
    }

    fn render_ok(raw: &str, lang: Lang, sources: &Fake) -> String {
        render(raw, lang, sources).unwrap_or_else(|error| panic!("解析应当通过：{raw:?} / {error}"))
    }

    #[test]
    fn strips_color_and_sprite_markers() {
        let text = render_ok(
            "<color=#FFFFFFFF>造成2点</color>{SPRITE_PRESET#2102}$[D__KEY__ELEMENT]。",
            Lang::ZhCn,
            &Fake::full(),
        );
        assert_eq!(text, "造成2点水元素伤害。");
    }

    #[test]
    fn replaces_names_by_language() {
        let raw = "$[C114181]$[A1418]使用$[S14181]。";
        assert_eq!(render_ok(raw, Lang::ZhCn, &Fake::full()), "幽焰显迹菲林斯使用普通攻击。");
        assert_eq!(render_ok(raw, Lang::EnUs, &Fake::full()), "Flame菲林斯使用Normal Attack。");
        assert_eq!(render_ok(raw, Lang::JaJp, &Fake::full()), "幽炎菲林斯使用通常攻撃。");
        assert_eq!(render_ok(raw, Lang::KoKr, &Fake::full()), "유염菲林斯使用일반 공격。");
    }

    #[test]
    fn replaces_keyword_and_ignores_variant() {
        assert_eq!(render_ok("然后$[K53]：", Lang::ZhCn, &Fake::full()), "然后准备技能：");
        // Q5：变体后缀忽略，按同一个 K 码取名字。
        assert_eq!(render_ok("然后$[K53|s1]：", Lang::EnUs, &Fake::full()), "然后Prepare Skill：");
    }

    #[test]
    fn replaces_damage_and_element() {
        let raw = "造成$[D__KEY__DAMAGE]点$[D__KEY__ELEMENT]，再造成$[D__KEY__DAMAGE_3]点。";
        assert_eq!(
            render_ok(raw, Lang::ZhCn, &Fake::full()),
            "造成2点水元素伤害，再造成4点。"
        );
        assert_eq!(
            render_ok(raw, Lang::EnUs, &Fake::full()),
            "造成2点Hydro DMG，再造成4点。"
        );
    }

    #[test]
    fn missing_names_keep_placeholder_or_empty() {
        let raw = "$[C999]$[K999]";
        assert_eq!(render_ok(raw, Lang::ZhCn, &Fake::full()), "$[C999]$[K999]");
        let mut fake = Fake::full();
        fake.missing = MissingPolicy::Empty;
        assert_eq!(render_ok(raw, Lang::ZhCn, &fake), "");
    }

    #[test]
    fn runtime_placeholders_are_kept() {
        let raw = "$[G16121]${[GCG_TOKEN_SHIELD]}";
        assert_eq!(
            render_ok(raw, Lang::ZhCn, &Fake::full()),
            "$[G16121]${[GCG_TOKEN_SHIELD]}"
        );
    }

    #[test]
    fn missing_damage_keeps_marker() {
        let fake = Fake {
            damage: None,
            numbered: None,
            element: Some(DamageElement::Pyro),
            missing: MissingPolicy::KeepPlaceholder,
        };
        assert_eq!(
            render_ok("$[D__KEY__DAMAGE]$[D__KEY__DAMAGE_3]", Lang::ZhCn, &fake),
            "$[D__KEY__DAMAGE]$[D__KEY__DAMAGE_3]"
        );
    }

    #[test]
    fn missing_element_keeps_marker() {
        let fake = Fake {
            damage: None,
            numbered: None,
            element: None,
            missing: MissingPolicy::KeepPlaceholder,
        };
        assert_eq!(render_ok("$[D__KEY__ELEMENT]", Lang::ZhCn, &fake), "$[D__KEY__ELEMENT]");
    }

    #[test]
    fn definitions_sources_uses_generated_keyword_table() {
        // 生成表里 K1 的中文名是「战斗行动」；其余名字来源缺失 -> 保留占位符。
        let text = render("$[K1]$[C1]", Lang::ZhCn, &DefinitionsSources)
            .unwrap_or_else(|error| panic!("解析应当通过：{error}"));
        assert_eq!(text, "战斗行动$[C1]");
    }
}
