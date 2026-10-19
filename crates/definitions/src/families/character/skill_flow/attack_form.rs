//! **普通攻击的「视为」判定**：重击（S8.9）/ 下落攻击（S8.10）。
//!
//! 规则书·特殊词条（权威第 1 档，原文）：
//!
//! - **重击**：「我方行动开始前，如果元素骰总数为偶数，则进行的『普通攻击』将被视为『重击』。」
//! - **下落攻击**：「角色被切换为『出战角色』后，本回合内的下一个战斗行动若为『普通攻击』，
//!   则被视为『下落攻击』。」
//!
//! D123 / D124 / D126：重击 / 下落攻击不是独立技能种类，只是普通攻击的**形态**；
//! 对普通攻击生效的效果对它们同样生效（判断用 `SkillKind::is_normal_attack`）。
//!
//! ## 口径
//!
//! - **只改「普通攻击本体」**（`NormalAttack(None)`）：元素战技 / 元素爆发 / 被动原样返回；
//!   定义里已经**写死**形态（`NormalAttack(Some(...))`）的技能也不改写（卡牌自己说了算）；
//! - **重击**只看「我方行动开始前」的元素骰总数是否为**偶数** —— 取样点是**行动开始前**
//!   （调用方在支付费用之前取 [`DicePool::total`]），不是支付后；
//! - **下落攻击**只看「切换为出战角色后、本回合下一个战斗行动」的**待消耗标记**
//!   [`PlungingMark`]：切换时置位（[`PlungingMark::armed`]），一次战斗行动结束后清除
//!   （[`PlungingMark::after_combat_action`]），回合边界清除（[`PlungingMark::cleared_round`]）；
//! - **两者同时成立**时本层按「**下落攻击优先**」（它是更具体的「下一个战斗行动」标记）；
//!   规则书没写叠加时的优先级 -> `待实测`（见 `留空.md` 纪律）；
//! - 本文件**不持有状态**：[`PlungingMark`] 是纯值（D42：区 = 状态里的变量），由角色区 / 回合区
//!   自己当字段持有；判定是纯函数，不读全局、不改入参。
//!
//! ## 依据
//!
//! - 规则书·特殊词条 重击 / 下落攻击（正文）；
//! - D124（特殊形态仍属普通攻击类）、D125（默认形态 = 普通攻击）、D126（只建形态枚举）；
//! - `.tmp/analysis/rulebook-coverage.md` §3 L5 / L6（「判定函数 + 在技能使用流程里标注」）。
//!
//! ## 待实测 / 待复核
//!
//! - 重击与下落攻击**同时成立**的优先级：规则书未写 -> `待实测`（本层下落优先）；
//! - 「下一个战斗行动**不是**普通攻击」时，下落标记是**立刻失效**还是**保留到本回合结束**：
//!   规则书只写「下一个战斗行动若为普通攻击」-> `待实测`（本层按「第一个战斗行动结束即清」）；
//! - 重击的骰数取样点：规则书写「我方行动开始前」，本层要求调用方在**支付前**取总数；
//!   若调用方在支付后才取，会拿到错误总数（调用方责任，见 [`AttackFormInputs`]）。

use crate::families::character::types::{SkillKind, SpecialNormalAttack};
use crate::resources::dice::DicePool;
use gi_genius_invokation_engine::{ndiv, nmul, Nat};

/// 一次普通攻击**生效的形态**（判定结果的标注；`None` = 不是普通攻击）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttackForm {
    /// 普通攻击本体。
    Normal,
    /// 重击（S8.9：行动开始前骰总数为偶数）。
    Charged,
    /// 下落攻击（S8.10：切换后本回合下一个战斗行动是普通攻击）。
    Plunging,
}

impl AttackForm {
    /// 技能种类 -> 形态；不是普通攻击类 -> `None`。
    pub const fn of(kind: SkillKind) -> Option<AttackForm> {
        match kind.special_normal_attack() {
            None => None,
            Some(None) => Some(AttackForm::Normal),
            Some(Some(SpecialNormalAttack::Charged)) => Some(AttackForm::Charged),
            Some(Some(SpecialNormalAttack::Plunging)) => Some(AttackForm::Plunging),
        }
    }

    /// 显示名（中文；只用于记录 / 文本，不参与判定）。
    pub const fn label(self) -> &'static str {
        match self {
            AttackForm::Normal => "普通攻击",
            AttackForm::Charged => "重击",
            AttackForm::Plunging => "下落攻击",
        }
    }
}

/// 判定「视为重击 / 下落攻击」需要的两个输入。
///
/// 只放**判定真正用到的**两个量：骰总数（重击）与待消耗标记（下落）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttackFormInputs {
    /// 我方**行动开始前**的元素骰总数（`DicePool::total`；决定重击）。
    pub dice_total: Nat,
    /// 「本回合下一个战斗行动视为下落攻击」的待消耗标记。
    pub plunging_pending: bool,
}

impl AttackFormInputs {
    /// 直接给两个判定量。
    pub const fn new(dice_total: Nat, plunging_pending: bool) -> AttackFormInputs {
        AttackFormInputs {
            dice_total,
            plunging_pending,
        }
    }

    /// 只有「行动开始前的骰总数」、没有下落标记（最常见的取样：只判重击）。
    pub const fn on_action_start(dice_total: Nat) -> AttackFormInputs {
        AttackFormInputs {
            dice_total,
            plunging_pending: false,
        }
    }

    /// 从骰池 + 下落标记取判定输入（`dice` 必须是**支付前**的骰池）。
    pub fn from_pool(dice: &DicePool, mark: PlungingMark) -> AttackFormInputs {
        AttackFormInputs {
            dice_total: dice.total(),
            plunging_pending: mark.is_pending(),
        }
    }
}

/// 元素骰总数是不是**偶数**（S8.9 的判定条件）。
///
/// 用 `Nat` 的除法 / 乘法实现（不写裸运算符、不借裸整型取模）：
/// `total / 2 * 2 == total`。骰子总数上限 16（D47），不会溢出。
pub fn is_even(total: Nat) -> bool {
    let half = ndiv!(total, Nat::new(2));
    nmul!(half, Nat::new(2)) == total
}

/// **生效形态判定**（S8.9 / S8.10）：给「定义上的技能种类」+ 判定输入，返回本次**生效**的种类。
///
/// 规则（按顺序）：
/// 1. 不是「普通攻击本体」（`NormalAttack(None)`）-> 原样返回（战技 / 爆发 / 被动 / 已写死形态）；
/// 2. 下落标记待消耗 -> `NormalAttack(Some(Plunging))`；
/// 3. 行动开始前骰总数为偶数 -> `NormalAttack(Some(Charged))`；
/// 4. 否则 -> 原样（普通攻击本体）。
///
/// 纯函数：不改入参、不消费标记（消费由调用方在战斗行动结束后用
/// [`PlungingMark::after_combat_action`] 做）。
pub fn effective_normal_attack(kind: SkillKind, inputs: AttackFormInputs) -> SkillKind {
    // 1) 只改「普通攻击本体」：非普通攻击 / 已写死特殊形态 -> 原样。
    if !matches!(kind, SkillKind::NormalAttack(None)) {
        return kind;
    }
    // 2) 下落攻击优先（规则书未写与重击叠加 -> 待实测）。
    if inputs.plunging_pending {
        return SkillKind::plunging_attack();
    }
    // 3) 行动开始前骰总数为偶数 -> 重击。
    if is_even(inputs.dice_total) {
        return SkillKind::charged_attack();
    }
    // 4) 普通攻击本体。
    kind
}

/// 生效形态的**标注**（`None` = 不是普通攻击）。
pub fn effective_attack_form(kind: SkillKind, inputs: AttackFormInputs) -> Option<AttackForm> {
    AttackForm::of(effective_normal_attack(kind, inputs))
}

/// 「切换后本回合下一个战斗行动视为下落攻击」的**待消耗标记**（S8.10）。
///
/// 状态机（全是纯值；由持有方替换字段）：
/// - 角色被切换为**出战角色** -> [`armed`](PlungingMark::armed)（置位）；
/// - 一次**战斗行动**结束后 -> [`after_combat_action`](PlungingMark::after_combat_action)（清除）；
/// - 回合结束 / 新回合开始 -> [`cleared_round`](PlungingMark::cleared_round)（清除）。
///
/// 「切换角色」本身也是战斗行动：切换后置位，**不是**立刻被这次切换消耗掉。
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct PlungingMark {
    pending: bool,
}

impl PlungingMark {
    /// 没有标记。
    pub const NONE: PlungingMark = PlungingMark { pending: false };

    /// 没有标记（同 [`PlungingMark::NONE`]）。
    pub const fn new() -> PlungingMark {
        PlungingMark::NONE
    }

    /// 标记是否待消耗（= 本次普通攻击视为下落攻击）。
    pub const fn is_pending(&self) -> bool {
        self.pending
    }

    /// 角色**被切换为出战角色**后调用：本回合「下一个战斗行动」的窗口打开。
    pub const fn armed(self) -> PlungingMark {
        PlungingMark { pending: true }
    }

    /// 一次**战斗行动**结束后调用：标记清除。
    ///
    /// `待实测`：若这次战斗行动不是普通攻击，规则书没写标记是否保留；本层按「清除」处理。
    pub const fn after_combat_action(self) -> PlungingMark {
        PlungingMark::NONE
    }

    /// 回合结束 / 新回合开始：清除（规则书：「本回合内」）。
    pub const fn cleared_round(self) -> PlungingMark {
        PlungingMark::NONE
    }

    /// 本次行动**是不是**会被视为下落攻击（= 标记待消耗且这次是普通攻击）。
    ///
    /// 便捷判定：调用方在派发「普通攻击后」/ 加伤时用；不消费标记。
    pub fn applies_to(self, kind: SkillKind) -> bool {
        self.pending && kind.is_normal_attack()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::dice::DieFace;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 建骰池：按 (骰面, 个数) 累加。
    fn pool(dice: &[(DieFace, Nat)]) -> DicePool {
        let mut built = DicePool::empty();
        for &(face, count) in dice {
            built = built.add(face, count);
        }
        built
    }

    #[test]
    fn even_parity_uses_nat_arithmetic() {
        // 0 / 2 / 8 / 16（骰池上限，D47）是偶数；1 / 3 / 15 不是。
        assert!(is_even(Nat::ZERO));
        assert!(!is_even(n(1)));
        assert!(is_even(n(2)));
        assert!(!is_even(n(3)));
        assert!(is_even(n(8)));
        assert!(!is_even(n(15)));
        assert!(is_even(n(16)));
    }

    #[test]
    fn even_dice_total_turns_normal_attack_into_charged() {
        // S8.9：行动开始前骰总数为偶数 -> 普通攻击视为重击。
        let even = AttackFormInputs::on_action_start(n(8));
        assert_eq!(
            effective_normal_attack(SkillKind::normal_attack(), even),
            SkillKind::charged_attack()
        );
        assert_eq!(
            effective_attack_form(SkillKind::normal_attack(), even),
            Some(AttackForm::Charged)
        );
        // 奇数 -> 仍是普通攻击本体。
        let odd = AttackFormInputs::on_action_start(n(7));
        assert_eq!(
            effective_normal_attack(SkillKind::normal_attack(), odd),
            SkillKind::normal_attack()
        );
        assert_eq!(
            effective_attack_form(SkillKind::normal_attack(), odd),
            Some(AttackForm::Normal)
        );
    }

    #[test]
    fn only_normal_attacks_are_rewritten() {
        // 战技 / 爆发 / 被动不吃这两条「视为」。
        let even = AttackFormInputs::on_action_start(n(8));
        for kind in [
            SkillKind::ElementalSkill,
            SkillKind::ElementalBurst,
            SkillKind::Passive,
        ] {
            assert_eq!(effective_normal_attack(kind, even), kind);
            assert_eq!(AttackForm::of(kind), None);
        }
        // 已写死的特殊形态也不改写（卡牌自己说了算）。
        assert_eq!(
            effective_normal_attack(SkillKind::plunging_attack(), even),
            SkillKind::plunging_attack()
        );
        assert_eq!(
            effective_normal_attack(SkillKind::charged_attack(), even),
            SkillKind::charged_attack()
        );
    }

    #[test]
    fn plunging_mark_turns_the_next_normal_attack_into_plunging() {
        // S8.10：切换为出战角色后，下一个战斗行动是普通攻击 -> 视为下落攻击。
        let mark = PlungingMark::new().armed();
        assert!(mark.is_pending());
        let inputs = AttackFormInputs::new(n(8), mark.is_pending());
        // 骰数为偶数（本会判重击）也按下落攻击优先。
        assert_eq!(
            effective_normal_attack(SkillKind::normal_attack(), inputs),
            SkillKind::plunging_attack()
        );
        assert!(mark.applies_to(SkillKind::normal_attack()));
        // 标记不消费（纯函数）：再判一次仍是下落攻击。
        assert_eq!(
            effective_normal_attack(SkillKind::normal_attack(), inputs),
            SkillKind::plunging_attack()
        );
    }

    #[test]
    fn plunging_mark_is_consumed_after_a_combat_action_and_cleared_at_round_end() {
        let armed = PlungingMark::new().armed();
        assert!(!armed.after_combat_action().is_pending());
        assert!(!armed.cleared_round().is_pending());
        assert!(!PlungingMark::NONE.is_pending());
        assert_eq!(PlungingMark::default(), PlungingMark::NONE);
    }

    #[test]
    fn plunging_mark_only_applies_to_normal_attacks() {
        let mark = PlungingMark::new().armed();
        assert!(!mark.applies_to(SkillKind::ElementalSkill));
        assert!(!mark.applies_to(SkillKind::ElementalBurst));
        assert!(!mark.applies_to(SkillKind::Passive));
        assert!(!PlungingMark::NONE.applies_to(SkillKind::normal_attack()));
    }

    #[test]
    fn inputs_can_be_sampled_from_the_dice_pool_before_payment() {
        // 行动开始前 3 水 + 2 万能 = 5 颗（奇数）-> 不判重击。
        let dice = pool(&[(DieFace::Hydro, n(3)), (DieFace::Omni, n(2))]);
        assert_eq!(dice.total(), n(5));
        let inputs = AttackFormInputs::from_pool(&dice, PlungingMark::NONE);
        assert_eq!(inputs, AttackFormInputs::on_action_start(n(5)));
        assert_eq!(
            effective_normal_attack(SkillKind::normal_attack(), inputs),
            SkillKind::normal_attack()
        );
        // 2 水 + 2 万能 = 4 颗（偶数）-> 重击。
        let even = pool(&[(DieFace::Hydro, n(2)), (DieFace::Omni, n(2))]);
        let inputs = AttackFormInputs::from_pool(&even, PlungingMark::NONE);
        assert_eq!(
            effective_normal_attack(SkillKind::normal_attack(), inputs),
            SkillKind::charged_attack()
        );
        assert_eq!(even.total(), n(4), "骰池总数按 4 读");
    }

    #[test]
    fn attack_form_labels_are_the_rulebook_names() {
        assert_eq!(AttackForm::Normal.label(), "普通攻击");
        assert_eq!(AttackForm::Charged.label(), "重击");
        assert_eq!(AttackForm::Plunging.label(), "下落攻击");
        assert_eq!(
            AttackForm::of(SkillKind::charged_attack()),
            Some(AttackForm::Charged)
        );
        assert_eq!(
            AttackForm::of(SkillKind::plunging_attack()),
            Some(AttackForm::Plunging)
        );
    }
}
