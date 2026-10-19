//! 伤害修正链的**加伤**部分：增伤 / 元素精通 / 反应加伤。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` + 卡面）
//!
//! - 规则书·特殊词条 **穿透伤害**：「『穿透伤害』不会受到其他效果的伤害加成，但是也不会被
//!   护盾等效果抵消。」-> 穿透伤害**不加伤**（本链把这一条放在第一步：直接给出原值）；
//! - 规则书·**元素精通**（自动巧局）：「提高元素精通可增强触发元素反应时造成的伤害，以及元素
//!   反应的效果。例如：每提高1点我方元素精通，我方结晶反应提供的护盾便会提高3层。」
//!   -> 元素精通**只在触发元素反应时**加伤；结晶护盾的强化在反应效果执行器里做
//!   （`MASTERY_SHIELD_PER_POINT`）；
//! - 规则书·元素反应·效果：每个反应条目的「本伤害+N」= 反应加伤（`damage_bonus`）；
//! - 卡面「造成的伤害+1」这类通用增伤 = `increases` 里的每一条。
//!
//! ## 口径（D190 / D183）
//!
//! - 加伤是**纯数值**：一条增伤一个 `Nat`，顺序 = 结算顺序 §一 第 2/3 条（来源类别 -> 进场先后）；
//!   加法可交换，本层按「来源类别在前、进场先后在后」的**登记顺序**求和，只用于观测与将来带倍率时保序；
//! - **零浮点**：全部 `Nat` 饱和运算（`nadd!` / `nmul!`）；
//! - 反应判定**不在本层重做**：`predict_reaction` 只用伤害入口的公共原语
//!   （`Attachment::matching_covered` + 规则表顺序），口径与 `damage()` 完全一致；
//!   真正的判定仍由 `damage()` 做一次（本层只是「调用前问一句会命中什么」）。
//!
//! ## 乘算 / 除算（分工计划 §4.4.5 的四段运算模型）
//!
//! 伤害的数值修正分四段，顺序**固定**（`分工计划.md` §4.4.5「结算顺序」）：
//!
//! 1. **加减（`in_mul_zone = true`）**：计入乘算区，会被乘数放大；
//! 2. **乘**：独立乘法各乘一次；进倍数区的乘数**相加**后统一乘一次；
//! 3. **除**：所有除数**相乘成一个分母**，一起除；随后**取整**；
//! 4. **加减（`in_mul_zone = false`）**：乘除之后再加减，不参与放大。
//!
//! - 第 1 段的加减总和由 [`DamageModifiers::scale`] 与调用方先算好的「基础 + 加伤」一起结算；
//! - 第 2 段用 [`MultiplierZone`]（`mul_sum` = 进倍数区的乘数之和）；
//! - 第 3 段先乘出分母再按 [`Rounding`] 取整（`Rounding::apply` 除零 panic，D44）；
//! - 穿透伤害**整段跳过**（规则书·特殊词条 穿透伤害：不吃任何伤害加成）；
//! - 伤害免疫同理整段跳过（本次伤害归 0，见 `chain`）。
//!
//! ## 待实测 / 待复核
//!
//! - `MASTERY_DAMAGE_PER_POINT`：规则书只说「增强」，**没给每点数值**；本块先按 +1/点，
//!   等实测 / 卡面量化后改这一个常量；
//! - 穿透伤害「不受伤害加成」的**判定位置**：本层按「加伤整段跳过」实现（见 `chain`）；
//! - 取整方向：规则书未写，[`Rounding`] 由调用方给；**默认按 `Down`**（整数除法），
//!   登记 `留空.md` 待实测；
//! - 倍数区内多个乘数「相加」：`mul_sum` 按分工计划取「乘数**值**之和」（单个 ×2 = `mul_sum` 2）；
//!   规则书 / 卡面没有「多个翻倍叠加」的条文 —— 待实测；没有乘数时整段跳过（否则会乘成 0）；
//! - 「本伤害+N」与通用增伤**谁先谁后**：都是第 1 段加法，可交换；只有与乘算的相对位置
//!   （本层按 `DamageBonus` 整块计入**乘算区内**）会影响结果 —— 待实测。

use crate::base::Element;
use crate::families::character::attachment::{
    same_reaction_identity, Attachment, ReactionEvent, ReactionIdentity,
};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::EffectContext;
use crate::reactions::table;
use gi_genius_invokation_engine::{nadd, ndiv, nmul, nsub, EntityId, Nat};

/// 元素精通：每点给**反应伤害**的加伤（**待实测**：规则书只说「增强触发元素反应时造成的伤害」，
/// 没给每点数值；先按 +1/点）。
pub const MASTERY_DAMAGE_PER_POINT: Nat = Nat::new(1);

/// 元素精通：每点给**结晶反应护盾**的额外层数（规则书·元素精通原文例：
/// 「每提高1点我方元素精通，我方结晶反应提供的护盾便会提高3层」）。
pub const MASTERY_SHIELD_PER_POINT: Nat = Nat::new(3);

/// 一次伤害的**加伤来源**（纯数据、`Copy`）。
///
/// 顺序：`increases` 按调用方给的登记顺序（结算顺序 §一 第 2/3 条）；
/// 反应加伤是「本伤害 +N」，与元素精通一起只在 `reacted` 为真时计入。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DamageBonus<'a> {
    /// 通用增伤（卡面「造成的伤害+N」/ 武器 / 状态；一条一个 `Nat`）。
    ///
    /// 来源类别与进场先后由调用方按结算顺序排好（本层不重排）。
    pub increases: &'a [Nat],
    /// 触发方元素精通（只在本次触发元素反应时加到伤害上）。
    pub mastery: Nat,
    /// 本次反应的「本伤害+N」（`reactions::table::resolve` 的 `damage_bonus`）。
    pub reaction_bonus: Nat,
    /// 本次是否触发了元素反应（元素精通只在反应时生效）。
    pub reacted: bool,
}

impl DamageBonus<'_> {
    /// 没有任何加伤（物理 / 穿透 / 无反应时用）。
    pub const NONE: DamageBonus<'static> = DamageBonus {
        increases: &[],
        mastery: Nat::ZERO,
        reaction_bonus: Nat::ZERO,
        reacted: false,
    };

    /// 元素精通给出的**反应**加伤（不反应 -> 0）。
    pub fn mastery_bonus(mastery: Nat, reacted: bool) -> Nat {
        if reacted {
            nmul!(mastery, MASTERY_DAMAGE_PER_POINT)
        } else {
            Nat::ZERO
        }
    }

    /// 加伤总计 = 各条通用增伤 + 反应加伤 + 元素精通加伤（只在反应时）。
    ///
    /// 只有反应时才会出现 `reaction_bonus` / 精通加伤，所以没反应时它们恒为 0
    /// （`reacted` 是唯一的开关，避免调用方漏清字段）。
    pub fn total(&self) -> Nat {
        let mut total = Nat::ZERO;
        for increase in self.increases {
            total = nadd!(total, *increase);
        }
        if self.reacted {
            total = nadd!(total, self.reaction_bonus);
            total = nadd!(total, Self::mastery_bonus(self.mastery, true));
        }
        total
    }
}

// ======================================================================
// 乘算 / 除算（分工计划 §4.4.5 的四段运算模型）
// ======================================================================

/// 乘法怎么算（分工计划 §4.4.5）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MulMode {
    /// 独立乘法：单独乘一次。
    Standalone,
    /// 加入统一倍数区：区内乘数**相加**后统一乘一次。
    Zone,
}

/// 取整方向（分工计划 §4.4.5：具体值**暂不决定**，由调用方给；默认 `Down`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    /// 向上取整（ceil）。
    Up,
    /// 向下取整（floor；整数除法的自然结果）。
    Down,
}

impl Rounding {
    /// 把 `numerator / denominator` 按本方向取整。
    ///
    /// - `Up` = `ceil`：`(numerator + denominator - 1) / denominator`；
    /// - `Down` = `floor`：整数除法；
    /// - `denominator` 为 0 -> **panic**（D44：除零改成崩溃，不静默返回 0）。
    ///
    /// 分母 / 分子都是 `Nat`：加法饱和、减法饱和，不会为负。
    ///
    /// （不是 `const fn`：引擎的加法 / 除法宏是 trait 方法，常量上下文里不能调。）
    pub fn apply(self, numerator: Nat, denominator: Nat) -> Nat {
        match self {
            Rounding::Down => ndiv!(numerator, denominator),
            Rounding::Up => {
                let adjusted = nsub!(nadd!(numerator, denominator), Nat::new(1));
                ndiv!(adjusted, denominator)
            }
        }
    }
}

/// 一条**数值修正**（分工计划 §4.4.5 的 `AmountOp`）：只允许「运算 + 数值」，
/// 不允许把代码（闭包 / trait 对象 / 回调）注入结算（分工计划 §4.4.5 的硬规矩）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AmountOp {
    /// 加：可声明是否计入乘算区（`true` = 乘除之前，会被乘数放大）。
    Add {
        /// 加多少。
        value: Nat,
        /// 是否计入乘算区（`true` = 第 1 段；`false` = 第 4 段）。
        in_mul_zone: bool,
    },
    /// 乘：必须声明是独立乘法还是进统一倍数区。
    Mul {
        /// 乘数（`2` = 翻倍）。
        value: Nat,
        /// 独立乘法 / 倍数区。
        mode: MulMode,
    },
    /// 除：所有除数相乘成一个分母后一起除。
    Div {
        /// 除数（**不能为 0**，否则结算时 panic，D44）。
        value: Nat,
    },
    /// 减（伤害减免 / 伤害降低）：同样可声明是否计入乘算区；用 `Nat` 饱和减法，不会为负。
    Reduce {
        /// 减多少。
        value: Nat,
        /// 是否计入乘算区（`true` = 第 1 段；`false` = 第 4 段）。
        in_mul_zone: bool,
    },
}

impl AmountOp {
    /// 在**乘除之前**加（会被乘数放大 = 计入乘算区）。
    pub const fn add_before_mul(value: Nat) -> AmountOp {
        AmountOp::Add {
            value,
            in_mul_zone: true,
        }
    }

    /// 在**乘除之后**加（不参与放大）。
    pub const fn add_after_mul(value: Nat) -> AmountOp {
        AmountOp::Add {
            value,
            in_mul_zone: false,
        }
    }

    /// 在**乘除之前**减（会被乘数放大 = 计入乘算区）。
    pub const fn reduce_before_mul(value: Nat) -> AmountOp {
        AmountOp::Reduce {
            value,
            in_mul_zone: true,
        }
    }

    /// 在**乘除之后**减（不参与放大）。
    pub const fn reduce_after_mul(value: Nat) -> AmountOp {
        AmountOp::Reduce {
            value,
            in_mul_zone: false,
        }
    }

    /// 独立乘法（单独乘一次）。
    pub const fn mul_standalone(value: Nat) -> AmountOp {
        AmountOp::Mul {
            value,
            mode: MulMode::Standalone,
        }
    }

    /// 进统一倍数区的乘法（区内乘数相加后统一乘一次）。
    pub const fn mul_zone(value: Nat) -> AmountOp {
        AmountOp::Mul {
            value,
            mode: MulMode::Zone,
        }
    }

    /// 除法（所有除数相乘成一个分母后一起除）。
    pub const fn div(value: Nat) -> AmountOp {
        AmountOp::Div { value }
    }
}

/// 统一的**倍数区**（分工计划 §4.4.5：只有一个，挂在角色 types 上）。
///
/// - `mul_sum` = 所有进倍数区的乘数**值**之和（单个「翻倍」= 2）；
/// - `is_empty()`（没有乘数）时 [`MultiplierZone::apply`] **原样返回**（否则会乘成 0）；
/// - 多个倍数叠加时规则书未定义 -> `待实测`（见模块文档）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MultiplierZone {
    /// 所有进倍数区的乘数之和（`0` = 区内没有乘数）。
    pub mul_sum: Nat,
    /// 取整方向（`Option`：未定 -> 用调用方 / 默认 `Down`）。
    pub rounding: Option<Rounding>,
}

impl MultiplierZone {
    /// 空倍数区（没有乘数；`apply` 原样返回）。
    pub const EMPTY: MultiplierZone = MultiplierZone {
        mul_sum: Nat::ZERO,
        rounding: None,
    };

    /// 从修正列表里收集进倍数区的乘数（`MulMode::Zone`）。
    pub fn from_ops(ops: &[AmountOp]) -> MultiplierZone {
        let mut mul_sum = Nat::ZERO;
        for op in ops {
            if let AmountOp::Mul {
                value,
                mode: MulMode::Zone,
            } = op
            {
                mul_sum = nadd!(mul_sum, *value);
            }
        }
        MultiplierZone {
            mul_sum,
            rounding: None,
        }
    }

    /// 补上取整方向（链式）。
    pub const fn with_rounding(mut self, rounding: Option<Rounding>) -> MultiplierZone {
        self.rounding = rounding;
        self
    }

    /// 区内有没有乘数（没有 -> 整段跳过，避免乘成 0）。
    pub fn is_empty(&self) -> bool {
        self.mul_sum == Nat::ZERO
    }

    /// 统一乘一次：`amount × mul_sum`；区内没有乘数 -> 原样返回。
    pub fn apply(self, amount: Nat) -> Nat {
        if self.is_empty() {
            amount
        } else {
            nmul!(amount, self.mul_sum)
        }
    }
}

/// 一次伤害的**乘算 / 除算输入**（第 2 / 3 / 4 段的修正列表 + 取整方向）。
///
/// 与 [`DamageBonus`] 分开：`DamageBonus` 是「加伤」的既有入口（签名不改），本类型只加
/// 「乘 / 除 / 区外加减」；调用方按需用 [`DamageChainEnv::with_modifiers`] 挂进伤修链。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DamageModifiers<'a> {
    /// 数值修正列表（顺序**不影响**结果：同类运算可交换；只用于观测与将来的保序扩展）。
    pub ops: &'a [AmountOp],
    /// 取整方向（`None` -> `Down`）。
    pub rounding: Option<Rounding>,
}

impl DamageModifiers<'_> {
    /// 没有任何乘算 / 除算 / 区外加减。
    pub const NONE: DamageModifiers<'static> = DamageModifiers {
        ops: &[],
        rounding: None,
    };

    /// 第 1 段里「计入乘算区」的加减合计：`(加, 减)`。
    pub fn in_zone_delta(&self) -> (Nat, Nat) {
        let mut added = Nat::ZERO;
        let mut reduced = Nat::ZERO;
        for op in self.ops {
            match op {
                AmountOp::Add {
                    value,
                    in_mul_zone: true,
                } => added = nadd!(added, *value),
                AmountOp::Reduce {
                    value,
                    in_mul_zone: true,
                } => reduced = nadd!(reduced, *value),
                _ => {}
            }
        }
        (added, reduced)
    }

    /// **完整结算四段运算**：输入 = 「基础伤害 + 加伤」（第 1 段的加法部分），
    /// 返回每一步的中间值 + 最终结果。
    ///
    /// 顺序（分工计划 §4.4.5）：
    /// 1. 加减（`in_mul_zone = true`）；
    /// 2. 乘（独立各乘一次；倍数区相加后统一乘一次）；
    /// 3. 除（所有除数相乘成一个分母后一起除，再按取整方向取整）；
    /// 4. 加减（`in_mul_zone = false`）。
    pub fn scale(&self, after_flat_bonus: Nat) -> ScaledDamage {
        // 1) 计入乘算区的加减。
        let (in_zone_added, in_zone_reduced) = self.in_zone_delta();
        let after_in_zone = nsub!(nadd!(after_flat_bonus, in_zone_added), in_zone_reduced);

        // 2) 乘：独立乘法各乘一次；倍数区相加后统一乘一次。
        let mut standalone = Nat::new(1);
        for op in self.ops {
            if let AmountOp::Mul {
                value,
                mode: MulMode::Standalone,
            } = op
            {
                standalone = nmul!(standalone, *value);
            }
        }
        let zone = MultiplierZone::from_ops(self.ops).with_rounding(self.rounding);
        let mut scaled = nmul!(after_in_zone, standalone);
        scaled = zone.apply(scaled);

        // 3) 除：所有除数相乘成一个分母，一起除；随后取整。
        let mut divisor = Nat::new(1);
        let mut has_div = false;
        for op in self.ops {
            if let AmountOp::Div { value } = op {
                divisor = nmul!(divisor, *value);
                has_div = true;
            }
        }
        if has_div {
            scaled = self.rounding.unwrap_or(Rounding::Down).apply(scaled, divisor);
        }

        // 4) 乘除区之外的加减（不参与放大）。
        let mut added = Nat::ZERO;
        let mut reduced = Nat::ZERO;
        for op in self.ops {
            match op {
                AmountOp::Add {
                    value,
                    in_mul_zone: false,
                } => added = nadd!(added, *value),
                AmountOp::Reduce {
                    value,
                    in_mul_zone: false,
                } => reduced = nadd!(reduced, *value),
                _ => {}
            }
        }
        let total = nsub!(nadd!(scaled, added), reduced);

        ScaledDamage {
            after_in_zone,
            standalone,
            zone,
            divisor,
            rounding: self.rounding,
            added,
            reduced,
            total,
        }
    }
}

/// 四段运算的**每一步中间值 + 最终结果**（纯数据；用于观测 / 记录）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScaledDamage {
    /// 第 1 段之后：基础 + 加伤 + 计入乘算区的加减（饱和）。
    pub after_in_zone: Nat,
    /// 第 2 段：独立乘法的乘积（没有 -> 1）。
    pub standalone: Nat,
    /// 第 2 段：统一倍数区（`mul_sum` = 进倍数区的乘数之和）。
    pub zone: MultiplierZone,
    /// 第 3 段：除数的乘积（没有 -> 1）。
    pub divisor: Nat,
    /// 第 3 段的取整方向（`None` = 没走除法 / 未给）。
    pub rounding: Option<Rounding>,
    /// 第 4 段：乘除之后加上的点数。
    pub added: Nat,
    /// 第 4 段：乘除之后减掉的点数。
    pub reduced: Nat,
    /// 最终伤害值（`>= 0`，饱和）。
    pub total: Nat,
}

impl ScaledDamage {
    /// 没有任何乘算 / 除算 / 区外加减时的恒等结果（穿透 / 免疫路径用）。
    pub const fn identity(amount: Nat) -> ScaledDamage {
        ScaledDamage {
            after_in_zone: amount,
            standalone: Nat::new(1),
            zone: MultiplierZone::EMPTY,
            divisor: Nat::new(1),
            rounding: None,
            added: Nat::ZERO,
            reduced: Nat::ZERO,
            total: amount,
        }
    }
}

/// 本次元素伤害**会**命中的反应（只读预测；在调 `damage()` **之前**用）。
///
/// 字段直接照抄 `ReactionEvent` 的四要素，调用方可以把它原样（或补上真正的
/// source / target）交给反应效果层。
#[derive(Clone, Copy, Debug)]
pub struct ReactionHit {
    /// 反应身份。
    pub reaction: &'static dyn ReactionIdentity,
    /// 命中的已覆盖元素（多元素共存时只有命中的那一个，D167）。
    pub covered: Element,
    /// 本次攻击元素。
    pub attack: Element,
    /// 谁触发（发起攻击的角色实体）。
    pub source: EntityId,
    /// 作用在谁身上（承载附着的角色实体）。
    pub target: EntityId,
}

/// 反应身份是**数据地址身份**（D57 / D58）：相等按 `identity_address()` 比（`dyn` 不能 derive Eq）。
impl PartialEq for ReactionHit {
    fn eq(&self, other: &ReactionHit) -> bool {
        same_reaction_identity(self.reaction, other.reaction)
            && self.covered == other.covered
            && self.attack == other.attack
            && self.source == other.source
            && self.target == other.target
    }
}

impl Eq for ReactionHit {}

impl ReactionHit {
    /// 组一个反应事件（交给注册表 / 效果层时用）。
    pub fn event(self) -> ReactionEvent {
        ReactionEvent::new(
            self.reaction,
            self.covered,
            self.attack,
            self.source,
            self.target,
        )
    }

    /// 本次反应的「本伤害 +N」（`reactions::table::resolve` 的 `damage_bonus`）。
    ///
    /// `EffectContext` 只影响效果**分支**（操作清单），表里所有反应函数的
    /// `damage_bonus` 都是常数，所以加伤与 context 无关（自测里钉死这条）。
    /// 没登记效果的反应 -> 0。
    pub fn damage_bonus(self, context: &EffectContext) -> Nat {
        table::resolve(&self.event(), context)
            .map(|result| result.damage_bonus)
            .unwrap_or(Nat::ZERO)
    }
}

/// **预测**本次元素伤害会命中哪一条反应（只读；口径 = `damage()` 的判定）。
///
/// - 规则表按顺序找**第一条**命中（与 `damage()` 同一个循环形状、同一批公共原语：
///   `Attachment::matching_covered`）；
/// - 调用方只应在**元素伤害**时调用（物理 / 穿透不判反应）；
/// - 预测与实际判定不一致的唯一可能是「两次调用之间附着被改了」——本层是纯函数，不持有状态。
pub fn predict_reaction(
    attachment: Attachment,
    attack: Element,
    rules: &[ReactionRule],
    source: EntityId,
    target: EntityId,
) -> Option<ReactionHit> {
    rules.iter().find_map(|rule| {
        attachment
            .matching_covered(attack, rule.conditions)
            .map(|covered| ReactionHit {
                reaction: rule.reaction,
                covered,
                attack,
                source,
                target,
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::attachment::{AttachmentAllowSet, ReactionCondition};
    use crate::reactions::{melt, superconduct, table, vaporize};
    use gi_genius_invokation_engine::Nat;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    fn rules() -> Vec<ReactionRule> {
        table::base_rules()
    }

    #[test]
    fn total_sums_increases_reaction_bonus_and_mastery() {
        let bonus = DamageBonus {
            increases: &[n(1), n(2)],
            mastery: n(3),
            reaction_bonus: n(2),
            reacted: true,
        };
        // 1 + 2 + 2（反应）+ 3 × 1（精通，每点 +1，待实测）
        assert_eq!(bonus.total(), n(8));
    }

    #[test]
    fn mastery_and_reaction_bonus_only_count_when_reacted() {
        let bonus = DamageBonus {
            increases: &[n(1)],
            mastery: n(5),
            reaction_bonus: n(2),
            reacted: false,
        };
        assert_eq!(bonus.total(), n(1));
        assert_eq!(DamageBonus::mastery_bonus(n(5), false), Nat::ZERO);
        assert_eq!(DamageBonus::mastery_bonus(n(5), true), n(5));
        assert_eq!(DamageBonus::NONE.total(), Nat::ZERO);
    }

    #[test]
    fn predict_reaction_matches_the_damage_entry_judgment() {
        let rules = rules();
        // 已有冰 + 本次火 -> 融化（规则表第一条命中的）。
        let hit = predict_reaction(
            Attachment::with(Element::Cryo),
            Element::Pyro,
            &rules,
            n(1),
            n(2),
        )
        .expect("融化");
        assert_eq!(hit.reaction.name(), "融化");
        assert_eq!(hit.covered, Element::Cryo);
        assert_eq!(hit.attack, Element::Pyro);
        // 没有附着 / 不反应的组合 -> None。
        assert!(predict_reaction(Attachment::none(), Element::Pyro, &rules, n(1), n(2)).is_none());
        // 冰 + 草不构成反应（融化 / 冻结 / 超导都不含草）-> None。
        assert!(predict_reaction(
            Attachment::with(Element::Cryo),
            Element::Dendro,
            &rules,
            n(1),
            n(2)
        )
        .is_none());
        // 空规则表 -> 永不命中（D128）。
        assert!(predict_reaction(
            Attachment::with(Element::Cryo),
            Element::Pyro,
            &[],
            n(1),
            n(2)
        )
        .is_none());
    }

    #[test]
    fn reaction_damage_bonus_matches_table_resolve() {
        let rules = rules();
        let context = EffectContext::no_hands(true);
        for (covered, attack, expected) in [
            (Element::Cryo, Element::Pyro, 2),    // 融化
            (Element::Hydro, Element::Pyro, 2),   // 蒸发
            (Element::Cryo, Element::Electro, 1), // 超导
        ] {
            let hit = predict_reaction(Attachment::with(covered), attack, &rules, n(1), n(2))
                .expect("应命中一条反应");
            assert_eq!(hit.damage_bonus(&context), n(expected));
        }
    }

    #[test]
    fn reaction_bonus_is_constant_across_effect_contexts() {
        // 加伤与 EffectContext（是否出战 / 双方手牌数）无关：表里所有反应函数的
        // damage_bonus 都是常数。这条钉死「调用方不必为加伤准备精确 context」。
        let rules = rules();
        let hit = predict_reaction(
            Attachment::with(Element::Cryo),
            Element::Pyro,
            &rules,
            n(1),
            n(2),
        )
        .expect("融化");
        let bonuses = [
            EffectContext::no_hands(false),
            EffectContext::new(true, n(0), n(0)),
            EffectContext::new(false, n(5), n(3)),
        ];
        for context in bonuses {
            assert_eq!(hit.damage_bonus(&context), n(2));
        }
    }

    #[test]
    fn hit_event_carries_source_target_and_identity() {
        let hit = ReactionHit {
            reaction: &melt::MELT,
            covered: Element::Cryo,
            attack: Element::Pyro,
            source: n(7),
            target: n(8),
        };
        let event = hit.event();
        assert_eq!(event.source, n(7));
        assert_eq!(event.target, n(8));
        assert_eq!(event.covered, Element::Cryo);
        assert_eq!(event.attack, Element::Pyro);
        assert!(same_reaction_identity(event.reaction, &melt::MELT));
    }

    #[test]
    fn superconduct_hit_uses_the_same_attachment_primitives() {
        // 用同一批公共原语（Attachment / ReactionCondition），不重造判定。
        let rules = vec![superconduct::rule(), vaporize::rule()];
        let hit = predict_reaction(
            Attachment::with(Element::Cryo),
            Element::Electro,
            &rules,
            n(1),
            n(2),
        )
        .expect("超导");
        assert!(same_reaction_identity(
            hit.reaction,
            &superconduct::SUPERCONDUCT
        ));
        // 允许集合不影响判定（D134）：附着被拒时反应仍按已有附着判定。
        let allow = AttachmentAllowSet::NONE;
        let (attachment, landed) = Attachment::with(Element::Cryo).attach(Element::Electro, allow);
        assert!(!landed);
        assert!(attachment.has(Element::Cryo));
        assert!(predict_reaction(attachment, Element::Electro, &rules, n(1), n(2)).is_some());
    }

    #[test]
    fn reaction_condition_list_is_order_insensitive_for_prediction() {
        // D132：两个先后各一条，预测两条都命中（与 damage() 的判定同口径）。
        static BOTH: [ReactionCondition; 2] =
            ReactionCondition::both_orders(Element::Hydro, Element::Pyro);
        let rules = vec![ReactionRule::new(&vaporize::VAPORIZE, &BOTH)];
        assert!(predict_reaction(
            Attachment::with(Element::Hydro),
            Element::Pyro,
            &rules,
            n(1),
            n(2)
        )
        .is_some());
        assert!(predict_reaction(
            Attachment::with(Element::Pyro),
            Element::Hydro,
            &rules,
            n(1),
            n(2)
        )
        .is_some());
    }

    // ------------------------------------------------------------------
    // 乘算 / 除算（分工计划 §4.4.5 四段模型）
    // ------------------------------------------------------------------

    #[test]
    fn rounding_down_is_floor_and_up_is_ceil() {
        // floor(7 / 2) = 3；ceil(7 / 2) = 4。
        assert_eq!(Rounding::Down.apply(n(7), n(2)), n(3));
        assert_eq!(Rounding::Up.apply(n(7), n(2)), n(4));
        // 整除时不进位。
        assert_eq!(Rounding::Up.apply(n(8), n(2)), n(4));
        assert_eq!(Rounding::Down.apply(n(8), n(2)), n(4));
        // 0 / d = 0（两个方向一致）。
        assert_eq!(Rounding::Up.apply(Nat::ZERO, n(3)), Nat::ZERO);
        assert_eq!(Rounding::Down.apply(Nat::ZERO, n(3)), Nat::ZERO);
    }

    #[test]
    #[should_panic]
    fn rounding_by_zero_panics() {
        // D44：除零改成崩溃，不静默返回 0。
        let _ = Rounding::Down.apply(n(4), Nat::ZERO);
    }

    #[test]
    fn empty_modifiers_are_identity() {
        let scaled = DamageModifiers::NONE.scale(n(5));
        assert_eq!(scaled, ScaledDamage::identity(n(5)));
        assert_eq!(scaled.total, n(5));
        // 空倍数区不乘成 0。
        assert!(MultiplierZone::EMPTY.is_empty());
        assert_eq!(MultiplierZone::EMPTY.apply(n(5)), n(5));
    }

    #[test]
    fn double_damage_is_applied() {
        // 卡面「此次伤害翻倍」：基础 3 -> 6。
        let ops = [AmountOp::mul_zone(n(2))];
        let modifiers = DamageModifiers {
            ops: &ops,
            rounding: None,
        };
        let scaled = modifiers.scale(n(3));
        assert_eq!(scaled.total, n(6));
        assert_eq!(scaled.zone.mul_sum, n(2));
    }

    #[test]
    fn standalone_multiplication_and_zone_addition_follow_the_four_stages() {
        // 基础 2，加伤 +1（= 3）；倍数区 ×2 -> 6；独立 ×3 -> 18；除 2 -> 9；
        // 区外加 1 -> 10；区外减 2 -> 8。
        let ops = [
            AmountOp::add_before_mul(n(1)),
            AmountOp::mul_zone(n(2)),
            AmountOp::mul_standalone(n(3)),
            AmountOp::div(n(2)),
            AmountOp::add_after_mul(n(1)),
            AmountOp::reduce_after_mul(n(2)),
        ];
        let modifiers = DamageModifiers {
            ops: &ops,
            rounding: Some(Rounding::Down),
        };
        let scaled = modifiers.scale(n(2));
        assert_eq!(scaled.after_in_zone, n(3));
        assert_eq!(scaled.standalone, n(3));
        assert_eq!(scaled.zone.mul_sum, n(2));
        assert_eq!(scaled.divisor, n(2));
        assert_eq!(scaled.added, n(1));
        assert_eq!(scaled.reduced, n(2));
        assert_eq!(scaled.total, n(8));
    }

    #[test]
    fn in_zone_addition_is_amplified_but_after_zone_addition_is_not() {
        // 基础 2：乘算区内 +1 会被 ×2 放大（3 × 2 = 6）；乘算区外 +1 不放大（6 + 1 = 7）。
        let in_zone_ops = [AmountOp::add_before_mul(n(1)), AmountOp::mul_zone(n(2))];
        let after_zone_ops = [AmountOp::add_after_mul(n(1)), AmountOp::mul_zone(n(2))];
        assert_eq!(
            DamageModifiers {
                ops: &in_zone_ops,
                rounding: None
            }
            .scale(n(2))
            .total,
            n(6)
        );
        assert_eq!(
            DamageModifiers {
                ops: &after_zone_ops,
                rounding: None
            }
            .scale(n(2))
            .total,
            n(5)
        );
    }

    #[test]
    fn all_divisors_are_multiplied_into_one_denominator() {
        // 除 2 再除 3 = 一起除 6：12 / 6 = 2（不是 12 / 2 / 3 的逐步取整差异）。
        let ops = [AmountOp::div(n(2)), AmountOp::div(n(3))];
        let modifiers = DamageModifiers {
            ops: &ops,
            rounding: None,
        };
        let scaled = modifiers.scale(n(12));
        assert_eq!(scaled.divisor, n(6));
        assert_eq!(scaled.total, n(2));
    }

    #[test]
    fn rounding_up_matters_when_dividing() {
        // 7 / 2：默认 Down -> 3；Rounding::Up -> 4。
        let ops = [AmountOp::div(n(2))];
        assert_eq!(
            DamageModifiers {
                ops: &ops,
                rounding: None
            }
            .scale(n(7))
            .total,
            n(3)
        );
        assert_eq!(
            DamageModifiers {
                ops: &ops,
                rounding: Some(Rounding::Up)
            }
            .scale(n(7))
            .total,
            n(4)
        );
    }

    #[test]
    fn reduce_saturates_and_never_goes_negative() {
        // 3 点伤害被区外减 5：饱和到 0（不 panic、不为负）。
        let ops = [AmountOp::reduce_after_mul(n(5))];
        let scaled = DamageModifiers {
            ops: &ops,
            rounding: None,
        }
        .scale(n(3));
        assert_eq!(scaled.reduced, n(5));
        assert_eq!(scaled.total, Nat::ZERO);
    }

    #[test]
    fn multiplier_zone_and_standalone_are_collected_separately() {
        let ops = [
            AmountOp::mul_zone(n(2)),
            AmountOp::mul_zone(n(3)),
            AmountOp::mul_standalone(n(2)),
        ];
        let zone = MultiplierZone::from_ops(&ops);
        // 分工计划：倍数区「乘数相加」-> 2 + 3 = 5；独立乘法单独成积 2。
        assert_eq!(zone.mul_sum, n(5));
        assert_eq!(zone.apply(n(2)), n(10));
    }
}
