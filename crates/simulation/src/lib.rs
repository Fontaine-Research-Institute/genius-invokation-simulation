//! `gi-genius-invokation-simulation`：本工作区的**对外唯一入口**（门面 crate）。
//!
//! 本 crate **只有 re-export，没有逻辑**：
//! - `engine`：机制原语（饱和数值 / 实体 / 区 / 随机与选择）；
//! - `definitions`：游戏定义层（基础通用数据 / 牌族 / 类型 / 标签 / 卡牌**资源与形状**）；
//!   **不含**具体卡牌（D240 / D241 / D243）；
//! - `richtext`：富文本解析（只做字符串处理，零游戏语义）；
//! - `cards`：具体卡牌（每张卡的数据 + 效果逻辑），**可选** feature `cards`（默认关闭，D243）。
//!
//! 依赖方向由 Cargo 强制：definitions 依赖 engine 与 richtext；cards 依赖 definitions 与 engine；
//! 具体卡牌**不在默认依赖图里**（用户口径「整个模拟 crate 默认不含具体 cards」），
//! 需要时显式打开 `--features cards`。
//! 外部使用者只需依赖本 crate，不必直接依赖内部 crate；跨 crate 的导入路径统一是
//! `gi_genius_invokation_simulation::{engine, definitions, richtext}`（打开 feature 后还有 `cards`）。

pub use gi_genius_invokation_engine as engine;
pub use gi_genius_invokation_definitions as definitions;
pub use gi_genius_invokation_richtext as richtext;

/// 具体卡牌（D240 / D241）：**可选**，默认不编译、不进入依赖图（D243）。
#[cfg(feature = "cards")]
pub use gi_genius_invokation_cards as cards;
