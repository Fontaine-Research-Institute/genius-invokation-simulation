//! 【角色牌】**区域承载**：角色区 / 出战状态区 / 装备栏（覆盖报告 R3「区域/状态承载」缺口）。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` 游戏区域 I / II + takeaction 战斗行动）
//!
//! - I. 角色区：角色牌放置、出战角色（每方一名）、后台角色与「切换角色」、被击倒后强制补位；
//! - II. 角色附属：装备牌（每角色 1 武器 / 1 圣遗物 / 1 特技 + 任意其他装备）、状态、出战状态
//!   （附属在出战角色上，切换时转移给新出战角色）。
//! - 战斗行动【切换角色】：支付 1 个任意元素骰，把后台角色切为出战角色（**费用支付不在本层**）。
//!
//! ## 三个区（每个区**自持状态**、对外**只读指针**、可建多个、变量型：D182 / D183）
//!
//! | 区 | 文件 | 上限 / 关键规则 |
//! |---|---|---|
//! | 角色区 | `field.rs` | 3 槽位；当前出战角色；切出 / 切入时机 |
//! | 出战状态区 | `combat_status.rs` | 附在出战角色上；切换时**转移**；持续回合走 `rules::ops::lifetime` |
//! | 装备栏 | `equipment.rs` | 武器 / 圣遗物 / 特技各 1；天赋同种覆盖；替换时旧装备弃置 |
//!
//! ## 时机派发（D185 / D186）
//!
//! - 每个区**自己定义时机键**（`CharacterZoneTiming` / `CombatStatusTiming` / `EquipmentTiming`），
//!   通用层 `timeline` 不认识它们；
//! - 每个区的操作在发生时**主动派发**，走 `timeline::dispatch`（按结算顺序排好、**不执行**），
//!   返回待执行列表由调用方执行（`execute_character_zone` / `execute_combat_status` / `execute_equipment`）。
//!
//! ## 统计（D187）
//!
//! 三个区的统计都下沉到 `stats/` 子模块；feature `stats` 关闭时零大小空结构、更新全空操作，
//! 统计不参与局面等价。对外只给 `stats()` 只读指针（D183）。

pub mod combat_status;
pub mod equipment;
pub mod field;
pub mod record;
pub mod stats;

pub use combat_status::{
    execute_combat_status, CombatStatusEffect, CombatStatusEntry, CombatStatusPending,
    CombatStatusResolution, CombatStatusTiming, CombatStatusTrigger, CombatStatusTriggers,
    CombatStatusZone,
};
pub use equipment::{
    execute_equipment, EquipmentEffect, EquipmentEntry, EquipmentOutcome, EquipmentPending,
    EquipmentResolution, EquipmentTiming, EquipmentTrigger, EquipmentTriggers, EquipmentZone,
};
pub use field::{
    execute_character_zone, CharacterSlot, CharacterZone, CharacterZoneEffect, CharacterZonePending,
    CharacterZoneResolution, CharacterZoneTiming, CharacterZoneTrigger, CharacterZoneTriggers,
    CHARACTER_SLOTS,
};
