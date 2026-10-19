//! 基础通用数据。
//!
//! **层级归属（逐层向上；本目录只有最顶层）**：
//! - **base**（本目录）：**牌族枚举 CardFamily**（也是 id 区域），加上 CardId / SkillId / Element、
//!   **视角方 [`Perspective`]**（敌我两个具名实例的「视角绑定」，不是编号；state 层也要用它，
//!   所以放最底层；`keywords::target` 重导出它，旧路径不变）；
//! - **families**（与 tags 同级）：只放**类型枚举** —— 行动牌 ActionCardType、状态 StatusType；
//!   其余牌族没有类型；
//! - **tags**：**独立顶层目录，与 families 同级**；每个标签**一个文件、一个零大小类型**
//!   （不是枚举、不是数字、不是字符串），路径为 tags/<family>/<type>/<tag>.rs。
//!
//! **标签不在 base 定义、也不用数字编号**（决策日志 D51 / D53 / D54）。
//! 没有「标签大全枚举」这东西：标签集合是若干具体标签类型的元组 / 集合。
//!
//! **唯一性口径**：CardId 只是一个数字，在**各自牌族的 id 区域内**唯一；
//! 跨牌族**不保证**不重复，所以筛选 / 查找一律用 (CardFamily, CardId)。
//! 按名字找卡只属于用户层（UI / 搜索框），游戏逻辑里不出现"名字 → 卡"的查找。

pub mod element;
pub mod family;
pub mod id;
pub mod perspective;

pub use element::{Element, ELEMENTS};
pub use family::CardFamily;
pub use id::{CardId, SkillId};
pub use perspective::Perspective;
