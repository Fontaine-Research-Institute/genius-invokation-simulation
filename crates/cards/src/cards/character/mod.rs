//! 【角色牌】具体卡牌（每张卡一个自建目录，D163）。
//!
//! 定义载体仍在 `gi-genius-invokation-definitions` 的 `families/character/`（D121 / D133 / D162）：
//! `CharacterDef` / `SkillDef` / `SkillCost` / `SkillEffect` 在
//! `families::character::{definition, skill}`；本目录**只保留数据**（卡牌定义 + 效果逻辑占位）。
//!
//! 本文件**只做声明**：没有集中注册表（结构.md §4.4 规则 1），每张卡在自己的目录里。
//!
//! **版本约定（D245 / D246）**：每张卡都必须在自己的文件顶部声明
//! `gi_genius_invokation_definitions::card_game_version!(major, minor);`，
//! 数值 = 这张卡**已同步（核对）到的最新游戏版本**；
//! 新版本没调整这张卡时**只改这个数字**（如 6, 2 -> 6, 3），卡牌内容不动、同样算同步。
//!
//! 目录结构**完全平移**：原来的 `definitions/src/cards/character/mod.rs` 原样搬到这里。

pub mod aino;
