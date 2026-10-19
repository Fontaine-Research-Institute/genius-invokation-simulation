//! 统一基础操作：**三级共用**的纯函数层。
//!
//! 设计口径（D20「框架只给接口」）：
//! - 本层**不持有状态、不新建容器**：区域 / 槽位 / 骰池的实例状态由各区域块持有，
//!   这里只提供「判定 + 结果」的纯函数，供它们调用；
//! - 计数值一律 `Nat`，不使用浮点；随机只给「范围 / 集合」，抽样走引擎随机接口；
//! - 每个文件对应一组机制；`rules/mod.rs` 的静态表用 `OpeningSpec` / `SlotRule` /
//!   `DamageSpec` 等把标签、类型、牌族关联到这里的操作（三级同一套接口）。

pub mod adventure;
pub mod arcane;
pub mod arkhe;
pub mod damage;
pub mod equip;
pub mod food;
pub mod lifetime;
pub mod opening;
pub mod order;
pub mod query;
pub mod resonance;
pub mod simulanka;
pub mod stage;
pub mod support;
