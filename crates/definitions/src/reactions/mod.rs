//! **具体元素反应效果**（覆盖报告 R5 缺口：具体反应效果 37 实体）。
//!
//! 行为依据**只读规则书**（两篇：`elementalreactionsrules.json` 元素反应·规则、
//! `elementalreactionseffects.json` 元素反应·效果）+ 卡面文本 + 权威关键字；不读任何参考实现。
//! 顺序依据 `结算顺序.md` §三「附着 -> 判定反应 -> 反应效果 -> 继续原步骤」（暂定版）。
//!
//! # 反应清单（规则书 20 条 -> 14 个反应身份）
//!
//! 规则书·元素反应·效果共 20 条：基础反应 17 条 + 月反应 3 条。扩散 4 条只差
//! 「扩散出去的元素」、结晶 4 条效果完全相同，各折叠成**一个身份**（元素由命中的
//! 已覆盖元素 / 固定值推出），所以是 11 个基础身份 + 3 个月反应身份 = 14 个。
//!
//! | 反应 | 元素对（已覆盖 / 本次攻击） | 效果（规则书原文摘要） | 依据 |
//! |---|---|---|---|
//! | 融化 | 冰/火（任一先后） | 本伤害 +2 | 元素反应·效果「融化」 |
//! | 蒸发 | 水/火 | 本伤害 +2 | 元素反应·效果「蒸发」 |
//! | 超载 | 雷/火 | 本伤害 +2；目标是出战角色时强制切换到下一个角色 | 元素反应·效果「超载」 |
//! | 超导 | 雷/冰 | 本伤害 +1；对目标以外所有敌方角色 1 点穿透伤害 | 元素反应·效果「超导」 |
//! | 感电 | 雷/水 | 本伤害 +1；对目标以外所有敌方角色 1 点穿透伤害 | 元素反应·效果「感电」 |
//! | 冻结 | 冰/水 | 本伤害 +1；目标附属[冻结]（无法使用技能，持续到回合结束；受火/物理伤害 +2 并移除） | 元素反应·效果「冻结」 |
//! | 扩散 | 风 + 冰/水/火/雷 | 对目标以外所有敌方角色 1 点**该元素**伤害 | 元素反应·效果「扩散」×4 |
//! | 结晶 | 岩 + 冰/水/火/雷 | 本伤害 +1；我方出战角色 1 点护盾（可叠加，最多 2 点） | 元素反应·效果「结晶」×4 |
//! | 燃烧 | 火/草 | 本伤害 +1；生成[燃烧烈焰]（结束阶段 1 火伤，可用 1，最多叠 2） | 元素反应·效果「燃烧」 |
//! | 绽放 | 水/草 | 本伤害 +1；生成[草原核]（火/雷对敌方出战 +2，可用 1） | 元素反应·效果「绽放」 |
//! | 原激化 | 雷/草 | 本伤害 +1；生成[激化领域]（雷/草对敌方出战 +1，可用 2） | 元素反应·效果「原激化」 |
//! | 月感电 | 水/雷 | 生成[雷暴云]（结束阶段 2 雷伤，可用 1，无限叠）；入场 / 可用增加时赋予敌方随机 1 张手牌[电击] | 元素反应·效果「月感电」、关键字 K3001 |
//! | 月绽放 | 水/草 | 本伤害 +1；赋予我方随机 1 张手牌[费用降低] | 元素反应·效果「月绽放」、关键字 K3009 |
//! | 月结晶 | 水/岩 | 本伤害 +1；生成 1 张[月笼协奏]加入手牌 | 元素反应·效果「月结晶」、关键字 K3010 |
//!
//! 规则书·元素反应·规则（另一篇）给出两条通用规则：
//! 1. 对已具有「元素附着」的目标造成特定类型「元素伤害」时，会引发「元素反应」；
//! 2. 以非伤害形式施加特定类型「元素附着」时也会引发反应 —— 结算这类反应时
//!    **忽略所有伤害类效果**。
//!
//! 第 2 条已接入关键字层（R2）：keywords::attach_element 判定反应后产出反应子结算，
//! 由步边界插入执行（D204）；「忽略所有伤害类效果」用
//! [`ReactionEffectResult::without_damage`]（只过滤 EffectOp::Damage + 本伤害加伤归零，
//! 不新增第二套效果语义）。
//!
//! # 用已有接口（不另造注册机制）
//!
//! - **反应身份**：[`ReactionIdentity`] 的零大小类型 + 静态实例（每反应一个文件，D95/D96）；
//!   身份比较一律用 `ReactionIdentity::identity_address()`（指向身份**数据**的地址，D57/D58）：
//!   反应身份是 ZST，**不同 ZST 静态实例的地址可能相同**，所以不能拿 `&dyn` 的地址当身份
//!   （`attachment::ReactionRegistry` 按种类命中已同步改口径）。
//!   按元素对注册（`register_by_elements`）是**值比较**，同样不受影响。
//! - **触发条件**：[`ReactionCondition`]（D132：同一对元素两种先后各一条）；
//! - **规则表**：[`damage::ReactionRule`] 的有序切片（直接交给 `DamageEnv.rules`）；
//! - **效果登记**：D186 —— 登记**执行函数指针**（[`effect::ReactionEffectFn`]）。
//!   本模块只给「反应身份 -> 函数指针」的**静态查表**（[`table::effect_table`] /
//!   [`table::effect_of`]），没有 `register_*`、没有堆分配、没有运行期状态；
//!   卡牌「反应时」通知仍用 `attachment::ReactionRegistry`（D127 / D136），两者不混用。
//!
//! # 效果怎么表达（纯函数 + 底层操作 + D190）
//!
//! 效果函数吃 [`ReactionEvent`] + [`EffectContext`]（本次效果需要的最小局面摘要），
//! 返回 [`ReactionEffectResult`]：
//! - `damage_bonus` = 规则书「本伤害 +N」；
//! - `branches` = 「底层操作清单 + 精确概率」。每个 [`EffectOp`] 对应**一个已有底层操作**
//!   （伤害 `damage()` / 附着 `Attachment` / 状态区 apply / 召唤物区 summon / 手牌区 add），
//!   效果函数只产出操作请求，**不自己写结算**；
//! - 需要随机的（月感电 / 月绽放）按 D190 展开成「具体分支 + `Fraction` 精确概率」，
//!   **不采样**；展开是临时的（D191：结果只是一次结算的待执行清单，不写进状态）。
//!
//! 反应对附着的消耗按 **D167**：由伤害入口在判定命中后直接完成（移除命中的已覆盖元素 +
//! 本次攻击元素，攻击元素不再附着）。本模块**不碰附着**。
//!
//! # 待接入（执行器）
//!
//! `damage()` 目前只做「扣血 + 附着 + 反应判定 / 消耗 + 通知注册表」，**不加伤**、
//! 也不执行反应效果（D184 第 3 步「伤害修正链」与第 5 步「具体反应效果」都还是缺口）。
//! 所以本模块产出的 `damage_bonus` 与 `EffectOp` 需要调用方（技能 / 时间线层）：
//! 1. 调 `damage()` **之前**先判定反应、把 `damage_bonus` 加到本次伤害上（规则书「本伤害+N」）；
//! 2. `damage()` 返回后按 `table::resolve` 的 `branches` 逐个执行 `EffectOp`；
//! 3. 执行器按 `结算顺序.md` §三 的插入点插入子结算（D196），做完回到原 cursor。
//!
//! # 待复核 / 待实测
//!
//! - 结晶护盾「最多 2 点」：护盾状态 203 卡面写「没有上限」，规则书结晶条写「最多 2 点」；
//!   暂按「结晶给的护盾叠到 2 点封顶」（`crystallize::SHIELD_STACK_CAP`），**待实测**；
//! - 「我方 / 敌方」：规则书从造成伤害一方视角写，代码用 [`effect::Side`] 的
//!   「触发方 / 承载方」相对关系表达；两侧如何绑定由调用方决定（引擎不分敌我，D29/D34）；
//! - 冻结的「受到火/物理伤害 +2 并移除此状态」属于状态牌 106 的实现，本模块只附属状态；
//! - 燃烧烈焰 / 雷暴云 / 月笼 / 月笼协奏 / 电击 / 费用降低的具体规则属于各自卡牌，
//!   本模块只生成 / 赋予，不实现它们的回合结束效果；
//! - 非伤害形式的元素附着引发反应已由 keywords 层接入（见上）；反应效果造成的伤害是否
//!   再引发第二层反应仍待复核（执行器内用 damage()，不递归执行效果）。

pub mod bloom;
pub mod burning;
pub mod crystallize;
pub mod effect;
pub mod electro_charged;
pub mod entities;
pub mod frozen;
pub mod lunar_bloom;
pub mod lunar_charged;
pub mod lunar_crystallize;
pub mod melt;
pub mod overloaded;
pub mod quicken;
pub mod superconduct;
pub mod swirl;
pub mod table;
pub mod vaporize;

pub use effect::{
    uniform_hand_branches, EffectContext, EffectOp, HandCardStatus, ReactionBranch,
    ReactionEffectFn, ReactionEffectResult, Side, StatusTarget, TargetScope,
};
pub use table::{
    base_rules, effect_of, effect_table, is_lunar, resolve, rule_matches, rules, EffectEntry,
    LunarOverrides, BASE_RULE_COUNT, EFFECTS, EFFECT_COUNT, LUNAR_RULE_COUNT,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::families::character::attachment::{
        Attachment, AttachmentAllowSet, ReactionCondition, ReactionEvent, ReactionHandler,
        ReactionRegistry,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::{damage, DamageEnv};
    use crate::families::character::types::DamageType;
    use gi_genius_invokation_engine::Nat;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 记录「反应时」注册表被调用的次数（证明本模块的规则表能接上已有注册接口）。
    #[derive(Debug)]
    struct Counter(&'static AtomicUsize);

    impl ReactionHandler for Counter {
        fn on_reaction(&self, _event: &ReactionEvent) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn character() -> CharacterState {
        CharacterState::new(Nat::new(10), Nat::new(10), Nat::ZERO, Nat::new(2))
    }

    #[test]
    fn melt_rule_plugs_into_the_existing_damage_entry() {
        // 本模块的规则表 + 已有伤害入口：反应被判定、参与元素被消耗（D167）、注册表被通知。
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        // 按元素对注册是**值比较**；按种类注册现在按 `identity_address`（身份数据地址）比。
        registry.register_by_elements(ReactionCondition::new(Element::Cryo, Element::Pyro), &HANDLER);

        let rules = base_rules();
        let env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let mut state = character();
        state.attachment = Attachment::with(Element::Cryo);
        let outcome = damage(
            &state,
            Nat::new(1),
            Nat::new(2),
            Nat::new(2),
            DamageType::elemental(Element::Pyro),
            &env,
        );
        assert_eq!(outcome.reaction.map(|reaction| reaction.name()), Some("融化"));
        // D167：水（参与）+ 火（本次攻击）都被移除，火不再附着。
        assert!(!outcome.attachment_landed);
        assert!(outcome.state.attachment.is_empty());
        assert_eq!(outcome.fired, Nat::new(1));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn damage_entry_does_not_apply_the_bonus_yet() {
        // 现状（待接入）：damage() 只扣原始伤害；「本伤害+N」要调用方在调用前加。
        let rules = base_rules();
        let registry = ReactionRegistry::new();
        let env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let mut state = character();
        state.attachment = Attachment::with(Element::Cryo);
        let outcome = damage(
            &state,
            Nat::new(1),
            Nat::new(2),
            Nat::new(2),
            DamageType::elemental(Element::Pyro),
            &env,
        );
        // 原始 2 点：先扣 2（10 -> 8），加伤 2 由效果函数给出（合力应为 6 点血的扣减）。
        assert_eq!(outcome.state.hp(), Nat::new(8));
        let event = ReactionEvent::new(
            &melt::MELT,
            Element::Cryo,
            Element::Pyro,
            Nat::new(1),
            Nat::new(2),
        );
        let result = resolve(&event, &EffectContext::no_hands(true)).expect("融化已登记");
        assert_eq!(result.damage_bonus, Nat::new(2));
    }

    #[test]
    fn multi_element_attachment_only_consumes_the_reacting_elements() {
        // D167：冰 + 草共存，本次火只与冰反应 -> 冰 + 火被消耗，草保留。
        let rules = base_rules();
        let registry = ReactionRegistry::new();
        let env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let mut state = character();
        state.attachment = Attachment::from_elements(&[Element::Cryo, Element::Dendro]);
        let outcome = damage(
            &state,
            Nat::new(1),
            Nat::new(2),
            Nat::new(1),
            DamageType::elemental(Element::Pyro),
            &env,
        );
        assert_eq!(outcome.reaction.map(|reaction| reaction.name()), Some("融化"));
        assert_eq!(outcome.state.attachment.elements(), vec![Element::Dendro]);
    }

    #[test]
    fn lunar_overrides_are_selected_by_the_caller_through_the_rule_table() {
        // D14：框架不设「角色级启用字段」；卡牌文字决定本次伤害用哪套表。
        let all = rules(LunarOverrides::ALL);
        let registry = ReactionRegistry::new();
        let env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &all,
            registry: &registry,
        };
        let mut state = character();
        state.attachment = Attachment::with(Element::Hydro);
        let outcome = damage(
            &state,
            Nat::new(1),
            Nat::new(2),
            Nat::new(1),
            DamageType::elemental(Element::Geo),
            &env,
        );
        assert_eq!(
            outcome.reaction.map(|reaction| reaction.name()),
            Some("月结晶")
        );
        assert!(state.attachment.has(Element::Hydro));
        assert!(outcome.state.attachment.is_empty());
    }

    #[test]
    fn every_reaction_effect_returns_probabilities_summing_to_one() {
        // D190：随机只展开成「分支 + 精确概率」，不采样；确定性效果则是单支概率 1。
        let context = EffectContext::new(true, Nat::new(2), Nat::new(3));
        for entry in effect_table() {
            let event = ReactionEvent::new(
                entry.reaction,
                Element::Hydro,
                Element::Pyro,
                Nat::new(1),
                Nat::new(2),
            );
            let result = (entry.effect)(&event, &context);
            assert!(
                result.probability_total().is_one(),
                "反应 {} 的分支概率之和不等于 1",
                entry.reaction.name()
            );
            assert!(
                !result.branches.is_empty(),
                "反应 {} 至少应有一支（确定性效果 = 一支概率 1）",
                entry.reaction.name()
            );
        }
    }
}