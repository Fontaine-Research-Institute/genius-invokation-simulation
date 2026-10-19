//! 角色（与敌人）的**元素标签**（一个标签一个文件）。
//!
//! D54：每个标签是零大小类型 + 一个文件；没有总枚举。
//! 元素标签全集（游戏内文本描述）：火 / 水 / 雷 / 草 / 冰 / 风 / 岩。
//! 敌人牌也使用这些标签，导入这一份。

pub mod anemo;
pub mod cryo;
pub mod dendro;
pub mod electro;
pub mod geo;
pub mod hydro;
pub mod pyro;
