//! 卡牌源码表（D240 / D241 / D242 / D243）。
//!
//! **机制在 definitions（D242）**：宏 [gi_genius_invokation_definitions::card_source!] /
//! [gi_genius_invokation_definitions::module_source!]、[SourceFile] / [SourceOwner]、
//! use 依赖解析器都是**通用定义**，不搬；本模块只维护**具体卡牌的 SOURCE_FILES 清单**
//! （每张卡一行 `&SOURCE`），definitions 自己只登记它自己的模块文件。
//!
//! **path 口径不变**：卡牌源码的 path 仍以 `src/cards/...` 开头（目录结构与在 definitions
//! 里时完全一致）；`include_str!` 以**本 crate** 的 `CARGO_MANIFEST_DIR` 为根，解析到
//! `crates/cards/src/cards/...`，取到的就是卡牌文件原文。
//!
//! 手写清单的限制（与 definitions::source 相同）：新卡在文件里用 `card_source!` 声明后，
//! 还要在这里加一行；漏加不会编译失败，但不会被 [source_files] / [source_of] 找到。
//!
//! 本 crate 通过 Cargo 依赖显式打开 definitions 的 `embed-source`（见 Cargo.toml），
//! 所以卡牌原文**一定**被嵌入，这里的表不做 feature 开关。

use gi_genius_invokation_definitions::source::SourceFile;

/// 机制层类型再导出（测试 / 上层读取源码时用；宏本身仍在 definitions 里）。
pub use gi_genius_invokation_definitions::source::{DependencyKind, SourceOwner};

/// 已接入的**卡牌**源码文件清单（手写登记，按 path 字典序升序）。
///
/// 新增卡：文件里写 `gi_genius_invokation_definitions::card_source!(path: [...]);` 后，
/// 在这里加一行 `&crate::cards::<牌族>::<卡>::SOURCE`。
pub static SOURCE_FILES: &[&SourceFile] = &[&crate::cards::character::aino::SOURCE];

/// 全部已登记的卡牌源码文件（只读；顺序 = [SOURCE_FILES]）。
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::character::aino;

    #[test]
    fn registry_lists_card_files_in_path_order() {
        assert!(!source_files().is_empty(), "至少登记了爱诺卡");
        let aino_source =
            source_of("src/cards/character/aino/mod.rs").expect("按 path 应能找到爱诺源码");
        assert_eq!(aino_source.owner, SourceOwner::Card(aino::ID));
        assert_eq!(
            aino_source.segments,
            ["character", "aino"],
            "分段列表原样保留"
        );
        assert!(
            std::ptr::addr_eq(aino_source, &aino::SOURCE),
            "登记项应指向同一份静态 SOURCE，而不是拷贝"
        );
        assert!(
            source_files()
                .windows(2)
                .all(|pair| pair[0].path < pair[1].path),
            "SOURCE_FILES 必须按 path 升序登记"
        );
        // 本表只登记具体卡牌：path 都以 src/cards/ 开头。
        assert!(
            source_files()
                .iter()
                .all(|file| file.path.starts_with("src/cards/")),
            "卡牌源码表里的 path 必须仍以 src/cards/ 开头（结构不变）"
        );
    }

    #[test]
    fn registry_keeps_verbatim_file_content() {
        let aino_source =
            source_of("src/cards/character/aino/mod.rs").expect("按 path 应能找到爱诺源码");
        assert!(aino_source.content.starts_with("//! 爱诺（角色牌 1216）"));
        // 原文里应保留宏调用本身（证明是原文，没有被展开）。
        assert!(
            aino_source.content.contains(
                r#"gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);"#
            ),
            "原文里应保留宏调用"
        );
        // 与磁盘原文逐字节一致（仅测试期读盘对照；运行期零 IO）。
        let disk = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cards/character/aino/mod.rs"
        ))
        .expect("读卡牌文件失败");
        assert_eq!(aino_source.content, disk, "嵌入文本必须与文件原文一致");
        assert!(!aino_source.dependencies().is_empty(), "use 依赖应被解析");
    }

    #[test]
    fn prefix_helper_filters_by_path_prefix() {
        let cards: Vec<&'static SourceFile> = source_files_under("src/cards/").collect();
        assert_eq!(cards.len(), 1);
        // 本 crate 只登记卡牌：不存在其它目录前缀。
        assert!(source_files_under("src/families/").next().is_none());
        assert!(source_files_under("src/不存在的目录/").next().is_none());
        assert_eq!(source_files_under("").count(), source_files().len());
    }

    /// 依赖方向守卫（D240 / D241）：cards -> definitions 单向，definitions 不得反向依赖 cards。
    ///
    /// 只看**依赖声明行**（行首就是 crate 名），注释里提到 crate 名不算依赖。
    #[test]
    fn dependency_direction_is_cards_to_definitions() {
        fn depends_on(manifest: &str, crate_name: &str) -> bool {
            manifest
                .lines()
                .any(|line| line.trim_start().starts_with(crate_name))
        }

        let cards_manifest = include_str!("../../Cargo.toml");
        assert!(
            depends_on(cards_manifest, "gi-genius-invokation-definitions"),
            "cards 必须依赖 definitions"
        );
        let definitions_manifest = include_str!("../../../definitions/Cargo.toml");
        assert!(
            !depends_on(definitions_manifest, "gi-genius-invokation-cards"),
            "definitions 不得依赖 cards（会形成反向依赖）"
        );
    }
}
