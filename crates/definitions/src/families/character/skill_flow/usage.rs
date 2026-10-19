//! **技能使用计数**（D15 / D181 / D182 / D183）：每个技能「每回合 N 次」这类上限 + 本回合已用次数。
//!
//! 为什么单独一个文件、而不放 `stats/`（D187）：使用次数是**规则状态**（「每回合 1 次」决定能不能用），
//! 不是只影响显示的统计 —— 关掉它会改变合法性，所以**不做 feature 裁剪**；底层计数器仍复用
//! `engine::stats::Counter`（D181：引擎只给计数器原语）。
//!
//! 形态（D182 / D183）：
//! - 普通值类型、由**角色区 / 出战角色区自己当字段持有**，没有全局状态、没有注册表、没有 id 绑定；
//! - 字段私有，对外只给只读指针（[`SkillUses::used`] 返回 `&Counter`）、只给 `&self` 查询；
//! - 更新是**纯函数**（[`SkillUses::recorded`] / [`SkillUses::reset_round`] 返回新值，不改入参），
//!   与「纯函数优先 + 失败不改状态」一致。
//!
//! 生命周期：回合开始时由持有方调用 [`SkillUses::reset_round`] 清零（规则书·回合流程「开始新的回合」）。

use crate::base::SkillId;
use gi_genius_invokation_engine::stats::Counter;
use gi_genius_invokation_engine::{nsub, Nat};

/// 一个技能的使用上限与已用次数（**纯数据**；字段私有，D183）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillUseEntry {
    skill: SkillId,
    limit: Option<Nat>,
    used: Counter,
}

impl SkillUseEntry {
    /// 这条统计是哪个技能。
    pub fn skill(&self) -> SkillId {
        self.skill
    }

    /// 每回合上限（`None` = 不限次数）。
    pub fn limit(&self) -> Option<Nat> {
        self.limit
    }

    /// 本回合已用次数（只读指针，D183）。
    pub fn used(&self) -> &Counter {
        &self.used
    }
}

/// 每个技能的**每回合使用计数**（区自持的普通值，D182）。
///
/// - 没有登记的技能 = **不限次数**；
/// - 上限由卡牌 / 状态自己登记（[`SkillUses::with_limit`]）—— 本层不预设「谁每回合几次」；
/// - 「每回合」的清零由持有方在回合开始调用 [`SkillUses::reset_round`]。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillUses {
    entries: Vec<SkillUseEntry>,
}

impl SkillUses {
    /// 空容器（所有技能都不限次数）。
    pub const fn new() -> SkillUses {
        SkillUses {
            entries: Vec::new(),
        }
    }

    /// 登记 / 覆盖一个技能的每回合上限（**纯函数**：返回新容器）。
    pub fn with_limit(&self, skill: SkillId, limit: Nat) -> SkillUses {
        let mut next = self.clone();
        match next.entries.iter_mut().find(|entry| entry.skill == skill) {
            Some(entry) => entry.limit = Some(limit),
            None => next.entries.push(SkillUseEntry {
                skill,
                limit: Some(limit),
                used: Counter::ZERO,
            }),
        }
        next
    }

    /// 这个技能的每回合上限（`None` = 不限）。
    pub fn limit(&self, skill: SkillId) -> Option<Nat> {
        self.entries
            .iter()
            .find(|entry| entry.skill == skill)
            .and_then(|entry| entry.limit)
    }

    /// 本回合已用次数（只读指针，D183）；没有记录 -> 静态 0 计数器。
    pub fn used(&self, skill: SkillId) -> &Counter {
        match self.entries.iter().find(|entry| entry.skill == skill) {
            Some(entry) => &entry.used,
            None => {
                static ZERO: Counter = Counter::ZERO;
                &ZERO
            }
        }
    }

    /// 本回合已用次数（数值便利读法）。
    pub fn used_count(&self, skill: SkillId) -> Nat {
        self.used(skill).get()
    }

    /// 本回合还能用几次（`None` = 不限；已达上限 -> `Some(ZERO)`）。
    pub fn remaining(&self, skill: SkillId) -> Option<Nat> {
        self.limit(skill)
            .map(|limit| nsub!(limit, self.used_count(skill)))
    }

    /// 这个技能本回合还能不能用（只看次数，不看费用 / 阻塞）。
    pub fn can_use(&self, skill: SkillId) -> bool {
        match self.limit(skill) {
            Some(limit) => self.used_count(skill) < limit,
            None => true,
        }
    }

    /// 记一次使用（**纯函数**：返回「已用 +1」的新容器，不改入参）。
    ///
    /// 没登记过的技能也会被记进容器（上限 `None`），便于查询「本回合用过几次」。
    pub fn recorded(&self, skill: SkillId) -> SkillUses {
        let mut next = self.clone();
        match next.entries.iter_mut().find(|entry| entry.skill == skill) {
            Some(entry) => {
                entry.used.add(Nat::new(1));
            }
            None => next.entries.push(SkillUseEntry {
                skill,
                limit: None,
                used: Counter::new(Nat::new(1)),
            }),
        }
        next
    }

    /// 回合开始：所有已用次数清零（上限保留；**纯函数**）。
    pub fn reset_round(&self) -> SkillUses {
        let mut next = self.clone();
        for entry in &mut next.entries {
            entry.used.clear();
        }
        next
    }

    /// 登记的技能条数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 是否没有任何登记项。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按登记顺序只读遍历。
    pub fn iter(&self) -> impl Iterator<Item = &SkillUseEntry> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL: SkillId = SkillId::new(12161);
    const BURST: SkillId = SkillId::new(12163);

    #[test]
    fn unregistered_skill_is_unlimited() {
        let uses = SkillUses::new();
        assert!(uses.is_empty());
        assert_eq!(uses.limit(NORMAL), None);
        assert_eq!(uses.used_count(NORMAL), Nat::ZERO);
        assert_eq!(uses.remaining(NORMAL), None);
        assert!(uses.can_use(NORMAL));
        assert_eq!(SkillUses::default(), uses);
    }

    #[test]
    fn limit_blocks_after_one_use() {
        let uses = SkillUses::new().with_limit(NORMAL, Nat::new(1));
        assert!(uses.can_use(NORMAL));
        assert_eq!(uses.remaining(NORMAL), Some(Nat::new(1)));
        let after = uses.recorded(NORMAL);
        assert_eq!(after.used_count(NORMAL), Nat::new(1));
        assert!(!after.can_use(NORMAL));
        assert_eq!(after.remaining(NORMAL), Some(Nat::ZERO));
        // 纯函数：入参一个字节都不改。
        assert_eq!(uses.used_count(NORMAL), Nat::ZERO);
        assert!(uses.can_use(NORMAL));
    }

    #[test]
    fn two_per_round_limit_allows_two_uses() {
        let uses = SkillUses::new().with_limit(NORMAL, Nat::new(2));
        let once = uses.recorded(NORMAL);
        assert!(once.can_use(NORMAL));
        let twice = once.recorded(NORMAL);
        assert!(!twice.can_use(NORMAL));
        assert_eq!(twice.used_count(NORMAL), Nat::new(2));
    }

    #[test]
    fn reset_round_clears_counts_and_keeps_limits() {
        let uses = SkillUses::new()
            .with_limit(NORMAL, Nat::new(1))
            .recorded(NORMAL);
        assert!(!uses.can_use(NORMAL));
        let next_round = uses.reset_round();
        assert!(next_round.can_use(NORMAL));
        assert_eq!(next_round.used_count(NORMAL), Nat::ZERO);
        assert_eq!(next_round.limit(NORMAL), Some(Nat::new(1)));
    }

    #[test]
    fn unregistered_use_is_recorded_without_limit() {
        let uses = SkillUses::new().recorded(BURST);
        assert_eq!(uses.used_count(BURST), Nat::new(1));
        assert_eq!(uses.limit(BURST), None);
        assert!(uses.can_use(BURST));
        assert_eq!(uses.len(), Nat::new(1));
    }

    #[test]
    fn entries_are_independent_and_readable() {
        let uses = SkillUses::new()
            .with_limit(NORMAL, Nat::new(1))
            .recorded(NORMAL)
            .recorded(BURST);
        assert_eq!(uses.used_count(NORMAL), Nat::new(1));
        assert_eq!(uses.used_count(BURST), Nat::new(1));
        assert!(!uses.can_use(NORMAL));
        assert!(uses.can_use(BURST));
        assert_eq!(uses.len(), Nat::new(2));
        // 只读指针：同一次调用拿到的值一致。
        assert_eq!(uses.used(NORMAL).get(), Nat::new(1));
        let listed: Vec<SkillId> = uses.iter().map(|entry| entry.skill()).collect();
        assert_eq!(listed, vec![NORMAL, BURST]);
    }

    #[test]
    fn read_only_pointer_for_unrecorded_skill_is_zero() {
        let uses = SkillUses::new().with_limit(NORMAL, Nat::new(1));
        assert!(uses.used(BURST).is_zero());
        // 有记录的技能：指针指向真实计数。
        let after = uses.recorded(NORMAL);
        assert_eq!(after.used(NORMAL).get(), Nat::new(1));
    }
}
