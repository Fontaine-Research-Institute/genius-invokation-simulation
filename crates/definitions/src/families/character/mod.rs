//! 【角色牌】牌族层：**没有 L2 类型枚举**（D51）；这里是角色牌的**通用类型 / 定义载体 / 底层操作**。
//!
//! 角色牌 = 一个牌族 + 若干标签，没有 L2 类型这一级；
//! 标签（元素 / 国家·势力 / 始基力 / 其他武器）在顶层 `tags/character/`，本目录不写标签。
//!
//! 范围口径：D121 的角色通用层清单，被 **D133**（只做底层数值操作 + 基础类型，不做高层语义封装）
//! 与 **D138**（不做「布尔获取充能」，调用方自己操作）收窄。
//!
//! 文件：
//! - `types.rs`：基础类型（`SkillKind` / `SpecialNormalAttack` / `DamageType`）；
//! - `skill.rs`：技能定义载体（`SkillCost` / `SkillEffect` / `SkillDef`，D162 迁入）；
//! - `definition.rs`：角色牌定义载体（`CharacterDef<Tags>`，D162 迁入）；
//! - `character.rs`：`CharacterState` + 生命 / 充能的加 / 减 / 设 / 读 + 上限读 + `from_def`；
//! - `energy.rs`：充能**数值修正**（获得量叠加；不做「充能变动」时机键，Lead 拍板）；
//! - `form.rs`：**转换形态**（覆盖报告 R4）：形态 = 名称 / 元素 / 标签 / 技能列表的替换 +
//!   `CharacterTiming::FormTransformed` 派发（纯函数 `transform_to`）；
//! - `growth.rs`：**角色成长**（覆盖报告 R4）：通用成长条目（属性 + 增量）+ 叠加 / 读取；
//! - `skill_flow/`：技能使用流程（能否使用 / 支付 / 使用计数 / 目标候选集 / 时机派发 / 准备技能；
//!   D184 第 2 步；覆盖报告 R1 缺口；R5 状态禁用查询在 `forbidden`、特技流程在 `technique`）；
//! - `satiated.rs`：**饱腹**状态身份 + 料理生效后赋予 / 回合结束与击倒解除（覆盖报告 L1 / S8.1）；
//! - `trigger.rs`：角色族的时机键（生命值变动 / 治疗后 / 被击倒 / 免于被击倒 / 使用技能时·后）
//!   + 触发式数值操作（D185）；
//! - `damage.rs`：底层伤害入口（D164 / D167：扣血 + 元素附着 + 反应判定 / 消耗参与元素 / 触发注册；
//!   不选目标、不做增减伤、不实现具体反应）；
//! - `attachment.rs`：元素附着（D167 布尔集合、可多元素共存、无计量）+ 元素反应注册接口（D127 / D128–D134 / D136）。
//!
//! **不做**（D133 / D138 / D164）：布尔获取充能、增减伤 / 减免结算、高层「对出战 / 后台 n 个造成伤害」封装、
//! 目标选择策略、**具体反应效果**（damage.rs 只负责判定、消耗参与元素、触发注册，效果由注册方给）。
//!
//! 准备技能（D165 / D166）：本轮用**普通函数接口**实现（`skill_flow/prepared.rs`：开始 / 完成 /
//! 打断 / 清除），**不写宏**；将来宏化时替换实现、保留接口，且宏不归角色模块（D166）。

pub mod attachment;
// 文件名与目录同名（families/character/character.rs）是既定布局；
// clippy 的 module_inception 在这里是无害的，显式豁免。
#[allow(clippy::module_inception)]
pub mod character;
pub mod damage;
/// 伤害修正链 + 反应效果执行器（覆盖报告 R3 / R4 的**执行侧**：加伤 / 护盾 / 减伤 + 反应效果执行；
/// 顺序以 `结算顺序.md` §三 为唯一权威，D196 插入式结算）。
pub mod damage_chain;
pub mod definition;
/// 充能的**数值修正**接口（获得量叠加；不做「充能变动」时机键，Lead 拍板）。
pub mod energy;
/// **转换形态**（覆盖报告 R4）：形态抽象 + 纯转换 + `CharacterTiming::FormTransformed` 派发。
pub mod form;
/// **角色成长**（覆盖报告 R4）：通用成长条目（属性 + 增量）+ 叠加 / 读取。
pub mod growth;
pub mod satiated;
pub mod skill;
pub mod skill_flow;
/// 角色族的第一份真实时机键 + 触发式数值操作（D185：时机键由角色族自己定义）。
pub mod trigger;
pub mod types;
/// 区域承载（D182 / D183 / D185 / D187）：角色区 / 出战状态区 / 装备栏。
pub mod zones;
