//! 【支援】**支援牌**的标签（一个标签一个文件）。
//!
//! D54：每个标签是零大小类型（无字段、编译后消失），一个标签一个文件；
//! **没有 `SupportTag` 总枚举**。卡牌只能导入具体标签，如
//! `use crate::tags::action_card::support::location::Location;`，导不到"支援的标签集合"。
//!
//! 本文件是支援标签的**公共模块位**：只做模块声明，不定义任何标签类型，
//! 也不会被卡牌直接导入（卡牌只导入叶子）。
//!
//! 支援标签全集（来自游戏内文本描述）：场地 / 伙伴 / 道具 / 元素幻变 / 冒险地点 / 未变身。

pub mod adventure_spot;
pub mod companion;
pub mod elemental_transmutation;
pub mod item;
pub mod location;
pub mod untransformed;
