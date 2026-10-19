//! **伤害修正链**：把 `结算顺序.md` §三 第 1 行「造成伤害」的插入点落成一段
//! **插入式子结算**（D196）。
//!
//! ## 顺序（权威 = `结算顺序.md` §三 第 1 行 + 规则书·特殊词条 + 分工计划 §4.4.5）
//!
//! 0. **伤害免疫**（`免疫所有伤害`）：本次伤害**归 0 / 不结算**，整链短路（只留「基础 ->
//!    免疫 -> 扣血 0」三步）；**穿透伤害不吃免疫**（K5：「穿透伤害无法被护盾、伤害免疫等
//!    效果抵消」）。顺序放最前 = 「不结算」语义；规则书未写免疫与加伤的先后 -> `待实测`；
//! 1. **加伤**：本次伤害值 = 基础伤害 + 通用增伤 + 反应加伤 + 元素精通加伤；
//!    穿透伤害**不吃任何伤害加成**（规则书·特殊词条 穿透伤害）；
//! 2. **乘算 / 除算**（分工计划 §4.4.5 四段模型）：乘算区内加减 -> 独立乘法 + 倍数区 ->
//!    所有除数相乘后一起除并取整 -> 乘除区外加减；穿透 / 免疫**整段跳过**；
//! 3. **护盾抵消**：从「出战状态区 + 角色状态列表」读护盾层数，消耗护盾值抵消伤害；
//!    穿透伤害**不吃**（规则书·特殊词条 穿透伤害）；
//! 4. **减伤扣减**：从「角色状态列表 + 出战状态区」读「伤害抵消」层数，按数值扣减；
//!    穿透伤害**不吃**；
//! 5. **扣血**：链只给出 `to_hp`，**不扣血**（扣血 / 附着 / 反应判定仍在
//!    `families::character::damage::damage` 里，语义一行不改）。
//!
//! ### 为什么「加伤」排在「护盾 / 减伤」前面（与任务口径的差异，**待复核**）
//!
//! 任务口径把三段列成「护盾 -> 减伤 -> 加伤」，那是**链的三段组成**枚举；但规则书里
//! 「伤害加成」属于**伤害值本身**（穿透伤害连加伤都不吃），而护盾 / 减伤属于**受伤时**的抵消
//! （`damage::mitigate` 的语义：先盾后减）。若按字面「先盾后减再加」，加伤会从已被
//! 护盾吸掉的伤害里「长回来」，与规则书矛盾。所以数值顺序 =
//! **（免疫）-> 加伤 -> 乘算 -> 护盾 -> 减伤 -> 扣血**；
//! 已有单测 `bonus_applies_before_shield` 钉死这条。
//!
//! ## 数据从「区」读（D183：对外只读指针，不新建第二份承载）
//!
//! [`DamageChainEnv`] 只持有**只读指针**：
//! - `combat_status`：出战状态区（`CombatStatusZone::entries()`）；
//! - `character_states`：目标角色的状态列表（`StateList::entries()`）；
//! - 哪两种状态算护盾 / 减伤由**身份集合**给（[`statuses::SHIELD_STATUSES`] /
//!   [`statuses::REDUCTION_STATUSES`]，D95 / D96 指针身份）。
//!
//! 护盾 / 减伤**挂在哪一处**都读：数据里护盾既可能是出战状态（203）也可能是角色状态（134），
//! 规则书没规定；本层按「两处相加」处理（**待复核**）。
//!
//! ## 插入式结算（D196）
//!
//! [`schedule`] 返回 [`ScheduledChain`]：内部是一个
//! `timeline::Resolution`（**帧 + cursor**），普通路径的步骤 =
//! 基础 -> 加伤 -> 乘算 -> 护盾 -> 减伤 -> 扣血（免疫 / 穿透是短路径）。
//! 调用方可以：
//! - 直接 [`ScheduledChain::run`] 跑完（= 子结算执行完回到插入点）；
//! - [`ScheduledChain::insert`] 在当前位置**插入**一段更小的子结算（例如「扣血后」反应），
//!   做完自动弹回本链的 cursor 继续；
//! - [`ScheduledChain::snapshot`] 导出只读嵌套状态（`结算顺序.md` §五 debug）。
//!
//! 本层**不是栈**：插入不重排已排好的步骤（`timeline::suspend` 的语义）。
//!
//! ## 纯函数 / 数值口径
//!
//! - 吃 `&self` 的区、返回新结果；不改入参（D183）；
//! - 全部 `Nat` 饱和运算（`nadd!` / `nsub!`），零浮点；
//! - 护盾 / 减伤的计算**复用** `rules::ops::damage::mitigate`（唯一一份实现），本层把两次
//!   调用拆成两段只为**观测**（盾段传空减伤、减伤段传 0 盾，同一条穿透分支）。
//!
//! ## 待接入 / 待实测
//!
//! - **护盾扣减写回**：出战状态区 / 状态列表都没有「层数扣减」入口（只有增 / 删），链先只给数值
//!   （`shield_used` / `to_hp`）；把消耗写回区归后续区域块（`待接入`）；
//! - 多个减伤的**叠加方式**：`mitigate` 按「各条相加后一次扣」，规则书没写 -> `待实测`
//!   （Lead 裁定 8）；
//! - 护盾是否在**加伤之后**再吸（本层按规则书：先算出最终伤害，再用盾吸）-> `待实测`；
//! - **免疫的位置 / 语义**：本层按「最前、短路、伤害归 0」实现；规则书无「伤害免疫」词条，
//!   依据是 K5 与卡面「所附属角色免疫所有伤害」；免疫时**是否仍附着元素 / 引发反应**、
//!   免疫与加伤 / 乘算的先后 -> `待实测`（本层仍把 0 点伤害交给 `damage()`，所以附着 /
//!   反应判定语义不变）；
//! - **倍数区叠加**：多个「翻倍」规则书未定义；本层按分工计划「乘数相加」-> `待实测`；
//! - **取整方向**：默认 `Down`，规则书未写 -> `待实测`；
//! - **乘算修饰的端到端接线**：本层已能在 `DamageChainEnv::with_modifiers` 里接收
//!   `AmountOp` 列表（关键字层 `DamageArgs.modifiers` 已接）；卡面「翻倍」的具体
//!   操作仍要由具体卡牌在效果里声明（`待接入` `cards/`）。

use crate::families::action_card::status::{StateIdentity, StateList};
use crate::families::character::damage_chain::bonus::{
    AmountOp, DamageBonus, DamageModifiers, Rounding, ScaledDamage, MASTERY_DAMAGE_PER_POINT,
};
use crate::families::character::damage_chain::record::{
    write_records, DamageChainRecord, DamageChainRecorder, STAGE_BONUS, STAGE_IMMUNITY,
    STAGE_REDUCTION, STAGE_SCALE, STAGE_SHIELD, STAGE_TO_HP, STAGE_TYPE,
};
use crate::families::character::damage_chain::statuses;
use crate::families::character::types::DamageType;
use crate::families::character::zones::combat_status::CombatStatusZone;
use crate::rules::ops::damage::mitigate;
use crate::rules::ops::order::EffectSource;
use crate::timeline::{Event, Queued, Resolution, ResolutionSnapshot};
use gi_genius_invokation_engine::{nadd, nmul, nsub, Nat};

/// 链的**入参**：本次伤害本身（加伤不在这里，由 [`DamageBonus`] 给）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DamageInput {
    /// 基础伤害（技能 / 卡面给的原始数值）。
    pub base: Nat,
    /// 伤害类型（元素 / 物理 / 穿透）。
    pub ty: DamageType,
}

impl DamageInput {
    /// 一次伤害。
    pub const fn new(base: Nat, ty: DamageType) -> DamageInput {
        DamageInput { base, ty }
    }
}

/// 修正链从「区」读数据的**只读依赖**（D183：只借不拥有、不拷贝、不重造承载）。
///
/// 生命周期都是 `&'a`：区由状态层的两个具名实例（我方 / 敌方）持有。
#[derive(Debug)]
pub struct DamageChainEnv<'a> {
    /// 出战状态区（只读指针）：护盾 / 减伤的**出战状态**承载。
    pub combat_status: &'a CombatStatusZone,
    /// 目标角色的状态列表（只读指针）：护盾 / 减伤的**角色状态**承载。
    pub character_states: &'a StateList,
    /// 算作**护盾**的状态身份集合（默认 [`statuses::SHIELD_STATUSES`]）。
    pub shields: &'a [&'static dyn StateIdentity],
    /// 算作**减伤**的状态身份集合（默认 [`statuses::REDUCTION_STATUSES`]）。
    pub reductions: &'a [&'static dyn StateIdentity],
    /// 算作**伤害免疫**的状态身份集合（默认 [`statuses::IMMUNITY_STATUSES`]）。
    ///
    /// 命中 = 本次伤害归 0（不结算）；**穿透伤害不吃免疫**（K5）。
    pub immunities: &'a [&'static dyn StateIdentity],
    /// 乘算 / 除算 / 区外加减（分工计划 §4.4.5；默认 [`DamageModifiers::NONE`]）。
    pub modifiers: DamageModifiers<'a>,
    /// **伤害类型覆盖**（卡面「把造成的物理伤害变为水元素伤害」类效果；默认 None = 不覆盖）。
    ///
    /// 作用点 = **进入计算前**：覆盖后的类型用于穿透 / 免疫 / 乘算等全部数值判定；
    /// 是否覆盖后**再判反应**由调用方决定（[crate::families::character::damage::damage_with_chain]
    /// 会用同一个 effective type 做反应预测与判定，所以覆盖后元素伤害正常引发反应）。
    pub type_override: Option<DamageType>,
}

impl<'a> DamageChainEnv<'a> {
    /// 用规则书默认身份集合建环境（护盾 = 203 + 134，减伤 = 伤害抵消）。
    pub fn new(combat_status: &'a CombatStatusZone, character_states: &'a StateList) -> Self {
        DamageChainEnv {
            combat_status,
            character_states,
            shields: statuses::SHIELD_STATUSES,
            reductions: statuses::REDUCTION_STATUSES,
            immunities: statuses::IMMUNITY_STATUSES,
            modifiers: DamageModifiers::NONE,
            type_override: None,
        }
    }

    /// 换一组护盾 / 减伤身份（自定义卡牌定义 / 测试用）。
    pub fn with_identities(
        mut self,
        shields: &'a [&'static dyn StateIdentity],
        reductions: &'a [&'static dyn StateIdentity],
    ) -> Self {
        self.shields = shields;
        self.reductions = reductions;
        self
    }

    /// 换一组**伤害免疫**身份（具体护罩状态的正式定义 / 测试用）。
    pub fn with_immunities(mut self, immunities: &'a [&'static dyn StateIdentity]) -> Self {
        self.immunities = immunities;
        self
    }

    /// 挂上乘算 / 除算修饰（分工计划 §4.4.5 的 `AmountOp` 列表 + 取整方向）。
    pub fn with_modifiers(mut self, modifiers: DamageModifiers<'a>) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// **覆盖本次伤害的类型**（默认 None = 不覆盖；None 时与改前逐字节等价）。
    ///
    /// 依据：天赋 / 状态卡面「使角色造成的物理伤害变为水元素伤害」类效果；规则书没有
    /// 「伤害类型转换」词条 —— 口径 = 覆盖后完全按新类型结算（含反应判定、穿透 / 免疫）；
    /// 覆盖发生在**进入链之前**，所以记录里能看到原始类型与新类型两条。
    pub fn with_type_override(mut self, ty: Option<DamageType>) -> Self {
        self.type_override = ty;
        self
    }

    /// 本次伤害**实际生效**的类型（有覆盖用覆盖值，否则用原始值）。
    pub fn effective_type(&self, original: DamageType) -> DamageType {
        self.type_override.unwrap_or(original)
    }

    /// 目标身上有没有**伤害免疫**（出战状态区 + 角色状态列表；两处都读）。
    ///
    /// 依据：卡面护罩类状态「所附属角色免疫所有伤害」；挂在哪一处规则书没规定，
    /// 与护盾 / 减伤同口径两处都读（`待复核`）。
    pub fn is_immune(&self) -> bool {
        self.combat_status
            .entries()
            .iter()
            .any(|entry| statuses::identity_in(entry.status, self.immunities))
            || self
                .character_states
                .entries()
                .iter()
                .any(|entry| statuses::identity_in(entry.state, self.immunities))
    }

    /// 当前**护盾点数**（出战状态层 + 角色状态层相加；布尔类按 1 点）。
    pub fn shield_points(&self) -> Nat {
        let mut total = Nat::ZERO;
        for entry in self.combat_status.entries() {
            if statuses::identity_in(entry.status, self.shields) {
                total = nadd!(total, layers_or_one(entry.layers));
            }
        }
        for entry in self.character_states.entries() {
            if statuses::identity_in(entry.state, self.shields) {
                total = nadd!(total, layers_or_one(entry.layers));
            }
        }
        total
    }

    /// 当前**各条减伤**（一条一个 `Nat`；布尔类按 1 点）。
    ///
    /// 顺序按 `结算顺序.md` §一 第 2 条：**角色装备和状态 -> 阵营出战状态**；
    /// 同一来源类别内按列表顺序（= 进场先后，第 3 条）。数值相加，顺序不影响总和。
    pub fn reduction_points(&self) -> Vec<Nat> {
        let mut points = Vec::new();
        for entry in self.character_states.entries() {
            if statuses::identity_in(entry.state, self.reductions) {
                points.push(layers_or_one(entry.layers));
            }
        }
        for entry in self.combat_status.entries() {
            if statuses::identity_in(entry.status, self.reductions) {
                points.push(layers_or_one(entry.layers));
            }
        }
        points
    }
}

/// 层数取点数：`Some(n)` -> n；布尔类（`None`）按 1 点。
///
/// 依据：「每层抵消 1 点」是卡面统一写法；布尔状态在列表里即代表「有 1 层」。
fn layers_or_one(layers: Option<Nat>) -> Nat {
    layers.unwrap_or(Nat::new(1))
}

/// 修正链的**一段**（观测用；顺序就是执行顺序，`结算顺序.md` §三 第 1 行）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChainStage {
    /// 基础伤害（链的起点）。
    Base {
        /// 基础伤害。
        amount: Nat,
    },
    /// 穿透伤害：不吃加伤（规则书·特殊词条 穿透伤害）。
    Bypassed {
        /// 原样通过的数值。
        amount: Nat,
    },
    /// 加伤之后的伤害值。
    Boosted {
        /// 本次加上的点数。
        added: Nat,
        /// 加伤后的伤害值。
        total: Nat,
    },
    /// **伤害免疫**：本次伤害归 0（不结算；穿透不吃免疫）。
    Immune {
        /// 被免疫掉的基础伤害。
        ignored: Nat,
    },
    /// **乘算 / 除算之后**的伤害值（分工计划 §4.4.5 第 2 / 3 / 4 段）。
    Scaled {
        /// 乘数合计（独立乘法的积 × 倍数区和；没有 -> 1）。
        multiplier: Nat,
        /// 除数的乘积（没有 -> 1）。
        divisor: Nat,
        /// 乘除之后的伤害值。
        total: Nat,
    },
    /// 护盾抵消之后。
    Shielded {
        /// 被护盾吸掉的点数。
        absorbed: Nat,
        /// 抵消后剩余的伤害值。
        remaining: Nat,
    },
    /// 减伤扣减之后。
    Reduced {
        /// 被减伤扣掉的点数。
        removed: Nat,
    },
    /// 最终扣到生命值的伤害。
    ToHp {
        /// 扣血数值（`<=` 前面各段的结果；不会为负）。
        amount: Nat,
    },
}

/// 修正链的**最终结果**（纯数据；不含区、不含引用）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChainOutcome {
    /// 入参（基础伤害 + 类型）。
    pub input: DamageInput,
    /// 加伤总计（穿透恒为 0）。
    pub bonus_total: Nat,
    /// 加伤后的伤害值（穿透 = 基础伤害；免疫 = 0）。
    pub damage_after_bonus: Nat,
    /// 乘算 / 除算之后的伤害值（穿透 / 免疫 = 加伤后的值）。
    pub after_scale: Nat,
    /// 读到的护盾总点数（穿透不读 / 不吃，这里是「本来有多少」）。
    pub shield_points: Nat,
    /// 被护盾消耗掉的点数（穿透恒为 0）。
    pub shield_used: Nat,
    /// 被减伤扣掉的点数（穿透恒为 0）。
    pub reduction_total: Nat,
    /// 最终扣到生命值的伤害。
    pub to_hp: Nat,
    /// 是不是穿透伤害（不吃加伤 / 乘算 / 护盾 / 减伤 / 免疫）。
    pub bypassed: bool,
    /// 目标身上**有没有**免疫（穿透也如实反映）。
    pub immunity_available: bool,
    /// 是不是**伤害免疫**（本次伤害归 0；穿透不吃免疫）。
    pub immune: bool,
    /// 乘算 / 除算的展开（`None` = 没走这一段：穿透 / 免疫）。
    pub scale: Option<ScaledDamage>,
    /// 各段记录（顺序 = 执行顺序）。
    pub stages: Vec<ChainStage>,
}

/// 链的**一步**的效果函数指针（D186：登记函数指针，不登记效果数据）。
pub type ChainEffect = fn(&mut ChainWork, &Event);

/// 链的**工作区**：步骤之间传递的全部数值（纯值，不含区、不含引用）。
///
/// 步骤函数只读写它；链跑完后 [`ChainWork::outcome`] 折叠成 [`ChainOutcome`]。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChainWork {
    input: DamageInput,
    bonus_total: Nat,
    shield_points: Nat,
    reductions: Vec<Nat>,
    damage_after_bonus: Nat,
    after_scale: Nat,
    after_shield: Nat,
    shield_used: Nat,
    reduction_total: Nat,
    after_reduction: Nat,
    to_hp: Nat,
    bypassed: bool,
    /// 目标身上**有没有**免疫（穿透也如实记：穿透不吃，但观测上知道它存在）。
    immunity_available: bool,
    /// 本次是否**真的**被免疫（穿透恒 false）。
    immune: bool,
    /// 乘算 / 除算的修正（从环境读进来的 owned 副本；步骤函数只吃工作区）。
    ops: Vec<AmountOp>,
    rounding: Option<Rounding>,
    /// 乘算 / 除算的展开（没走那一段 -> `None`）。
    scale: Option<ScaledDamage>,
    stages: Vec<ChainStage>,
    /// 是否**打开记录**（只有记录版入口会打开；普通入口连记录内容都不构造，D223 零开销）。
    recording: bool,
    /// 统一记录器（D223 / D224；关闭 feature 时零大小类型、push 空操作）。
    records: DamageChainRecorder,
}

impl ChainWork {
    /// 用读好的区数据建工作区（区只在这一步被读，之后工作区不再碰区）。
    pub fn new(
        input: DamageInput,
        bonus_total: Nat,
        shield_points: Nat,
        reductions: Vec<Nat>,
    ) -> ChainWork {
        ChainWork {
            input,
            bonus_total,
            shield_points,
            reductions,
            damage_after_bonus: input.base,
            after_scale: input.base,
            after_shield: input.base,
            shield_used: Nat::ZERO,
            reduction_total: Nat::ZERO,
            after_reduction: input.base,
            to_hp: Nat::ZERO,
            bypassed: false,
            immunity_available: false,
            immune: false,
            ops: Vec::new(),
            rounding: None,
            scale: None,
            stages: Vec::new(),
            recording: false,
            records: DamageChainRecorder::new(),
        }
    }

    /// 链的入参。
    pub fn input(&self) -> DamageInput {
        self.input
    }

    /// 加伤总计。
    pub fn bonus_total(&self) -> Nat {
        self.bonus_total
    }

    /// 读到的护盾总点数。
    pub fn shield_points(&self) -> Nat {
        self.shield_points
    }

    /// 被护盾消耗的点数。
    pub fn shield_used(&self) -> Nat {
        self.shield_used
    }

    /// 被减伤扣掉的点数。
    pub fn reduction_total(&self) -> Nat {
        self.reduction_total
    }

    /// 最终扣血数值。
    pub fn to_hp(&self) -> Nat {
        self.to_hp
    }

    /// 是不是穿透伤害。
    pub fn bypassed(&self) -> bool {
        self.bypassed
    }

    /// 目标身上有没有伤害免疫（穿透也如实反映）。
    pub fn immunity_available(&self) -> bool {
        self.immunity_available
    }

    /// 本次是不是**真的**被免疫（穿透恒 `false`）。
    pub fn immune(&self) -> bool {
        self.immune
    }

    /// 乘算 / 除算之后的伤害值（没走那一段 = 加伤后的值）。
    pub fn after_scale(&self) -> Nat {
        self.after_scale
    }

    /// 乘算 / 除算的展开（没走那一段 -> `None`）。
    pub fn scale(&self) -> Option<ScaledDamage> {
        self.scale
    }

    /// 各段记录（按执行顺序）。
    pub fn stages(&self) -> &[ChainStage] {
        &self.stages
    }

    /// 是否打开了记录。
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// 记录器（只读；关闭 feature 时恒为空）。
    pub fn records(&self) -> &DamageChainRecorder {
        &self.records
    }

    /// 折叠成最终结果（不改工作区）。
    pub fn outcome(&self) -> ChainOutcome {
        ChainOutcome {
            input: self.input,
            bonus_total: self.bonus_total,
            damage_after_bonus: self.damage_after_bonus,
            after_scale: self.after_scale,
            shield_points: self.shield_points,
            shield_used: self.shield_used,
            reduction_total: self.reduction_total,
            to_hp: self.to_hp,
            bypassed: self.bypassed,
            immunity_available: self.immunity_available,
            immune: self.immune,
            scale: self.scale,
            stages: self.stages.clone(),
        }
    }
}

/// **记一条**（D223 / D224：属于哪一步 + 任意内容）。
///
/// 只在「记录版入口」（[ChainWork::is_recording]）且 feature 打开时生效：
/// - 未打开记录 -> 内容构造函数根本不调用（热路径不多分配一个字节）；
/// - 关闭 feature -> 记录器是零大小类型、本函数空实现（D223：可整体裁剪）。
///
/// 内容构造函数吃 `&ChainWork` 只是为了让调用点能**在计算处**读真实字段（例如减伤各条），
/// 调用方自己决定要不要 clone。
#[cfg(feature = "records")]
fn record_with(
    work: &mut ChainWork,
    step: Nat,
    make: impl FnOnce(&ChainWork) -> DamageChainRecord,
) {
    if !work.recording {
        return;
    }
    let content = make(work);
    work.records.push(step, content);
}

/// 关闭 feature 时的空实现：**连内容都不构造**（零开销，D223）。
#[cfg(not(feature = "records"))]
fn record_with(
    _work: &mut ChainWork,
    _step: Nat,
    _make: impl FnOnce(&ChainWork) -> DamageChainRecord,
) {
}

/// 第 0 步：**基础伤害**（链的起点；每条路径都走，观测上永远第一段）。
fn step_base(work: &mut ChainWork, _event: &Event) {
    work.stages.push(ChainStage::Base {
        amount: work.input.base,
    });
}

/// 第 0.5 步：**伤害免疫**（本次伤害归 0 / 不结算；穿透不进这一步）。
///
/// 依据：卡面护罩类状态「所附属角色免疫所有伤害」+ 权威关键字 K5（穿透伤害「无法被
/// 护盾、伤害免疫等效果抵消」）。规则书正文没有该词条，所以「免疫在加伤之前」是
/// 本层对「不结算」语义的落地，`待实测`。
fn step_immunity(work: &mut ChainWork, _event: &Event) {
    let ignored = work.input.base;
    work.immune = true;
    work.bonus_total = Nat::ZERO;
    work.damage_after_bonus = Nat::ZERO;
    work.after_scale = Nat::ZERO;
    work.after_shield = Nat::ZERO;
    work.after_reduction = Nat::ZERO;
    work.stages.push(ChainStage::Immune { ignored });
    // 记录：本次伤害被免疫（归 0），被免疫掉的数值是真实值。
    record_with(work, STAGE_IMMUNITY, move |_work| {
        DamageChainRecord::Immune { ignored }
    });
}

/// 第 1 步：**加伤**（穿透不吃）。
fn step_bonus(work: &mut ChainWork, _event: &Event) {
    if work.input.ty == DamageType::Piercing {
        // 规则书：穿透伤害「不会受到其他效果的伤害加成」-> 实际生效的加伤为 0。
        let ignored_bonus = work.bonus_total;
        work.bypassed = true;
        work.bonus_total = Nat::ZERO;
        work.damage_after_bonus = work.input.base;
        work.after_scale = work.input.base;
        work.stages.push(ChainStage::Bypassed {
            amount: work.input.base,
        });
        // 记录：被忽略的加伤 / 护盾 / 减伤 / 免疫（真实值，不是 diff）。
        let ignored_immunity = work.immunity_available;
        record_with(work, STAGE_BONUS, move |work| {
            let mut ignored_reduction = Nat::ZERO;
            for reduction in &work.reductions {
                ignored_reduction = nadd!(ignored_reduction, *reduction);
            }
            DamageChainRecord::PiercingBypass {
                ignored_bonus,
                ignored_shield: work.shield_points,
                ignored_reduction,
                ignored_immunity,
            }
        });
        return;
    }
    work.damage_after_bonus = nadd!(work.input.base, work.bonus_total);
    work.stages.push(ChainStage::Boosted {
        added: work.bonus_total,
        total: work.damage_after_bonus,
    });
    // 记录：加伤合计（明细在 `build` 里按 bonus 各项推入）。
    record_with(work, STAGE_BONUS, |work| DamageChainRecord::BonusTotal {
        total: work.bonus_total,
    });
}

/// 第 2 步：**乘算 / 除算**（分工计划 §4.4.5 四段模型；穿透 / 免疫不走这一步）。
fn step_scale(work: &mut ChainWork, _event: &Event) {
    let modifiers = DamageModifiers {
        // `ops` 是 owned 的 Vec：借用期只在本步骤内，复制到临时借用即可。
        ops: &work.ops,
        rounding: work.rounding,
    };
    let scaled = modifiers.scale(work.damage_after_bonus);
    work.after_scale = scaled.total;
    work.scale = Some(scaled);
    let zone_factor = if scaled.zone.is_empty() {
        Nat::new(1)
    } else {
        scaled.zone.mul_sum
    };
    let multiplier = nmul!(scaled.standalone, zone_factor);
    work.stages.push(ChainStage::Scaled {
        multiplier,
        divisor: scaled.divisor,
        total: scaled.total,
    });
    // 记录：乘数 / 除数 / 取整 / 区外加減 / 最终值（都是计算处的真实值）。
    record_with(work, STAGE_SCALE, move |_work| {
        DamageChainRecord::Scale {
            standalone: scaled.standalone,
            zone_mul: scaled.zone.mul_sum,
            divisor: scaled.divisor,
            rounding: scaled.rounding,
            added: scaled.added,
            reduced: scaled.reduced,
            total: scaled.total,
        }
    });
}

/// 第 3 步：**护盾抵消**（复用 `mitigate`；穿透走它的穿透分支，吃不到盾）。
fn step_shield(work: &mut ChainWork, _event: &Event) {
    let mitigation = mitigate(work.after_scale, work.input.ty, work.shield_points, &[]);
    work.shield_used = mitigation.shield_used;
    work.after_shield = mitigation.to_hp;
    work.stages.push(ChainStage::Shielded {
        absorbed: mitigation.shield_used,
        remaining: mitigation.to_hp,
    });
    // 记录：可吸收 / 实际吸收 / 剩余伤害（三个都是这一步算出来的）。
    let absorbed = mitigation.shield_used;
    let remaining = mitigation.to_hp;
    record_with(work, STAGE_SHIELD, move |work| DamageChainRecord::Shield {
        available: work.shield_points,
        absorbed,
        remaining,
    });
}

/// 第 3 步：**减伤扣减**（复用 `mitigate`，盾传 0 -> 只吃减伤分支）。
fn step_reduction(work: &mut ChainWork, _event: &Event) {
    let mitigation = mitigate(
        work.after_shield,
        work.input.ty,
        Nat::ZERO,
        &work.reductions,
    );
    work.reduction_total = nsub!(work.after_shield, mitigation.to_hp);
    work.after_reduction = mitigation.to_hp;
    work.stages.push(ChainStage::Reduced {
        removed: work.reduction_total,
    });
    // 记录：各条减伤 / 请求合计 / 实际扣掉 / 减后剩余。
    let removed = work.reduction_total;
    let remaining = mitigation.to_hp;
    record_with(work, STAGE_REDUCTION, move |work| {
        let mut requested = Nat::ZERO;
        for reduction in &work.reductions {
            requested = nadd!(requested, *reduction);
        }
        DamageChainRecord::Reduction {
            items: work.reductions.clone(),
            requested,
            removed,
            remaining,
        }
    });
}

/// 第 4 步：**扣血数值**（本层不扣血；扣血归 `damage()`）。
fn step_to_hp(work: &mut ChainWork, _event: &Event) {
    work.to_hp = work.after_reduction;
    work.stages.push(ChainStage::ToHp { amount: work.to_hp });
    // 记录：最终扣到生命值的伤害。
    record_with(work, STAGE_TO_HP, |work| DamageChainRecord::ToHp {
        amount: work.to_hp,
    });
}

/// 把一段效果函数指针包成一条**待执行步骤**（`owner` = 0：链不挂在具体实体上）。
fn queued(effect: ChainEffect, entry: u32, source_kind: EffectSource) -> Queued<ChainEffect> {
    Queued {
        effect,
        event: Event::EMPTY,
        owner: Nat::ZERO,
        source_kind,
        entry: Nat::new(entry),
    }
}

/// 排好的**伤修链子结算**（D196：帧 + cursor；可以继续插入更小的子结算）。
pub struct ScheduledChain {
    work: ChainWork,
    resolution: Resolution<ChainEffect>,
}

impl ScheduledChain {
    /// 工作区（步骤执行前后的数值都在这里；只读）。
    pub fn work(&self) -> &ChainWork {
        &self.work
    }

    /// 只读快照：当前帧 / 剩余步骤 / 插入点（`结算顺序.md` §五 debug）。
    pub fn snapshot(&self) -> ResolutionSnapshot {
        self.resolution.snapshot()
    }

    /// **插入子结算**（D196）：在当前位置压入新的一帧；做完回到本链的 cursor 继续。
    ///
    /// 不重排本链已排好的步骤（不是 LIFO 触发栈）。
    pub fn insert(&mut self, steps: Vec<Queued<ChainEffect>>) {
        self.resolution.insert(steps);
    }

    /// 跑完整个子结算（= 插入点后面继续原结算）。
    pub fn run(mut self) -> ChainOutcome {
        while let Some(step) = self.resolution.next_step() {
            (step.effect)(&mut self.work, &step.event);
        }
        self.work.outcome()
    }

    /// 跑完整个子结算，**并把记录器交出来**（D223：只读观测，不改结果）。
    ///
    /// 记录里的步号是链内阶段步号（[STAGE_BONUS] 等）；要并进外层的结算步号时用
    /// `record::write_records` 重盖（见 [settle_recorded]）。
    pub fn run_recorded(mut self) -> (ChainOutcome, DamageChainRecorder) {
        while let Some(step) = self.resolution.next_step() {
            (step.effect)(&mut self.work, &step.event);
        }
        let outcome = self.work.outcome();
        // 直接移出记录器（不依赖 Recorder: Default：关闭 feature 时它是零大小类型，
        // derive 出来的 Default 会额外要求内容类型实现 Default）。
        let ChainWork { records, .. } = self.work;
        (outcome, records)
    }
}

/// 排子结算的**唯一实现**：先读区，再按路径排步骤。
///
/// 路径（按顺序）：
/// - **免疫**（目标有免疫且不是穿透）：`基础 -> 免疫 -> 扣血 0`（短路，不结算）；
/// - **穿透**：`基础 -> 加伤（整段跳过）-> 护盾 -> 减伤 -> 扣血`（不吃乘算 / 免疫）；
/// - **普通**：`基础 -> 加伤 -> 乘算 -> 护盾 -> 减伤 -> 扣血`。
///
/// `recording` = false 时不构造任何记录内容（热路径零额外分配）；true 时先按 `bonus` 的
/// 各项推入「加伤明细」（明细是对 `DamageBonus` 的逐项展开，不是前后 diff）。
fn build(
    input: DamageInput,
    env: &DamageChainEnv<'_>,
    bonus: &DamageBonus<'_>,
    recording: bool,
) -> ScheduledChain {
    // 「伤害类型转换」在**进入计算前**生效：之后的穿透 / 免疫 / 乘算 / 反应全按新类型。
    // 没有覆盖时 original_ty == effective_ty，逐字节等价。
    let original_ty = input.ty;
    let effective_ty = env.effective_type(original_ty);
    let input = DamageInput::new(input.base, effective_ty);
    let immunity_available = env.is_immune();
    // 穿透不吃免疫（K5：「穿透伤害……无法被护盾、伤害免疫等效果抵消」）。
    let immune = immunity_available && input.ty != DamageType::Piercing;
    let mut work = ChainWork::new(
        input,
        if immune { Nat::ZERO } else { bonus.total() },
        env.shield_points(),
        env.reduction_points(),
    );
    work.recording = recording;
    work.immunity_available = immunity_available;
    work.immune = immune;
    // 记录：伤害类型覆盖（原始 -> 实际）；Hit 记录由调用方写原始类型，这里补实际类型。
    if recording && effective_ty != original_ty {
        record_with(&mut work, STAGE_TYPE, move |_work| {
            DamageChainRecord::TypeOverride {
                original: original_ty,
                actual: effective_ty,
            }
        });
    }
    work.ops = env.modifiers.ops.to_vec();
    work.rounding = env.modifiers.rounding;
    // 加伤明细（穿透 / 免疫不吃加伤 -> 不记明细）。
    if recording && input.ty != DamageType::Piercing && !immune {
        for (index, increase) in bonus.increases.iter().enumerate() {
            let index = Nat::new(index as _);
            let amount = *increase;
            record_with(&mut work, STAGE_BONUS, move |_work| {
                DamageChainRecord::Increase { index, amount }
            });
        }
        if bonus.reacted {
            let amount = bonus.reaction_bonus;
            record_with(&mut work, STAGE_BONUS, move |_work| {
                DamageChainRecord::ReactionBonus { amount }
            });
            let mastery = bonus.mastery;
            let per_point = MASTERY_DAMAGE_PER_POINT;
            let added = DamageBonus::mastery_bonus(mastery, true);
            record_with(&mut work, STAGE_BONUS, move |_work| {
                DamageChainRecord::Mastery {
                    mastery,
                    per_point,
                    added,
                }
            });
        }
    }
    let steps = if immune {
        // 免疫短路：不结算加伤 / 乘算 / 护盾 / 减伤。
        vec![
            queued(step_base, 0, EffectSource::CharacterCard),
            queued(step_immunity, 1, EffectSource::CombatStatus),
            queued(step_to_hp, 2, EffectSource::CharacterCard),
        ]
    } else if input.ty == DamageType::Piercing {
        // 穿透：加伤整段跳过、不走乘算、不吃护盾 / 减伤 / 免疫（规则书·特殊词条 + K5）。
        vec![
            queued(step_base, 0, EffectSource::CharacterCard),
            queued(step_bonus, 1, EffectSource::CharacterCard),
            queued(step_shield, 2, EffectSource::CombatStatus),
            queued(step_reduction, 3, EffectSource::CharacterEquipmentOrStatus),
            queued(step_to_hp, 4, EffectSource::CharacterCard),
        ]
    } else {
        // 普通：加伤（角色牌 / 状态 / 装备）-> 乘算 -> 护盾 -> 减伤 -> 扣血。
        vec![
            queued(step_base, 0, EffectSource::CharacterCard),
            queued(step_bonus, 1, EffectSource::CharacterCard),
            queued(step_scale, 2, EffectSource::CharacterEquipmentOrStatus),
            queued(step_shield, 3, EffectSource::CombatStatus),
            queued(step_reduction, 4, EffectSource::CharacterEquipmentOrStatus),
            queued(step_to_hp, 5, EffectSource::CharacterCard),
        ]
    };
    ScheduledChain {
        work,
        resolution: Resolution::new(steps),
    }
}

/// 把「受伤时修正链」**排成子结算**（D196）：先读区，再按路径排步骤（见 [`build`]）。
///
/// **不记录**（热路径）：记录版入口是 [schedule_records]。
pub fn schedule(
    input: DamageInput,
    env: &DamageChainEnv<'_>,
    bonus: &DamageBonus<'_>,
) -> ScheduledChain {
    build(input, env, bonus, false)
}

/// 把「受伤时修正链」排成子结算，并**打开记录**（D223：只多写观测，不改结算）。
pub fn schedule_records(
    input: DamageInput,
    env: &DamageChainEnv<'_>,
    bonus: &DamageBonus<'_>,
) -> ScheduledChain {
    build(input, env, bonus, true)
}

/// **跑一次伤修链**（= `schedule(...).run()`；最常用的入口，不记录）。
pub fn settle(
    input: DamageInput,
    env: &DamageChainEnv<'_>,
    bonus: &DamageBonus<'_>,
) -> ChainOutcome {
    schedule(input, env, bonus).run()
}

/// **跑一次伤修链并收集记录**：返回 (结果, 记录器)。
///
/// 记录里的步号 = 链内阶段步号（0..=3）；要并进外层结算步号用 [settle_recorded]。
pub fn settle_records(
    input: DamageInput,
    env: &DamageChainEnv<'_>,
    bonus: &DamageBonus<'_>,
) -> (ChainOutcome, DamageChainRecorder) {
    schedule_records(input, env, bonus).run_recorded()
}

/// **跑一次伤修链并把记录写进调用方记录器**：记录步号统一为 `step`（D223）。
///
/// 行为与 [settle] 完全一致（同一份计算），只额外写记录；
/// 关闭 feature 时是空写（记录器恒空、零开销）。
pub fn settle_recorded(
    input: DamageInput,
    env: &DamageChainEnv<'_>,
    bonus: &DamageBonus<'_>,
    step: Nat,
    recorder: &mut DamageChainRecorder,
) -> ChainOutcome {
    let (outcome, records) = settle_records(input, env, bonus);
    write_records(&records, step, recorder);
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::damage_chain::statuses::{
        DAMAGE_IMMUNITY, DAMAGE_NEGATION, SHIELD_CHARACTER, SHIELD_COMBAT,
    };
    use crate::families::character::zones::combat_status::CombatStatusTriggers;
    use gi_genius_invokation_engine::EntityId;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 空出战状态区。
    fn empty_combat_status() -> CombatStatusZone {
        CombatStatusZone::new()
    }

    /// 出战状态区：给 id 为 `owner` 的出战角色挂护盾 `points` 点。
    fn combat_status_with_shield(owner: EntityId, points: u32) -> CombatStatusZone {
        let triggers = CombatStatusTriggers::new();
        CombatStatusZone::new()
            .attach_with_triggers(owner, &SHIELD_COMBAT, Some(n(points)), None, &triggers)
            .zone
    }

    /// 角色状态列表：护盾 `shield_points` 点 + 减伤 `negation_points` 点。
    fn states(shield_layers: Option<u32>, negation_layers: Option<u32>) -> StateList {
        let mut list = StateList::new();
        if let Some(layers) = shield_layers {
            list.add(Nat::ZERO, &SHIELD_CHARACTER, Some(n(layers)));
        }
        if let Some(layers) = negation_layers {
            list.add(Nat::ZERO, &DAMAGE_NEGATION, Some(n(layers)));
        }
        list
    }

    fn no_bonus() -> DamageBonus<'static> {
        DamageBonus::NONE
    }

    #[test]
    fn shield_absorbs_then_reduction_subtracts_then_hp() {
        // 5 点元素伤害：盾 2 先吸 2 -> 3；减伤 1 -> 扣血 2。
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = states(None, Some(1));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let outcome = settle(
            DamageInput::new(n(5), DamageType::elemental(crate::base::Element::Pyro)),
            &env,
            &no_bonus(),
        );
        assert_eq!(outcome.shield_points, n(2));
        assert_eq!(outcome.shield_used, n(2));
        assert_eq!(outcome.reduction_total, n(1));
        assert_eq!(outcome.to_hp, n(2));
        assert!(!outcome.bypassed);
        // 阶段顺序 = 基础 -> 加伤 -> 乘算 -> 护盾 -> 减伤 -> 扣血。
        assert_eq!(
            outcome.stages,
            vec![
                ChainStage::Base { amount: n(5) },
                ChainStage::Boosted {
                    added: Nat::ZERO,
                    total: n(5)
                },
                // 没有乘算修饰：恒等段（乘数 / 除数都是 1）。
                ChainStage::Scaled {
                    multiplier: n(1),
                    divisor: n(1),
                    total: n(5)
                },
                ChainStage::Shielded {
                    absorbed: n(2),
                    remaining: n(3)
                },
                ChainStage::Reduced { removed: n(1) },
                ChainStage::ToHp { amount: n(2) },
            ]
        );
    }

    #[test]
    fn shield_larger_than_damage_absorbs_everything() {
        // 盾 6 > 伤害 2：全部吸掉，扣血 0（减伤不再有东西可减）。
        let combat_status = combat_status_with_shield(n(1), 6);
        let character_states = states(None, Some(2));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let outcome = settle(
            DamageInput::new(n(2), DamageType::Physical),
            &env,
            &no_bonus(),
        );
        assert_eq!(outcome.shield_used, n(2));
        assert_eq!(outcome.to_hp, Nat::ZERO);
        assert_eq!(outcome.reduction_total, Nat::ZERO);
    }

    #[test]
    fn shield_points_come_from_both_zone_carriers() {
        // 护盾在两处承载（出战状态 203 + 角色状态 134）时相加（**待复核**：规则书没规定挂哪）。
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = states(Some(3), None);
        let env = DamageChainEnv::new(&combat_status, &character_states);
        assert_eq!(env.shield_points(), n(5));
        let outcome = settle(
            DamageInput::new(n(4), DamageType::Physical),
            &env,
            &no_bonus(),
        );
        assert_eq!(outcome.shield_used, n(4));
        assert_eq!(outcome.to_hp, Nat::ZERO);
    }

    #[test]
    fn multiple_reductions_stack_additively() {
        // 3 条减伤（角色状态 2 条 + 出战状态 1 条）合计 1+2+3 = 6。
        let mut combat_status = CombatStatusZone::new();
        let triggers = CombatStatusTriggers::new();
        combat_status = combat_status
            .attach_with_triggers(n(1), &DAMAGE_NEGATION, Some(n(3)), None, &triggers)
            .zone;
        let character_states = states(None, Some(1));
        let env = DamageChainEnv::new(&combat_status, &character_states)
            .with_identities(statuses::SHIELD_STATUSES, statuses::REDUCTION_STATUSES);
        assert_eq!(env.reduction_points(), vec![n(1), n(3)]);
        let outcome = settle(
            DamageInput::new(n(8), DamageType::Physical),
            &env,
            &no_bonus(),
        );
        assert_eq!(outcome.reduction_total, n(4));
        assert_eq!(outcome.to_hp, n(4));
    }

    #[test]
    fn piercing_bypasses_bonus_shield_and_reduction() {
        // 穿透：加伤不生效、盾不吃、减伤不吃（规则书·特殊词条 穿透伤害）。
        let combat_status = combat_status_with_shield(n(1), 10);
        let character_states = states(None, Some(5));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let bonus = DamageBonus {
            increases: &[n(4)],
            mastery: n(3),
            reaction_bonus: n(2),
            reacted: true,
        };
        let outcome = settle(DamageInput::new(n(3), DamageType::Piercing), &env, &bonus);
        assert!(outcome.bypassed);
        assert_eq!(outcome.bonus_total, n(0), "穿透伤害不吃任何伤害加成");
        assert_eq!(outcome.damage_after_bonus, n(3));
        assert_eq!(outcome.shield_used, Nat::ZERO);
        assert_eq!(outcome.reduction_total, Nat::ZERO);
        assert_eq!(outcome.to_hp, n(3));
        // 阶段里明确记下「穿透通过」，盾 / 减伤段都是 0。
        assert_eq!(
            outcome.stages,
            vec![
                ChainStage::Base { amount: n(3) },
                ChainStage::Bypassed { amount: n(3) },
                ChainStage::Shielded {
                    absorbed: Nat::ZERO,
                    remaining: n(3)
                },
                ChainStage::Reduced { removed: Nat::ZERO },
                ChainStage::ToHp { amount: n(3) },
            ]
        );
    }

    #[test]
    fn bonus_applies_before_shield() {
        // 顺序钉死：先加伤再盾。基础 2 + 增伤 2 = 4；盾 3 吸 3 -> 扣血 1。
        // 若按「先盾后加伤」：2 - 3 = 0，再加 2 -> 2（与规则书矛盾）。
        let combat_status = combat_status_with_shield(n(1), 3);
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let bonus = DamageBonus {
            increases: &[n(2)],
            ..DamageBonus::NONE
        };
        let outcome = settle(
            DamageInput::new(n(2), DamageType::elemental(crate::base::Element::Cryo)),
            &env,
            &bonus,
        );
        assert_eq!(outcome.damage_after_bonus, n(4));
        assert_eq!(outcome.shield_used, n(3));
        assert_eq!(outcome.to_hp, n(1));
    }

    #[test]
    fn chain_is_a_sub_resolution_with_cursor_and_snapshot() {
        // D196：链 = 一帧 + cursor；排好 4 步、cursor 在 0、剩余 4。
        let combat_status = empty_combat_status();
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let scheduled = schedule(
            DamageInput::new(n(3), DamageType::Physical),
            &env,
            &no_bonus(),
        );
        let snapshot = scheduled.snapshot();
        assert_eq!(snapshot.frames.len(), 1);
        assert_eq!(snapshot.current, Some(Nat::ZERO));
        assert_eq!(snapshot.frames[0].cursor, Nat::ZERO);
        // 普通路径 6 步：基础 -> 加伤 -> 乘算 -> 护盾 -> 减伤 -> 扣血。
        assert_eq!(snapshot.frames[0].remaining, n(6));
        assert!(scheduled.work().stages().is_empty());
        // 跑完：4 段记录都在。
        let outcome = scheduled.run();
        assert_eq!(outcome.to_hp, n(3));
    }

    /// 测试用插入步骤：往阶段里塞一条「假扣血」，用来观察插入点。
    fn fake_step(work: &mut ChainWork, _event: &Event) {
        work.stages.push(ChainStage::ToHp { amount: n(99) });
    }

    #[test]
    fn inserted_sub_resolution_runs_at_the_insertion_point_then_parent_continues() {
        // D196：插入的子结算在当前位置执行完，回到本链 cursor 继续父步骤（不重排）。
        let combat_status = empty_combat_status();
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let mut scheduled = schedule(
            DamageInput::new(n(3), DamageType::Physical),
            &env,
            &no_bonus(),
        );
        scheduled.insert(vec![Queued {
            effect: fake_step,
            event: Event::EMPTY,
            owner: Nat::ZERO,
            source_kind: EffectSource::CharacterCard,
            entry: Nat::ZERO,
        }]);
        assert_eq!(scheduled.snapshot().frames.len(), 2);
        let outcome = scheduled.run();
        // 插入段先跑（99），然后父链 4 步按原顺序跑完。
        assert_eq!(
            outcome.stages,
            vec![
                ChainStage::ToHp { amount: n(99) },
                ChainStage::Base { amount: n(3) },
                ChainStage::Boosted {
                    added: Nat::ZERO,
                    total: n(3)
                },
                ChainStage::Scaled {
                    multiplier: n(1),
                    divisor: n(1),
                    total: n(3)
                },
                ChainStage::Shielded {
                    absorbed: Nat::ZERO,
                    remaining: n(3)
                },
                ChainStage::Reduced { removed: Nat::ZERO },
                ChainStage::ToHp { amount: n(3) },
            ]
        );
        assert_eq!(outcome.to_hp, n(3));
    }

    #[test]
    fn zero_damage_chain_still_runs_every_stage() {
        // 0 点伤害：链照样跑完（护盾 / 减伤都不吃，扣血 0）。
        let combat_status = combat_status_with_shield(n(1), 3);
        let character_states = states(None, Some(2));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let outcome = settle(
            DamageInput::new(Nat::ZERO, DamageType::Physical),
            &env,
            &no_bonus(),
        );
        assert_eq!(outcome.to_hp, Nat::ZERO);
        assert_eq!(outcome.shield_used, Nat::ZERO);
        // 6 段：基础 / 加伤 / 乘算 / 护盾 / 减伤 / 扣血。
        assert_eq!(outcome.stages.len(), 6);
    }

    #[test]
    fn state_lists_are_read_without_mutation() {
        // D183：链只读区；跑完后区一个字节都没变。
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = states(Some(1), Some(2));
        let before_combat = combat_status.clone();
        let before_states = character_states.clone();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let _ = settle(
            DamageInput::new(n(9), DamageType::elemental(crate::base::Element::Hydro)),
            &env,
            &no_bonus(),
        );
        assert_eq!(combat_status, before_combat);
        assert_eq!(character_states, before_states);
    }

    #[test]
    fn boolean_status_counts_as_one_point_of_its_kind() {
        // 布尔状态在列表里即代表「有 1 层」：护盾 / 减伤都按 **1 点** 读（卡面「每层抵消 1 点」）。
        let triggers = CombatStatusTriggers::new();
        let combat_status = CombatStatusZone::new()
            .attach_with_triggers(n(1), &statuses::FROZEN, None, None, &triggers)
            .zone;
        let character_states = StateList::new();
        // 冻结是**布尔**身份（没有层数）；这里把它临时当「减伤身份」读 -> 1 点。
        const FROZEN_ONLY: [&'static dyn StateIdentity; 1] = [&statuses::FROZEN];
        let reductions = DamageChainEnv::new(&combat_status, &character_states)
            .with_identities(statuses::SHIELD_STATUSES, &FROZEN_ONLY);
        assert_eq!(reductions.reduction_points(), vec![n(1)]);
        // 同样把它当「护盾身份」读 -> 1 点，且真的按 1 点抵消伤害。
        let shields = DamageChainEnv::new(&combat_status, &character_states)
            .with_identities(&FROZEN_ONLY, statuses::REDUCTION_STATUSES);
        assert_eq!(shields.shield_points(), n(1));
        let outcome = settle(
            DamageInput::new(n(1), DamageType::Physical),
            &shields,
            &no_bonus(),
        );
        assert_eq!(outcome.shield_used, n(1));
        assert_eq!(outcome.to_hp, Nat::ZERO);
    }

    // ------------------------------------------------------------------
    // 伤害免疫（卡面「免疫所有伤害」+ K5：穿透不吃免疫）
    // ------------------------------------------------------------------

    /// 出战状态区：给 id 为 `owner` 的出战角色挂「伤害免疫」。
    fn combat_status_with_immunity(owner: EntityId) -> CombatStatusZone {
        let triggers = CombatStatusTriggers::new();
        CombatStatusZone::new()
            .attach_with_triggers(owner, &DAMAGE_IMMUNITY, None, None, &triggers)
            .zone
    }

    #[test]
    fn immunity_short_circuits_the_chain_and_deals_zero() {
        // 免疫：本次伤害归 0，整链短路（基础 -> 免疫 -> 扣血 0），加伤 / 护盾 / 减伤都不结算。
        let combat_status = combat_status_with_immunity(n(1));
        let character_states = states(Some(3), Some(2));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let bonus = DamageBonus {
            increases: &[n(4)],
            mastery: n(3),
            reaction_bonus: n(2),
            reacted: true,
        };
        let outcome = settle(
            DamageInput::new(n(5), DamageType::elemental(crate::base::Element::Pyro)),
            &env,
            &bonus,
        );
        assert!(outcome.immune);
        assert_eq!(outcome.to_hp, Nat::ZERO);
        assert_eq!(outcome.bonus_total, Nat::ZERO, "免疫时不结算加伤");
        assert_eq!(outcome.shield_used, Nat::ZERO);
        assert_eq!(outcome.reduction_total, Nat::ZERO);
        assert_eq!(outcome.scale, None, "免疫时不结算乘算");
        assert_eq!(
            outcome.stages,
            vec![
                ChainStage::Base { amount: n(5) },
                ChainStage::Immune { ignored: n(5) },
                ChainStage::ToHp { amount: Nat::ZERO },
            ]
        );
    }

    #[test]
    fn piercing_ignores_immunity() {
        // K5：「穿透伤害……无法被护盾、伤害免疫等效果抵消」-> 穿透照常扣血。
        let combat_status = combat_status_with_immunity(n(1));
        let character_states = states(None, Some(5));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let outcome = settle(DamageInput::new(n(3), DamageType::Piercing), &env, &no_bonus());
        assert!(!outcome.immune);
        assert!(outcome.immunity_available, "免疫确实存在，只是穿透不吃");
        assert!(outcome.bypassed);
        assert_eq!(outcome.to_hp, n(3));
        assert_eq!(outcome.reduction_total, Nat::ZERO);
    }

    #[test]
    fn immunity_is_read_from_the_character_state_list_too() {
        // 免疫也可能挂在角色状态上（卡面护罩类状态是角色状态）：两处都读（待复核）。
        let combat_status = empty_combat_status();
        let mut character_states = StateList::new();
        character_states.add(Nat::ZERO, &DAMAGE_IMMUNITY, None);
        let env = DamageChainEnv::new(&combat_status, &character_states);
        assert!(env.is_immune());
        let outcome = settle(DamageInput::new(n(4), DamageType::Physical), &env, &no_bonus());
        assert!(outcome.immune);
        assert_eq!(outcome.to_hp, Nat::ZERO);
    }

    #[test]
    fn custom_immunity_set_can_replace_the_default() {
        // 具体护罩状态的正式定义落地后，用 with_immunities 换一组身份即可（不新建承载）。
        let combat_status = combat_status_with_shield(n(1), 1);
        let character_states = states(Some(2), None);
        let triggers = CombatStatusTriggers::new();
        let mut with_frozen = empty_combat_status();
        with_frozen = with_frozen
            .attach_with_triggers(n(1), &statuses::FROZEN, None, None, &triggers)
            .zone;
        const FROZEN_ONLY: [&'static dyn StateIdentity; 1] = [&statuses::FROZEN];
        let env = DamageChainEnv::new(&with_frozen, &character_states)
            .with_immunities(&FROZEN_ONLY);
        assert!(env.is_immune());
        let outcome = settle(DamageInput::new(n(3), DamageType::Physical), &env, &no_bonus());
        assert!(outcome.immune);
        // 默认环境（没挂免疫）不免疫。
        let no_states = StateList::new();
        let plain = DamageChainEnv::new(&combat_status, &no_states);
        assert!(!plain.is_immune());
    }

    // ------------------------------------------------------------------
    // 乘算 / 除算（分工计划 §4.4.5）
    // ------------------------------------------------------------------

    #[test]
    fn multipliers_apply_between_bonus_and_shield() {
        // 基础 2 + 增伤 1 = 3；倍数区 ×2 -> 6；盾 1 吸 1 -> 扣血 5。
        let combat_status = combat_status_with_shield(n(1), 1);
        let character_states = StateList::new();
        let ops = [AmountOp::mul_zone(n(2))];
        let env = DamageChainEnv::new(&combat_status, &character_states).with_modifiers(
            DamageModifiers {
                ops: &ops,
                rounding: None,
            },
        );
        let bonus = DamageBonus {
            increases: &[n(1)],
            ..DamageBonus::NONE
        };
        let outcome = settle(
            DamageInput::new(n(2), DamageType::elemental(crate::base::Element::Cryo)),
            &env,
            &bonus,
        );
        assert_eq!(outcome.damage_after_bonus, n(3));
        assert_eq!(outcome.after_scale, n(6));
        assert_eq!(outcome.shield_used, n(1));
        assert_eq!(outcome.to_hp, n(5));
        let scale = outcome.scale.expect("普通路径一定有乘算展开");
        assert_eq!(scale.total, n(6));
        assert_eq!(scale.zone.mul_sum, n(2));
        assert_eq!(
            outcome.stages[2],
            ChainStage::Scaled {
                multiplier: n(2),
                divisor: n(1),
                total: n(6)
            }
        );
    }

    #[test]
    fn division_rounds_and_zone_outside_adjustment_happens_after_division() {
        // 基础 5：除 2（Down）= 2，再区外 +1 = 3。
        let combat_status = empty_combat_status();
        let character_states = StateList::new();
        let ops = [AmountOp::div(n(2)), AmountOp::add_after_mul(n(1))];
        let env = DamageChainEnv::new(&combat_status, &character_states).with_modifiers(
            DamageModifiers {
                ops: &ops,
                rounding: None,
            },
        );
        let outcome = settle(DamageInput::new(n(5), DamageType::Physical), &env, &no_bonus());
        assert_eq!(outcome.after_scale, n(3));
        assert_eq!(outcome.to_hp, n(3));
        let scale = outcome.scale.expect("有乘算");
        assert_eq!(scale.divisor, n(2));
        assert_eq!(scale.added, n(1));
    }

    #[test]
    fn piercing_ignores_multipliers_too() {
        // 穿透：加伤 / 乘算 / 盾 / 减伤 / 免疫全不吃（规则书·特殊词条 + K5）。
        let combat_status = empty_combat_status();
        let character_states = StateList::new();
        let ops = [AmountOp::mul_zone(n(3))];
        let env = DamageChainEnv::new(&combat_status, &character_states).with_modifiers(
            DamageModifiers {
                ops: &ops,
                rounding: None,
            },
        );
        let outcome = settle(DamageInput::new(n(4), DamageType::Piercing), &env, &no_bonus());
        assert!(outcome.bypassed);
        assert_eq!(outcome.scale, None, "穿透不走乘算段");
        assert_eq!(outcome.to_hp, n(4));
    }

    #[test]
    fn type_override_rewrites_the_type_before_the_chain() {
        // 5 点物理：盾 2 吸 2 -> 扣血 3（默认，无覆盖）。
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let plain = settle(DamageInput::new(n(5), DamageType::Physical), &env, &no_bonus());
        assert_eq!(plain.input.ty, DamageType::Physical);
        assert_eq!(plain.shield_used, n(2));
        assert_eq!(plain.to_hp, n(3));
        // 覆盖为穿透：进入计算前换成穿透 -> 不吃盾 / 不吃加伤 / 直接扣 5。
        let env = DamageChainEnv::new(&combat_status, &character_states)
            .with_type_override(Some(DamageType::Piercing));
        let overridden = settle(DamageInput::new(n(5), DamageType::Physical), &env, &no_bonus());
        assert_eq!(overridden.input.ty, DamageType::Piercing);
        assert!(overridden.bypassed);
        assert_eq!(overridden.shield_used, Nat::ZERO);
        assert_eq!(overridden.to_hp, n(5));
    }

    #[test]
    fn type_override_is_recorded_as_original_then_actual() {
        let combat_status = empty_combat_status();
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states)
            .with_type_override(Some(DamageType::elemental(crate::base::Element::Hydro)));
        let (_outcome, records) = settle_records(
            DamageInput::new(n(3), DamageType::Physical),
            &env,
            &no_bonus(),
        );
        let overrides: Vec<(DamageType, DamageType)> = records
            .step_entries(STAGE_TYPE)
            .filter_map(|content| match content {
                DamageChainRecord::TypeOverride { original, actual } => Some((*original, *actual)),
                _ => None,
            })
            .collect();
        assert_eq!(
            overrides,
            vec![(
                DamageType::Physical,
                DamageType::elemental(crate::base::Element::Hydro)
            )]
        );
        // 没有覆盖 -> 不写 TypeOverride（默认逐字节等价）。
        let plain_env = DamageChainEnv::new(&combat_status, &character_states);
        let (_outcome, plain_records) = settle_records(
            DamageInput::new(n(3), DamageType::Physical),
            &plain_env,
            &no_bonus(),
        );
        assert!(
            plain_records.step_entries(STAGE_TYPE).all(|content| !matches!(
                content,
                DamageChainRecord::TypeOverride { .. }
            ))
        );
    }

    #[test]
    fn immune_chain_skips_multipliers() {
        // 免疫优先于乘算：即便有倍数修饰，伤害仍是 0（顺序：免疫在最前，待实测）。
        let combat_status = combat_status_with_immunity(n(1));
        let character_states = StateList::new();
        let ops = [AmountOp::mul_zone(n(4))];
        let env = DamageChainEnv::new(&combat_status, &character_states).with_modifiers(
            DamageModifiers {
                ops: &ops,
                rounding: None,
            },
        );
        let outcome = settle(DamageInput::new(n(6), DamageType::Physical), &env, &no_bonus());
        assert!(outcome.immune);
        assert_eq!(outcome.scale, None);
        assert_eq!(outcome.to_hp, Nat::ZERO);
    }
}
