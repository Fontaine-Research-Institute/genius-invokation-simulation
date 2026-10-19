//! **全量文本解析测试**（D63 / D65）：遍历 build.rs 生成的全部文本，断言 0 条解析不了。
//!
//! 覆盖：卡牌 / 技能 / 状态 / 召唤物 / 敌人的**名字与描述**（4 语）+ 标签名 + 类型名 +
//! 元素名 + 关键字名。失败时逐条打印来源、语言、位置与卡住的内容，并给出失败总数。

use gi_genius_invokation_richtext::{
    parse, ParseError, Segment, ALLOWED_ANY_MATCHES, KEYWORD_VARIANTS, SPRITE_PRESETS,
    TOKEN_VALUES,
};

use super::render::DamageElement;
use crate::families::action_card::ActionCardType;
use crate::families::status::StatusType;
use gi_genius_invokation_engine::Nat;

use crate::generated::localized::{Lang, Localized};
use crate::generated::{action_card_type_text, all_tag_data, keywords_all, status_type_text};

/// 一条解析失败（来源 + 语言 + 人读的位置信息）。
struct Failure {
    source: String,
    lang: Lang,
    detail: String,
}

/// 统计器（测试通过时也打印总量，能看出覆盖面）。
///
/// 计数用 Nat（§8.3：definitions 里不写裸整型）。
#[derive(Default)]
struct Stats {
    checked: Nat,
    any_matches: Nat,
    color_segments: Nat,
}

fn check_text(raw: &str, source: String, lang: Lang, stats: &mut Stats, failures: &mut Vec<Failure>) {
    stats.checked = stats.checked.saturating_add(Nat::new(1));
    let segments = match parse(raw) {
        Ok(segments) => segments,
        Err(error) => {
            failures.push(Failure {
                source,
                lang,
                detail: format_failure(&error, raw),
            });
            return;
        }
    };

    for segment in &segments {
        // D65：解析结果里的 any 匹配点必须全部在解析器登记的白名单内。
        if let Some(any_match) = segment.any_match() {
            stats.any_matches = stats.any_matches.saturating_add(Nat::new(1));
            assert!(
                ALLOWED_ANY_MATCHES.contains(&any_match),
                "出现白名单外的 any 匹配：{any_match:?}（来源 {source} / {lang:?}）"
            );
        }
        match segment {
            Segment::ColorStart(value) => {
                stats.color_segments = stats.color_segments.saturating_add(Nat::new(1));
                if !is_color_value(value) {
                    failures.push(Failure {
                        source: source.clone(),
                        lang,
                        detail: format!("颜色片段取值不合规：{value:?}"),
                    });
                }
            }
            Segment::Icon(name) => {
                let preset = name.strip_prefix("SPRITE_PRESET#").unwrap_or("");
                if !SPRITE_PRESETS.contains(&preset) {
                    failures.push(Failure {
                        source: source.clone(),
                        lang,
                        detail: format!("图标预设名不在固定允许集：{name:?}"),
                    });
                }
            }
            Segment::Keyword { variant, .. } => {
                if let Some(variant) = variant {
                    if !KEYWORD_VARIANTS.contains(&variant.as_str()) {
                        failures.push(Failure {
                            source: source.clone(),
                            lang,
                            detail: format!("关键字变体不在固定允许集：{variant:?}"),
                        });
                    }
                }
            }
            Segment::TokenValue(raw_value) => {
                let name = raw_value
                    .strip_prefix("${[")
                    .and_then(|rest| rest.strip_suffix("]}"))
                    .unwrap_or("");
                if !TOKEN_VALUES.contains(&name) {
                    failures.push(Failure {
                        source: source.clone(),
                        lang,
                        detail: format!("token 名不在固定允许集：{raw_value:?}"),
                    });
                }
            }
            Segment::Text(text) => {
                // 段落里不许残留任何标记（否则说明有东西被当普通文本吞了）。
                for marker in ["$[", "${[", "{SPRITE_PRESET#", "<color=", "</color>"] {
                    if text.contains(marker) {
                        failures.push(Failure {
                            source: source.clone(),
                            lang,
                            detail: format!("普通文本片段里残留标记 {marker:?}：{text:?}"),
                        });
                    }
                }
            }
            Segment::ColorEnd
            | Segment::Card(_)
            | Segment::Character(_)
            | Segment::Skill(_)
            | Segment::Damage(_)
            | Segment::Variable(_) => {}
        }
    }
}

fn check_localized(
    localized: &'static Localized,
    source: &str,
    stats: &mut Stats,
    failures: &mut Vec<Failure>,
) {
    for lang in Lang::ALL {
        if let Some(text) = localized.get(lang).text() {
            check_text(text, format!("{source}（{lang:?}）"), lang, stats, failures);
        }
    }
}

fn is_color_value(value: &str) -> bool {
    value.len() == 9
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn format_failure(error: &ParseError, raw: &str) -> String {
    let (line, col) = error.line_col(raw);
    format!(
        "字节 {}（第 {line} 行第 {col} 列）期望{}，卡在 {:?}；上下文 {:?}",
        error.position,
        error.expected,
        error.found,
        error.snippet(raw)
    )
}

#[test]
fn every_generated_text_parses_strictly() {
    let mut stats = Stats::default();
    let mut failures: Vec<Failure> = Vec::new();

    // 1) 全量卡牌 / 技能 / 状态 / 召唤物 / 敌人文本（名字 + 描述，4 语）。
    for entry in &crate::generated::TEXT_CORPUS {
        for lang in Lang::ALL {
            let Some(raw) = entry.texts.get(lang).text() else {
                continue;
            };
            let source = format!(
                "{} id={} skill_id={} 字段={}",
                entry.family, entry.id, entry.skill_id, entry.field
            );
            check_text(raw, source, lang, &mut stats, &mut failures);
        }
    }

    // 2) 关键字名（4 语）。
    for keyword in keywords_all() {
        check_localized(
            &keyword.name,
            &format!("关键字 K{}", keyword.code.get()),
            &mut stats,
            &mut failures,
        );
    }

    // 3) 类型名（行动牌 3 种 + 状态 2 种）。
    for card_type in ActionCardType::ALL {
        let entry = action_card_type_text(card_type).expect("行动牌类型文本必须存在");
        check_localized(
            &entry.name,
            &format!("行动牌类型 {card_type:?}"),
            &mut stats,
            &mut failures,
        );
    }
    for status_type in StatusType::ALL {
        let entry = status_type_text(status_type).expect("状态类型文本必须存在");
        check_localized(
            &entry.name,
            &format!("状态类型 {status_type:?}"),
            &mut stats,
            &mut failures,
        );
    }

    // 4) 伤害 / 元素名（8 种）。
    for element in DamageElement::ALL {
        check_localized(
            element.name(),
            &format!("伤害元素 {element:?}"),
            &mut stats,
            &mut failures,
        );
    }

    // 5) 标签名（52 个；标签描述本轮为空，不参与）。
    for data in all_tag_data() {
        check_localized(&data.name, "标签名字", &mut stats, &mut failures);
    }

    // 失败条数用积攒计数（不用 len() 的 usize，也不写裸算术）。
    let failure_count = failures
        .iter()
        .fold(Nat::ZERO, |count, _| count.saturating_add(Nat::new(1)));
    if failure_count != Nat::ZERO {
        const SHOWN: Nat = Nat::new(40);
        let mut report = String::new();
        for (index, failure) in failures.iter().take(40).enumerate() {
            report.push_str(&format!(
                "\n#{}. [{}] 语言 {:?}：{}",
                index.saturating_add(1),
                failure.source,
                failure.lang,
                failure.detail
            ));
        }
        if failure_count > SHOWN {
            report.push_str(&format!(
                "\n… 其余 {} 条未列出。",
                failure_count.saturating_sub(SHOWN)
            ));
        }
        panic!(
            "全量解析失败：共检查 {} 条文本，其中 {failure_count} 条解析不了（下面列出前 {} 条）：{report}",
            stats.checked,
            std::cmp::min(failure_count, SHOWN)
        );
    }

    println!(
        "全量解析通过：{} 条文本；颜色片段 {} 个（唯一 any 匹配点）；关键字 {} 个；标签 {} 个",
        stats.checked,
        stats.color_segments,
        keywords_all().len(),
        all_tag_data().len()
    );
    // D65：any 只允许出现在颜色上 —— any 出现次数必须与颜色片段数一致。
    assert_eq!(
        stats.any_matches, stats.color_segments,
        "any 匹配点不止颜色值：白名单被破坏"
    );
}

#[test]
fn keyword_descriptions_parse_strictly() {
    // 关键字说明来自权威 keyword 数据（129 个码有 4 语说明），**单独**解析，
    // 不改上面主全量语料的 28,352 条计数（主语料只覆盖名字/描述，不含关键字说明）。
    let mut failures: Vec<String> = Vec::new();
    let mut parsed_texts = Nat::ZERO;
    for keyword in crate::generated::keywords_all() {
        for lang in Lang::ALL {
            let Some(text) = keyword.description.get(lang).text() else {
                continue;
            };
            parsed_texts = parsed_texts.saturating_add(Nat::new(1));
            if let Err(error) = parse(text) {
                failures.push(format!(
                    "K{} {:?}：{}",
                    keyword.code.get(),
                    lang,
                    format_failure(&error, text)
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "关键字说明解析失败 {} 条：\n{}",
        failures.len(),
        failures.join("\n")
    );
    println!("关键字说明解析通过：{parsed_texts} 条（129 码 × 4 语）");
}
