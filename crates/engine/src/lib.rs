//! `gi-genius-invokation-engine`：机制原语 crate。
//!
//! 契约：
//! - **默认零依赖**（只有显式打开 feature `dll` 时才引 `libloading`）；
//! - **不含任何上层语义**：只有数值 / 标识 / 实体 / 区 / 随机与选择这些机制原语；
//! - 上层（`definitions` / `solver`）依赖本 crate，本 crate 不依赖任何业务 crate；
//! - **概率与随机全部是精确整数**（D159：crate 内不出现浮点）：概率质量用 `ExactNat128`
//!   与 [`prob::Fraction`]，随机数用 128 位整数（D160）。
//!
//! 模块：
//! - [`nat`]：两族数值——**饱和族** `Nat`（默认 `Nat = Nat32`；另有 `Nat8/16/64/128`，
//!   溢出停在 `MAX` / `ZERO`）与**精确族** `ExactNat`（默认 `ExactNat = ExactNat128`，
//!   加/减/乘上溢、下溢都 panic，D153）。`nadd!` / `nsub!` / `nmul!` / `ndiv!` 宏两族共用：
//!   传 `Nat` 就是饱和、传 `ExactNat` 就是精确（D155）；
//! - [`id`]：`EntityId` / `ActionId`（**引擎不分敌我，也没有区编号/区表**：只认实体；
//!   「谁」由定义层的**两个具名实例**表达，不是编号/下标，D34）；
//! - `entity` / `area` / `select` / `random`：实体表 / 区 / 选择 / 随机与去重；
//!   选择与随机的概率都是精确分数 [`prob::Fraction`]（D158）；
//! - [`prob`]：分数 [`prob::Fraction`]（分子 / 分母 `ExactNat128`，分母 0 panic）——
//!   加 / 乘 / 除、归一化、按分数占比落单（128 位整数区间映射，D160）；
//! - [`rng`]：真实随机数 [`rng::Rng`]（SplitMix64；对外只给 `next_u128` /
//!   `next_below_u128`，D160；枚举不消费它、只有采样才消费，D149）；
//! - [`rules`]：求解接口（**仅 `feature = "rules"` 时编译**，决策日志 D24）；
//! - `load`：机制 dll 加载（**仅 `feature = "dll"` 时编译**）。

pub mod nat;

pub mod id;

pub mod entity;

pub mod area;

pub mod select;

pub mod random;

pub mod prob;

pub mod rng;

pub mod stats;

#[cfg(feature = "rules")]
pub mod rules;

#[cfg(feature = "dll")]
pub mod load;

pub use area::{Area, SingleArea};
pub use entity::EntityTable;
pub use id::{ActionId, EntityId};
pub use nat::Nat;
pub use random::{enumerate_random, DedupFn};
pub use select::{enumerate_select, SelectionSource};
