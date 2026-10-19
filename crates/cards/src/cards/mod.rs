//! 具体卡牌（`cards/`）：**每张卡自己的识别信息 + 效果逻辑**（结构.md §4.4；D240 起在本 crate）。
//!
//! 与 `gi-genius-invokation-definitions` 的 `families/` 分工：
//! - `families/`（definitions）：牌族的**类型与通用操作**（结构、数值、区域状态）；
//! - `cards/`（本 crate）：**落到具体某张卡**的数据与效果。
//!
//! 本文件**只做声明 / 导出**：
//! - **没有集中注册表**（结构.md §4.4 规则 1：将来每张卡在自己文件里自注册）；
//! - 第一张样板卡（爱诺）**先不注册**，只导出定义常量，等注册机制（缺，P3）建好再接。
//!
//! 目录与 definitions 的 `families/` 对齐：没有 L2 类型的牌族（角色 / 召唤物 / 状态 / 敌人）
//! 直接铺；有类型的行动牌按 支援 / 装备 / 事件 分层。本轮只建角色牌。
//!
//! 目录结构**完全平移自** definitions（用户口径「结构还这么放」）：本文件就是原来的
//! `definitions/src/cards/mod.rs`，只是 crate 根变成了 `crates/cards/`。

pub mod character;
