//! 【事件】**事件牌**的标签（一个标签一个文件）。
//!
//! D54：每个标签是零大小类型（无字段、编译后消失），一个标签一个文件；
//! **没有 `EventTag` 总枚举**。卡牌只能导入具体标签，如
//! `use crate::tags::action_card::event::food::Food;`。
//!
//! 本文件是事件标签的**公共模块位**：只做模块声明，不定义任何标签类型。
//!
//! 事件标签全集（来自游戏内文本描述）：料理 / 秘传 / 元素共鸣 / 希穆兰卡。
//! 「希穆兰卡」也被召唤物使用 —— 唯一定义在本处，召唤物导入这个文件。

pub mod arcane_legend;
pub mod food;
pub mod resonance;
pub mod simulanka;
