//! 编译期注入的数据 + 描述 + 渲染（结构.md §4.2 / 分工计划.md §4.4.16）。
//!
//! - **数据源**：generated/data/*.json（入库）；build.rs 在编译期读它，生成静态表到
//!   OUT_DIR，下面 include! 进来。运行期不读文件。
//! - **本地化文本**：localized 模块（Lang 枚举 / Localized / Resolved，D56）。
//! - **富文本两段式**（D62/D75）：解析由独立的 gi-genius-invokation-richtext crate 做（只认结构、严格，D63/D65）；
//!   text::render 吃片段 + 很小的取值来源，拼出最终字符串。
//! - **标签数据唯一一份**（D67）：build.rs 生成的 TAGS（一个标签一个字段，字段访问，不查表）。

pub mod localized;
pub mod text;

pub use localized::{Lang, Localized, Resolved};
pub use text::{
    parse, AnyMatch, DamageKey, ParseError, Segment, ALLOWED_ANY_MATCHES, KEYWORD_VARIANTS,
    SPRITE_PRESETS, TOKEN_VALUES,
};
pub use text::render::{
    render, render_segments, DamageElement, DefinitionsSources, MissingPolicy, TextSources,
};

use gi_genius_invokation_engine::Nat;

/// 关键字 K 码（数值；如 K53 = 准备技能、K3001 = 月感电）。
///
/// 载体是 Nat（禁止裸整型，§8.3）；数值只用于查生成表，不参与算术。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct KeywordCode(Nat);

impl KeywordCode {
    /// const 构造（生成表 / 代码里的常量用）。
    pub const fn new(value: Nat) -> KeywordCode {
        KeywordCode(value)
    }

    /// 取数值。
    pub const fn get(self) -> Nat {
        self.0
    }
}

/// 关键字：K 码 + 4 语名字 + 4 语说明。
///
/// 名字与说明都来自**权威七圣召唤 keyword 数据**（142 码 × 4 语；129 码有 4 语说明，
/// 13 码无说明——含手牌状态 3003~3008，数据没有就保持空，不回退、不编造）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeywordDef {
    /// 官方 K 码。
    pub code: KeywordCode,
    /// 4 语名字（渲染 $[K…] 用）。
    pub name: Localized,
    /// 4 语说明（缺语言的槽位为 None，取用时会显式报告回退/缺失）。
    pub description: Localized,
}

/// 类型 / 标签的文本条目（4 语名字 + 4 语描述）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextEntry {
    /// 4 语名字。
    pub name: Localized,
    /// 4 语描述。
    pub description: Localized,
}

/// 测试用全量文本条目（descriptions.json；**不进运行时 API**）。
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub struct TextCorpusEntry {
    /// 牌族（ActionCard / Character / Enemy / Summon / Status）。
    pub family: &'static str,
    /// 实体 id（牌族内编号，载体 Nat）。
    pub id: Nat,
    /// 技能 id（实体文本为 Nat::ZERO）。
    pub skill_id: Nat,
    /// 字段：name / description。
    pub field: &'static str,
    /// 4 语原文（保留富文本标记；按语言槽位取，不存数组）。
    pub texts: Localized,
}

// build.rs 生成：TAG_ENTRY_COUNT / KEYWORD_COUNT / 类型与伤害名静态表 / KEYWORDS / TAGS…
include!(concat!(env!("OUT_DIR"), "/generated_data.rs"));

/// 关键字查询：按 K 码二分查找（生成表按 code 升序）。
///
/// **数据来源**：generated/data/keywords.json（权威七圣召唤 keyword 数据，142 码 × 4 语名字 +
/// 说明；129 码有 4 语说明，13 码无说明——含手牌状态 3003~3008，数据没有就保持空）。
/// 名字/说明都**不是反推**的。将来换数据时**只覆盖 keywords.json 再 rebuild**，本函数与生成表结构不变。
pub fn keyword(code: KeywordCode) -> Option<&'static KeywordDef> {
    KEYWORDS
        .binary_search_by_key(&code.get(), |definition| definition.code.get())
        .ok()
        .map(|index| &KEYWORDS[index])
}

/// 全部关键字（按 K 码升序）。
pub fn keywords_all() -> &'static [KeywordDef] {
    &KEYWORDS
}

/// 行动牌类型文本（4 语名字；描述本轮为空）。
pub fn action_card_type_text(card_type: ActionCardType) -> Option<&'static TextEntry> {
    Some(match card_type {
        ActionCardType::Support => &TYPE_SUPPORT,
        ActionCardType::Equipment => &TYPE_EQUIPMENT,
        ActionCardType::Event => &TYPE_EVENT,
    })
}

/// 状态类型文本（4 语名字；描述本轮为空）。
pub fn status_type_text(status_type: StatusType) -> Option<&'static TextEntry> {
    Some(match status_type {
        StatusType::CharacterStatus => &TYPE_CHARACTER_STATUS,
        StatusType::CombatStatus => &TYPE_COMBAT_STATUS,
    })
}

use crate::families::action_card::ActionCardType;
use crate::families::status::StatusType;

#[cfg(test)]
pub(crate) fn all_tag_data() -> [&'static TagText; 52] {
    [
        crate::tags::action_card::common::automaton_card::data(),
        crate::tags::action_card::common::combat_action::data(),
        crate::tags::action_card::common::damage_reduction::data(),
        crate::tags::action_card::common::scrapped::data(),
        crate::tags::action_card::common::shield::data(),
        crate::tags::action_card::common::undiscoverable::data(),
        crate::tags::action_card::equipment::artifact::data(),
        crate::tags::action_card::equipment::bow::data(),
        crate::tags::action_card::equipment::catalyst::data(),
        crate::tags::action_card::equipment::claymore::data(),
        crate::tags::action_card::equipment::hexerei::data(),
        crate::tags::action_card::equipment::polearm::data(),
        crate::tags::action_card::equipment::sword::data(),
        crate::tags::action_card::equipment::talent::data(),
        crate::tags::action_card::equipment::technique::data(),
        crate::tags::action_card::equipment::weapon::data(),
        crate::tags::action_card::event::arcane_legend::data(),
        crate::tags::action_card::event::food::data(),
        crate::tags::action_card::event::resonance::data(),
        crate::tags::action_card::event::simulanka::data(),
        crate::tags::action_card::support::adventure_spot::data(),
        crate::tags::action_card::support::companion::data(),
        crate::tags::action_card::support::elemental_transmutation::data(),
        crate::tags::action_card::support::item::data(),
        crate::tags::action_card::support::location::data(),
        crate::tags::action_card::support::untransformed::data(),
        crate::tags::character::arkhe::ousia::data(),
        crate::tags::character::arkhe::pneuma::data(),
        crate::tags::character::element::anemo::data(),
        crate::tags::character::element::cryo::data(),
        crate::tags::character::element::dendro::data(),
        crate::tags::character::element::electro::data(),
        crate::tags::character::element::geo::data(),
        crate::tags::character::element::hydro::data(),
        crate::tags::character::element::pyro::data(),
        crate::tags::character::nation::consecrated_beast::data(),
        crate::tags::character::nation::cosmic_calamity::data(),
        crate::tags::character::nation::eremites::data(),
        crate::tags::character::nation::fatui::data(),
        crate::tags::character::nation::fontaine::data(),
        crate::tags::character::nation::hilichurl::data(),
        crate::tags::character::nation::inazuma::data(),
        crate::tags::character::nation::kairagi::data(),
        crate::tags::character::nation::liyue::data(),
        crate::tags::character::nation::mondstadt::data(),
        crate::tags::character::nation::monster::data(),
        crate::tags::character::nation::natlan::data(),
        crate::tags::character::nation::nodkrai::data(),
        crate::tags::character::nation::sumeru::data(),
        crate::tags::character::other_weapons::data(),
        crate::tags::enemy::no_elemental_type::data(),
        crate::tags::summon::dendro_construct::data(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_table_is_non_empty_sorted_and_complete() {
        let count = KEYWORDS.len();
        assert_eq!(count, 142, "权威 keyword 数据共 142 个 K 码");
        assert_eq!(count, KEYWORD_COUNT);
        assert!(KEYWORDS.windows(2).all(|pair| pair[0].code < pair[1].code), "关键字表必须按 K 码严格升序");

        let combat_action = keyword(KeywordCode::new(Nat::new(1))).expect("K1（战斗行动）必须有");
        assert_eq!(combat_action.name.get(Lang::ZhCn), Resolved::Exact("战斗行动"));
        assert_eq!(combat_action.name.get(Lang::EnUs), Resolved::Exact("Combat Action"));
        // 权威数据里 K1 有 4 语说明（不是反推）。
        assert!(
            combat_action
                .description
                .get(Lang::ZhCn)
                .text()
                .is_some_and(|text| text.starts_with("我方执行了一次战斗行动后")),
            "K1 的中文说明应来自权威 keyword 数据"
        );
        assert!(!combat_action.description.get(Lang::EnUs).is_missing());

        // 数据里没有说明的码保持空（不回退、不编造）：K3012 与手牌状态 K3003~K3008。
        let monthly = keyword(KeywordCode::new(Nat::new(3012))).expect("K3012（月笼）必须有");
        assert_eq!(monthly.name.get(Lang::ZhCn), Resolved::Exact("月笼"));
        assert!(monthly.description.get(Lang::ZhCn).is_missing(), "K3012 数据里没有说明，应保持空");
        // 新增的权威关键字（旧反推法拿不到的）也必须在表里。
        let stun = keyword(KeywordCode::new(Nat::new(55))).expect("K55（眩晕）必须有");
        assert_eq!(stun.name.get(Lang::ZhCn), Resolved::Exact("眩晕"));
        let burst = keyword(KeywordCode::new(Nat::new(501))).expect("K501（元素爆发）必须有");
        assert_eq!(burst.name.get(Lang::ZhCn), Resolved::Exact("元素爆发"));
        for code in [3003, 3004, 3005, 3006, 3007, 3008] {
            let keyword = keyword(KeywordCode::new(Nat::new(code))).expect("手牌状态关键字必须存在");
            assert!(
                keyword.description.get(Lang::ZhCn).is_missing(),
                "手牌状态 K{code} 数据里没有说明，应保持空"
            );
        }

        // 说明覆盖：129 个码在 4 个语言上都有说明（与权威数据一致）。
        for lang in Lang::ALL {
            let described = keywords_all()
                .iter()
                .filter(|keyword| !keyword.description.get(lang).is_missing())
                .count();
            assert_eq!(described, 129, "有说明的关键字数应为 129（{lang:?}）");
        }

        assert!(keyword(KeywordCode::new(Nat::new(999_999))).is_none(), "不存在的 K 码应返回 None");
        assert_eq!(keywords_all().len(), KEYWORD_COUNT);
    }

    #[test]
    fn type_texts_cover_all_four_languages() {
        for card_type in ActionCardType::ALL {
            let entry = action_card_type_text(card_type).expect("行动牌类型文本必须存在");
            for lang in Lang::ALL {
                assert!(!entry.name.get(lang).is_missing(), "行动牌类型名字缺语言：{card_type:?} {lang:?}");
            }
        }
        for status_type in StatusType::ALL {
            let entry = status_type_text(status_type).expect("状态类型文本必须存在");
            for lang in Lang::ALL {
                assert!(!entry.name.get(lang).is_missing(), "状态类型名字缺语言：{status_type:?} {lang:?}");
            }
        }
        assert_eq!(
            action_card_type_text(ActionCardType::Support)
                .expect("支援类型文本")
                .name
                .get(Lang::ZhCn),
            Resolved::Exact("支援牌")
        );
        assert_eq!(
            status_type_text(StatusType::CombatStatus)
                .expect("出战状态文本")
                .name
                .get(Lang::ZhCn),
            Resolved::Exact("出战状态")
        );
    }

    #[test]
    fn damage_element_names_are_complete() {
        for element in DamageElement::ALL {
            let name = element.name();
            for lang in Lang::ALL {
                assert!(!name.get(lang).is_missing(), "伤害元素名缺语言：{element:?} {lang:?}");
            }
        }
        assert_eq!(DamageElement::Physical.name().get(Lang::ZhCn), Resolved::Exact("物理伤害"));
        assert_eq!(DamageElement::Hydro.name().get(Lang::EnUs), Resolved::Exact("Hydro DMG"));
    }

    #[test]
    fn tag_data_is_one_field_per_tag_and_pointers_are_distinct() {
        let datas = all_tag_data();
        assert_eq!(datas.len(), TAG_ENTRY_COUNT, "标签数据条数必须等于 build.rs 统计的条目数");
        assert_eq!(datas.len(), 52, "52 个标签 = 52 份 TagText");

        // 每个标签都有 4 语名字（标签描述本轮为空）。
        for data in datas {
            for lang in Lang::ALL {
                assert!(!data.name.get(lang).is_missing(), "标签名字缺语言：{lang:?}");
                assert!(data.description.get(Lang::ZhCn).is_missing(), "标签描述本轮应为空");
            }
        }

        // D67：运行期身份 = 字段地址；52 个字段地址必须互不相同（两两比较，不转成整数下标）。
        for (index, data) in datas.iter().enumerate() {
            for other in datas.iter().skip(index.saturating_add(1)) {
                assert!(
                    !std::ptr::addr_eq(*data, *other),
                    "标签 data() 地址出现重复：字段不是一一对应"
                );
            }
        }
    }

    #[test]
    fn tag_files_have_no_string_literals_and_all_expose_data() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("tags");
        let mut files = Vec::new();
        collect_rs_files(&root, &mut files);
        files.sort();

        let mut violations = Vec::new();
        let mut with_data = 0;
        for file in &files {
            if file.file_name().is_some_and(|name| name == "mod.rs") {
                continue;
            }
            let text = std::fs::read_to_string(file)
                .unwrap_or_else(|error| panic!("读标签文件 {} 失败：{error}", file.display()));
            if text.contains("pub fn data()") {
                with_data += 1;
            } else {
                violations.push(format!("{} 没有 pub fn data()", file.display()));
            }
            for (index, line) in text.lines().enumerate() {
                // 只检查代码部分（// 之后是注释，允许写说明文字）。
                let code = line.split("//").next().unwrap_or("");
                if code.contains('"') {
                    violations.push(format!("{}:{} 出现字符串字面量：{line}", file.display(), index.saturating_add(1)));
                }
            }
        }
        assert_eq!(with_data, 52, "52 个标签文件都必须有 data()");
        assert!(violations.is_empty(), "标签文件违反 D67：\n{}", violations.join("\n"));
    }

    #[test]
    fn tags_struct_fields_match_tag_files_one_to_one() {
        // D67：TAGS 的字段与 52 个标签文件必须一一对应（少一个 / 多一个都红）。
        // 直接检查 build.rs 生成的源码文本（include_str!），不额外建索引表。
        let generated = include_str!(concat!(env!("OUT_DIR"), "/generated_data.rs"));
        let (struct_fields, static_fields) = tags_struct_fields(generated);

        assert_eq!(struct_fields.len(), TAG_ENTRY_COUNT, "Tags 结构体字段数必须等于标签条目数");
        assert_eq!(static_fields.len(), TAG_ENTRY_COUNT, "TAGS 静态初始化的字段数必须相等");
        assert_eq!(struct_fields, static_fields, "Tags 定义与 TAGS 初始化字段必须完全一致（顺序/名字）");

        let (mut refs, files) = tag_file_field_refs();
        assert_eq!(files.len(), 52, "标签文件数必须是 52");
        assert_eq!(refs.len(), 52, "52 个标签文件必须各引用一个 TAGS 字段");
        refs.sort();
        let unique = refs.len();
        refs.dedup();
        assert_eq!(refs.len(), unique, "有多个标签文件引用同一个 TAGS 字段");

        let mut fields = struct_fields;
        fields.sort();
        assert_eq!(
            refs, fields,
            "标签文件引用的字段集合必须与 TAGS 字段集合完全相同（少一个 / 多一个 / 重名都红）"
        );
    }

    #[test]
    fn no_index_access_to_tag_data_in_sources() {
        // D67：标签数据只允许字段访问（TAGS.x / Localized 命名槽位），不许下标（x[...]）。
        let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&source_root.join("tags"), &mut files);
        collect_rs_files(&source_root.join("generated"), &mut files);

        let banned = ["TAGS", "Localized", "TagText"];
        let mut violations = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file)
                .unwrap_or_else(|error| panic!("读源码 {} 失败：{error}", file.display()));
            for name in banned {
                for (index, _) in text.match_indices(name) {
                    let rest = &text[index.saturating_add(name.len())..];
                    if rest.trim_start().starts_with('[') {
                        violations.push(format!("{}: {} 后面出现 [ 下标取值", file.display(), name));
                    }
                }
            }
        }
        assert!(
            violations.is_empty(),
            "D67 禁止对标签 / 文本数据做下标访问（只许字段访问）：\n{}",
            violations.join("\n")
        );
    }

    /// 从生成的源码文本里抽 Tags 结构体字段与 TAGS 静态初始化字段。
    fn tags_struct_fields(source: &str) -> (Vec<String>, Vec<String>) {
        let mut struct_fields = Vec::new();
        let mut static_fields = Vec::new();
        let mut in_struct = false;
        let mut in_static = false;
        for line in source.lines() {
            let trimmed = line.trim();
            if in_struct {
                if trimmed == "}" {
                    in_struct = false;
                } else if let Some(rest) = trimmed.strip_prefix("pub ") {
                    if let Some(name) = rest.strip_suffix(": TagText,") {
                        struct_fields.push(name.trim().to_string());
                    }
                }
                continue;
            }
            if in_static {
                if trimmed == "};" {
                    in_static = false;
                } else if trimmed.contains("TagText {") {
                    if let Some((name, _)) = trimmed.split_once(':') {
                        static_fields.push(name.trim().to_string());
                    }
                }
                continue;
            }
            if trimmed == "pub struct Tags {" {
                in_struct = true;
            } else if trimmed == "pub static TAGS: Tags = Tags {" {
                in_static = true;
            }
        }
        (struct_fields, static_fields)
    }

    /// 扫全部标签文件里 \`&crate::generated::TAGS.<字段>\` 的引用；返回（字段列表, 文件数）。
    fn tag_file_field_refs() -> (Vec<String>, Vec<std::path::PathBuf>) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("tags");
        let mut files = Vec::new();
        collect_rs_files(&root, &mut files);
        files.retain(|file| !file.file_name().is_some_and(|name| name == "mod.rs"));
        files.sort();

        let mut refs = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file)
                .unwrap_or_else(|error| panic!("读标签文件 {} 失败：{error}", file.display()));
            let marker = "crate::generated::TAGS.";
            for (index, _) in text.match_indices(marker) {
                let rest = &text[index.saturating_add(marker.len())..];
                let field: String = rest
                    .chars()
                    .take_while(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || *ch == '_')
                    .collect();
                if !field.is_empty() {
                    refs.push(field);
                }
            }
        }
        (refs, files)
    }

    fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("读目录 {} 失败：{error}", dir.display()));
        for entry in entries {
            let path = entry.expect("目录项").path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn descriptions_corpus_is_non_empty() {
        let count = TEXT_CORPUS.len();
        assert!(count > 3_000, "全量文本语料太小：{count}");
        assert_eq!(count, TEXT_CORPUS_COUNT);
        assert!(TEXT_CORPUS.iter().any(|entry| entry.family == "ActionCard"));
        assert!(TEXT_CORPUS.iter().any(|entry| entry.family == "Character" && entry.skill_id != Nat::ZERO));
        assert!(TEXT_CORPUS.iter().any(|entry| entry.family == "Status"));
    }
}
