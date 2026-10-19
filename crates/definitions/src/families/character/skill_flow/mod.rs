//! **技能使用流程**（覆盖报告 R1 缺口：技能使用 + 准备技能流程，163 个实体；D184 第 2 步）。
//!
//! 本块把「使用一个技能」拆成可以单独调用的纯函数步骤，组合入口是 [`use_skill`]：
//!
//! | 步骤 | 接口 | 说明 |
//! |---|---|---|
//! | 能否使用 | [`can_use`] | 技能种类 / 准备技能阻塞 / **状态禁用（冻结·石化·眩晕）** / 每回合次数 / 充能 / 骰子 |
//! | 禁用查询 | `forbidden` | 某状态存在时禁用哪些入口（普通攻击 / 战技 / 爆发 / 特技；R5 / S6.6 / S8.4） |
//! | 特技 | `technique` | 特技流程入口：战斗行动、不派发 SkillUsed、伤害不算角色造成（S8.4 / K58） |
//! | 生效形态 | [`attack_form`] | 普通攻击的「视为」判定：重击（S8.9，行动开始前骰总数为偶数）/ 下落攻击（S8.10，切换后下一个战斗行动） |
//! | 支付 | [`pay_skill_cost`] | 骰子（复用 `resources::dice` 的 `can_afford` / `pay`）+ 充能；失败不改状态 |
//! | 使用计数 | [`usage::SkillUses`] | 每回合 N 次（`engine::stats::Counter` 承载，区自持、无全局状态） |
//! | 目标候选集 | [`target_candidates`] / [`TargetCandidates`] | **只给候选集 + 谓词接口**，不做选择策略（D25 / D26） |
//! | 时机派发 | `timing::dispatch_skill` | 使用技能时 / 后；`timeline` + 角色时机键（D185 / D186 / D196） |
//! | 准备技能 | `prepared::PreparedSkills` | 开始 / 完成 / 打断 / 清除（普通函数，**不写宏**） |
//! | 组合 | [`use_skill`] | 上面几步按固定顺序串起来 |
//!
//! 「使用技能时 / 后」的时机键是 `CharacterTiming::SkillUsed { phase, scope }`（在
//! `families::character::trigger`），与角色族其它时机**共用同一张注册表**（`CharacterTriggers`）；
//! 三档作用域的派发与归并在 `timing::dispatch_skill`。技能身份由时机键的作用域表达，
//! 不进 `timeline::Event`（Event 是通用五字段，本轮不可写 `timeline/**`）。
//!
//! ## 依据（唯一）
//!
//! - 规则书·进行行动 战斗行动【使用技能】：
//!   > 用元素骰支付所需的费用，使我方「出战角色」使用技能攻击对方的「出战角色」。
//!   > 角色每次使用「普通攻击」或「元素战技」后，会获得 1 点「充能」。
//!   > 角色「充能」达到上限后，可以花费所有「充能」来使用威力强大的「元素爆发」！
//! - 权威关键字 **K53 准备技能**、**K58 特技**、**K61 蓄力**
//!   （`generated/data/keywords.json` 中/英/日/韩四语）；
//! - `结算顺序.md` §一（同一时机内的顺序）/ §二（插入式结算，不是栈）/ §三（常见插入点）；
//! - 卡面文本（游戏内文本描述）与
//!   `generated/data/descriptions.json` 的措辞统计（「使用技能后」119、「使用技能时」52、
//!   「使用元素爆发后」10、「使用普通攻击后」1、具体技能「使用 `$[S…]` 」10）。
//!
//! ## 一次技能使用的顺序（本层口径）
//!
//! 1. [`can_use`]：先判能不能用（**失败 -> 什么都不改**）；
//! 2. 派发「使用技能时」（`SkillPhase::Before`）：对应卡面「使用技能时：少花费 1 个元素骰」这类时点；
//! 3. [`pay_skill_cost`]：支付骰子 + 充能（**失败 -> 什么都不改**）；
//! 4. 技能效果体结算（`SkillDef::effect` 现在是占位 `fn()`，**本层不执行**，见下）；
//! 5. 派发「使用技能后」（`SkillPhase::After`）；
//! 6. 记一次使用（[`usage::SkillUses::recorded`]）+ 按规则书给充能（[`skill_energy_gain`]，
//!    普通攻击 / 元素战技 +1；获得量修正走 `families::character::energy::energy_gain`）。
//!
//! 上面第 2 / 5 步的派发**只产出待执行列表**（D186），执行由调用方按
//! 「before -> 技能效果 -> after」的顺序做；要插进当前结算时用 `timeline::Resolution::insert`
//! （D196：插到当前位置，做完回到插入点继续；见本文件测试
//! `use_skill_steps_insert_into_resolution`）。
//!
//! ## 准备技能（K53）与特技（K58）
//!
//! - 准备技能的「轮到行动时跳过 / 完成后直接使用 / 切换到后台打断」全部在
//!   `prepared::PreparedSkills`，由 [`can_use`] 作为**阻塞**检查；
//! - **准备技能的使用不派发**「使用技能时 / 使用技能后」等效果（K53 原文），所以
//!   [`use_skill`] 只用于**正常使用**；准备技能由 `prepared::PreparedSkills::complete`
//!   + 调用方直接执行效果体；
//! - **特技不是技能**（K58：「使用『特技』时不视为使用技能，无法触发『使用技能后』……」）：
//!   独立流程在 `technique`（`use_technique` / `SkillUseClass`），与技能分类**并列**但分开；
//! - **蓄力**（K61，自行巧局）与**额外行动**（额外行动中施放的技能不给充能）本轮不接，见「待接入」。
//!
//! ## 不做（本轮口径 / D133 / D177）
//!
//! - **不做目标选择策略**：只给候选集与谓词（选择由上层 / 选择原语做，D25 / D26）；
//! - **不做费用修正链**：「使用技能时：少花费 1 个元素骰」这类效果需要费用修正链（D184 第 3 步），
//!   本层不改费用，只在「使用技能时」时点把触发排出来；
//! - **不执行技能效果体**：`SkillDef::effect` 仍是 D162 的占位 `fn()`，签名换成带上下文
//!   之后由调用方执行（TODO(P0) 在 `skill.rs`）；
//! - **不判定行动权 / 阶段**：使用技能是「战斗行动」（规则书·进行行动 行动），本层不改变行动权；
//! - **不判定「谁是出战角色」/ 实体→角色映射**：没有 DefState，本层只接收「出战角色的状态」；
//! - **不做蓄力**（K61，自行巧局）；特技在 `technique` 里另走流程；
//! - **不采样、不展开不确定量**（D190）：本层全部确定；骰子折叠组按既有语义（`can_afford` 是
//!   「可能就行」上界，`pay` 可能需要先展开，见 [`CannotUse::NotEnoughDice`] 的说明）。
//!
//! ## 待接入（本轮做不到、留给后续块）
//!
//! 1. **费用修正链**（52 处「使用技能时」少花费类）：接在 [`pay_skill_cost`] 之前；
//! 2. **实体 → 角色映射 / DefState**：目标候选集的宇宙（`&[EntityId]`）现在由调用方给；
//! 3. **技能效果体上下文**（`SkillEffect` 占位签名，D162）；
//! 4. **状态导致的「无法使用技能」**：查询在 `forbidden`（冻结 / 石化 / 眩晕），已接进 [`can_use`]；
//!    眩晕 / 石化的**正式状态定义**仍待状态块（本层先给共享身份）；
//! 5. **特技（K58）端到端 flow 入口**（`flow::use_technique_me/opp`）不在本轮写范围：本块给
//!    `technique::can_use_technique` / `use_technique`（判定 + 结果）；**蓄力（K61）**、
//!    **额外行动不给充能**仍是自行巧局待办；
//! 6. **事件不带技能 id**：技能身份由时机键作用域表达（`timing` 顶部说明）；若将来要让条件函数
//!    直接看技能 id，需要先给 `timeline::Event` 加字段（本任务不可写 `timeline/**`）。

pub mod attack_form;
pub mod forbidden;
pub mod prepared;
pub mod technique;
pub mod timing;
pub mod usage;

pub use attack_form::{
    effective_attack_form, effective_normal_attack, is_even, AttackForm, AttackFormInputs,
    PlungingMark,
};
pub use forbidden::{
    forbidden_by, forbidding_status, skill_allowed, technique_allowed, ForbiddenState,
    ForbiddenUses, Petrified, Stun, PETRIFIED, PETRIFIED_CARD, SKILL_FORBIDDING_STATUSES, STUN,
    STUN_CARD,
};
pub use prepared::{PreparedAction, PreparedSkill, PreparedSkills};
pub use technique::{
    can_use_technique, dispatch_technique, technique_timing, use_technique, CannotUseTechnique,
    SkillUseClass, TechniqueContext, TechniqueUse, UseFlow,
};
pub use timing::{
    dispatch_skill, skill_event, skill_timings, CharacterTiming, SkillFrames, SkillPending,
    SkillPhase, SkillScope, SkillTrigger, SkillTriggers, AFTER_ANY, BEFORE_ANY,
};
pub use usage::{SkillUseEntry, SkillUses};

use crate::base::{Element, SkillId};
use crate::families::character::character::CharacterState;
use crate::families::character::energy::energy_gain;
use crate::families::character::skill::SkillDef;
use crate::families::character::types::SkillKind;
use crate::resources::dice::DicePool;
use gi_genius_invokation_engine::{EntityId, Nat};

// ======================================================================
// 策略 / 上下文 / 失败
// ======================================================================

/// 使用技能的**策略开关**（D177：形式优雅优先，规则没定的先给开关）。
///
/// 默认值见 [`SkillUsePolicy::DEFAULT`]：
/// - `allow_passive: false` —— **被动技能不可主动使用**（被动只有「被触发」的形态；
///   卡面「【被动】…」都是触发式，从未出现「使用被动技能」的口径）；
/// - `blocked_while_preparing: true` —— 有准备技能槽时**不能主动选技能**（K53：正在准备 -> 跳过行动；
///   已完成准备 -> 直接使用该技能）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillUsePolicy {
    /// 是否允许把「被动技能」当主动技能使用（默认不允许；测试 / 特殊规则可打开）。
    pub allow_passive: bool,
    /// 是否按 K53 让「准备技能」阻塞主动技能选择（默认阻塞）。
    pub blocked_while_preparing: bool,
}

impl SkillUsePolicy {
    /// 默认策略：被动不可主动使用、准备技能阻塞。
    pub const DEFAULT: SkillUsePolicy = SkillUsePolicy {
        allow_passive: false,
        blocked_while_preparing: true,
    };
}

impl Default for SkillUsePolicy {
    fn default() -> SkillUsePolicy {
        SkillUsePolicy::DEFAULT
    }
}

/// 一次「使用技能」判定的**只读局面上下文**（D42：容器没有身份，就是个值）。
///
/// 字段都是只读借用：判定 / 支付都**不改入参**（失败时什么都不改）。
#[derive(Clone, Copy, Debug)]
pub struct SkillUseContext<'a> {
    /// 要使用的技能定义。
    pub skill: &'a SkillDef,
    /// 使用者的状态（出战角色；充能在这里）。
    pub character: &'a CharacterState,
    /// 我方骰池。
    pub dice: &'a DicePool,
    /// 每个技能的每回合次数（区自持）。
    pub uses: &'a SkillUses,
    /// 该角色身上「准备技能」的槽（区自持；没有就传空容器）。
    pub prepared: &'a PreparedSkills,
    /// 策略开关。
    pub policy: SkillUsePolicy,
    /// 「获得充能时获得量 +N」类修正（`energy::energy_gain` 的 modifiers；没有就传空切片）。
    pub energy_gain_bonus: &'a [Nat],
}

/// 不能使用技能的原因。
///
/// 判定顺序固定（[`can_use`] 文档里写明），同一个局面只会返回**第一个**命中的原因，
/// 便于调用方 / 测试稳定断言。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CannotUse {
    /// 被动技能不可主动使用（[`SkillUsePolicy::allow_passive`] 为 false）。
    PassiveSkill,
    /// 该角色有技能**正在准备**：本次行动被跳过（K53）。
    Preparing {
        /// 正在准备的技能。
        skill: SkillId,
    },
    /// 该角色有技能**已完成准备**：将直接使用它，不能再选别的技能（K53）。
    PreparedSkillReady {
        /// 已完成准备的技能。
        skill: SkillId,
    },
    /// 本回合使用次数已达上限（「每回合 N 次」）。
    UsageLimit {
        /// 达到上限的技能。
        skill: SkillId,
        /// 每回合上限。
        limit: Nat,
    },
    /// 充能不够（元素爆发的充能费用）。
    NotEnoughEnergy {
        /// 技能要求的充能。
        required: Nat,
        /// 当前充能。
        available: Nat,
    },
    /// 骰子不够付。
    ///
    /// 注意：`DicePool::can_afford` 是「**可能**就行」（折叠组按可能颜色计入上界），
    /// 所以 [`can_use`] 通过后 [`pay_skill_cost`] **仍可能**返回这个失败（折叠组需要先显式展开）。
    NotEnoughDice,
    /// 角色被状态**禁止使用技能**（冻结 / 石化 / 眩晕；S6.6 / K55 / S8.4）。
    ///
    /// 查询实现在 [`forbidden`]；规则书原文「角色无法使用技能」。
    ForbiddenByStatus {
        /// 命中的禁用状态（身份 + 被禁用的入口）。
        forbidden: ForbiddenState,
    },
}

// ======================================================================
// 能否使用
// ======================================================================

/// **能否使用**这个技能（只读；成功 -> `Ok(())`，失败 -> 第一个命中的原因）。
///
/// 判定顺序（固定）：
/// 1. **[`CannotUse::PassiveSkill`]**：被动技能不可主动使用（策略可关）；
/// 2. **[`CannotUse::ForbiddenByStatus`]**：角色被状态禁止使用技能（冻结 / 石化 / 眩晕；
///    S6.6 / K55 / S8.4）；查询在 [`forbidden`]，**不受策略开关影响**；
/// 3. **[`CannotUse::Preparing`] / [`CannotUse::PreparedSkillReady`]**：K53 准备技能阻塞
///    （策略可关）；判定用 `PreparedSkills::pending_action`；
/// 4. **[`CannotUse::UsageLimit`]**：本回合次数（`SkillUses`）；
/// 5. **[`CannotUse::NotEnoughEnergy`]**：充能（`SkillCost::energy`；元素爆发的卡面数据里它等于
///    充能上限，即规则书「花费所有充能」的编码方式）；
/// 6. **[`CannotUse::NotEnoughDice`]**：骰子（`DicePool::can_afford`，可能就行）。
///
/// **不检查**（见模块文档「不做 / 待接入」）：出战角色是谁、行动权 / 阶段、
/// 目标是否存在、费用修正链。
pub fn can_use(context: &SkillUseContext<'_>) -> Result<(), CannotUse> {
    // 1) 技能种类：被动不可主动使用。
    if matches!(context.skill.kind, SkillKind::Passive) && !context.policy.allow_passive {
        return Err(CannotUse::PassiveSkill);
    }
    // 2) 状态禁用技能（冻结 / 石化 / 眩晕；S6.6 / K55 / S8.4）——不受策略开关影响。
    if let Some(forbidden) = context.forbidden() {
        return Err(CannotUse::ForbiddenByStatus { forbidden });
    }
    // 3) 准备技能阻塞（K53）。
    if context.policy.blocked_while_preparing {
        match context.prepared.pending_action() {
            None => {}
            Some(PreparedAction::Skip { skill }) => return Err(CannotUse::Preparing { skill }),
            Some(PreparedAction::Use { skill }) => {
                return Err(CannotUse::PreparedSkillReady { skill });
            }
        }
    }
    // 4) 每回合使用次数。
    if let Some(limit) = context.uses.limit(context.skill.id) {
        if !context.uses.can_use(context.skill.id) {
            return Err(CannotUse::UsageLimit {
                skill: context.skill.id,
                limit,
            });
        }
    }
    // 5) 充能。
    let required = context.skill.cost.energy;
    let available = context.character.energy();
    if available < required {
        return Err(CannotUse::NotEnoughEnergy {
            required,
            available,
        });
    }
    // 6) 骰子（可能就行）。
    if !context.dice.can_afford(context.skill.cost.dice) {
        return Err(CannotUse::NotEnoughDice);
    }
    Ok(())
}

// ======================================================================
// 支付
// ======================================================================

/// 支付之后的局面（骰池 + 角色状态；都是新值，入参没被改）。
#[derive(Clone, Debug, PartialEq)]
pub struct SkillPayment {
    /// 支付后的骰池。
    pub dice: DicePool,
    /// 支付后的角色状态（充能已扣）。
    pub character: CharacterState,
}

/// **支付技能费用**：骰子（复用 `DicePool::pay`）+ 充能（`CharacterState::energy_sub`）。
///
/// **纯函数**：成功返回新状态；失败返回 `Err`，**入参一个字节都不改**。
///
/// - 充能先判（`SkillCost::energy`；为 0 的技能不扣）；
/// - 骰子按 `resources::dice` 的支付语义：① 指定元素骰 -> ② 同色（颜色由 `same_color` 给）->
///   ③ 任意骰；折叠组未显式展开时不能用于 ① / ②（见 `DicePool::pay` 文档）；
/// - 规则书「元素爆发花费所有充能」由**卡面数据**表达（爆发的 `SkillCost::energy` = 充能上限），
///   本层不写死 `max_energy`（将来若出现充能费用加减，只改费用，不改支付）。
pub fn pay_skill_cost(
    skill: &SkillDef,
    character: &CharacterState,
    dice: &DicePool,
    same_color: Option<Element>,
) -> Result<SkillPayment, CannotUse> {
    let required = skill.cost.energy;
    let available = character.energy();
    if available < required {
        return Err(CannotUse::NotEnoughEnergy {
            required,
            available,
        });
    }
    let Some(paid_dice) = dice.pay(skill.cost.dice, same_color) else {
        return Err(CannotUse::NotEnoughDice);
    };
    Ok(SkillPayment {
        dice: paid_dice,
        character: character.energy_sub(required),
    })
}

/// 使用这个技能**后**的基础充能获得量（规则书·进行行动 使用技能）：
///
/// - 普通攻击（含重击 / 下落攻击，D124）/ 元素战技 -> 1 点；
/// - 元素爆发 / 被动 -> 0 点（爆发的充能是**花费**，不是获得）。
///
/// 「获得量 +N」类修正由调用方用 `energy::energy_gain` 叠加后传入
/// [`SkillUseContext::energy_gain_bonus`]。
pub fn skill_energy_gain(skill: &SkillDef) -> Nat {
    if skill.kind.is_normal_attack() || matches!(skill.kind, SkillKind::ElementalSkill) {
        Nat::new(1)
    } else {
        Nat::ZERO
    }
}

// ======================================================================
// 组合流程
// ======================================================================

/// 一次**成功使用技能**的结果（纯值；入参没被改）。
///
/// 调用方按「执行 `before` -> 执行技能效果体 -> 执行 `after`」的顺序做；
/// 要插进当前结算时用 `timeline::Resolution::insert`（D196）。
#[derive(Clone, Debug)]
pub struct SkillUse {
    /// 本次使用的技能。
    pub skill: SkillId,
    /// **本次生效的技能种类**（含重击 / 下落攻击的「视为」判定；S8.9 / S8.10）。
    ///
    /// 与 `SkillDef::kind` 的关系见 [`attack_form::effective_normal_attack`]：
    /// 战技 / 爆发 / 被动与已写死形态的技能**原样**；普通攻击本体按「骰总数为偶数 -> 重击」
    /// 判定（[use_skill] 取样的是**支付前**的骰池总数）。
    pub effective_kind: SkillKind,
    /// 支付后的骰池。
    pub dice: DicePool,
    /// 支付 + 使用后获得充能之后的角色状态。
    pub character: CharacterState,
    /// 记录了一次使用之后的每回合次数容器（调用方替换自己的字段）。
    pub uses: SkillUses,
    /// 「使用技能时」的待执行（由调用方执行；本层不执行）。
    pub before: Vec<SkillPending>,
    /// 「使用技能后」的待执行（由调用方执行；本层不执行）。
    pub after: Vec<SkillPending>,
}

impl SkillUse {
    /// 本次生效形态的**标注**（`None` = 不是普通攻击）。
    pub fn attack_form(&self) -> Option<AttackForm> {
        AttackForm::of(self.effective_kind)
    }
}

/// **使用技能**：把「能否使用 -> 派发使用技能时 -> 支付 -> 派发使用技能后 -> 记使用 + 给充能」
/// 按固定顺序串起来（顺序见模块文档）。
///
/// - 失败返回 `Err`，**入参一个字节都不改**（[`can_use`] / [`pay_skill_cost`] 都是纯函数，
///   派发也是只读的）；
/// - **不执行技能效果体**（`SkillDef::effect` 还是占位 `fn()`，D162 TODO）；
/// - **不执行派发结果**（D186）：`before` / `after` 只是待执行列表；
/// - **准备技能不走这里**（K53：不派发「使用技能时 / 后」；走 `prepared::PreparedSkills`）；
/// - 规则书的「普通攻击 / 元素战技 后获得 1 点充能」**已作用**在返回的 `character` 上（先于调用方执行
///   `after`）；规则书没规定它与「使用技能后」触发的先后，本层取「获得充能先」暂定。
pub fn use_skill(
    triggers: &SkillTriggers,
    context: &SkillUseContext<'_>,
    who: EntityId,
    target: Option<EntityId>,
    same_color: Option<Element>,
) -> Result<SkillUse, CannotUse> {
    // 没有「下落攻击」待消耗标记时的判定：只按「行动开始前的骰总数」判重击（S8.9）。
    use_skill_determined(
        triggers,
        context,
        who,
        target,
        same_color,
        PlungingMark::NONE,
    )
}

/// **使用技能（带下落攻击标记）**：与 [`use_skill`] 完全一致，只是把 [`PlungingMark`]
/// 纳入「生效形态」判定（S8.10：切换为出战角色后，本回合下一个战斗行动若为普通攻击 ->
/// 视为下落攻击）。
///
/// - 判定只影响返回的 [`SkillUse::effective_kind`]（观测 / 后续「普通攻击后」派发用）；
///   费用 / 充能 / 次数与 [`use_skill`] 相同（重击 / 下落攻击仍属普通攻击类，D124）；
/// - 本层**不消费**标记：战斗行动结束后由调用方调 [`PlungingMark::after_combat_action`]；
/// - 骰总数取样 = 调用瞬间（支付前）的 `context.dice`。
pub fn use_skill_with_plunging(
    triggers: &SkillTriggers,
    context: &SkillUseContext<'_>,
    who: EntityId,
    target: Option<EntityId>,
    same_color: Option<Element>,
    plunging: PlungingMark,
) -> Result<SkillUse, CannotUse> {
    use_skill_determined(triggers, context, who, target, same_color, plunging)
}

/// [`use_skill`] / [`use_skill_with_plunging`] 的**唯一实现**。
fn use_skill_determined(
    triggers: &SkillTriggers,
    context: &SkillUseContext<'_>,
    who: EntityId,
    target: Option<EntityId>,
    same_color: Option<Element>,
    plunging: PlungingMark,
) -> Result<SkillUse, CannotUse> {
    can_use(context)?;
    // 生效形态判定：**支付前**取骰池总数（规则书「我方行动开始前」；本层按调用瞬间为准）。
    let form_inputs = AttackFormInputs::from_pool(context.dice, plunging);
    let effective_kind = effective_normal_attack(context.skill.kind, form_inputs);
    let event = skill_event(who, target);
    let before = dispatch_skill(triggers, SkillPhase::Before, context.skill, &event);
    let payment = pay_skill_cost(context.skill, context.character, context.dice, same_color)?;
    let gain = energy_gain(skill_energy_gain(context.skill), context.energy_gain_bonus);
    let character = payment.character.energy_add(gain);
    let after = dispatch_skill(triggers, SkillPhase::After, context.skill, &event);
    let uses = context.uses.recorded(context.skill.id);
    Ok(SkillUse {
        skill: context.skill.id,
        effective_kind,
        dice: payment.dice,
        character,
        uses,
        before,
        after,
    })
}

// ======================================================================
// 目标候选集（只给候选 / 谓词；不做选择策略）
// ======================================================================

/// 目标谓词：`fn(实体) -> bool`（D186：函数指针，不是闭包 / trait object）。
///
/// 「这个实体能不能被这个技能选为目标」由**具体技能 / 上层**给（例如「对方出战角色」
/// 「敌方后台角色」）；本层不认识角色、出战、敌我，也不认识技能目标范围。
pub type TargetPredicate = fn(EntityId) -> bool;

/// 技能**目标候选集**（只读包装；D183 风格：字段私有，只给只读入口）。
///
/// 本层只负责「从给定宇宙里筛出满足谓词的实体」，**不做选择**（D25 / D26：选择原语只收列表与数量，
/// 选择策略在上层）；候选集为空表示「没有合法目标」（要不要因此禁止使用技能，由上层决定）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetCandidates {
    ids: Vec<EntityId>,
}

impl TargetCandidates {
    /// 空候选集。
    pub const fn new() -> TargetCandidates {
        TargetCandidates { ids: Vec::new() }
    }

    /// 直接给一组候选（上层自己筛好后用）。
    pub fn from_ids(ids: Vec<EntityId>) -> TargetCandidates {
        TargetCandidates { ids }
    }

    /// 从**宇宙**里筛出满足谓词的候选（保持宇宙原有顺序）。
    pub fn select(universe: &[EntityId], predicate: TargetPredicate) -> TargetCandidates {
        TargetCandidates {
            ids: universe
                .iter()
                .copied()
                .filter(|id| predicate(*id))
                .collect(),
        }
    }

    /// 候选数量。
    pub fn len(&self) -> Nat {
        Nat::new(self.ids.len() as _)
    }

    /// 是否没有候选。
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// 是否包含某个实体。
    pub fn contains(&self, id: EntityId) -> bool {
        self.ids.contains(&id)
    }

    /// 只读切片（顺序 = 宇宙顺序）。
    pub fn as_slice(&self) -> &[EntityId] {
        &self.ids
    }

    /// 按顺序只读遍历。
    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.ids.iter().copied()
    }
}

/// **目标候选集**：从 `universe`（候选宇宙，由上层给：没有 DefState，本层不知道实体是谁）里
/// 筛出满足 `predicate` 的实体；**不做选择策略**（选择由上层 / 选择原语做）。
#[must_use]
pub fn target_candidates(universe: &[EntityId], predicate: TargetPredicate) -> TargetCandidates {
    TargetCandidates::select(universe, predicate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
    use crate::families::character::skill::SkillCost;
    use crate::families::character::skill_flow::forbidden::{PETRIFIED, STUN};
    use crate::families::status::known::FROZEN;
    use crate::generated::Localized;
    use crate::resources::dice::{CostComponent, DieFace, FoldedGroup};
    use gi_genius_invokation_engine::nat::ExactNat128;
    use gi_genius_invokation_engine::{nadd, nmul};

    // ------------------------------------------------------------------
    // 测试用技能 / 局面
    // ------------------------------------------------------------------

    /// 空效果体（`SkillDef.effect` 是占位 `fn()`，D162）。
    fn noop() {}

    /// 1 水 + 2 无色。
    static ONE_HYDRO_TWO_ANY: [CostComponent; 2] = [
        CostComponent::SpecificElement(Element::Hydro, Nat::new(1)),
        CostComponent::AnyColor(Nat::new(2)),
    ];
    /// 3 水。
    static THREE_HYDRO: [CostComponent; 1] =
        [CostComponent::SpecificElement(Element::Hydro, Nat::new(3))];

    /// 普通攻击 12161：1 水 + 2 无色。
    static NORMAL: SkillDef = SkillDef {
        id: SkillId::new(12161),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::normal_attack(),
        cost: SkillCost::from_dice(&ONE_HYDRO_TWO_ANY),
        effect: noop,
    };

    /// 元素战技 12162：3 水。
    static SKILL: SkillDef = SkillDef {
        id: SkillId::new(12162),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::ElementalSkill,
        cost: SkillCost::from_dice(&THREE_HYDRO),
        effect: noop,
    };

    /// 元素爆发 12163：3 水 + 2 充能。
    static BURST: SkillDef = SkillDef {
        id: SkillId::new(12163),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::ElementalBurst,
        cost: SkillCost::from_dice_and_energy(&THREE_HYDRO, Nat::new(2)),
        effect: noop,
    };

    /// 被动技能 12164：无消耗。
    static PASSIVE: SkillDef = SkillDef {
        id: SkillId::new(12164),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::Passive,
        cost: SkillCost::FREE,
        effect: noop,
    };

    /// 建角色状态：生命 / 上限 10、充能 / 充能上限由参数给。
    fn character(energy: Nat, max_energy: Nat) -> CharacterState {
        CharacterState::new(Nat::new(10), Nat::new(10), energy, max_energy)
    }

    /// 建**顺序记号用**的角色状态：生命 0 / 上限 MAX（顺序记号 `hp * 10 + digit` 不受钳制）。
    fn marking_character(energy: Nat, max_energy: Nat) -> CharacterState {
        CharacterState::new(Nat::ZERO, Nat::MAX, energy, max_energy)
    }

    /// 建骰池：按 (骰面, 个数) 列表累加。
    fn pool(dice: &[(DieFace, Nat)]) -> DicePool {
        let mut built = DicePool::empty();
        for &(face, count) in dice {
            built = built.add(face, count);
        }
        built
    }

    /// 够付普通攻击的骰池：1 水 + 2 万能。
    fn normal_pool() -> DicePool {
        pool(&[(DieFace::Hydro, Nat::new(1)), (DieFace::Omni, Nat::new(2))])
    }

    /// 够付元素战技 / 元素爆发的骰池：3 水。
    fn three_hydro() -> DicePool {
        pool(&[(DieFace::Hydro, Nat::new(3))])
    }

    /// 判定（默认策略、没有获得量修正）。
    fn check(
        skill: &SkillDef,
        character: &CharacterState,
        dice: &DicePool,
        uses: &SkillUses,
        prepared: &PreparedSkills,
    ) -> Result<(), CannotUse> {
        let context = SkillUseContext {
            skill,
            character,
            dice,
            uses,
            prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        can_use(&context)
    }

    /// 执行一批待执行（调用方行为；本层不执行）。
    fn execute(pending: &[SkillPending], state: &mut CharacterState) {
        crate::families::character::trigger::execute_character(pending, state);
    }

    /// 顺序记号效果：`hp = hp * 10 + digit`。
    fn mark_one(state: &mut CharacterState, _event: &crate::timeline::Event, _owner: EntityId) {
        let scaled = nmul!(state.hp(), Nat::new(10));
        let next = state.hp_set(nadd!(scaled, Nat::new(1)));
        *state = next;
    }
    fn mark_two(state: &mut CharacterState, _event: &crate::timeline::Event, _owner: EntityId) {
        let scaled = nmul!(state.hp(), Nat::new(10));
        let next = state.hp_set(nadd!(scaled, Nat::new(2)));
        *state = next;
    }

    // ------------------------------------------------------------------
    // 能否使用
    // ------------------------------------------------------------------

    #[test]
    fn normal_attack_is_usable_when_dice_are_enough() {
        let state = character(Nat::ZERO, Nat::new(2));
        let dice = normal_pool();
        assert_eq!(
            check(
                &NORMAL,
                &state,
                &dice,
                &SkillUses::new(),
                &PreparedSkills::new()
            ),
            Ok(())
        );
    }

    #[test]
    fn passive_skill_cannot_be_used_actively_by_default() {
        let state = character(Nat::new(2), Nat::new(2));
        let dice = DicePool::empty();
        assert_eq!(
            check(
                &PASSIVE,
                &state,
                &dice,
                &SkillUses::new(),
                &PreparedSkills::new()
            ),
            Err(CannotUse::PassiveSkill)
        );
        // 策略打开（测试 / 特殊规则）就允许。
        let context = SkillUseContext {
            skill: &PASSIVE,
            character: &state,
            dice: &dice,
            uses: &SkillUses::new(),
            prepared: &PreparedSkills::new(),
            policy: SkillUsePolicy {
                allow_passive: true,
                ..SkillUsePolicy::DEFAULT
            },
            energy_gain_bonus: &[],
        };
        assert_eq!(can_use(&context), Ok(()));
    }

    #[test]
    fn insufficient_dice_is_rejected_and_changes_nothing() {
        let state = character(Nat::ZERO, Nat::new(2));
        let dice = pool(&[(DieFace::Hydro, Nat::new(1))]);
        assert_eq!(
            check(
                &NORMAL,
                &state,
                &dice,
                &SkillUses::new(),
                &PreparedSkills::new()
            ),
            Err(CannotUse::NotEnoughDice)
        );
        // 判定纯函数：骰池与角色都没变。
        assert_eq!(dice.total(), Nat::new(1));
        assert_eq!(state.energy(), Nat::ZERO);
    }

    #[test]
    fn burst_requires_two_energy() {
        let dice = three_hydro();
        let not_enough = character(Nat::new(1), Nat::new(2));
        assert_eq!(
            check(
                &BURST,
                &not_enough,
                &dice,
                &SkillUses::new(),
                &PreparedSkills::new()
            ),
            Err(CannotUse::NotEnoughEnergy {
                required: Nat::new(2),
                available: Nat::new(1),
            })
        );
        let enough = character(Nat::new(2), Nat::new(2));
        assert_eq!(
            check(
                &BURST,
                &enough,
                &dice,
                &SkillUses::new(),
                &PreparedSkills::new()
            ),
            Ok(())
        );
    }

    #[test]
    fn usage_limit_blocks_until_round_reset() {
        let state = character(Nat::ZERO, Nat::new(2));
        let dice = normal_pool();
        let limited = SkillUses::new().with_limit(NORMAL.id, Nat::new(1));
        assert_eq!(
            check(&NORMAL, &state, &dice, &limited, &PreparedSkills::new()),
            Ok(())
        );
        let used = limited.recorded(NORMAL.id);
        assert_eq!(
            check(&NORMAL, &state, &dice, &used, &PreparedSkills::new()),
            Err(CannotUse::UsageLimit {
                skill: NORMAL.id,
                limit: Nat::new(1),
            })
        );
        // 新回合清零 -> 又能用。
        assert_eq!(
            check(
                &NORMAL,
                &state,
                &dice,
                &used.reset_round(),
                &PreparedSkills::new()
            ),
            Ok(())
        );
    }

    #[test]
    fn prepared_skill_blocks_action_choice() {
        let state = character(Nat::ZERO, Nat::new(2));
        let dice = normal_pool();
        let preparing = PreparedSkills::new().begin(SKILL.id, Nat::new(1));
        assert_eq!(
            check(&NORMAL, &state, &dice, &SkillUses::new(), &preparing),
            Err(CannotUse::Preparing { skill: SKILL.id })
        );
        let ready = preparing.tick_action_round();
        assert_eq!(
            check(&NORMAL, &state, &dice, &SkillUses::new(), &ready),
            Err(CannotUse::PreparedSkillReady { skill: SKILL.id })
        );
        // 策略关掉阻塞 -> 正常可用（K53 之外的特殊规则 / 测试）。
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &dice,
            uses: &SkillUses::new(),
            prepared: &ready,
            policy: SkillUsePolicy {
                blocked_while_preparing: false,
                ..SkillUsePolicy::DEFAULT
            },
            energy_gain_bonus: &[],
        };
        assert_eq!(can_use(&context), Ok(()));
    }

    // ------------------------------------------------------------------
    // 状态禁用技能（R5 / S6.6 冻结 / K55 眩晕 / S8.4 石化）
    // ------------------------------------------------------------------

    /// 给角色状态列表挂一个布尔状态。
    fn with_state(
        mut state: CharacterState,
        identity: &'static dyn crate::families::action_card::status::StateIdentity,
    ) -> CharacterState {
        state.states.add(CHARACTER_STATE_POS, identity, None);
        state
    }

    #[test]
    fn forbidden_status_blocks_skill_use_and_changes_nothing() {
        // 冻结 / 眩晕 / 石化：角色无法使用技能（S6.6 / K55 / S8.4）。
        let dice = normal_pool();
        let frozen = with_state(character(Nat::ZERO, Nat::new(2)), &FROZEN);
        assert!(matches!(
            check(&NORMAL, &frozen, &dice, &SkillUses::new(), &PreparedSkills::new()),
            Err(CannotUse::ForbiddenByStatus { .. })
        ));
        for identity in [&STUN as &'static dyn crate::families::action_card::status::StateIdentity, &PETRIFIED] {
            let state = with_state(character(Nat::ZERO, Nat::new(2)), identity);
            assert!(matches!(
                check(&NORMAL, &state, &dice, &SkillUses::new(), &PreparedSkills::new()),
                Err(CannotUse::ForbiddenByStatus { .. })
            ));
        }
        // use_skill 同样拒绝：不扣骰、不给充能、不记次数、不派发。
        let triggers = SkillTriggers::new();
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &frozen,
            dice: &dice,
            uses: &SkillUses::new(),
            prepared: &PreparedSkills::new(),
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        assert!(matches!(
            use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None),
            Err(CannotUse::ForbiddenByStatus { .. })
        ));
        assert_eq!(dice.total(), Nat::new(3), "判定失败不改骰池");
        assert_eq!(frozen.energy(), Nat::ZERO, "判定失败不获得充能");
        // 状态解除后可以正常使用。
        let mut cleared = frozen.clone();
        cleared.states.remove(CHARACTER_STATE_POS, &FROZEN);
        assert_eq!(
            check(&NORMAL, &cleared, &dice, &SkillUses::new(), &PreparedSkills::new()),
            Ok(())
        );
        // 冻结不影响「被动技能不可主动使用」的既有判定顺序（被动先被拒）。
        let passive_context = SkillUseContext {
            skill: &PASSIVE,
            character: &frozen,
            dice: &DicePool::empty(),
            uses: &SkillUses::new(),
            prepared: &PreparedSkills::new(),
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        assert_eq!(can_use(&passive_context), Err(CannotUse::PassiveSkill));
    }

    // ------------------------------------------------------------------
    // 支付
    // ------------------------------------------------------------------

    #[test]
    fn pay_deducts_dice_and_energy() {
        let state = character(Nat::new(2), Nat::new(2));
        let dice = three_hydro();
        let paid = pay_skill_cost(&BURST, &state, &dice, None).expect("够付");
        assert_eq!(paid.dice.total(), Nat::ZERO);
        assert_eq!(paid.character.energy(), Nat::ZERO);
        // 纯函数：入参没变。
        assert_eq!(dice.total(), Nat::new(3));
        assert_eq!(state.energy(), Nat::new(2));
    }

    #[test]
    fn pay_without_energy_cost_keeps_energy() {
        let state = character(Nat::new(2), Nat::new(2));
        let dice = normal_pool();
        let paid = pay_skill_cost(&NORMAL, &state, &dice, None).expect("够付");
        assert_eq!(paid.character.energy(), Nat::new(2));
        assert_eq!(paid.dice.total(), Nat::ZERO);
    }

    #[test]
    fn pay_failure_leaves_everything_unchanged() {
        // 充能不够。
        let not_enough_energy = character(Nat::new(1), Nat::new(2));
        let dice = three_hydro();
        assert_eq!(
            pay_skill_cost(&BURST, &not_enough_energy, &dice, None),
            Err(CannotUse::NotEnoughEnergy {
                required: Nat::new(2),
                available: Nat::new(1),
            })
        );
        assert_eq!(not_enough_energy.energy(), Nat::new(1));
        assert_eq!(dice.total(), Nat::new(3));
        // 骰子不够。
        let poor = pool(&[(DieFace::Hydro, Nat::new(1))]);
        let state = character(Nat::ZERO, Nat::new(2));
        assert_eq!(
            pay_skill_cost(&NORMAL, &state, &poor, None),
            Err(CannotUse::NotEnoughDice)
        );
        assert_eq!(poor.total(), Nat::new(1));
        assert_eq!(state.energy(), Nat::ZERO);
    }

    #[test]
    fn pay_can_fail_when_can_afford_said_possible() {
        // 折叠组：can_afford 按「可能是什么色」计入上界（可能为 3 水），
        // 但 pay 的 ① 指定元素骰只从 known / 万能扣 -> 付不起（D144 的既定语义）。
        let folded = DicePool::new(
            Nat::ZERO,
            vec![FoldedGroup {
                size: Nat::new(3),
                color_mass: [ExactNat128::new(1); 7],
                source: Nat::new(1),
            }],
            Vec::new(),
        );
        let state = character(Nat::ZERO, Nat::new(2));
        assert_eq!(
            check(&SKILL, &state, &folded, &SkillUses::new(), &PreparedSkills::new()),
            Ok(())
        );
        assert_eq!(
            pay_skill_cost(&SKILL, &state, &folded, None),
            Err(CannotUse::NotEnoughDice)
        );
    }

    // ------------------------------------------------------------------
    // 充能获得（规则书·进行行动 使用技能）
    // ------------------------------------------------------------------

    #[test]
    fn energy_gain_follows_rulebook() {
        let triggers = SkillTriggers::new();
        let empty_uses = SkillUses::new();
        let empty_prepared = PreparedSkills::new();

        // 普通攻击 +1。
        let state = character(Nat::ZERO, Nat::new(3));
        let dice = normal_pool();
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &dice,
            uses: &empty_uses,
            prepared: &empty_prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        assert_eq!(used.character.energy(), Nat::new(1));

        // 元素战技 +1。
        let state = character(Nat::ZERO, Nat::new(3));
        let dice = three_hydro();
        let context = SkillUseContext {
            skill: &SKILL,
            character: &state,
            dice: &dice,
            uses: &empty_uses,
            prepared: &empty_prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        assert_eq!(used.character.energy(), Nat::new(1));

        // 元素爆发：先花 2 点，不给充能。
        let state = character(Nat::new(2), Nat::new(3));
        let dice = three_hydro();
        let context = SkillUseContext {
            skill: &BURST,
            character: &state,
            dice: &dice,
            uses: &empty_uses,
            prepared: &empty_prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        assert_eq!(used.character.energy(), Nat::ZERO);

        // 获得量修正：普通攻击 1 + 2 = 3。
        let state = character(Nat::ZERO, Nat::new(3));
        let dice = normal_pool();
        let bonus = [Nat::new(2)];
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &dice,
            uses: &empty_uses,
            prepared: &empty_prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &bonus,
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        assert_eq!(used.character.energy(), Nat::new(3));
    }

    // ------------------------------------------------------------------
    // 组合流程 + 时机派发
    // ------------------------------------------------------------------

    #[test]
    fn use_skill_dispatches_before_then_after_and_records_use() {
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            BEFORE_ANY,
            Nat::new(1),
            crate::rules::ops::order::EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            crate::rules::ops::order::EffectSource::CharacterCard,
            Nat::ZERO,
            mark_two,
        ));
        let state = marking_character(Nat::ZERO, Nat::new(3));
        let dice = normal_pool();
        let uses = SkillUses::new().with_limit(NORMAL.id, Nat::new(1));
        let prepared = PreparedSkills::new();
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &dice,
            uses: &uses,
            prepared: &prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        assert_eq!(used.skill, NORMAL.id);
        assert_eq!(used.before.len(), 1);
        assert_eq!(used.after.len(), 1);
        // 记了一次使用（上限 1 -> 该技能本回合不能再用了）。
        assert_eq!(used.uses.used_count(NORMAL.id), Nat::new(1));
        assert!(!used.uses.can_use(NORMAL.id));
        // 骰子已付、充能 +1。
        assert_eq!(used.dice.total(), Nat::ZERO);
        assert_eq!(used.character.energy(), Nat::new(1));
        // 调用方按 before -> 技能效果 -> after 执行。
        let mut after_effect = used.character.clone();
        execute(&used.before, &mut after_effect);
        assert_eq!(after_effect.hp(), Nat::new(1));
        assert_eq!(after_effect.energy(), Nat::new(1));
        execute(&used.after, &mut after_effect);
        assert_eq!(after_effect.hp(), Nat::new(12));
    }

    #[test]
    fn use_skill_failure_leaves_everything_untouched() {
        let triggers = SkillTriggers::new();
        let state = character(Nat::ZERO, Nat::new(2));
        let poor = pool(&[(DieFace::Hydro, Nat::new(1))]);
        let uses = SkillUses::new().with_limit(NORMAL.id, Nat::new(1));
        let prepared = PreparedSkills::new();
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &poor,
            uses: &uses,
            prepared: &prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        assert!(matches!(
            use_skill(&triggers, &context, Nat::new(1), None, None),
            Err(CannotUse::NotEnoughDice)
        ));
        // 骰池、角色、次数、准备槽都没变。
        assert_eq!(poor.total(), Nat::new(1));
        assert_eq!(state.energy(), Nat::ZERO);
        assert_eq!(uses.used_count(NORMAL.id), Nat::ZERO);
        assert!(prepared.is_empty());
    }

    #[test]
    fn use_skill_steps_insert_into_resolution() {
        // D196：结算帧里「使用技能后」可以作为子结算插入当前位置，做完回到插入点。
        let mut triggers = SkillTriggers::new();
        triggers.register(SkillTrigger::new(
            BEFORE_ANY,
            Nat::new(1),
            crate::rules::ops::order::EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        triggers.register(SkillTrigger::new(
            AFTER_ANY,
            Nat::new(1),
            crate::rules::ops::order::EffectSource::CharacterCard,
            Nat::ZERO,
            mark_two,
        ));
        let state = marking_character(Nat::ZERO, Nat::new(3));
        let dice = normal_pool();
        let uses = SkillUses::new();
        let prepared = PreparedSkills::new();
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &dice,
            uses: &uses,
            prepared: &prepared,
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        // 「使用技能时」组成一层结算，先执行它。
        let mut frames = SkillFrames::new(used.before);
        let mut state_after = used.character;
        let first = frames.next_step().expect("使用技能时");
        (first.effect)(&mut state_after, &first.event, first.owner);
        assert_eq!(state_after.hp(), Nat::new(1));
        // 技能效果体在这里结算（占位 fn()，本层不执行）；之后把「使用技能后」插进来。
        frames.insert(used.after);
        assert_eq!(frames.depth(), Nat::new(2));
        while let Some(step) = frames.next_step() {
            (step.effect)(&mut state_after, &step.event, step.owner);
        }
        assert_eq!(state_after.hp(), Nat::new(12));
        assert!(frames.is_finished());
    }

    // ------------------------------------------------------------------
    // 目标候选集
    // ------------------------------------------------------------------

    #[test]
    fn target_candidates_filter_and_keep_universe_order() {
        fn low_ids(id: EntityId) -> bool {
            id <= Nat::new(2)
        }
        let universe = [Nat::new(3), Nat::new(1), Nat::new(2)];
        let candidates = target_candidates(&universe, low_ids);
        assert_eq!(candidates.as_slice(), &[Nat::new(1), Nat::new(2)]);
        assert_eq!(candidates.len(), Nat::new(2));
        assert!(!candidates.is_empty());
        assert!(candidates.contains(Nat::new(1)));
        assert!(!candidates.contains(Nat::new(3)));
        let listed: Vec<EntityId> = candidates.iter().collect();
        assert_eq!(listed, vec![Nat::new(1), Nat::new(2)]);
    }

    #[test]
    fn target_candidates_can_be_empty() {
        fn nobody(_id: EntityId) -> bool {
            false
        }
        let universe = [Nat::new(1), Nat::new(2)];
        let none = target_candidates(&universe, nobody);
        assert!(none.is_empty());
        assert_eq!(none.len(), Nat::ZERO);
        assert_eq!(none, TargetCandidates::new());
        assert_eq!(none, TargetCandidates::default());
        assert_eq!(TargetCandidates::from_ids(Vec::new()), none);
    }

    #[test]
    fn policy_default_flags() {
        let policy = SkillUsePolicy::DEFAULT;
        assert!(!policy.allow_passive);
        assert!(policy.blocked_while_preparing);
        assert_eq!(SkillUsePolicy::default(), policy);
    }

    #[test]
    fn skill_energy_gain_is_one_for_normal_attack_and_skill() {
        assert_eq!(skill_energy_gain(&NORMAL), Nat::new(1));
        assert_eq!(skill_energy_gain(&SKILL), Nat::new(1));
        assert_eq!(skill_energy_gain(&BURST), Nat::ZERO);
        assert_eq!(skill_energy_gain(&PASSIVE), Nat::ZERO);
        // 重击 / 下落攻击仍属普通攻击类（D124）-> 也给 1 点。
        static CHARGED: SkillDef = SkillDef {
            id: SkillId::new(12161),
            name: Localized::EMPTY,
            description: Localized::EMPTY,
            kind: SkillKind::charged_attack(),
            cost: SkillCost::FREE,
            effect: noop,
        };
        assert_eq!(skill_energy_gain(&CHARGED), Nat::new(1));
    }

    // ------------------------------------------------------------------
    // 生效形态标注（S8.9 重击 / S8.10 下落攻击）
    // ------------------------------------------------------------------

    /// 4 颗骰（偶数）且够付普通攻击：2 水 + 2 万能。
    fn even_pool() -> DicePool {
        pool(&[(DieFace::Hydro, Nat::new(2)), (DieFace::Omni, Nat::new(2))])
    }

    #[test]
    fn use_skill_annotates_charged_when_dice_total_is_even() {
        // S8.9：我方行动开始前骰总数为偶数 -> 本次普通攻击视为重击。
        let triggers = SkillTriggers::new();
        let state = character(Nat::ZERO, Nat::new(3));
        let dice = even_pool();
        assert_eq!(dice.total(), Nat::new(4));
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &dice,
            uses: &SkillUses::new(),
            prepared: &PreparedSkills::new(),
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        assert_eq!(used.effective_kind, SkillKind::charged_attack());
        assert_eq!(used.attack_form(), Some(AttackForm::Charged));
        // 重击仍属普通攻击类（D124）：充能照常 +1。
        assert_eq!(used.character.energy(), Nat::new(1));
        // 骰数为奇数时仍是普通攻击本体。
        let odd = normal_pool();
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &odd,
            uses: &SkillUses::new(),
            prepared: &PreparedSkills::new(),
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill(&triggers, &context, Nat::new(1), Some(Nat::new(2)), None)
            .expect("可用");
        assert_eq!(used.effective_kind, SkillKind::normal_attack());
        assert_eq!(used.attack_form(), Some(AttackForm::Normal));
    }

    #[test]
    fn use_skill_with_plunging_marks_plunging_before_charged() {
        // S8.10：切换后本回合下一个战斗行动若为普通攻击 -> 视为下落攻击；
        // 与 S8.9（骰数偶数 -> 重击）同时成立时本层按下落优先（待实测）。
        let triggers = SkillTriggers::new();
        let state = character(Nat::ZERO, Nat::new(3));
        let dice = even_pool();
        let mark = PlungingMark::new().armed();
        let context = SkillUseContext {
            skill: &NORMAL,
            character: &state,
            dice: &dice,
            uses: &SkillUses::new(),
            prepared: &PreparedSkills::new(),
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill_with_plunging(
            &triggers,
            &context,
            Nat::new(1),
            Some(Nat::new(2)),
            None,
            mark,
        )
        .expect("可用");
        assert_eq!(used.effective_kind, SkillKind::plunging_attack());
        assert_eq!(used.attack_form(), Some(AttackForm::Plunging));
        // 纯函数：标记不被消费（由调用方在战斗行动结束后清）。
        assert!(mark.is_pending());
        // 战技不吃这两条「视为」。
        let skill_context = SkillUseContext {
            skill: &SKILL,
            character: &state,
            dice: &three_hydro(),
            uses: &SkillUses::new(),
            prepared: &PreparedSkills::new(),
            policy: SkillUsePolicy::DEFAULT,
            energy_gain_bonus: &[],
        };
        let used = use_skill_with_plunging(
            &triggers,
            &skill_context,
            Nat::new(1),
            Some(Nat::new(2)),
            None,
            mark,
        )
        .expect("可用");
        assert_eq!(used.effective_kind, SkillKind::ElementalSkill);
        assert_eq!(used.attack_form(), None);
    }
}
