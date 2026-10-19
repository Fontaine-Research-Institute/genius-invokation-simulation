//! 【角色牌】**伤害修正链 + 反应效果执行器**（覆盖报告 R3 / R4 缺口的执行侧；影响 125 实体）。
//!
//! 本目录补的是两件「触发点已经存在、但没人执行」的事：
//! 1. **伤害修正链**：把 `结算顺序.md` §三 第 1 行「造成伤害」的插入点接进
//!    `families::character::damage::damage` 之前 —— 伤害免疫（归 0，穿透不吃）->
//!    加伤（增伤 / 元素精通 / 反应加伤）-> 乘算 / 除算（分工计划 §4.4.5）-> 护盾抵消 ->
//!    减伤扣减 -> 扣血；**穿透伤害全部不吃**；
//! 2. **反应效果执行器**：把 `reactions::effect::EffectOp` 的 6 个变体分别接到
//!    角色区 / 出战状态区 / 召唤物区 / 手牌区 / 手牌卡状态表上，按 `table::resolve`
//!    的分支**逐支**执行（每支带 `Fraction` 精确概率，**不采样**，D190）。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` + 卡面 + 关键字；顺序只认 `结算顺序.md`）
//!
//! - 规则书·特殊词条 **护盾**：「『护盾』所保护的角色受到伤害时，将会消耗『护盾值』来抵消伤害。」
//! - 规则书·特殊词条 **穿透伤害**：「『穿透伤害』不会受到其他效果的伤害加成，但是也不会被护盾等
//!   效果抵消。」
//! - 规则书·**元素精通**：「提高元素精通可增强触发元素反应时造成的伤害，以及元素反应的效果。
//!   例如：每提高1点我方元素精通，我方结晶反应提供的护盾便会提高3层。」
//! - 规则书·其他规则 **结算顺序**；`结算顺序.md` §一（同一时机内）/ §二（插入式结算）/
//!   §三（常见插入点）/ §五（debug 快照）。
//!
//! ## 四个文件
//!
//! | 文件 | 职责 |
//! |---|---|
//! | `bonus.rs` | 加伤 + **乘算 / 除算**（`AmountOp` 四段模型：加伤 -> 乘数区 -> 除数 -> 区外加减）；**只读预测**本次反应（与 `damage()` 同口径） |
//! | `chain.rs` | 伤修链：免疫（归 0，穿透不吃）-> 加伤 -> 乘算 -> 盾 -> 减伤 -> 扣血；D196 子结算帧 |
//! | `statuses.rs` | 反应会附属的状态 / 手牌状态的**身份静态表**（卡 id -> 状态身份） |
//! | `executor.rs` | 反应效果执行器：`EffectOp` -> 各区操作；分支概率不采样 |
//! | `record.rs` | **记录内容类型** `DamageChainRecord`（D225）+ `ShowRecord`；统一记录器接线（D223 / D224） |
//!
//! ## 记录（D221 / D223 / D224 / D225）
//!
//! 记录走公用的 [`crate::records::Recorder`]（只提供「属于哪一步 + 任意内容」），内容类型是本目录的
//! [`record::DamageChainRecord`]（单独文件，D225）。**在干活处直接 push 真实数值**：
//! 加伤各项 / 合计、护盾可吸收与实际吸收、减伤各条与合计、穿透影响、最终扣血、命中的反应 ——
//! **不靠前后局面 diff 推导**（diff 看不到这些字段，keywords::trace 里 `shield_used` / `bonus`
//! 恒 0 就是这个原因）。
//!
//! - 普通入口（[`chain::settle`] / [`damage::damage_with_chain`]）**不记录**（热路径不多分配）；
//! - 记录版入口：[`chain::settle_records`] / [`chain::settle_recorded`] /
//!   [`damage::damage_with_chain_recorded`]（多一个 `&mut RecordedStep` 参数，旧入口签名不变）；
//! - 链内阶段步号会被**重盖**成调用方给的结算步号（一次伤害属于同一个结算步）；
//! - 关闭 `records` feature 时记录器是零大小类型、push 空操作，链里连记录内容都不构造（零开销）。
//!
//! ## 口径
//!
//! - **区自持状态、对外只读指针**（D183）：链只借 `&CombatStatusZone` / `&StateList`
//!   读护盾 / 减伤，不新建第二份承载；执行器按值收进 / 交出各区（各区自己是纯函数风格）；
//! - **插入式结算**（D196）：链 = `timeline::Resolution`（帧 + cursor），可以继续
//!   `insert` 更小的子结算；反应效果 = `damage()` 返回「命中反应」之后的插入段；
//! - **纯函数优先**：所有入口吃 `&`、返回新值；失败用 `Option` / `Skip`
//!   （不改状态、不 panic）；
//! - **数值**都是 `Nat`（饱和）；概率都是 `Fraction`（分子分母 `ExactNat128`）；
//!   全目录**零浮点、零 `HashMap`**；
//! - **既有判定 / 附着语义一行不改**：`damage()` 只在 `damage_with_chain` 里被**调用**
//!   （扣血 / 附着 / 反应判定 / 消耗参与元素 / 触发注册全是原来那份代码）。
//!
//! ## 待接入 / 待复核（汇总）
//!
//! - 护盾 / 减伤的**层数扣减写回**：出战状态区 / 状态列表都没有「减层」入口，链先只给数值
//!   （`to_hp` / `shield_used`）；写回归区域块（`待接入`）；
//! - `statuses.rs`：5 张状态卡已接 `families::status::known` 的正式定义；手牌状态（电击 / 费用降低）
//!   仍待行动牌状态块落地（见 statuses.rs 文档）；「伤害免疫」只有共享身份（无正式卡牌定义，
//!   见 statuses.rs 文档）；
//! - **记录内容的下游消费**：keywords::trace / flow 目前仍按前后局面 diff 推导伤害结果；本目录已能在
//!   **计算处**给出真实值（`damage_with_chain_recorded`），**待接入**到那些调用点
//!   （本轮写范围不含 keywords / flow）；
//! - 反应效果执行器的 `uses` / `stack_limit` 字段暂被忽略（区里没有对应字段）；
//! - 强制切换的「下一个角色」按槽位顺序取下一个（规则书未定义，`待实测`）；
//! - 元素精通的**每点伤害数值**未知（`MASTERY_DAMAGE_PER_POINT` 暂按 +1，`待实测`）；
//! - 反应效果造成的伤害**是否再引发反应**（本层按 `damage()` 原语义：会）—— `待复核`；
//! - **伤害免疫**：规则书无该词条（依据 = K5 + 卡面「免疫所有伤害」）；「免疫在加伤之前」
//!   「免疫时是否仍附着 / 反应」-> `待实测`；具体护罩状态的正式定义归状态块；
//! - **乘算 / 除算**：倍数区多个乘数「相加」、取整方向（默认 `Down`）、免疫与乘算先后
//!   -> `待实测`；卡面「翻倍」的具体操作仍由具体卡牌在效果里声明。

pub mod bonus;
pub mod chain;
pub mod executor;
pub mod record;
pub mod statuses;

pub use bonus::{
    predict_reaction, AmountOp, DamageBonus, DamageModifiers, MulMode, MultiplierZone, ReactionHit,
    Rounding, ScaledDamage, MASTERY_DAMAGE_PER_POINT, MASTERY_SHIELD_PER_POINT,
};
pub use chain::{
    schedule, schedule_records, settle, settle_recorded, settle_records, ChainEffect, ChainOutcome,
    ChainStage, ChainWork, DamageChainEnv, DamageInput, ScheduledChain,
};
pub use executor::{
    capped_stacks, execute_reaction, next_character, scope_side, BranchExecution, DamageEntry,
    EntityAlloc, ExecutionEnv, OpRecord, OpResult, ReactionExecution, ReactionExecutor,
    ReactionPending, ReactionTriggers, ReactionZones, SidePending, SideTriggers, SideZones, Skip,
    StatusPlace, CHARACTER_STATE_POS,
};
pub use record::{
    damage_type_text, element_text, write_records, DamageChainRecord, DamageChainRecorder,
    RecordedStep, STAGE_BONUS, STAGE_IMMUNITY, STAGE_REDUCTION, STAGE_SCALE, STAGE_SHIELD,
    STAGE_TO_HP,
};
pub use statuses::{
    hand_status_identity, identity_in, same_identity, status_of_card, GameStatus, StatusEntity,
    DAMAGE_IMMUNITY, DAMAGE_NEGATION, IMMUNITY_STATUSES, REDUCTION_STATUSES, SHIELD_STATUSES,
};
