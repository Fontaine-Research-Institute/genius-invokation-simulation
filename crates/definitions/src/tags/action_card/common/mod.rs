//! 行动牌的**跨类型公共标签**（与具体类型无关，三种行动牌都可能带）。
//!
//! D54：每个标签是零大小类型（无字段、编译后消失），一个标签一个文件；
//! **没有 `ActionCardCommonTag` 总枚举**。卡牌只能导入具体标签，如
//! `use crate::tags::action_card::common::undiscoverable::Undiscoverable;`。
//!
//! 用户拍板：家庭层级的公共标签放这里，需要的人**直接导入这一份**，不在自己的类型目录重抄；
//! 召唤物也使用其中几个（不可发现 / 减伤 / 希穆兰卡按唯一定义就近放，见各文件说明）。
//!
//! 本文件是公共标签的**公共模块位**：只做模块声明，不定义任何标签类型。

pub mod automaton_card;
pub mod combat_action;
pub mod damage_reduction;
pub mod scrapped;
pub mod shield;
pub mod undiscoverable;
