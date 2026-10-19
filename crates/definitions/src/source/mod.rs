//! **每文件自声明源码**（D213 / D214 / D215 / D216 / D242）：文件在头部用宏把「本文件 ↔ 归属」
//! 绑定成一份编译期静态数据（原文全文 + 解析出的依赖 + 归属）。
//!
//! 宏与机制是**通用定义**（D242），所以留在本 crate：card_source! / module_source!、
//! SourceFile / SourceOwner、use 依赖解析器都可以被上层 crate 直接复用。
//!
//! ## 两种宏（D216）
//!
//! - 卡牌文件：gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);
//!   - 卡牌文件在 **gi-genius-invokation-cards** crate 里（D240 / D241），本 crate 不存具体卡牌；
//!   - 路径是**分段列表**（每段一个字符串），实际代表 character/aino；
//!   - 宏拼出 src/cards/<segments...>/mod.rs（卡牌按 D163 一卡一目录，文件是 mod.rs）；
//!     include_str! 以**调用方 crate** 的 CARGO_MANIFEST_DIR 为根，所以从 cards crate 调用时
//!     解析到 crates/cards/src/cards/...，而 path 字段仍是 src/cards/...（结构不变）；
//!   - **不写 id**（D215）：展开直接绑定本模块的 pub const ID: CardId；
//!   - 归属 = SourceOwner::Card(ID)。
//! - 非卡牌模块文件：crate::module_source!(path: ["families", "character", "damage"], name: MODULE_NAME);
//!   - 路径分段拼出 src/families/character/damage.rs（模块文件带 .rs）；
//!   - NAME 是 Localized（可多语），声明模块自己的名字；
//!   - 归属 = SourceOwner::Module(&NAME)。
//!
//! 两个宏都展开出 pub static SOURCE: SourceFile：
//! - path：相对 crate 根的路径（段用 / 拼接，层级原样保留）；
//! - segments：宏里写的路径分段原样保留（&'static [&'static str]）；
//! - content：include_str! **本文件原文**（不解析、不展开宏；连宏调用自身都在文本里）；
//! - dependencies：**编译期解析**本文件 use 语句得到的依赖（D214 / D216；最小实现，
//!   解析不了的条目记 Unparsed 并保留原文，不会 panic）。
//!
//! 编译期嵌入、运行期零 IO/零分配：依赖只存原文里的字节区间（[Span]），
//! 取名字时才切原文（[Dependency::path] 等）。
//!
//! **为什么依赖存 span 而不是 &'static [Dependency]**：本仓库工具链的 const eval 里
//! str 范围切片还不是稳定 const 操作（Index 未稳定为 const trait），且 crate 不用 unsafe；
//! 所以编译期只算区间，运行期安全切原文——语义等价、零分配。待复核：等 const 字符串切片
//! 稳定后可改成直接存 &'static str 段。
//!
//! ## 汇总入口的限制（重要）
//!
//! Rust（不引第三方 crate、不写 build.rs 扫描源码）**没有**「自动收集各文件静态项」的稳定
//! 机制，所以汇总表 [SOURCE_FILES] 是**手写清单**：新文件用宏声明后，还要在
//! source/mod.rs 的 [SOURCE_FILES] 里加一行引用。漏加不会编译失败，但不会被
//! [source_files] / [source_of] 找到；直接按文件常数（如
//! crate::families::character::damage::SOURCE）读取不受影响。
//!
//! **登记表按 crate 分（D242）**：本文件的 [SOURCE_FILES] 只登记 definitions 自己的
//! 模块文件；**卡牌的源码表在 gi-genius-invokation-cards 的 source 模块**
//! （cards::source::SOURCE_FILES，每张卡一行），因为具体卡牌已独立成 crate（D240 / D241）。
//!
//! feature embed-source 默认开启；关闭时 [SOURCE_FILES] 为空、各文件
//! SOURCE.content 为空串、依赖表为空（不占体积），所有访问器照常可调用。

use crate::base::CardId;
use crate::generated::Localized;

/// 单个依赖最多允许的路径段数（超出 -> 记 Unparsed，不崩）。
pub const MAX_DEPENDENCY_SEGMENTS: usize = 8;
/// 单个花括号依赖最多允许的成员数（超出 -> 记 Unparsed，不崩）。
pub const MAX_DEPENDENCY_MEMBERS: usize = 8;
/// 单个文件最多记录的依赖条数（超出则忽略后续；原文不受影响）。
pub const MAX_DEPENDENCIES: usize = 32;

/// 原文里的一个字节区间（半开区间 [start, end)；由编译期解析给出，保证 UTF-8 边界）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    /// 起始字节偏移（含）。
    pub start: usize,
    /// 结束字节偏移（不含）。
    pub end: usize,
}

impl Span {
    /// 空区间。
    pub const EMPTY: Span = Span { start: 0, end: 0 };

    /// 从原文切出这一段（区间来自编译期解析，安全；span 越界会 panic 而不是返回错数据）。
    pub fn of<'a>(&self, content: &'a str) -> &'a str {
        &content[self.start..self.end]
    }
}

/// 一条依赖的解析种类（D216 最小实现）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DependencyKind {
    /// use a::b;
    Single,
    /// use a::b as c;
    Alias,
    /// use a::b::{c, d};（路径 = 花括号前的公共前缀，成员另存）
    Brace,
    /// 解析不了：raw 保留整条原文，不崩。
    Unparsed,
}

/// 编译期解析出的一条依赖（只存原文区间，零拷贝；不对外暴露）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct DependencyEntry {
    /// 解析种类。
    kind: DependencyKind,
    /// 路径分段（原文区间；Brace 时是花括号前的公共前缀）。
    segments: [Span; MAX_DEPENDENCY_SEGMENTS],
    /// segments 里真正有效的段数。
    segment_count: usize,
    /// 花括号成员（原文区间；仅 Brace 时有效）。
    members: [Span; MAX_DEPENDENCY_MEMBERS],
    /// members 里真正有效的成员数。
    member_count: usize,
    /// use ... as X 的别名区间（无别名 = None）。
    alias: Option<Span>,
    /// Unparsed 时保留的整条原文区间（成功解析 = None）。
    raw: Option<Span>,
}

const EMPTY_DEPENDENCY: DependencyEntry = DependencyEntry {
    kind: DependencyKind::Unparsed,
    segments: [Span::EMPTY; MAX_DEPENDENCY_SEGMENTS],
    segment_count: 0,
    members: [Span::EMPTY; MAX_DEPENDENCY_MEMBERS],
    member_count: 0,
    alias: None,
    raw: None,
};

/// 一个文件**编译期解析出的依赖表**（定长数组 + 长度；无堆分配）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParsedDependencies {
    /// 定长槽位；有效范围 items[..len]。
    items: [DependencyEntry; MAX_DEPENDENCIES],
    /// 有效条数。
    len: usize,
}

impl ParsedDependencies {
    /// 空表（feature 关闭 / 解析前）。
    pub const EMPTY: ParsedDependencies = ParsedDependencies {
        items: [EMPTY_DEPENDENCY; MAX_DEPENDENCIES],
        len: 0,
    };
}

/// 一份源码文件的**归属**（D216）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceOwner {
    /// 卡牌：归属到卡 id（宏自动绑定本模块的 ID）。
    Card(CardId),
    /// 非卡牌模块：归属到模块自己的多语名字。
    Module(&'static Localized),
}

/// 一份**编译期嵌入的完整源代码**（D213 / D214 / D215 / D216）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// 归属（卡牌 id / 模块多语名字）。
    pub owner: SourceOwner,
    /// 相对 crate 根的路径（段用 / 拼接，层级原样保留）。
    pub path: &'static str,
    /// 宏里写的路径分段（原样保留）。
    pub segments: &'static [&'static str],
    /// 本文件全文（include_str! 原文嵌入；不做宏展开 / 解析）。
    pub content: &'static str,
    /// 编译期解析出的依赖表（D214 / D216；用 [SourceFile::dependencies] 取结构化视图）。
    pub dependencies: ParsedDependencies,
}

impl SourceFile {
    /// 本文件解析出的依赖列表（只读视图；路径分段按需切原文，零分配）。
    pub fn dependencies(&self) -> DependencyList<'_> {
        DependencyList {
            content: self.content,
            parsed: self.dependencies,
        }
    }
}

/// 一段源码的依赖列表视图（[SourceFile::dependencies] 的返回类型）。
#[derive(Clone, Copy, Debug)]
pub struct DependencyList<'a> {
    /// 源码原文（span 的切片来源）。
    content: &'a str,
    /// 编译期解析结果。
    parsed: ParsedDependencies,
}

impl<'a> DependencyList<'a> {
    /// 直接解析一段源码（测试 / 工具用；正式使用走 [SourceFile::dependencies]）。
    pub const fn parse(content: &'a str) -> DependencyList<'a> {
        DependencyList {
            content,
            parsed: parse_dependencies(content),
        }
    }

    /// 依赖条数。
    pub const fn len(&self) -> usize {
        self.parsed.len
    }

    /// 是否没有解析出任何依赖。
    pub const fn is_empty(&self) -> bool {
        self.parsed.len == 0
    }

    /// 逐条遍历（零分配）。
    pub fn iter(&self) -> impl Iterator<Item = Dependency<'a>> + '_ {
        self.parsed.items[..self.parsed.len]
            .iter()
            .map(move |entry| Dependency {
                content: self.content,
                entry: *entry,
            })
    }
}

/// 一条依赖的只读视图（路径分段 / 别名 / 原文都按需从 content 切）。
#[derive(Clone, Copy, Debug)]
pub struct Dependency<'a> {
    /// 源码原文。
    content: &'a str,
    /// 编译期解析结果。
    entry: DependencyEntry,
}

impl<'a> Dependency<'a> {
    /// 依赖种类。
    pub const fn kind(self) -> DependencyKind {
        self.entry.kind
    }

    /// 是否解析成功（Unparsed = false，此时用 [Dependency::raw] 看原文）。
    pub const fn is_parsed(self) -> bool {
        !matches!(self.entry.kind, DependencyKind::Unparsed)
    }

    /// 路径分段（**token 列表**：每段一个 &str；Brace 时是花括号前的公共前缀）。
    pub fn path(self) -> impl Iterator<Item = &'a str> + 'a {
        let content = self.content;
        self.entry
            .segments
            .into_iter()
            .take(self.entry.segment_count)
            .map(move |span| span.of(content))
    }

    /// 花括号成员（仅 Brace；每段一个 &str）。
    pub fn members(self) -> impl Iterator<Item = &'a str> + 'a {
        let content = self.content;
        self.entry
            .members
            .into_iter()
            .take(self.entry.member_count)
            .map(move |span| span.of(content))
    }

    /// use ... as X 的别名（无别名 = None）。
    pub fn alias(self) -> Option<&'a str> {
        match self.entry.alias {
            Some(span) => Some(span.of(self.content)),
            None => None,
        }
    }

    /// Unparsed 时保留的整条原文（成功解析 = None）。
    pub fn raw(self) -> Option<&'a str> {
        match self.entry.raw {
            Some(span) => Some(span.of(self.content)),
            None => None,
        }
    }
}

/// 卡牌文件宏（D214 / D215 / D216 / D242）。
///
/// 用法（在卡牌 crate 里）：gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);
///
/// - 具体卡牌在 gi-genius-invokation-cards crate（D240 / D241）；本宏是**通用定义**，留在本 crate；
/// - 路径是**分段字符串列表**（每段一个 "..."，内部用 / 拼接）；
/// - 拼出 src/cards/<segments...>/mod.rs；include_str! 以**调用方 crate** 的
///   CARGO_MANIFEST_DIR 为根（从 cards crate 调用时读到 crates/cards/src/cards/...），
///   path 字段仍是 src/cards/...（结构不变）；
/// - **不写 id**：展开直接引用本模块的 pub const ID: CardId。本模块没有 ID
///   （或类型不是 CardId）时，展开里的 const _: CardId = ID; 会**编译报错**，
///   提示「本模块需定义 pub const ID: CardId」，不会静默绑定错 id。
#[cfg(feature = "embed-source")]
#[macro_export]
macro_rules! card_source {
    (path: [$($segment:literal),+ $(,)?] $(,)?) => {
        /// 本文件的归属 + 编译期嵌入的原文 + 解析出的依赖（D213 / D214 / D215 / D216）。
        pub static SOURCE: $crate::source::SourceFile = {
            // D215：宏里不写 id —— 绑定本模块的 pub const ID: CardId；
            // 本模块缺 ID 或类型不对时这里编译报错（不静默）。
            // 本模块需定义 pub const ID: CardId。
            const _: $crate::base::CardId = ID;
            const CONTENT: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/",
                concat!("src/cards", $("/", $segment),+, "/mod.rs")
            ));
            $crate::source::SourceFile {
                // D215：id 自动绑定到本模块的 ID（调用处解析，不重复写卡 id）。
                owner: $crate::source::SourceOwner::Card(ID),
                path: concat!("src/cards", $("/", $segment),+, "/mod.rs"),
                segments: &[$($segment),+],
                content: CONTENT,
                dependencies: $crate::source::parse_dependencies(CONTENT),
            }
        };
    };
}

/// 卡牌文件宏（feature embed-source 关闭版：只留归属与路径）。
#[cfg(not(feature = "embed-source"))]
#[macro_export]
macro_rules! card_source {
    (path: [$($segment:literal),+ $(,)?] $(,)?) => {
        /// 本文件的归属信息（D216；feature embed-source 关闭：不嵌入全文）。
        pub static SOURCE: $crate::source::SourceFile = {
            // 本模块需定义 pub const ID: CardId（缺失 / 类型不对时编译报错）。
            const _: $crate::base::CardId = ID;
            $crate::source::SourceFile {
                owner: $crate::source::SourceOwner::Card(ID),
                path: concat!("src/cards", $("/", $segment),+, "/mod.rs"),
                segments: &[$($segment),+],
                content: "",
                dependencies: $crate::source::ParsedDependencies::EMPTY,
            }
        };
    };
}

/// 非卡牌模块文件宏（D216）。
///
/// 用法：crate::module_source!(path: ["families", "character", "damage"], name: MODULE_NAME);
///
/// - 路径是**分段字符串列表**，拼出 src/<segments...>.rs（模块文件）；
/// - name 是 Localized 表达式（模块自己的多语名字），归属 = SourceOwner::Module(&name)；
///   建议用模块里的 static MODULE_NAME: Localized，这样引用地址稳定（'static）。
#[cfg(feature = "embed-source")]
#[macro_export]
macro_rules! module_source {
    (path: [$($segment:literal),+ $(,)?], name: $name:expr $(,)?) => {
        /// 本模块的归属 + 编译期嵌入的原文 + 解析出的依赖（D216）。
        pub static SOURCE: $crate::source::SourceFile = {
            const CONTENT: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/",
                concat!("src", $("/", $segment),+, ".rs")
            ));
            $crate::source::SourceFile {
                owner: $crate::source::SourceOwner::Module(&$name),
                path: concat!("src", $("/", $segment),+, ".rs"),
                segments: &[$($segment),+],
                content: CONTENT,
                dependencies: $crate::source::parse_dependencies(CONTENT),
            }
        };
    };
}

/// 非卡牌模块文件宏（feature embed-source 关闭版：只留归属与路径）。
#[cfg(not(feature = "embed-source"))]
#[macro_export]
macro_rules! module_source {
    (path: [$($segment:literal),+ $(,)?], name: $name:expr $(,)?) => {
        /// 本模块的归属信息（D216；feature embed-source 关闭：不嵌入全文）。
        pub static SOURCE: $crate::source::SourceFile = {
            $crate::source::SourceFile {
                owner: $crate::source::SourceOwner::Module(&$name),
                path: concat!("src", $("/", $segment),+, ".rs"),
                segments: &[$($segment),+],
                content: "",
                dependencies: $crate::source::ParsedDependencies::EMPTY,
            }
        };
    };
}

/// 已接入的**模块**源码文件清单（**手写登记**，按 path 字典序升序；限制见模块文档）。
///
/// 新增模块文件：文件里用 [module_source!] 声明后，在这里加一行 &crate::...::SOURCE。
/// 卡牌的登记表不在这里 —— 见 gi-genius-invokation-cards 的 source::SOURCE_FILES（D242）。
#[cfg(feature = "embed-source")]
pub static SOURCE_FILES: &[&SourceFile] = &[
    // 只登记 definitions 自己的模块文件；具体卡牌已独立成 crate（D240 / D242）。
    &crate::families::character::damage::SOURCE,
];

/// feature embed-source 关闭时：汇总表为空（不占体积）。
#[cfg(not(feature = "embed-source"))]
pub static SOURCE_FILES: &[&SourceFile] = &[];

/// 全部已登记的源码文件（只读；顺序 = [SOURCE_FILES]）。
pub fn source_files() -> &'static [&'static SourceFile] {
    SOURCE_FILES
}

/// 按 path 精确查找（path 口径 = 宏路径分段拼出的相对 crate 根路径）。
pub fn source_of(path: &str) -> Option<&'static SourceFile> {
    SOURCE_FILES.iter().copied().find(|file| file.path == path)
}

/// 按路径前缀筛出某目录下的文件（path 口径同上；只读迭代器）。
pub fn source_files_under(prefix: &str) -> impl Iterator<Item = &'static SourceFile> + '_ {
    SOURCE_FILES
        .iter()
        .copied()
        .filter(move |file| file.path.starts_with(prefix))
}

// ===========================================================================
// 编译期解析 use 依赖（const fn；只用字节扫描 + 区间，不切片、不分配、不 panic）
// ===========================================================================

/// 把整份源码的 use 语句解析成依赖表（编译期调用；解析不了记 Unparsed）。
#[doc(hidden)]
pub const fn parse_dependencies(source: &str) -> ParsedDependencies {
    let bytes = source.as_bytes();
    let mut result = ParsedDependencies::EMPTY;
    let mut line = 0usize;
    loop {
        if line >= bytes.len() {
            return result;
        }
        let line_end = match find_byte(bytes, line, bytes.len(), 0x0A) {
            Some(found) => found,
            None => bytes.len(),
        };
        let mut cursor = skip_ws(bytes, line, line_end);
        // 支持 pub use ...;
        if matches_keyword(bytes, cursor, line_end, b"pub") {
            cursor = match cursor.checked_add(3) {
                Some(value) => skip_ws(bytes, value, line_end),
                None => return result,
            };
        }
        if matches_keyword(bytes, cursor, line_end, b"use") {
            let after_use = match cursor.checked_add(3) {
                Some(value) => value,
                None => return result,
            };
            if after_use < line_end && is_ws(bytes[after_use]) {
                line = parse_use_statement(
                    bytes,
                    &mut result,
                    cursor,
                    skip_ws(bytes, after_use, line_end),
                    line_end,
                );
                continue;
            }
        }
        line = match line_end.checked_add(1) {
            Some(value) => value,
            None => return result,
        };
    }
}

/// 解析一条 use ...; ；返回下一次扫描位置（语句之后 / 行尾之后）。
const fn parse_use_statement(
    bytes: &[u8],
    result: &mut ParsedDependencies,
    use_start: usize,
    body_start: usize,
    line_end: usize,
) -> usize {
    let (semi, next) = match find_statement_end(bytes, body_start) {
        Some(found) => found,
        None => {
            push_unparsed(result, Span { start: use_start, end: line_end });
            return next_line(line_end, bytes.len());
        }
    };
    let body = trim_bytes(bytes, body_start, semi);
    let raw = Span {
        start: use_start,
        end: match semi.checked_add(1) {
            Some(value) => value,
            None => semi,
        },
    };
    if body.start >= body.end {
        push_unparsed(result, raw);
        return next;
    }
    let parsed = match find_byte(bytes, body.start, body.end, b'{') {
        Some(_) => parse_brace_dependency(bytes, body, result),
        None => parse_plain_dependency(bytes, body, result),
    };
    if !parsed {
        push_unparsed(result, raw);
    }
    next
}

/// use a::b; / use a::b as c;
const fn parse_plain_dependency(bytes: &[u8], body: Span, result: &mut ParsedDependencies) -> bool {
    let mut path_end = body.end;
    let mut alias = None;
    let mut kind = DependencyKind::Single;
    if let Some(position) = find_subsequence(bytes, body.start, body.end, b" as ") {
        let alias_start = match position.checked_add(4) {
            Some(value) => value,
            None => return false,
        };
        let alias_span = trim_bytes(bytes, alias_start, body.end);
        if alias_span.start >= alias_span.end {
            return false;
        }
        // 别名必须是单个标识符（_ 也算），不允许再出现 :: / 空白 / 分隔符。
        if contains_any(bytes, alias_span.start, alias_span.end, SEPARATORS) {
            return false;
        }
        alias = Some(alias_span);
        path_end = position;
        kind = DependencyKind::Alias;
    }
    let mut segments = [Span::EMPTY; MAX_DEPENDENCY_SEGMENTS];
    let segment_count = match split_path(bytes, body.start, path_end, &mut segments) {
        Some(count) => count,
        None => return false,
    };
    push_dependency(
        result,
        DependencyEntry {
            kind,
            segments,
            segment_count,
            members: [Span::EMPTY; MAX_DEPENDENCY_MEMBERS],
            member_count: 0,
            alias,
            raw: None,
        },
    )
}

/// use a::b::{c, d}; （最小实现：花括号前是 :: 结尾的公共前缀，成员是单段标识符）。
const fn parse_brace_dependency(bytes: &[u8], body: Span, result: &mut ParsedDependencies) -> bool {
    let open = match find_byte(bytes, body.start, body.end, b'{') {
        Some(found) => found,
        None => return false,
    };
    let close = match find_matching_brace(bytes, open, body.end) {
        Some(found) => found,
        None => return false,
    };
    let after_open = match open.checked_add(1) {
        Some(value) => value,
        None => return false,
    };
    let after_close = match close.checked_add(1) {
        Some(value) => value,
        None => return false,
    };
    // } 之后只能剩空白（as 等不支持 -> Unparsed）。
    let tail = trim_bytes(bytes, after_close, body.end);
    if tail.start < tail.end {
        return false;
    }
    // 前缀：crate::base:: -> ["crate", "base"]。
    let trimmed_prefix = trim_bytes(bytes, body.start, open);
    let mut prefix_end = trimmed_prefix.end;
    while let Some(last_part) = prefix_end.checked_sub(2) {
        let second = match last_part.checked_add(1) {
            Some(value) => value,
            None => break,
        };
        if bytes[last_part] == b':' && bytes[second] == b':' {
            prefix_end = last_part;
        } else {
            break;
        }
    }
    let prefix = trim_bytes(bytes, trimmed_prefix.start, prefix_end);
    let mut segments = [Span::EMPTY; MAX_DEPENDENCY_SEGMENTS];
    let segment_count = if prefix.start >= prefix.end {
        0
    } else {
        match split_path(bytes, prefix.start, prefix.end, &mut segments) {
            Some(count) => count,
            None => return false,
        }
    };
    // 成员：花括号内按 , 切（不支持嵌套花括号 / 成员别名）。
    if contains_any(bytes, after_open, close, BRACES) {
        return false;
    }
    let mut members = [Span::EMPTY; MAX_DEPENDENCY_MEMBERS];
    let mut member_count = 0usize;
    let mut cursor = after_open;
    loop {
        let comma = find_byte(bytes, cursor, close, b',');
        let piece_end = match comma {
            Some(found) => found,
            None => close,
        };
        let piece = trim_bytes(bytes, cursor, piece_end);
        if piece.start < piece.end {
            if member_count >= MAX_DEPENDENCY_MEMBERS {
                return false;
            }
            // 成员只允许单段标识符（:: / as / 花括号 / 通配都算解析不了）。
            if contains_any(bytes, piece.start, piece.end, SEPARATORS) {
                return false;
            }
            members[member_count] = piece;
            member_count = match member_count.checked_add(1) {
                Some(value) => value,
                None => return false,
            };
        }
        match comma {
            Some(found) => {
                cursor = match found.checked_add(1) {
                    Some(value) => value,
                    None => return false,
                };
            }
            None => break,
        }
    }
    if member_count == 0 {
        return false;
    }
    push_dependency(
        result,
        DependencyEntry {
            kind: DependencyKind::Brace,
            segments,
            segment_count,
            members,
            member_count,
            alias: None,
            raw: None,
        },
    )
}

/// 记录一条「未解析」依赖（原文区间保留，不崩）。
const fn push_unparsed(result: &mut ParsedDependencies, raw: Span) {
    push_dependency(
        result,
        DependencyEntry {
            kind: DependencyKind::Unparsed,
            segments: [Span::EMPTY; MAX_DEPENDENCY_SEGMENTS],
            segment_count: 0,
            members: [Span::EMPTY; MAX_DEPENDENCY_MEMBERS],
            member_count: 0,
            alias: None,
            raw: Some(raw),
        },
    );
}

/// 追加一条依赖；表满时丢弃并返回 false（不 panic）。
const fn push_dependency(result: &mut ParsedDependencies, entry: DependencyEntry) -> bool {
    if result.len >= MAX_DEPENDENCIES {
        return false;
    }
    result.items[result.len] = entry;
    result.len = match result.len.checked_add(1) {
        Some(value) => value,
        None => result.len,
    };
    true
}

/// 找语句结束 ; （返回 (分号位置, 下次扫描位置)）；顶层遇到换行 / EOF 视为没有语句结束。
const fn find_statement_end(bytes: &[u8], body_start: usize) -> Option<(usize, usize)> {
    let mut index = body_start;
    let mut depth = 0usize;
    loop {
        if index >= bytes.len() {
            return None;
        }
        match bytes[index] {
            b';' if depth == 0 => {
                let next = match index.checked_add(1) {
                    Some(value) => value,
                    None => return None,
                };
                return Some((index, next));
            }
            0x0A if depth == 0 => return None,
            b'{' => {
                depth = match depth.checked_add(1) {
                    Some(value) => value,
                    None => return None,
                };
            }
            b'}' if depth > 0 => {
                depth = match depth.checked_sub(1) {
                    Some(value) => value,
                    None => return None,
                };
            }
            _ => {}
        }
        index = match index.checked_add(1) {
            Some(value) => value,
            None => return None,
        };
    }
}

/// 找与 open 配对的 }（只在 [open, end) 内扫；配不上返回 None）。
const fn find_matching_brace(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;
    loop {
        if index >= end {
            return None;
        }
        match bytes[index] {
            b'{' => {
                depth = match depth.checked_add(1) {
                    Some(value) => value,
                    None => return None,
                };
            }
            b'}' => {
                depth = match depth.checked_sub(1) {
                    Some(value) => value,
                    None => return None,
                };
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index = match index.checked_add(1) {
            Some(value) => value,
            None => return None,
        };
    }
}

/// 把路径文本按 :: 切成段（每段 trim；空段 / 非法字符 -> None）。
const fn split_path(
    bytes: &[u8],
    start: usize,
    end: usize,
    out: &mut [Span; MAX_DEPENDENCY_SEGMENTS],
) -> Option<usize> {
    let mut count = 0usize;
    let mut cursor = start;
    loop {
        let separator = find_subsequence(bytes, cursor, end, b"::");
        let piece_end = match separator {
            Some(found) => found,
            None => end,
        };
        let piece = trim_bytes(bytes, cursor, piece_end);
        if piece.start >= piece.end {
            return None;
        }
        if contains_any(bytes, piece.start, piece.end, PATH_ILLEGAL) {
            return None;
        }
        if count >= out.len() {
            return None;
        }
        out[count] = piece;
        count = match count.checked_add(1) {
            Some(value) => value,
            None => return None,
        };
        match separator {
            Some(found) => {
                cursor = match found.checked_add(2) {
                    Some(value) => value,
                    None => return None,
                };
            }
            None => return Some(count),
        }
    }
}

/// 从 [start, end) 里找第一个 needle 出现位置。
const fn find_subsequence(bytes: &[u8], start: usize, end: usize, needle: &[u8]) -> Option<usize> {
    let mut index = start;
    loop {
        if index >= end {
            return None;
        }
        if matches_at(bytes, index, end, needle) {
            return Some(index);
        }
        index = match index.checked_add(1) {
            Some(value) => value,
            None => return None,
        };
    }
}

/// bytes[at..] 是否以 needle 开头（不越过 end）。
const fn matches_at(bytes: &[u8], at: usize, end: usize, needle: &[u8]) -> bool {
    let mut offset = 0usize;
    loop {
        if offset >= needle.len() {
            return true;
        }
        let position = match at.checked_add(offset) {
            Some(value) => value,
            None => return false,
        };
        if position >= end {
            return false;
        }
        if bytes[position] != needle[offset] {
            return false;
        }
        offset = match offset.checked_add(1) {
            Some(value) => value,
            None => return false,
        };
    }
}

/// at 处是否是 word，且后面跟空白 / 行尾（避免 user 命中 use）。
const fn matches_keyword(bytes: &[u8], at: usize, end: usize, word: &[u8]) -> bool {
    if !matches_at(bytes, at, end, word) {
        return false;
    }
    let after = match at.checked_add(word.len()) {
        Some(value) => value,
        None => return false,
    };
    if after >= end {
        return true;
    }
    is_ws(bytes[after])
}

/// 找第一个 target 字节。
const fn find_byte(bytes: &[u8], start: usize, end: usize, target: u8) -> Option<usize> {
    let mut index = start;
    loop {
        if index >= end {
            return None;
        }
        if bytes[index] == target {
            return Some(index);
        }
        index = match index.checked_add(1) {
            Some(value) => value,
            None => return None,
        };
    }
}

/// [start, end) 里是否出现 targets 中任一字节。
const fn contains_any(bytes: &[u8], start: usize, end: usize, targets: &[u8]) -> bool {
    let mut index = start;
    loop {
        if index >= end {
            return false;
        }
        let byte = bytes[index];
        let mut offset = 0usize;
        loop {
            if offset >= targets.len() {
                break;
            }
            if targets[offset] == byte {
                return true;
            }
            offset = match offset.checked_add(1) {
                Some(value) => value,
                None => return true,
            };
        }
        index = match index.checked_add(1) {
            Some(value) => value,
            None => return false,
        };
    }
}

/// 把 [start, end) 两端的空白去掉，返回区间（可能为空）。
const fn trim_bytes(bytes: &[u8], start: usize, end: usize) -> Span {
    let mut left = start;
    loop {
        if left >= end {
            return Span {
                start: left,
                end: left,
            };
        }
        if is_ws(bytes[left]) {
            left = match left.checked_add(1) {
                Some(value) => value,
                None => return Span { start: left, end: left },
            };
        } else {
            break;
        }
    }
    let mut right = end;
    loop {
        if right <= left {
            return Span { start: left, end: left };
        }
        let last = match right.checked_sub(1) {
            Some(value) => value,
            None => return Span { start: left, end: left },
        };
        if is_ws(bytes[last]) {
            right = last;
        } else {
            break;
        }
    }
    Span { start: left, end: right }
}

/// 从 start 跳过空白（不超过 end）。
const fn skip_ws(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut index = start;
    loop {
        if index >= end {
            return index;
        }
        if is_ws(bytes[index]) {
            index = match index.checked_add(1) {
                Some(value) => value,
                None => return index,
            };
        } else {
            return index;
        }
    }
}

/// 是否空白字节（空格 / 制表 / 回车 / 换行）。
const fn is_ws(byte: u8) -> bool {
    matches!(byte, b' ' | 0x09 | 0x0D | 0x0A)
}

/// 行尾之后的下一个扫描位置（到文件尾 = 文件长度）。
const fn next_line(line_end: usize, len: usize) -> usize {
    if line_end < len {
        match line_end.checked_add(1) {
            Some(value) => value,
            None => len,
        }
    } else {
        len
    }
}

/// 别名 / 成员里不允许出现的字节（空白 + Rust 路径分隔符）。
const SEPARATORS: &[u8] = &[b' ', 0x09, 0x0D, 0x0A, b':', b'*', b'{', b'}', b',', b';'];

/// 嵌套花括号检测用（花括号内不允许再出现花括号）。
const BRACES: &[u8] = b"{}";

/// 路径段里不允许出现的字节。
const PATH_ILLEGAL: &[u8] = &[b'*', b'{', b'}', b',', b';', b' ', 0x09, 0x0D, 0x0A];

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "embed-source")]
    #[test]
    fn registry_lists_onboarded_module_files_in_path_order() {
        assert!(!source_files().is_empty(), "至少接入了伤害模块");
        let damage =
            source_of("src/families/character/damage.rs").expect("按 path 应能找到伤害模块源码");
        assert_eq!(
            damage.owner,
            SourceOwner::Module(&crate::families::character::damage::MODULE_NAME)
        );
        assert!(std::ptr::addr_eq(
            damage,
            &crate::families::character::damage::SOURCE
        ));
        assert!(
            source_files().windows(2).all(|pair| pair[0].path < pair[1].path),
            "SOURCE_FILES 必须按 path 升序登记"
        );
        // D242：具体卡牌的源码表在 cards crate，本表只登记 definitions 自己的模块文件。
        assert!(
            source_files()
                .iter()
                .all(|file| !file.path.starts_with("src/cards/")),
            "definitions 的源码表不应再登记具体卡牌"
        );
    }

    #[cfg(feature = "embed-source")]
    #[test]
    fn prefix_helper_filters_by_path_prefix() {
        let families: Vec<&'static SourceFile> = source_files_under("src/families/").collect();
        assert_eq!(families.len(), 1);
        // 具体卡牌已独立成 crate（D240 / D242），definitions 里不再有 src/cards/ 前缀的文件。
        let cards: Vec<&'static SourceFile> = source_files_under("src/cards/").collect();
        assert!(cards.is_empty(), "definitions 不再登记卡牌源码");
        assert!(source_files_under("src/不存在的目录/").next().is_none());
        assert_eq!(source_files_under("").count(), source_files().len());
    }

    #[cfg(feature = "embed-source")]
    #[test]
    fn module_owner_keeps_multilingual_name() {
        use crate::generated::{Lang, Resolved};
        let damage = source_of("src/families/character/damage.rs").expect("伤害模块源码");
        assert_eq!(damage.segments, ["families", "character", "damage"]);
        let name = match damage.owner {
            SourceOwner::Module(name) => name,
            SourceOwner::Card(_) => panic!("damage.rs 是模块，不是卡牌"),
        };
        assert_eq!(name.get(Lang::ZhCn), Resolved::Exact("角色伤害"));
        // 多语：另外 3 个语言槽位都填了（D216）。
        for lang in [Lang::EnUs, Lang::JaJp, Lang::KoKr] {
            assert!(!name.get(lang).is_missing(), "模块名缺语言：{lang:?}");
        }
    }

    #[cfg(not(feature = "embed-source"))]
    #[test]
    fn disabled_embed_means_empty_registry() {
        assert!(source_files().is_empty());
        assert!(source_of("src/families/character/damage.rs").is_none());
        assert!(source_files_under("src/").next().is_none());
        assert!(crate::families::character::damage::SOURCE.content.is_empty());
        assert!(crate::families::character::damage::SOURCE
            .dependencies()
            .is_empty());
    }

    /// 测试用源码（解析器夹具；不是真实文件）。
    const SAMPLE: &str = "use a::b;
use a::b as c;
use a::b::{c, d};
use broken::*;
";

    #[test]
    fn parses_single_alias_brace_and_keeps_unparsed_raw() {
        let deps: Vec<Dependency<'_>> = DependencyList::parse(SAMPLE).iter().collect();
        assert_eq!(deps.len(), 4);

        // 单条。
        assert_eq!(deps[0].kind(), DependencyKind::Single);
        assert_eq!(deps[0].path().collect::<Vec<_>>(), ["a", "b"]);
        assert!(deps[0].is_parsed());
        assert_eq!(deps[0].alias(), None);

        // 别名。
        assert_eq!(deps[1].kind(), DependencyKind::Alias);
        assert_eq!(deps[1].path().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(deps[1].alias(), Some("c"));
        assert!(deps[1].is_parsed());

        // 花括号：公共前缀 + 成员列表。
        assert_eq!(deps[2].kind(), DependencyKind::Brace);
        assert_eq!(deps[2].path().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(deps[2].members().collect::<Vec<_>>(), ["c", "d"]);
        assert!(deps[2].is_parsed());

        // 通配解析不了：保留原文、标未解析，不崩。
        assert_eq!(deps[3].kind(), DependencyKind::Unparsed);
        assert!(!deps[3].is_parsed());
        assert_eq!(deps[3].raw(), Some("use broken::*;"));
        assert_eq!(deps[3].path().count(), 0);
    }

    #[test]
    fn parses_multi_line_brace_import() {
        const MULTI: &str = "use x::{
    a,
    b,
};
";
        let deps: Vec<Dependency<'_>> = DependencyList::parse(MULTI).iter().collect();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].kind(), DependencyKind::Brace);
        assert_eq!(deps[0].path().collect::<Vec<_>>(), ["x"]);
        assert_eq!(deps[0].members().collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn unsupported_forms_are_unparsed_with_raw_text() {
        for source in [
            "use weird::{a::{b}, c};
",
            "use weird::a as b::c;
",
            "use weird::a::{b as c, d};
",
            "use weird::a::{b, c} as d;
",
        ] {
            let deps: Vec<Dependency<'_>> = DependencyList::parse(source).iter().collect();
            assert_eq!(deps.len(), 1, "整条语句应记为一条：{source:?}");
            assert_eq!(deps[0].kind(), DependencyKind::Unparsed, "{source:?}");
            assert!(
                deps[0].raw().is_some_and(|raw| raw.starts_with("use ")),
                "未解析必须保留原文：{source:?}"
            );
        }
    }

    #[test]
    fn non_use_lines_and_comments_are_ignored() {
        const SOURCE: &str = "// use fake::ignored;
fn main() {}
use real::path;
";
        let deps: Vec<Dependency<'_>> = DependencyList::parse(SOURCE).iter().collect();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].path().collect::<Vec<_>>(), ["real", "path"]);
    }
}
