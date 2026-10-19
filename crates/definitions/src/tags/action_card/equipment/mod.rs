//! 【装备】**装备牌**的标签（一个标签一个文件）。
//!
//! D54：每个标签是零大小类型（无字段、编译后消失），一个标签一个文件；
//! **没有 `EquipmentTag` 总枚举**。卡牌只能导入具体标签，如
//! `use crate::tags::action_card::equipment::weapon::Weapon;`。
//!
//! 武器子类（单手剑 / 弓 / 双手剑 / 法器 / 长柄武器）也是标签、和「武器」多值同时出现；
//! 它们同时是角色的「武器类型」标签 —— 唯一定义在这里，角色导入这一份。
//!
//! 本文件是装备标签的**公共模块位**：只做模块声明，不定义任何标签类型。

pub mod artifact;
pub mod bow;
pub mod catalyst;
pub mod claymore;
pub mod hexerei;
pub mod polearm;
pub mod sword;
pub mod talent;
pub mod technique;
pub mod weapon;
