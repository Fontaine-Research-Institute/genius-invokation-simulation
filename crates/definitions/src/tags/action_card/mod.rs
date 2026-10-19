//! 行动牌的标签（按**类型**分组；D54：一个标签一个零大小类型 + 一个文件）。
//!
//! 行动牌有三个类型（`ActionCardType`：支援 / 装备 / 事件），标签属于具体类型：
//! - `support`：场地 / 伙伴 / 道具 / 元素幻变 / 冒险地点 / 未变身；
//! - `equipment`：武器 + 五个武器子类 / 圣遗物 / 天赋 / 特技 / 魔导；
//! - `event`：料理 / 秘传 / 元素共鸣 / 希穆兰卡；
//! - `common`：**跨类型公共标签**（家庭层级那份）—— 不可发现 / 弃用 / 自走牌 / 战斗行动 / 护盾 / 减伤。
//!
//! **没有 `SupportTag` / `EquipmentTag` / `EventTag` / `ActionCardCommonTag` 这类总枚举**：
//! 卡牌只能导入具体标签，如 `use crate::tags::action_card::support::location::Location;`。
//! 唯一定义：武器子类也被角色使用，角色导入这里这一份；召唤物需要的公共标签导入这里这一份。

pub mod common;
pub mod equipment;
pub mod event;
pub mod support;
