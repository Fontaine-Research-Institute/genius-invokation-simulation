//! 富文本**解析**（D62 / D63 / D65 / D75 / D76）：只认**结构**，不查任何表、不需要任何上下文。
//!
//! 输入：带标记的原文，例如
//! `造成$[D__KEY__DAMAGE]点$[D__KEY__ELEMENT]，然后$[K53]：<color=#FFFFFFFF>$[S22035]</color>`。
//! 输出：Segment 片段序列（文本 / 颜色 / 关键字 / 卡牌名 / 角色名 / 技能名 / 伤害 / 图标 …）。
//!
//! **严格（D63）**：任何没匹配上的内容一律返回 ParseError。
//! 这里**没有兜底分支**：未知 $[…]、畸形颜色标签、未登记的图标 / 关键字变体 / token 名、
//! 残留标记都算解析失败，不会被当成普通文本吞掉。
//!
//! **匹配口径（D65）**：每条模式都锚定**头尾**按固定串匹配；中间只允许白名单里的 any。
//! 目前白名单**只有一项**：颜色值（纯显示，渲染时删掉，不影响文本结果）。
//! 图标预设名 / 关键字变体 / token 名都按**枚举出来的固定串**精确比，未登记即失败。
//!
//! **职责边界（D76）**：本 crate 只做解析，产物是**中性的片段序列**：
//! 不认识语言（不引用 Localized / Lang）、不认识名字表、不认识卡牌 / 标签 / 关键字表。
//! 名字替换与语言选择由 definitions 侧的渲染器完成；K 码在这里只是一个裸 `u32`。

#![allow(clippy::arithmetic_side_effects)] // 只在 &str 的 usize 下标上做 +1 / 比较；越界前先校验长度

// 注：位置 / 长度用 `usize`，解析器内部按位置切 `&str` 是允许的
//（D75：不属于「按编号取数据表」，见 D67）。

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

/// 关键字变体的**固定允许集**（D65：不许 any）。
///
/// - **内容**：`$[K54|s1]` 这类变体后缀，去掉竖线后的固定串。数据里目前只出现过 `s1`，
///   它只影响关键字名的显示变体，渲染时按同一 K 码取名字。
/// - **依据**：D65 / D66（未证明不影响结果 → 固定匹配，不许 any）。
/// - **未登记即失败**：`$[K53|s2]` 这类未登记变体直接返回 ParseError，不兜底。
/// - **维护**：将来数据出现新变体时，**只改这份清单**，不改解析逻辑。
pub const KEYWORD_VARIANTS: [&str; 1] = ["s1"];

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

impl DamageKey {
    /// 还原成原文标记（渲染时「找不到值」要按原样保留用）。
    pub fn marker(self) -> String {
        match self {
            DamageKey::Element => String::from("$[D__KEY__ELEMENT]"),
            DamageKey::Damage => String::from("$[D__KEY__DAMAGE]"),
            DamageKey::Numbered(n) => format!("$[D__KEY__DAMAGE_{n}]"),
        }
    }
}

/// 解析后的一段（渲染器只吃这些，不再看原文）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Segment {
    /// 普通文本。
    Text(String),
    /// `<color=#RRGGBBAA>`：颜色开始（色值原样，含 #）。**唯一的 any 匹配点**。
    ColorStart(String),
    /// `</color>`：颜色结束。
    ColorEnd,
    /// `$[K53]` / `$[K54|s1]`：关键字（K 码 + 变体后缀，变体是固定允许集）。
    Keyword {
        /// K 码（数值；这是参数不是 any：不同 K 码取不同名字）。
        ///
        /// 本 crate 不认识关键字表，所以这里只是裸数值；definitions 侧再映射。
        code: u32,
        /// `|s1` 这类变体后缀（去掉竖线后原样保留；渲染时由 definitions 侧忽略，按同一 K 码取名字）。
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

impl Segment {
    /// 该片段是否来自 D65 允许的「any 匹配」；不是则 None。
    ///
    /// 穷尽 match：新增片段种类必须回来回答「它是不是 any」，全量测试会盯住这一点。
    pub fn any_match(&self) -> Option<AnyMatch> {
        match self {
            Segment::ColorStart(_) => Some(AnyMatch::ColorDigits),
            Segment::Text(_)
            | Segment::ColorEnd
            | Segment::Keyword { .. }
            | Segment::Card(_)
            | Segment::Character(_)
            | Segment::Skill(_)
            | Segment::Damage(_)
            | Segment::Variable(_)
            | Segment::TokenValue(_)
            | Segment::Icon(_) => None,
        }
    }

    /// 该片段是否自带「原文」可直接拼回（渲染时找不到值就原样输出）。
    pub fn raw_marker(&self) -> Option<String> {
        match self {
            Segment::Keyword { code, variant } => {
                let mut text = format!("$[K{}", code);
                if let Some(variant) = variant {
                    text.push('|');
                    text.push_str(variant);
                }
                text.push(']');
                Some(text)
            }
            Segment::Card(id) => Some(format!("$[C{id}]")),
            Segment::Character(id) => Some(format!("$[A{id}]")),
            Segment::Skill(id) => Some(format!("$[S{id}]")),
            Segment::Damage(key) => Some(key.marker()),
            _ => None,
        }
    }
}

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

impl ParseError {
    /// 由字节下标算 (行, 列)，都从 1 起（列按字符数）。
    pub fn line_col(&self, raw: &str) -> (usize, usize) {
        let head = raw.get(..self.position).unwrap_or(raw);
        let line = head.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let col = head.chars().rev().take_while(|ch| *ch != '\n').count() + 1;
        (line, col)
    }

    /// 出错处附近的一小段原文（用于测试失败信息）。
    pub fn snippet(&self, raw: &str) -> String {
        let start = self.position.saturating_sub(12);
        let end = self.position.saturating_add(24).min(raw.len());
        raw.get(start..end).unwrap_or("").to_string()
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "字节位置 {} 解析失败：期望{}，实际卡在 {:?}",
            self.position, self.expected, self.found
        )
    }
}

impl std::error::Error for ParseError {}

/// 解析整段原文：原始标记 → 片段序列。
///
/// **严格（D63）**：任何未识别的内容都返回 ParseError，不兜底、不当普通文本吞掉。
/// 解析器保证把整段原文消化完（片段拼回去等于原文）。
pub fn parse(raw: &str) -> Result<Vec<Segment>, ParseError> {
    Parser { text: raw, pos: 0 }.parse_all()
}

/// 解析器（只在原文上走一遍）。
struct Parser<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    fn error(&self, expected: &'static str) -> ParseError {
        let found: String = self.rest().chars().take(24).collect();
        ParseError {
            position: self.pos,
            found,
            expected,
        }
    }

    fn parse_all(mut self) -> Result<Vec<Segment>, ParseError> {
        let mut segments: Vec<Segment> = Vec::new();
        let mut pending = String::new();
        let raw_len = self.text.len();

        while self.pos < raw_len {
            let rest = self.rest();
            if rest.starts_with("<color=") {
                Self::flush_text(&mut pending, &mut segments);
                segments.push(self.parse_color()?);
            } else if rest.starts_with("</color>") {
                Self::flush_text(&mut pending, &mut segments);
                self.pos += "</color>".len();
                segments.push(Segment::ColorEnd);
            } else if rest.starts_with("$[") {
                Self::flush_text(&mut pending, &mut segments);
                segments.push(self.parse_placeholder()?);
            } else if rest.starts_with("${[") {
                Self::flush_text(&mut pending, &mut segments);
                segments.push(self.parse_token_value()?);
            } else if rest.starts_with("{SPRITE_PRESET#") {
                Self::flush_text(&mut pending, &mut segments);
                segments.push(self.parse_icon()?);
            } else {
                let ch = rest.chars().next().expect("pos < len 时必有字符");
                match ch {
                    '$' => {
                        return Err(self.error("$ 只能出现在 $[…]/${[…] 占位符里"));
                    }
                    '{' => {
                        return Err(self.error("{ 只能出现在 {SPRITE_PRESET#…} 图标里"));
                    }
                    '<' => {
                        return Err(self.error("< 只能出现在 <color=#…>/</color> 里"));
                    }
                    _ => {
                        pending.push(ch);
                        self.pos += ch.len_utf8();
                    }
                }
            }
        }

        Self::flush_text(&mut pending, &mut segments);
        Ok(segments)
    }

    fn flush_text(pending: &mut String, segments: &mut Vec<Segment>) {
        if pending.is_empty() {
            return;
        }
        segments.push(Segment::Text(std::mem::take(pending)));
    }

    /// 颜色标签：头固定 `<color=`，尾固定 `>`，中间是唯一的 any（颜色值）。
    fn parse_color(&mut self) -> Result<Segment, ParseError> {
        let rest = self.rest();
        let close = rest
            .find('>')
            .ok_or_else(|| self.error("颜色标签的 '>'"))?;
        let body = &rest["<color=".len()..close];
        let valid = body.len() == 9
            && body.starts_with('#')
            && body[1..].bytes().all(|byte| byte.is_ascii_hexdigit());
        if !valid {
            return Err(self.error("形如 #RRGGBBAA 的颜色值"));
        }
        self.pos += close + 1;
        Ok(Segment::ColorStart(body.to_string()))
    }

    /// 占位符：头固定 `$[`，尾固定 `]`；中间必须是已知的固定形态（参数不是 any）。
    fn parse_placeholder(&mut self) -> Result<Segment, ParseError> {
        let rest = self.rest();
        let close = rest
            .find(']')
            .ok_or_else(|| self.error("占位符的结尾 ']'"))?;
        let inner = &rest["$[".len()..close];
        let segment = parse_placeholder_inner(inner)
            .ok_or_else(|| self.error("已知占位符：$[K…]/$[C…]/$[A…]/$[S…]/$[D__KEY__…]/$[G…]"))?;
        self.pos += close + 1;
        Ok(segment)
    }

    /// token 数值占位符：头固定 `${[`，尾固定 `]}`，中间是**固定允许集**里的 token 名。
    fn parse_token_value(&mut self) -> Result<Segment, ParseError> {
        let rest = self.rest();
        let close = rest
            .find("]}")
            .ok_or_else(|| self.error("token 数值占位符的结尾 ']}'"))?;
        // 头固定 "${["，尾固定 "]}"；中间是 token 名（固定允许集）。
        let name = &rest["${[".len()..close];
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
        {
            return Err(self.error("形如 `${[GCG_TOKEN_SHIELD]}` 的 token 占位符"));
        }
        if !TOKEN_VALUES.contains(&name) {
            return Err(self.error("已登记的 token 名（见 TOKEN_VALUES）"));
        }
        let raw = rest[..close + 2].to_string();
        self.pos += close + 2;
        Ok(Segment::TokenValue(raw))
    }

    /// 图标：头固定 `{SPRITE_PRESET#`，尾固定 `}`，中间是**逐个枚举**的预设名（D65：不许 any）。
    fn parse_icon(&mut self) -> Result<Segment, ParseError> {
        let rest = self.rest();
        let close = rest
            .find('}')
            .ok_or_else(|| self.error("图标占位符的结尾 '}'"))?;
        let inner = &rest["{".len()..close];
        let preset = inner
            .strip_prefix("SPRITE_PRESET#")
            .ok_or_else(|| self.error("形如 {SPRITE_PRESET#…} 的图标"))?;
        if !SPRITE_PRESETS.contains(&preset) {
            return Err(self.error("已登记的图标预设名（见 SPRITE_PRESETS）"));
        }
        let name = inner.to_string();
        self.pos += close + 1;
        Ok(Segment::Icon(name))
    }
}

fn parse_number(body: &str) -> Option<u32> {
    if body.is_empty() || !body.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    body.parse().ok()
}

fn parse_placeholder_inner(inner: &str) -> Option<Segment> {
    if let Some(body) = inner.strip_prefix('K') {
        let (code_text, variant) = match body.split_once('|') {
            Some((code_text, variant)) => {
                if !KEYWORD_VARIANTS.contains(&variant) {
                    return None;
                }
                (code_text, Some(variant.to_string()))
            }
            None => (body, None),
        };
        return Some(Segment::Keyword {
            code: parse_number(code_text)?,
            variant,
        });
    }
    if let Some(body) = inner.strip_prefix('C') {
        return Some(Segment::Card(parse_number(body)?));
    }
    if let Some(body) = inner.strip_prefix('A') {
        return Some(Segment::Character(parse_number(body)?));
    }
    if let Some(body) = inner.strip_prefix('S') {
        return Some(Segment::Skill(parse_number(body)?));
    }
    if let Some(body) = inner.strip_prefix('G') {
        parse_number(body)?;
        return Some(Segment::Variable(format!("$[{inner}]")));
    }
    if let Some(body) = inner.strip_prefix("D__KEY__") {
        return match body {
            "ELEMENT" => Some(Segment::Damage(DamageKey::Element)),
            "DAMAGE" => Some(Segment::Damage(DamageKey::Damage)),
            _ => Some(Segment::Damage(DamageKey::Numbered(parse_number(
                body.strip_prefix("DAMAGE_")?,
            )?))),
        };
    }
    None
}

/// 图标预设名：数据里实际出现过的 **43 种**（**逐个枚举**，固定匹配，不许 any）。
///
/// - **内容**：`{SPRITE_PRESET#3503}` 里的预设名（不含 `SPRITE_PRESET#` 前缀）。
///   游戏内文本描述的全量 4 语描述统计。
/// - **依据**：D65 / D66 —— 图标预设名**未证明**「取任何值都不影响结果」，所以必须固定匹配；
///   渲染层会删掉图标，但这不构成 any 的证明。
/// - **未登记即失败**：遇到没登记的预设名直接返回 ParseError（D63），这是有意的：
///   先看那是什么，再决定要不要登记。
/// - **维护**：将来数据变化要增删预设名时，**只改这份清单**，不改解析逻辑。
pub const SPRITE_PRESETS: [&str; 43] = [
    "1101", "1102", "1103", "1104", "1105", "1106", "1107", "1109", "1110", "2101", "2102",
    "2103", "2104", "2105", "2106", "2107", "3003", "3004", "3006", "3007", "3008", "3101",
    "3102", "3103", "3104", "3201", "3202", "3203", "3204", "3205", "3401", "3402", "3403",
    "3404", "3405", "3406", "3407", "3501", "3502", "3503", "3504", "3505", "4007",
];

/// token 数值占位名：数据里实际出现过的 **1 种**（**逐个枚举**，固定匹配，不许 any）。
///
/// - **内容**：`${[GCG_TOKEN_SHIELD]}` 里的 token 名；结构认识，但没有静态值可填，
///   解析产物里原样保留整个标记。
/// - **依据**：D65 / D66（未证明不影响结果 → 固定匹配，不许 any）。
/// - **未登记即失败**：`${[GCG_TOKEN_ENERGY]}` 这类未登记名字直接返回 ParseError，不兜底。
/// - **维护**：将来数据出现新 token 时，**只改这份清单**，不改解析逻辑。
pub const TOKEN_VALUES: [&str; 1] = ["GCG_TOKEN_SHIELD"];
