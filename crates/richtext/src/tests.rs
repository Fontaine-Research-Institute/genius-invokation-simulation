//! 解析器单测（从 definitions/src/generated/text/parse.rs 搬来；D75 / D76：只搬解析）。
//!
//! 覆盖：普通文本 / 颜色结构 / 关键字与变体 / 名字与伤害占位符 / 运行期占位符与图标 /
//! 严格失败（未知占位符、畸形颜色、裸标记字符、未登记白名单）/ any 白名单 / 错误行列定位 /
//! 空输入 / raw_marker 回拼。
use crate::parse::{parse, AnyMatch, DamageKey, ParseError, Segment, ALLOWED_ANY_MATCHES};

fn parse_ok(raw: &str) -> Vec<Segment> {
    parse(raw).unwrap_or_else(|error| panic!("应当解析成功：{raw:?}，错误：{error}"))
}

fn parse_err(raw: &str) -> ParseError {
    match parse(raw) {
        Ok(parsed) => panic!("应当解析失败：{raw:?}，实际得到 {parsed:?}"),
        Err(error) => error,
    }
}

#[test]
fn plain_text_is_one_segment() {
    let parsed = parse_ok("造成2点物理伤害。");
    assert_eq!(parsed, vec![Segment::Text(String::from("造成2点物理伤害。"))]);
}

#[test]
fn color_tags_are_structure_not_text() {
    let parsed = parse_ok("<color=#FFFFFFFF>准备技能</color>");
    assert_eq!(
        parsed,
        vec![
            Segment::ColorStart(String::from("#FFFFFFFF")),
            Segment::Text(String::from("准备技能")),
            Segment::ColorEnd,
        ]
    );
}

#[test]
fn keyword_with_and_without_variant() {
    let parsed = parse_ok("$[K53] 和 $[K54|s1]");
    assert_eq!(
        parsed,
        vec![
            Segment::Keyword {
                code: 53,
                variant: None
            },
            Segment::Text(String::from(" 和 ")),
            Segment::Keyword {
                code: 54,
                variant: Some(String::from("s1"))
            },
        ]
    );
}

#[test]
fn name_and_damage_placeholders() {
    let parsed = parse_ok("$[C114181]$[A1418]$[S14181]$[D__KEY__DAMAGE]$[D__KEY__DAMAGE_3]$[D__KEY__ELEMENT]");
    assert_eq!(
        parsed,
        vec![
            Segment::Card(114181),
            Segment::Character(1418),
            Segment::Skill(14181),
            Segment::Damage(DamageKey::Damage),
            Segment::Damage(DamageKey::Numbered(3)),
            Segment::Damage(DamageKey::Element),
        ]
    );
}

#[test]
fn runtime_placeholders_and_icons() {
    let parsed = parse_ok("$[G16121]点治疗${[GCG_TOKEN_SHIELD]}{SPRITE_PRESET#3503}");
    assert_eq!(
        parsed,
        vec![
            Segment::Variable(String::from("$[G16121]")),
            Segment::Text(String::from("点治疗")),
            Segment::TokenValue(String::from("${[GCG_TOKEN_SHIELD]}")),
            Segment::Icon(String::from("SPRITE_PRESET#3503")),
        ]
    );
}

#[test]
fn brackets_in_plain_text_are_text() {
    // 韩文里的 "[패시브]" 是普通方括号，不是标记。
    let parsed = parse_ok("[패시브] 전투 시작 시");
    assert_eq!(parsed, vec![Segment::Text(String::from("[패시브] 전투 시작 시"))]);
}

#[test]
fn unknown_placeholder_is_error() {
    let error = parse_err("造成$[Q1]伤害");
    assert_eq!(error.position, "造成".len());
    assert_eq!(error.found, "$[Q1]伤害");
    assert!(error.expected.contains("已知占位符"));
}

#[test]
fn malformed_color_is_error() {
    let error = parse_err("<color=#GGGGGGGG>x</color>");
    assert_eq!(error.position, 0);
    assert!(error.expected.contains("#RRGGBBAA"));
}

#[test]
fn unterminated_color_is_error() {
    let error = parse_err("<color=#FFFFFFFF");
    assert!(error.found.starts_with("<color="));
    assert!(error.expected.contains("'>'"));
}

#[test]
fn lone_marker_characters_are_errors() {
    for raw in ["价格 $ 5", "花括号 {", "小于号 <"] {
        let error = parse_err(raw);
        let expected_position = raw.find(['$', '{', '<']).expect("测试输入必须含标记字符");
        assert_eq!(
            error.position, expected_position,
            "字面量 {raw:?} 必须在标记字符处报错"
        );
    }
}

#[test]
fn unregistered_sprite_preset_is_error() {
    // D65：图标预设名是固定枚举，没登记过的一律失败（不是 any）。
    let error = parse_err("{SPRITE_PRESET#9999}");
    assert!(error.expected.contains("SPRITE_PRESETS"));
    // 已登记的可以过。
    assert!(parse("{SPRITE_PRESET#3503}").is_ok());
}

#[test]
fn unregistered_keyword_variant_and_token_are_errors() {
    assert!(parse("$[K53|s2]").is_err(), "未登记的变体必须失败");
    assert!(parse("${[GCG_TOKEN_ENERGY]}").is_err(), "未登记的 token 名必须失败");
}

#[test]
fn bad_bodies_are_errors() {
    for raw in [
        "$[K]",
        "$[Kabc]",
        "$[K53|]",
        "$[C12x]",
        "$[A]",
        "$[S-1]",
        "$[D__KEY__FOO]",
        "$[D__KEY__DAMAGE_x]",
        "$[GCG_TOKEN_SHIELD]",
        "{SPRITE_PRESET#abc}",
        "${[GCG_TOKEN_SHIELD]",
        "$[G]",
    ] {
        let _ = parse_err(raw);
    }
}

#[test]
fn color_is_the_only_any_match() {
    // D65：any 白名单目前只有颜色；其余片段都不是 any。
    assert_eq!(ALLOWED_ANY_MATCHES, [AnyMatch::ColorDigits]);
    assert_eq!(
        Segment::ColorStart(String::from("#FFFFFFFF")).any_match(),
        Some(AnyMatch::ColorDigits)
    );
    assert_eq!(Segment::Text(String::from("x")).any_match(), None);
    assert_eq!(Segment::Icon(String::from("SPRITE_PRESET#3503")).any_match(), None);
    assert_eq!(
        Segment::Keyword {
            code: 1,
            variant: None
        }
        .any_match(),
        None
    );
}

#[test]
fn line_col_points_at_error() {
    let raw = "第一行\n第二行$[Q9]";
    let error = parse_err(raw);
    let (line, col) = error.line_col(raw);
    assert_eq!(line, 2);
    assert_eq!(col, 4, "第二行第 4 个字符是 '$'");
}

#[test]
fn empty_input_is_empty_sequence() {
    assert_eq!(parse(""), Ok(Vec::new()));
}

#[test]
fn raw_marker_rebuilds_placeholders() {
    // definitions 渲染器「找不到名字时按原样保留」依赖 raw_marker（D63 不兜底：只回拼原文）。
    let segments = parse_ok("$[K54|s1]$[C1]$[A2]$[S3]$[D__KEY__DAMAGE]$[D__KEY__DAMAGE_2]$[D__KEY__ELEMENT]");
    let markers: Vec<Option<String>> = segments.iter().map(Segment::raw_marker).collect();
    assert_eq!(
        markers,
        vec![
            Some(String::from("$[K54|s1]")),
            Some(String::from("$[C1]")),
            Some(String::from("$[A2]")),
            Some(String::from("$[S3]")),
            Some(String::from("$[D__KEY__DAMAGE]")),
            Some(String::from("$[D__KEY__DAMAGE_2]")),
            Some(String::from("$[D__KEY__ELEMENT]")),
        ]
    );
    // 文本 / 颜色 / 运行期占位符 / 图标没有 raw_marker（渲染规则不同）。
    let other = parse_ok("x<color=#FFFFFFFF></color>$[G1]${[GCG_TOKEN_SHIELD]}{SPRITE_PRESET#3503}");
    assert!(other.iter().all(|segment| segment.raw_marker().is_none()));
}
