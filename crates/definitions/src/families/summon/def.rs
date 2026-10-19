//! **召唤物定义载体**（`SummonDef`）与**效果类型**（`SummonEffectKind`）。
//!
//! 本文件只有**定义数据**，不含召唤物区（`zones/`）的承载与结算：
//! - 召唤物区（上限 4 / 同名合并 / 可用次数递减与耗尽弃置）在 `zones/mod.rs`（D182 / D183 / D185 / D187）；
//! - 本文件的职责只有「一个召唤物长什么样」：卡 id、名字 / 描述、效果类型、效果量、默认可用次数、标签。
//!
//! ## 数据口径（只读游戏内文本描述）
//!
//! - `id` / `name` / `description`：逐字取数据（中文；D120，其它语言槽位 `None`，D56 / D68）；
//! - `effect_kind` + `effect_points`：数据`没有`结构化的「效果类型 / 效果量」字段，
//!   只能从 `description` 的句式反推（如「结束阶段：造成1点雷元素伤害。」-> 伤害 1 点；
//!   「治疗…」-> 治疗；「赋予…状态」-> 状态）—— 标 `待复核`；
//! - `default_uses`：数据没有结构化字段，从描述「可用次数：N」反推（D12：可用次数**按触发次数递减**；
//!   数据 `countingtype = GCG_TOKEN_LIFE` 只表示「这是可用次数类计量」，不是数值）；
//! - 标签：数据 `tags` 里出现的减伤 / 希穆兰卡 / 不可发现 / 草元素产物都有唯一定义（D55），
//!   集合形态本轮未定 -> 类型参数 `Tags`（没有标签用 `()`），同 `CharacterDef`。
//!
//! ## 与召唤物区的关系（`待接入`）
//!
//! 召出召唤物时，区域要的是 `SummonEntry`（实体 + 卡 id + 可用次数 + 效果量 + 效果类型）。
//! `SummonDef::entry()` 给这个「定义 -> 条目」的纯搬运：默认可用次数 / 效果量 / 效果类型
//! 都从定义来；同名合并的「次数与效果量取最大、效果类型取最新」仍在区域层（分工计划 §4.4.15）。
//!
//! `待接入`：`execute`（具体执行动作 `SummonAction` / `SummonMeasure`）等执行器块；
//! 本块按任务只给最小效果类型集合，不发明执行器。

use crate::base::CardId;
use crate::families::summon::zones::SummonEntry;
use crate::generated::Localized;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 召唤物的**效果类型**（最小集合：伤害 / 治疗 / 状态 / 其它）。
///
/// 区域层（`zones/mod.rs`）不解释这个枚举，只把它当「效果类型标识」参与同名合并
/// （取最新，分工计划 §4.4.15）；具体效果由召唤物定义 / 执行动作解释。
///
/// `待复核`：数据里没有效果类型字段，本枚举是**反推**的最小集合；
/// 元素伤害的「元素」不在这里（那是伤害类型，归角色族 / 伤害块），本枚举只分「做什么」。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SummonEffectKind {
    /// 造成伤害（效果量 = 伤害点数）。
    Damage,
    /// 治疗（效果量 = 治疗点数）。
    Heal,
    /// 赋予 / 附属状态（效果量 = 状态层数 / 可用次数，含义由状态定义解释）。
    Status,
    /// 其它（加伤 / 减伤 / 特殊效果等没有可计量点数的效果；效果量通常为 `Nat::ZERO`）。
    Other,
}

impl SummonEffectKind {
    /// 全部 4 个（只用于遍历；数组顺序不表示任何规则）。
    pub const ALL: [SummonEffectKind; 4] = [
        SummonEffectKind::Damage,
        SummonEffectKind::Heal,
        SummonEffectKind::Status,
        SummonEffectKind::Other,
    ];
}

/// 一个**召唤物**的定义（纯数据）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
#[derive(Clone, Copy, Debug)]
pub struct SummonDef<Tags> {
    /// 召唤物卡的卡 id（召唤物牌族 id 区域内唯一）。
    pub id: CardId,
    /// 4 语名字（本块只填中文，D120）。
    pub name: Localized,
    /// 4 语描述（本块只填中文，D120）。
    pub description: Localized,
    /// 效果类型（最小集合，见 [`SummonEffectKind`]）。
    pub effect_kind: SummonEffectKind,
    /// 效果量：伤害点数 / 治疗点数 / 状态层数等；`Nat::ZERO` = 无可计量效果量。
    pub effect_points: Nat,
    /// 默认可用次数：`None` = 没有可用次数（触发不递减，D12 / 分工计划「三项改可选」）。
    pub default_uses: Option<Nat>,
    /// 标签集合（D54 / D55）。
    pub tags: Tags,
}

impl<Tags> SummonDef<Tags> {
    /// 建一份召唤物定义（常量 / 测试用）。
    pub const fn new(
        id: CardId,
        name: Localized,
        description: Localized,
        effect_kind: SummonEffectKind,
        effect_points: Nat,
        default_uses: Option<Nat>,
        tags: Tags,
    ) -> SummonDef<Tags> {
        SummonDef {
            id,
            name,
            description,
            effect_kind,
            effect_points,
            default_uses,
            tags,
        }
    }

    /// 是否有可用次数（数据描述写了「可用次数：N」）。
    pub const fn has_uses(&self) -> bool {
        self.default_uses.is_some()
    }

    /// 是否有可计量效果量（`Nat::ZERO` = 没有）。
    pub const fn has_effect_points(&self) -> bool {
        self.effect_points.get() != 0
    }

    /// 用这份定义造召唤物区条目（实体 + 卡 id + 可用次数 + 效果量 + 效果类型）。
    ///
    /// 纯搬运：不改定义、不查任何表；同名合并口径在区域层（D12 / 分工计划 §4.4.15）。
    pub fn entry(&self, summon: EntityId) -> SummonEntry {
        SummonEntry::new(summon, self.id)
            .with_uses(self.default_uses)
            .with_points(Some(self.effect_points))
            .with_effect_kind(Some(self.effect_kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Lang, Resolved};

    /// 测试用召唤物定义（**不是真实召唤物数据**）。
    static SAMPLE: SummonDef<()> = SummonDef::new(
        CardId::new(900_101),
        Localized::from_parts(Some("测试召唤物"), None, None, None),
        Localized::from_parts(Some("结束阶段：造成2点雷元素伤害。\n可用次数：3"), None, None, None),
        SummonEffectKind::Damage,
        Nat::new(2),
        Some(Nat::new(3)),
        (),
    );

    /// 没有可用次数、没有效果量的定义。
    static BUFF_ONLY: SummonDef<()> = SummonDef::new(
        CardId::new(900_102),
        Localized::from_parts(Some("测试增益"), None, None, None),
        Localized::from_parts(Some("我方造成的伤害+1。"), None, None, None),
        SummonEffectKind::Other,
        Nat::ZERO,
        None,
        (),
    );

    #[test]
    fn summon_def_carries_id_text_kind_points_uses_and_tags() {
        assert_eq!(SAMPLE.id, CardId::new(900_101));
        assert_eq!(
            SAMPLE.name.get(Lang::ZhCn),
            Resolved::Exact("测试召唤物")
        );
        assert_eq!(
            SAMPLE.description.get(Lang::ZhCn),
            Resolved::Exact("结束阶段：造成2点雷元素伤害。\n可用次数：3")
        );
        assert_eq!(SAMPLE.effect_kind, SummonEffectKind::Damage);
        // 效果量是 Nat（§8.3：数值统一 Nat，不用裸整型）。
        assert_eq!(SAMPLE.effect_points, Nat::new(2));
        assert_eq!(SAMPLE.default_uses, Some(Nat::new(3)));
        assert_eq!(SAMPLE.tags, ());
        assert!(SAMPLE.has_uses());
        assert!(SAMPLE.has_effect_points());
        // 只有中文（D120）：其它语言槽位是 None（D56 显式缺失；get() 取用时会报告回退）。
        assert!(SAMPLE.name.slot(Lang::EnUs).is_none());
        assert!(!SAMPLE.name.has(Lang::EnUs));

        assert_eq!(BUFF_ONLY.effect_points, Nat::ZERO);
        assert_eq!(BUFF_ONLY.default_uses, None);
        assert!(!BUFF_ONLY.has_uses());
        assert!(!BUFF_ONLY.has_effect_points());
        assert_eq!(BUFF_ONLY.effect_kind, SummonEffectKind::Other);
    }

    #[test]
    fn effect_kind_is_a_formal_minimal_enum() {
        // 最小集合 = 伤害 / 治疗 / 状态 / 其它；四个值互不相同。
        assert_eq!(SummonEffectKind::ALL.len(), 4);
        let mut kinds = SummonEffectKind::ALL.to_vec();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.len(), SummonEffectKind::ALL.len());
        assert_ne!(SummonEffectKind::Damage, SummonEffectKind::Heal);
        assert_ne!(SummonEffectKind::Status, SummonEffectKind::Other);
    }

    #[test]
    fn entry_carries_the_def_measurements_into_the_zone_entry() {
        // 定义 -> 召唤物区条目的纯搬运：效果量 / 可用次数 / 效果类型都来自定义。
        let entry = SAMPLE.entry(Nat::new(7));
        assert_eq!(entry.summon, Nat::new(7));
        assert_eq!(entry.card, SAMPLE.id);
        assert_eq!(entry.uses, SAMPLE.default_uses);
        assert_eq!(entry.points, Some(SAMPLE.effect_points));
        assert_eq!(entry.effect_kind, Some(SAMPLE.effect_kind));
        // 没有可用次数的定义：条目也没有可用次数（区域层不递减，D12）。
        let buff = BUFF_ONLY.entry(Nat::new(8));
        assert_eq!(buff.uses, None);
        assert_eq!(buff.points, Some(Nat::ZERO));
        assert_eq!(buff.effect_kind, Some(SummonEffectKind::Other));
    }
}
