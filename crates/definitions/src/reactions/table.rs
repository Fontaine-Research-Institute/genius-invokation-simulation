//! 反应表：**规则表**（基础反应 + 月反应覆写）+ **效果函数指针表**（D186）。
//!
//! 两类表都不新造机制：
//! - 规则表 = `damage::ReactionRule` 的有序切片（交给 `DamageEnv.rules`，已有接口）；
//! - 效果表 = 「反应身份 -> 执行函数指针」的**静态查表**，没有任何 `register_*` 接口、
//!   没有堆分配、没有运行期状态；这不是第二套注册机制，而是 D186 的「静态数据 + 函数指针表」。
//!   卡牌「反应时」通知仍走 `attachment::ReactionRegistry`（D127 / D136），两者不混用。
//!
//! **月反应覆写**（D14 / D128 / D129）：
//! - 月感电 / 月绽放 / 月结晶分别与基础感电 / 绽放 / 结晶**同元素对**；
//! - 覆写是「反应部分级」：只替换效果，原效果完全不执行；
//! - 框架**不设角色级启用字段**（D14）：由卡牌文字决定本次伤害该用哪套规则，
//!   所以 [`rules`] 收一个调用方给的 [`LunarOverrides`]，把启用的月反应规则排在基础规则前面
//!   （同元素对先命中 -> 月反应生效，基础反应不执行）。
//!
//! 判定顺序（`damage` 内）：规则表按顺序找**第一条命中**的规则；因此只要把要覆写的
//! 月反应放在前面就能覆写，不需要改动基础规则。

use crate::families::character::attachment::{same_reaction_identity, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, ReactionEffectFn, ReactionEffectResult};
use crate::reactions::{
    bloom, burning, crystallize, electro_charged, frozen, lunar_bloom, lunar_charged,
    lunar_crystallize, melt, overloaded, quicken, superconduct, swirl, vaporize,
};

// ======================================================================
// 规则表（damage::ReactionRule，已有接口）
// ======================================================================

/// 基础反应规则条数（规则书 17 条基础反应条目，按同一身份折叠后 11 条：
/// 扩散 4 条 -> 1 个身份、结晶 4 条 -> 1 个身份）。
pub const BASE_RULE_COUNT: usize = 11;

/// 月反应规则条数（月感电 / 月绽放 / 月结晶）。
pub const LUNAR_RULE_COUNT: usize = 3;

/// 基础反应规则表（顺序 = 规则书条目顺序；元素对互不重叠，顺序不影响判定）。
///
/// 交给 `damage::DamageEnv { rules: &base_rules(), .. }`；
/// 需要月反应覆写时改用 [`rules`]。
pub fn base_rules() -> Vec<ReactionRule> {
    vec![
        melt::rule(),
        vaporize::rule(),
        overloaded::rule(),
        superconduct::rule(),
        electro_charged::rule(),
        frozen::rule(),
        swirl::rule(),
        crystallize::rule(),
        burning::rule(),
        bloom::rule(),
        quicken::rule(),
    ]
}

/// **月反应覆写开关**（D14：不做「角色定义里声明启用哪些月反应」这种字段；
/// 由卡牌文字在调用点决定本次伤害用哪套规则）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct LunarOverrides {
    /// 感电 -> 月感电。
    pub electro_charged: bool,
    /// 绽放 -> 月绽放。
    pub bloom: bool,
    /// 结晶（水 + 岩那一对）-> 月结晶。
    pub crystallize: bool,
}

impl LunarOverrides {
    /// 全部不覆写（纯基础反应）。
    pub const NONE: LunarOverrides = LunarOverrides {
        electro_charged: false,
        bloom: false,
        crystallize: false,
    };

    /// 三条月反应全部覆写。
    pub const ALL: LunarOverrides = LunarOverrides {
        electro_charged: true,
        bloom: true,
        crystallize: true,
    };

    /// 有没有启用任何月反应。
    pub const fn any(self) -> bool {
        self.electro_charged || self.bloom || self.crystallize
    }
}

/// **本次伤害使用的规则表**：启用的月反应规则在前，基础规则在后。
///
/// - 返回 `Vec`：切片要活到 `DamageEnv` 用完（`DamageEnv.rules` 是借用）；
/// - 月反应与基础反应同元素对 -> 先命中月反应（覆写：原效果完全不执行，D128 / D129）；
/// - 未启用的月反应不出现 -> 对应元素对走基础反应。
pub fn rules(overrides: LunarOverrides) -> Vec<ReactionRule> {
    let mut rules = Vec::new();
    if overrides.electro_charged {
        rules.push(lunar_charged::rule());
    }
    if overrides.bloom {
        rules.push(lunar_bloom::rule());
    }
    if overrides.crystallize {
        rules.push(lunar_crystallize::rule());
    }
    rules.extend(base_rules());
    rules
}

// ======================================================================
// 效果函数指针表（D186）
// ======================================================================

/// 一条**效果登记项**：反应身份 + 执行函数指针（D186：登记函数指针，不登记效果数据）。
#[derive(Clone, Copy, Debug)]
pub struct EffectEntry {
    /// 反应身份（与规则表里的身份是同一份静态实例，按**指针**比较）。
    pub reaction: &'static dyn ReactionIdentity,
    /// 效果函数指针（纯函数，不执行、不采样）。
    pub effect: ReactionEffectFn,
}

impl EffectEntry {
    /// 一条登记项。
    pub const fn new(
        reaction: &'static dyn ReactionIdentity,
        effect: ReactionEffectFn,
    ) -> EffectEntry {
        EffectEntry { reaction, effect }
    }
}

/// 效果登记项条数（基础 11 + 月反应 3）。
pub const EFFECT_COUNT: usize = 14;

/// 全部效果登记项（基础反应在前、月反应在后；顺序不参与判定，只影响 debug / 清单）。
pub const EFFECTS: [EffectEntry; EFFECT_COUNT] = [
    EffectEntry::new(&melt::MELT, melt::effect),
    EffectEntry::new(&vaporize::VAPORIZE, vaporize::effect),
    EffectEntry::new(&overloaded::OVERLOADED, overloaded::effect),
    EffectEntry::new(&superconduct::SUPERCONDUCT, superconduct::effect),
    EffectEntry::new(&electro_charged::ELECTRO_CHARGED, electro_charged::effect),
    EffectEntry::new(&frozen::FROZEN, frozen::effect),
    EffectEntry::new(&swirl::SWIRL, swirl::effect),
    EffectEntry::new(&crystallize::CRYSTALLIZE, crystallize::effect),
    EffectEntry::new(&burning::BURNING, burning::effect),
    EffectEntry::new(&bloom::BLOOM, bloom::effect),
    EffectEntry::new(&quicken::QUICKEN, quicken::effect),
    EffectEntry::new(&lunar_charged::LUNAR_CHARGED, lunar_charged::effect),
    EffectEntry::new(&lunar_bloom::LUNAR_BLOOM, lunar_bloom::effect),
    EffectEntry::new(
        &lunar_crystallize::LUNAR_CRYSTALLIZE,
        lunar_crystallize::effect,
    ),
];

/// 全部效果登记项（只读切片）。
pub fn effect_table() -> &'static [EffectEntry] {
    &EFFECTS
}

/// 按**反应身份数据地址**查效果函数指针（没登记 -> `None`）。
///
/// 身份 = `ReactionIdentity::identity_address()`（D57 / D58；ZST 身份没有独立地址）：不建编号表、不用字符串匹配。
pub fn effect_of(reaction: &'static dyn ReactionIdentity) -> Option<ReactionEffectFn> {
    EFFECTS
        .iter()
        .find(|entry| same_reaction_identity(entry.reaction, reaction))
        .map(|entry| entry.effect)
}

/// 对一次已判定成功的反应求效果（纯函数；没登记 -> `None`）。
///
/// 调用时机：`damage` 返回 `reaction: Some(..)` 之后（插入式结算：附着 -> 判定反应 ->
/// 反应效果 -> 继续原步骤，见 `结算顺序.md` §三）。
pub fn resolve(
    event: &ReactionEvent,
    context: &EffectContext,
) -> Option<ReactionEffectResult> {
    effect_of(event.reaction).map(|effect| effect(event, context))
}

/// 这条反应是不是月反应（月感电 / 月绽放 / 月结晶）。
pub fn is_lunar(reaction: &'static dyn ReactionIdentity) -> bool {
    same_reaction_identity(reaction, &lunar_charged::LUNAR_CHARGED)
        || same_reaction_identity(reaction, &lunar_bloom::LUNAR_BLOOM)
        || same_reaction_identity(reaction, &lunar_crystallize::LUNAR_CRYSTALLIZE)
}

/// 这条规则的条件是否命中（`damage` 判定用同一口径；这里给测试 / 调用方查表用）。
pub fn rule_matches(
    rule: &ReactionRule,
    attachment: &crate::families::character::attachment::Attachment,
    attack: crate::base::Element,
) -> Option<crate::base::Element> {
    attachment.matching_covered(attack, rule.conditions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::families::character::attachment::{Attachment, ReactionCondition};
    use gi_genius_invokation_engine::Nat;

    /// 规则书 14 个反应身份及「本伤害 +N」（清单核对用）。
    const EXPECTED_BONUS: [(&str, u32); EFFECT_COUNT] = [
        ("融化", 2),
        ("蒸发", 2),
        ("超载", 2),
        ("超导", 1),
        ("感电", 1),
        ("冻结", 1),
        ("扩散", 0),
        ("结晶", 1),
        ("燃烧", 1),
        ("绽放", 1),
        ("原激化", 1),
        ("月感电", 0),
        ("月绽放", 1),
        ("月结晶", 1),
    ];

    fn benign_event(reaction: &'static dyn ReactionIdentity) -> ReactionEvent {
        // 元素对只影响扩散的伤害元素，加伤是常数：用任意元素构造即可。
        ReactionEvent::new(
            reaction,
            Element::Hydro,
            Element::Pyro,
            Nat::new(1),
            Nat::new(2),
        )
    }

    #[test]
    fn base_and_lunar_rule_counts_match_the_rulebook() {
        assert_eq!(base_rules().len(), BASE_RULE_COUNT);
        assert_eq!(rules(LunarOverrides::NONE).len(), BASE_RULE_COUNT);
        assert_eq!(
            rules(LunarOverrides::ALL).len(),
            BASE_RULE_COUNT + LUNAR_RULE_COUNT
        );
        assert_eq!(EFFECTS.len(), EFFECT_COUNT);
    }

    #[test]
    fn every_effect_entry_has_a_unique_reaction_identity_and_a_rule() {
        // 身份唯一（指针身份，D95 / D96）：两两不同。
        for (left_index, left) in EFFECTS.iter().enumerate() {
            for right in EFFECTS.iter().skip(left_index + 1) {
                assert!(
                    !same_reaction_identity(left.reaction, right.reaction),
                    "反应身份重复：{} / {}",
                    left.reaction.name(),
                    right.reaction.name()
                );
            }
        }
        // 每个效果身份都能在基础 / 月规则表里找到规则（基础 11 + 月 3 = 14）。
        let all_rules = rules(LunarOverrides::ALL);
        assert_eq!(all_rules.len(), EFFECT_COUNT);
        for entry in &EFFECTS {
            assert!(
                all_rules
                    .iter()
                    .any(|rule| same_reaction_identity(rule.reaction, entry.reaction)),
                "反应 {} 没有规则表条目",
                entry.reaction.name()
            );
        }
    }

    #[test]
    fn bonus_matches_the_rulebook_for_every_reaction() {
        for (name, expected) in EXPECTED_BONUS {
            let entry = EFFECTS
                .iter()
                .find(|entry| entry.reaction.name() == name)
                .expect("规则书里的每个反应都必须有登记项");
            let result = (entry.effect)(
                &benign_event(entry.reaction),
                &EffectContext::no_hands(true),
            );
            assert_eq!(
                result.damage_bonus,
                Nat::new(expected),
                "{name} 的「本伤害+N」与规则书不一致"
            );
        }
    }

    #[test]
    fn every_rule_condition_pair_is_claimed_by_exactly_one_base_rule() {
        // 基础反应的元素对互不重叠（月反应除外，它们只在启用的表里排在前面）。
        let base = base_rules();
        for rule in &base {
            for condition in rule.conditions {
                let claims = base
                    .iter()
                    .filter(|candidate| {
                        candidate
                            .conditions
                            .contains(&ReactionCondition::new(condition.covered, condition.attack))
                    })
                    .count();
                assert_eq!(
                    claims, 1,
                    "{:?}+{:?} 被 {claims} 条基础规则同时认领",
                    condition.covered, condition.attack
                );
            }
        }
    }

    #[test]
    fn lunar_rules_come_first_and_override_their_base_counterparts() {
        let all = rules(LunarOverrides::ALL);
        for (index, lunar) in [
            &lunar_charged::LUNAR_CHARGED as &'static dyn ReactionIdentity,
            &lunar_bloom::LUNAR_BLOOM,
            &lunar_crystallize::LUNAR_CRYSTALLIZE,
        ]
        .iter()
        .enumerate()
        {
            assert!(same_reaction_identity(all[index].reaction, *lunar));
            assert!(is_lunar(all[index].reaction));
        }

        // 水 + 岩：启用月结晶 -> 第一条命中月结晶；不启用 -> 命中基础结晶。
        let attached = Attachment::with(Element::Hydro);
        let pick = |rules: &[ReactionRule]| {
            rules
                .iter()
                .find(|rule| rule_matches(rule, &attached, Element::Geo).is_some())
                .map(|rule| rule.reaction)
        };
        let lunar_pick = pick(&all).expect("应命中一条规则");
        assert!(same_reaction_identity(lunar_pick, &lunar_crystallize::LUNAR_CRYSTALLIZE));
        let base_pick = pick(&base_rules()).expect("应命中一条规则");
        assert!(same_reaction_identity(base_pick, &crystallize::CRYSTALLIZE));

        // 水 + 雷：启用月感电 -> 月感电；水 + 草：启用月绽放 -> 月绽放。
        for (covered, attack, expected) in [
            (
                Element::Hydro,
                Element::Electro,
                &lunar_charged::LUNAR_CHARGED as &'static dyn ReactionIdentity,
            ),
            (
                Element::Hydro,
                Element::Dendro,
                &lunar_bloom::LUNAR_BLOOM,
            ),
        ] {
            let attached = Attachment::with(covered);
            let picked = all
                .iter()
                .find(|rule| rule_matches(rule, &attached, attack).is_some())
                .map(|rule| rule.reaction)
                .expect("应命中一条规则");
            assert!(same_reaction_identity(picked, expected));
        }
    }

    #[test]
    fn resolve_returns_the_registered_effect() {
        let event = benign_event(&vaporize::VAPORIZE);
        let result = resolve(&event, &EffectContext::no_hands(true)).expect("蒸发已登记");
        assert_eq!(result.damage_bonus, Nat::new(2));
        assert!(result.probability_total().is_one());
    }

    #[test]
    fn lunar_override_flags_are_independent() {
        assert!(!LunarOverrides::NONE.any());
        assert!(LunarOverrides::ALL.any());
        let only_charged = LunarOverrides {
            electro_charged: true,
            ..LunarOverrides::NONE
        };
        assert_eq!(rules(only_charged).len(), BASE_RULE_COUNT + 1);
        assert_eq!(
            rules(only_charged)[0].reaction.name(),
            "月感电",
            "只有启用的月反应排在前面"
        );
    }
}