//! gi-genius-invokation-definitions：游戏定义层（框架 → 卡牌资源与形状）。
//!
//! 依赖 gi-genius-invokation-engine（机制原语）；本 crate 默认零外部依赖。
//!
//! **层级归属（一层一层枚举，逐层向上，D51 / D54）**：
//! - **base**：定义 L1 **牌族枚举 CardFamily**（也是 id 区域），以及 CardId / SkillId / Element；
//! - **families**：每个牌族定义自己的 **类型枚举** —— 行动牌 ActionCardType
//!   { Support, Equipment, Event }、状态 StatusType { CharacterStatus, CombatStatus }；
//!   其余牌族没有类型；
//! - **tags**：与 families 同级的顶层目录，**标签**按 牌族 / 类型 分组；
//!   每个标签是一个**零大小类型** + 一个文件（D54），**没有"总标签枚举"**；
//!   没有类型的牌族（如角色）标签直接挂 `tags/<牌族>/`。
//!
//! **标签不是数字、也不是字符串**：标签就是它自己那个类型（编译后消失）；
//! 卡牌只能导入具体标签（`tags::action_card::support::location::Location`），导不到上层大集合。
//! 卡牌的标签集合形态（元组 / Vec / 位图）本轮未定，等标签定义块再定。
//!
//! 本块只包含 base、families（类型枚举）与 tags（标签类型）；resources / timeline /
//! loader 按 结构.md §4 的顺序后续加入。
//!
//! **具体卡牌不在本 crate（D240 / D241 / D243）**：本 crate 只定义「卡牌长什么样、有哪些资源」
//! （CardId / CardFamily / 类型 / 标签 / CostComponent / CharacterDef 等定义载体）；
//! 「这张卡具体是什么」在独立的 gi-genius-invokation-cards crate（可选接入，simulation 默认不含）。
//! 规格入口：分工计划.md §4.4.0.0（三级模型）、结构.md §4.1（base 目录与职责）、决策日志 D51 / D54。

// §8.3：definitions 里禁止手写裸 + - * /（第二道保险；第一道是 tools/check_bare_int.py）。
#![deny(clippy::arithmetic_side_effects)]

pub mod base;
pub mod families;
pub mod flow;
pub mod generated;
pub mod keywords;
pub mod reactions;
pub mod records;
pub mod resources;
pub mod rules;
pub mod source;
pub mod state;
pub mod tags;
pub mod timeline;
pub mod version;
