//! **关键字函数 + 一步**：统一的效果组合层（D204 / D205 / D206 / D207 / D208 / D209）。
//!
//! ## 这一层解决什么
//!
//! 卡面写法是「一次效果里按顺序做几件事」，例如菲林斯元素爆发：
//!
//! ```text
//! step([消耗充能(自己, 全部),
//!       造成伤害(目标: 敌方出战角色, 类型: 雷元素, 数值: 6, 属性: 元素爆发),
//!       造成伤害(目标: 所有敌方后台角色, 类型: 穿透, 数值: 2, 属性: 元素爆发)])
//! ```
//!
//! 用户口径（原文）：「角色这种比较复杂的，我们按照关键字写对应的函数，包括之前的我们也这么
//! 处理一下，比如：一步([造成伤害(目标:get_敌方出战角色, 伤害类型:枚举伤害类型(七元素/物理/穿透),
//! 伤害属性(...)), 造成伤害(目标:get_敌方出战角色(下)(1)个角色, …)])（一步是用来定义显示顺序的，
//! 一步中进行的视为一步）」。
//!
//! ## 三块
//!
//! | 模块 | 内容 |
//! |---|---|
//! | [target] | 目标选择器：把「我方 / 敌方 / 自己 / 出战 / 后台 / (下)(n) / 生命最低…」解析成具体实体集合（不采样、不做选择策略） |
//! | [keyword] | 关键字函数：每个关键字一个纯函数，内部调用已实现的底层操作（伤害链 / 各区 *_with_triggers / 骰池 / 抽牌） |
//! | [step] | 一步：步内按顺序、**禁止插入**；触发只在步末派发；步与步之间才能插入（D204） |
//!
//! 另有 [op]（操作 / 参数 / 返回类型）、[effect]（执行上下文 EffectEnv）、[migrate]（旧写法桥接）。
//!
//! ## 硬约束（Lead 拍板，逐条）
//!
//! - **D204**：一步原子；步内按书写顺序执行；步内禁止插入（触发 / 其它效果 / 子结算都不行）；
//!   插入只能发生在步与步之间；一步同时是显示单位和不可中断的原子结算单位；
//! - **D205**：目标选择器包含「自己」，与「我方 / 敌方」并列；「弃置」是关键字函数
//!   （支援区弃置自己 = 弃置(Subject)）；自指操作同样用 Subject；
//! - **D206**：自己 / 我方 / 敌方都由**执行上下文的函数**解析，写效果不手写实体号；
//!   关键字函数签名不出现「让你传 owner / entity id」的参数；
//! - **D207**：菲林斯爆发 = 一步（消耗充能 + 两次伤害，含多目标多实例）；步末才派发触发；
//! - **D208**：召唤物多次执行 / 沙中遗事逐张置底 / 月兆两次赋予都包装成一步；
//! - **D209**：通用「重复(n, 序号 -> 关键字)」包装；顺序可注入（OrderFn），不写死。
//!
//! ## 记录规范（D225）
//!
//! 每个模块的**记录内容类型**（实现 [crate::records::ShowRecord] 的类型）单独一个文件，
//! 放在该模块目录下：关键字的记录内容在 [record]（[record::KeywordRecord]）；
//! 记录器实例化与 push 调用留在业务处（谁干活谁记录），不混进业务实现文件。
//!
//! ## 依据
//!
//! 规则书（进行行动 / 游戏区域 / 特殊词条 / 元素反应）、结算顺序.md §二 / §三；
//! D190 / D191（惰性不确定 + 观测展开 + 折叠回）、D196（插入式结算）、
//! D182 / D183（区自持、对外只读指针）、D34（敌我两个具名实例，不编号）。

pub mod effect;
pub mod keyword;
pub mod migrate;
pub mod op;
pub mod reaction;
pub mod record;
pub mod step;
pub mod target;
pub mod trace;

pub use effect::{EffectEnv, TriggerBook};
pub use keyword::{
    apply_card_status, apply_status, attach_element, cause_damage, consume_energy, convert_dice,
    create_card, discard, draw_cards, force_switch, gain_dice, heal, put_hand_to_pile_bottom,
    summon, switch_character,
};
pub use op::{
    AttachElementArgs, CardStatusArgs, ConsumeEnergyArgs, ConvertDiceArgs, CreateCardArgs,
    DamageArgs, DamageSourceKind, DiscardArgs, DrawCardsArgs, DrawCount, EnergyAmount,
    ForceSwitchArgs, GainDiceArgs, HealArgs, KeywordBranch, KeywordOp, KeywordResult,
    OrderFn, PutHandToPileBottomArgs, RepeatArgs, RepeatOpFn, StatusArgs, StatusPlace, SummonArgs,
    SwitchCharacterArgs, TargetPick,
};
pub use step::{
    execute_step_me, execute_step_opp, execute_steps_me, execute_steps_opp, Step, StepOutcome,
};
pub use target::{
    player_of, resolve, Perspective, ResolveContext, SideScope, TargetSelector, TargetSet,
};
pub use reaction::{
    settle_inserted, settle_reaction, settle_reactions, ReactionFrames, ReactionRequest,
};
pub use record::{
    execute_step_me_recorded, execute_step_opp_recorded, execute_steps_me_recorded,
    execute_steps_opp_recorded, record_trace, KeywordRecord,
};
pub use trace::{
    execute_step_me_traced, execute_step_opp_traced, execute_steps_me_traced,
    execute_steps_opp_traced, OpOutcome, OpTrace, StepTrace, Trace, TracedStep,
};
