//! **三级统一基础操作层（L1 牌族 / L2 类型 / L3 标签）**。
//!
//! 用户口径：「把所有 families/types/tags（除自走牌）都按统一基础操作建好」
//! 「统一的基础操作指的是对于每个 tags 内统一的、基础的，操作」「就比如元素幻变，实现开局那一套」。
//!
//! **结构**：
//! - 本目录 = 统一层；三级**同一张表、同一形状**（RuleRow），不按级别各造一套签名；
//! - 每一行 = 一个牌族 / 类型 / 标签：kind（机制 / 仅分类）+ stages（在哪些阶段生效）
//!   + query_roles（被谁当筛选条件）+ evidence（依据卡 id + 一句话）+ basis（规格依据）
//!   + 可选的基础操作数据（槽位 / 上限 / 开局钩子 / 伤害角色 / 查询策略）；
//! - 具体**操作**是纯函数，集中在 rules/ops/（不持有状态、不新建容器，D20）；
//! - 「自走牌」按任务要求**排除**：表里没有行、也不实现；它的「不可发现」过滤仍由
//!   families/action_card/zones/query.rs 负责。
//!
//! **为什么是静态表而不是 61 个 trait impl**：行数据全静态、没有多态需求，静态表可遍历、
//! 可测试（每行都有依据 / 种类断言），也不需要为每行造样板。统一接口由 BaseRule trait
//! 提供（唯一实现 = impl BaseRule for RuleRow），三级共用同一组访问器。
//!
//! **行为依据优先级（用户口径）**：规则书（tcgdetailedrules）> 权威关键字 > 卡面文本；
//! 规则书与旧决策冲突时以规则书为准，改不了的标 `// 待接入`。**不读参考实现**（D122）。
//!
//! **数值口径**：规则数字一律 Nat；本层不做概率运算（随机只给「范围 / 集合」，
//! 抽样走引擎随机接口）；全仓不使用浮点，一律整数。

use crate::base::CardFamily;
use crate::families::action_card::ActionCardType;
use crate::families::status::StatusType;
use crate::generated::{Lang, Localized, TagText};
use gi_genius_invokation_engine::Nat;

pub mod families;
pub mod ops;
pub mod tags_action;
pub mod tags_character;
pub mod tags_summon_enemy;
pub mod types;

use ops::equip::SlotRule;
use ops::query::QueryPolicy;

// ======================================================================
// 统一接口：三级共用同一组枚举 / 行形状 / 访问器
// ======================================================================

/// 规则所属层级（L1 / L2 / L3，同一张表混装）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// L1：牌族（行动牌 / 角色 / 召唤物 / 状态 / 敌人）。
    Family,
    /// L2：类型（支援 / 装备 / 事件 / 角色状态 / 出战状态）。
    CardType,
    /// L3：标签（数据里真实出现的 52 个，本表实现 51 个，排除自走牌）。
    Tag,
}

/// 规则种类。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleKind {
    /// 有基础操作 / 规则（槽位、上限、开局、伤害、查询策略…）。
    Mechanism,
    /// **仅分类、无操作**：只是数据里的一个分类标签（可能被当查询条件）。
    Classification,
}

/// 规则在哪些阶段生效（同一行可多个）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// 开局（起手 / 首发 / 元素幻变的「行动阶段开始时」）。
    Opening,
    /// 投掷阶段（元素幻变：每轮固定 2+2）。
    RollPhase,
    /// 行动阶段（打出 / 装备 / 区域进出 / 料理等）。
    ActionPhase,
    /// 伤害结算（护盾吸收 / 减伤扣减）。
    Damage,
    /// 查询 / 检索（不可发现 / 弃用）。
    Query,
    /// 建局前（组牌门槛：元素共鸣 / 国家共鸣）。
    DeckBuilding,
}

/// 「作为筛选条件」的角色：本标签 / 类型 / 牌族被哪些机制查询。
///
/// 纯分类标签也会列出一到两个查询角色 —— 这就是它们**唯一的用途**，不代表有操作。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QueryRole {
    /// 角色元素（用于元素共鸣组牌、元素幻变队伍条件）。
    TeamElement,
    /// 元素共鸣：牌组至少 2 名同元素角色。
    ResonanceElement,
    /// 武器类型门（「『X』角色才能装备」）。
    WeaponTypeGate,
    /// 牌种条件（「从 3 张『场地』/『伙伴』/『道具』中挑选 1 张」类效果）。
    CardKindGate,
    /// 势力组牌门槛（「牌组包含至少 2 个『愚人众』/『魔物』角色」）。
    CampDeckGate,
    /// 元素幻变队伍条件（恰含两种元素）。
    TransmutationCondition,
    /// 敌人分类（技能 / 效果按此筛选）。
    EnemyClass,
    /// 召唤物分类（效果按此筛选）。
    SummonClass,
    /// 始基力极性（荒 / 芒，荒芒湮灭效应）。
    Arkhe,
}

/// 本行用到的**基础操作**（与 rules/ops/ 的模块一一对应；表是数据，函数按需读表）。
///
/// 加这个字段是为了让「规则行 -> 纯函数模块」的对应关系写在数据里，
/// 消费者可以按操作反查（如找所有参与伤害结算的行），而不是在代码里散落 if 特例。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleOp {
    /// 开局流程（ops::opening；元素幻变 / 起手 / 首发）。
    Opening,
    /// 装备槽与互斥 / 替换（ops::equip）。
    Equip,
    /// 支援区上限（ops::support）。
    Support,
    /// 料理与饱腹（ops::food）。
    Food,
    /// 秘传与秘传揭令（ops::arcane）。
    Arcane,
    /// 行动权：战斗行动 / 快速行动（ops::stage）。
    Timing,
    /// 护盾 / 减伤（ops::damage）。
    Damage,
    /// 查询策略：不可发现 / 弃用（ops::query）。
    Query,
    /// 元素 / 国家共鸣组牌门槛（ops::resonance）。
    Resonance,
    /// 冒险动作（ops::adventure）。
    Adventure,
    /// 希穆兰卡集合 / 随机生成范围（ops::simulanka）。
    Simulanka,
    /// 始基力荒 / 芒对立（ops::arkhe）。
    Arkhe,
    /// 可用次数 / 持续回合（ops::lifetime；规则书·特殊词条）。
    Lifetime,
    /// 结算顺序（ops::order；规则书·其他规则·结算顺序）。
    Order,
}

/// 一条卡牌依据：原始卡 id + 一句话卡文（中文原文 / 摘要）。
///
/// id 只用于审计与展示，**不参与游戏运算**（跨牌族同 id 不保证唯一，这里只是数据来源编号）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Evidence {
    /// 原始卡 id。
    pub card: Nat,
    /// 一句话依据（卡面原文或括号规则）。
    pub note: &'static str,
}

impl Evidence {
    /// 构造一条依据。
    ///
    /// 参数用 u32 只为写**常量字面量**（与 base::CardId::new 同一口径）；行里存的是 Nat。
    pub const fn new(card: u32, note: &'static str) -> Evidence { // bare-int-ok: 常量构造的字面量入口（同 CardId::new，存储仍是 Nat）
        Evidence {
            card: Nat::new(card),
            note,
        }
    }
}

/// 规则数字（一律 Nat；如支援区上限 4、料理每回合 1 次、共鸣至少 2 名）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NumRule {
    /// 这个数字是什么。
    pub what: &'static str,
    /// 数值。
    pub value: Nat,
}

impl NumRule {
    /// 构造一条规则数字。
    ///
    /// 参数用 u32 只为写**常量字面量**；行里存的是 Nat。
    pub const fn new(what: &'static str, value: u32) -> NumRule { // bare-int-ok: 常量构造的字面量入口（存储仍是 Nat）
        NumRule {
            what,
            value: Nat::new(value),
        }
    }
}

/// 开局钩子（三级共用；具体流程在 ops::opening）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpeningSpec {
    /// 行动牌族：起手 5 张（先按「秘传」标签抓，再补足；D17 / D90）。
    OpeningHand,
    /// 角色族：开局选首发角色（接口先留，等角色块实现）。
    FirstCharacterChoice,
    /// 秘传（事件 / 标签）：起手必带（14 张卡面「一定在你的起始手牌中」）。
    ArcaneLegendOpeningHand,
    /// 元素幻变（支援 / 标签）：行动阶段开始时抓牌 -> 弃置 -> 生成支援区 -> 4 骰转 2+2。
    ElementalTransmutation,
}

/// 伤害结算里的角色（护盾吸收 / 减伤扣减；穿透两者都不吃，见 ops::damage）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DamageRole {
    /// 护盾：先吸收伤害。
    Shield,
    /// 减伤：护盾之后再扣减。
    Reduction,
}

/// 标签数据访问器（D67：运行期身份 = &TAGS.<字段> 的地址）。
pub type TagData = fn() -> &'static TagText;

/// 一行的名字：**统一从既有数据源取，不在规则表里另存一份**（D58：数据只有一份）。
#[derive(Clone, Copy, Debug)]
pub enum RuleName {
    /// 标签 / 类型：generated 里那份 4 语文本的引用（零拷贝；D58 / D64）。
    Text(&'static Localized),
    /// 牌族：generated 没有牌族的 4 语文本，走 [`family_name`] 这一处唯一映射。
    Family(&'static str),
}

impl RuleName {
    /// 中文名（日志 / 测试用；标签 / 类型缺中文时按 Localized 的回退规则取）。
    pub fn zh(self) -> &'static str {
        match self {
            RuleName::Text(text) => text.get(Lang::ZhCn).text().unwrap_or_default(),
            RuleName::Family(name) => name,
        }
    }
}

/// 牌族的显示名：**全仓唯一一处映射**（generated 里没有牌族的 4 语文本）。
pub const fn family_name(family: CardFamily) -> &'static str {
    match family {
        CardFamily::ActionCard => "行动牌",
        CardFamily::Character => "角色",
        CardFamily::Summon => "召唤物",
        CardFamily::Status => "状态",
        CardFamily::Enemy => "敌人",
    }
}

/// **统一规则行**：三级共用同一形状。
///
/// 用 ..RuleRow::EMPTY 只写本行用到的字段；表在 families / types / tags_* 五个模块里。
#[derive(Clone, Copy, Debug)]
pub struct RuleRow {
    /// 所属层级。
    pub layer: Layer,
    /// 机制 / 仅分类。
    pub kind: RuleKind,
    /// 本行用到的基础操作（与 rules/ops/ 模块一一对应）。
    pub ops: &'static [RuleOp],
    /// 生效阶段。
    pub stages: &'static [Stage],
    /// 作为筛选条件被谁引用。
    pub query_roles: &'static [QueryRole],
    /// 依据卡（id + 一句话）。
    pub evidence: &'static [Evidence],
    /// 规格 / 决策依据（分工计划 / 决策日志 / 关键字条目的文字依据）。
    pub basis: &'static [&'static str],
    /// 仅标签行：标签静态数据的访问器（指针身份，D54 / D57）。
    pub tag: Option<TagData>,
    /// 仅牌族行：牌族枚举。
    pub family: Option<CardFamily>,
    /// 仅行动牌类型行：类型枚举。
    pub action_card_type: Option<ActionCardType>,
    /// 仅状态类型行：状态类型枚举。
    pub status_type: Option<StatusType>,
    /// 槽位规则（装备类）。
    pub slots: &'static [SlotRule],
    /// 规则数字（上限 / 次数 / 门槛）。
    pub nums: &'static [NumRule],
    /// 开局钩子。
    pub opening: Option<OpeningSpec>,
    /// 支援区上限（支援类型 / 支援类标签）。
    pub support_cap: Option<Nat>,
    /// 伤害结算角色（护盾 / 减伤）。
    pub damage: Option<DamageRole>,
    /// 查询策略（不可发现 / 弃用）。
    pub query_policy: Option<QueryPolicy>,
}

impl RuleRow {
    /// 全空行（表里用 ..RuleRow::EMPTY 补默认值）。
    pub const EMPTY: RuleRow = RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        ops: &[],
        stages: &[],
        query_roles: &[],
        evidence: &[],
        basis: &[],
        tag: None,
        family: None,
        action_card_type: None,
        status_type: None,
        slots: &[],
        nums: &[],
        opening: None,
        support_cap: None,
        damage: None,
        query_policy: None,
    };
}

/// 统一访问器：三级共用这一组签名（唯一实现 = RuleRow）。
///
/// 消费者（区域块 / 测试 / 卡牌）只依赖这个 trait，不直接依赖行结构。
pub trait BaseRule {
    /// 所属层级。
    fn layer(&self) -> Layer;
    /// 行名：统一从既有数据源取（标签 / 类型取自 generated，牌族走唯一映射）。
    fn name(&self) -> RuleName;

    /// 行名的中文（日志 / 测试便利方法）。
    fn name_zh(&self) -> &'static str {
        self.name().zh()
    }
    /// 机制 / 仅分类。
    fn kind(&self) -> RuleKind;
    /// 本行用到的基础操作。
    fn ops(&self) -> &'static [RuleOp];
    /// 生效阶段。
    fn stages(&self) -> &'static [Stage];
    /// 作为筛选条件被谁引用。
    fn query_roles(&self) -> &'static [QueryRole];
    /// 依据卡。
    fn evidence(&self) -> &'static [Evidence];
    /// 规格 / 决策依据。
    fn basis(&self) -> &'static [&'static str];
    /// 标签静态数据（非标签行 -> None）。
    fn tag(&self) -> Option<&'static TagText>;
    /// 槽位规则。
    fn slots(&self) -> &'static [SlotRule];
    /// 规则数字。
    fn nums(&self) -> &'static [NumRule];
    /// 开局钩子。
    fn opening(&self) -> Option<OpeningSpec>;
    /// 支援区上限。
    fn support_cap(&self) -> Option<Nat>;
    /// 伤害结算角色。
    fn damage(&self) -> Option<DamageRole>;
    /// 查询策略。
    fn query_policy(&self) -> Option<QueryPolicy>;
    /// 是不是「仅分类、无操作」。
    fn is_classification_only(&self) -> bool {
        self.kind() == RuleKind::Classification
    }
}

impl BaseRule for RuleRow {
    fn layer(&self) -> Layer {
        self.layer
    }

    fn name(&self) -> RuleName {
        if let Some(tag) = self.tag {
            return RuleName::Text(&tag().name);
        }
        if let Some(action_card_type) = self.action_card_type {
            if let Some(entry) = crate::generated::action_card_type_text(action_card_type) {
                return RuleName::Text(&entry.name);
            }
        }
        if let Some(status_type) = self.status_type {
            if let Some(entry) = crate::generated::status_type_text(status_type) {
                return RuleName::Text(&entry.name);
            }
        }
        if let Some(family) = self.family {
            return RuleName::Family(family_name(family));
        }
        // 只有 RuleRow::EMPTY 会走到这里（空行不该被查询到）。
        RuleName::Family("")
    }

    fn kind(&self) -> RuleKind {
        self.kind
    }

    fn ops(&self) -> &'static [RuleOp] {
        self.ops
    }

    fn stages(&self) -> &'static [Stage] {
        self.stages
    }

    fn query_roles(&self) -> &'static [QueryRole] {
        self.query_roles
    }

    fn evidence(&self) -> &'static [Evidence] {
        self.evidence
    }

    fn basis(&self) -> &'static [&'static str] {
        self.basis
    }

    fn tag(&self) -> Option<&'static TagText> {
        self.tag.map(|data| data())
    }

    fn slots(&self) -> &'static [SlotRule] {
        self.slots
    }

    fn nums(&self) -> &'static [NumRule] {
        self.nums
    }

    fn opening(&self) -> Option<OpeningSpec> {
        self.opening
    }

    fn support_cap(&self) -> Option<Nat> {
        self.support_cap
    }

    fn damage(&self) -> Option<DamageRole> {
        self.damage
    }

    fn query_policy(&self) -> Option<QueryPolicy> {
        self.query_policy
    }
}

// ======================================================================
// 全表（三级混装）与查询
// ======================================================================

/// 遍历全表（顺序：牌族 -> 类型 -> 标签）。
pub fn rows() -> impl Iterator<Item = &'static RuleRow> {
    families::ROWS
        .iter()
        .chain(types::ROWS.iter())
        .chain(tags_action::ROWS.iter())
        .chain(tags_character::ROWS.iter())
        .chain(tags_summon_enemy::ROWS.iter())
}

/// 全表（Vec，测试 / 一次性遍历用）。
pub fn all_rows() -> Vec<&'static RuleRow> {
    rows().collect()
}

/// 按标签指针查规则行（D57：按指针身份，不比名字、不用编号）。
pub fn row_of_tag(tag: &'static TagText) -> Option<&'static RuleRow> {
    rows().find(|row| row.tag.is_some_and(|data| std::ptr::addr_eq(data(), tag)))
}

/// 按牌族查规则行。
pub fn row_of_family(family: CardFamily) -> Option<&'static RuleRow> {
    rows().find(|row| row.family == Some(family))
}

/// 按行动牌类型查规则行。
pub fn row_of_action_card_type(action_card_type: ActionCardType) -> Option<&'static RuleRow> {
    rows().find(|row| row.action_card_type == Some(action_card_type))
}

/// 按状态类型查规则行。
pub fn row_of_status_type(status_type: StatusType) -> Option<&'static RuleRow> {
    rows().find(|row| row.status_type == Some(status_type))
}

// ======================================================================
// 测试：全表一致性 + 每行「机制 / 仅分类」断言
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::zones::query::{is_discoverable, CardMeta};
    use crate::families::action_card::ActionCardType;
    use crate::tags::action_card::common::automaton_card;
    use gi_genius_invokation_engine::Nat;

    /// **逐个标签**的机制 / 仅分类口径（显式清单：标签增删或改口径时这里必须同步）。
    ///
    /// 机制标签 22 个；仅分类 29 个（52 个标签排除自走牌 = 51）。
    #[test]
    fn every_tag_kind_is_pinned_person_by_person() {
        const MECHANISM_TAGS: [&str; 22] = [
            "元素幻变",
            "冒险地点",
            "武器",
            "单手剑",
            "弓",
            "双手剑",
            "法器",
            "长柄武器",
            "圣遗物",
            "天赋",
            "特技",
            "料理",
            "秘传",
            "元素共鸣",
            "希穆兰卡",
            "战斗行动",
            "护盾",
            "减伤",
            "不可发现",
            "弃用",
            "始基力：荒性",
            "始基力：芒性",
        ];
        const CLASSIFICATION_TAGS: [&str; 29] = [
            "场地",
            "伙伴",
            "道具",
            "未变身",
            "魔导",
            "火元素",
            "水元素",
            "雷元素",
            "草元素",
            "冰元素",
            "风元素",
            "岩元素",
            "蒙德",
            "璃月",
            "稻妻",
            "须弥",
            "枫丹",
            "纳塔",
            "挪德卡莱",
            "愚人众",
            "魔物",
            "丘丘人",
            "镀金旅团",
            "海乱鬼",
            "圣骸兽",
            "寰宇劫灭",
            "其他武器",
            "草元素产物",
            "无元素类型",
        ];

        let tag_rows: Vec<&'static RuleRow> =
            rows().filter(|row| row.layer() == Layer::Tag).collect();
        assert_eq!(tag_rows.len(), 51, "标签行 51 个（排除自走牌）");
        assert_eq!(
            tag_rows
                .iter()
                .filter(|row| row.kind() == RuleKind::Mechanism)
                .count(),
            MECHANISM_TAGS.len(),
            "机制标签个数"
        );

        for name in MECHANISM_TAGS {
            let matched: Vec<&&'static RuleRow> = tag_rows
                .iter()
                .filter(|row| row.name_zh() == name)
                .collect();
            assert_eq!(matched.len(), 1, "机制标签应恰好一行：{name}");
            assert_eq!(
                matched[0].kind(),
                RuleKind::Mechanism,
                "应为机制标签：{name}"
            );
            assert!(!matched[0].ops().is_empty(), "机制标签必须声明基础操作：{name}");
        }

        for name in CLASSIFICATION_TAGS {
            let matched: Vec<&&'static RuleRow> = tag_rows
                .iter()
                .filter(|row| row.name_zh() == name)
                .collect();
            assert_eq!(matched.len(), 1, "仅分类标签应恰好一行：{name}");
            assert_eq!(
                matched[0].kind(),
                RuleKind::Classification,
                "应为仅分类标签：{name}"
            );
            // 「仅分类」= 不提供任何基础操作，只作为查询条件。
            assert!(matched[0].ops().is_empty(), "仅分类标签不该有基础操作：{name}");
            assert!(
                !matched[0].query_roles().is_empty(),
                "仅分类标签该说明被谁当筛选条件：{name}"
            );
        }
    }
    /// **名字不在表里另存一份**：标签 / 类型名字必须逐字等于 generated 的数据；牌族名字只来自
    /// 唯一映射 [`family_name`]。这条测试保证 D58「数据只有一份」不被悄悄破坏。
    #[test]
    fn names_come_from_generated_data_or_single_mapping() {
        for row in rows() {
            if let Some(tag) = row.tag() {
                let expected = tag
                    .name
                    .get(Lang::ZhCn)
                    .text()
                    .expect("标签数据必须有中文名");
                assert_eq!(row.name_zh(), expected, "标签名必须等于 TAGS 数据里的名字");
            }
            if let Some(action_card_type) = row.action_card_type {
                let entry = crate::generated::action_card_type_text(action_card_type)
                    .expect("行动牌类型文本必须存在");
                assert_eq!(
                    row.name_zh(),
                    entry.name.get(Lang::ZhCn).text().expect("类型有中文名")
                );
            }
            if let Some(status_type) = row.status_type {
                let entry = crate::generated::status_type_text(status_type)
                    .expect("状态类型文本必须存在");
                assert_eq!(
                    row.name_zh(),
                    entry.name.get(Lang::ZhCn).text().expect("状态类型有中文名")
                );
            }
            if let Some(family) = row.family {
                assert_eq!(row.name_zh(), family_name(family));
            }
        }
        // 唯一映射覆盖 5 个牌族，且互不相同。
        let names: Vec<&'static str> = [
            CardFamily::ActionCard,
            CardFamily::Character,
            CardFamily::Summon,
            CardFamily::Status,
            CardFamily::Enemy,
        ]
        .iter()
        .map(|family| family_name(*family))
        .collect();
        assert!(names.iter().all(|name| !name.is_empty()));
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "牌族名字必须互不相同");
    }
    /// 三级行数：牌族 5 + 类型 5 + 标签 51（52 个标签排除自走牌）= 61。
    #[test]
    fn table_covers_all_three_layers() {
        assert_eq!(families::ROWS.len(), 5, "牌族 5 个");
        assert_eq!(types::ROWS.len(), 5, "类型 5 个（行动牌 3 + 状态 2）");
        assert_eq!(tags_action::ROWS.len(), 25, "行动牌标签 25 个");
        assert_eq!(tags_character::ROWS.len(), 24, "角色标签 24 个");
        assert_eq!(tags_summon_enemy::ROWS.len(), 2, "召唤物 / 敌人标签 2 个");
        assert_eq!(all_rows().len(), 61, "三级合计 61 行");

        let family_rows = rows().filter(|row| row.layer() == Layer::Family).count();
        let type_rows = rows().filter(|row| row.layer() == Layer::CardType).count();
        let tag_rows = rows().filter(|row| row.layer() == Layer::Tag).count();
        assert_eq!((family_rows, type_rows, tag_rows), (5, 5, 51));
    }

    /// 每个标签行都能按指针查回自己，且 51 个标签恰好覆盖数据里的 52 个标签（排除自走牌）。
    #[test]
    fn tag_rows_cover_every_tag_except_automaton() {
        let all_tags = crate::generated::all_tag_data();
        let expected: Vec<&'static TagText> = all_tags
            .iter()
            .copied()
            .filter(|tag| !std::ptr::addr_eq(*tag, automaton_card::data()))
            .collect();
        assert_eq!(expected.len(), 51, "52 个标签排除「自走牌」= 51");

        let actual: Vec<&'static TagText> = rows().filter_map(|row| row.tag()).collect();
        assert_eq!(actual.len(), 51, "标签行数 = 51");
        for tag in &expected {
            assert!(
                actual.iter().any(|candidate| std::ptr::addr_eq(*candidate, *tag)),
                "标签表缺了数据里存在的标签（指针比对失败）"
            );
            assert!(row_of_tag(tag).is_some());
        }
        // 「自走牌」不进表（任务排除项）
        assert!(row_of_tag(automaton_card::data()).is_none());
        // 没有重复标签行
        for (index, tag) in actual.iter().enumerate() {
            for other in actual.iter().skip(index.saturating_add(1)) {
                assert!(!std::ptr::addr_eq(*tag, *other), "同一个标签出现了两行");
            }
        }
    }

    /// 每行都必须有可审计的依据：至少一条依据卡，或至少一条规格依据。
    #[test]
    fn every_row_has_evidence_or_basis() {
        for row in rows() {
            assert!(
                !row.evidence().is_empty() || !row.basis().is_empty(),
                "规则行没有依据：{}",
                row.name_zh()
            );
            assert!(!row.name_zh().is_empty(), "规则行缺名字");
        }
    }

    /// 「仅分类」的落锤测试：仅分类行不得带任何机制数据。
    #[test]
    fn classification_rows_have_no_operations() {
        for row in rows().filter(|row| row.is_classification_only()) {
            assert!(row.ops().is_empty(), "仅分类标签不该有基础操作：{}", row.name_zh());
            assert!(row.slots().is_empty(), "仅分类标签不该有槽位：{}", row.name_zh());
            assert!(row.nums().is_empty(), "仅分类标签不该有规则数字：{}", row.name_zh());
            assert_eq!(row.opening(), None, "仅分类标签不该进开局：{}", row.name_zh());
            assert_eq!(row.support_cap(), None, "仅分类标签不该有支援上限：{}", row.name_zh());
            assert_eq!(row.damage(), None, "仅分类标签不该参与伤害结算：{}", row.name_zh());
            assert_eq!(row.query_policy(), None, "仅分类标签不该有查询策略：{}", row.name_zh());
            assert!(
                !row.query_roles().is_empty(),
                "仅分类标签该说明被谁当筛选条件：{}",
                row.name_zh()
            );
        }
    }

    /// 机制行：必须有生效阶段，且至少有一项操作数据。
    #[test]
    fn mechanism_rows_declare_operations() {
        for row in rows().filter(|row| row.kind() == RuleKind::Mechanism) {
            assert!(!row.stages().is_empty(), "机制行必须声明生效阶段：{}", row.name_zh());
            let has_data = !row.ops().is_empty()
                || !row.slots().is_empty()
                || !row.nums().is_empty()
                || row.opening().is_some()
                || row.support_cap().is_some()
                || row.damage().is_some()
                || row.query_policy().is_some()
                || !row.query_roles().is_empty();
            assert!(has_data, "机制行没有任何操作数据：{}", row.name_zh());
        }
    }

    /// 每个基础操作枚举都至少被一行声明（表与 ops 模块一一对应，避免「建了模块没人用」）。
    #[test]
    fn every_rule_op_is_declared_by_some_row() {
        let all = [
            RuleOp::Opening,
            RuleOp::Equip,
            RuleOp::Support,
            RuleOp::Food,
            RuleOp::Arcane,
            RuleOp::Timing,
            RuleOp::Damage,
            RuleOp::Query,
            RuleOp::Resonance,
            RuleOp::Adventure,
            RuleOp::Simulanka,
            RuleOp::Arkhe,
            RuleOp::Lifetime,
            RuleOp::Order,
        ];
        for op in all {
            assert!(
                rows().any(|row| row.ops().contains(&op)),
                "没有任何规则行声明 {op:?}"
            );
        }
    }

    /// 三级查找：牌族 5 / 行动牌类型 3 / 状态类型 2 都能查到。
    #[test]
    fn lookups_work_for_families_and_types() {
        for family in [
            CardFamily::ActionCard,
            CardFamily::Character,
            CardFamily::Summon,
            CardFamily::Status,
            CardFamily::Enemy,
        ] {
            assert!(row_of_family(family).is_some(), "缺牌族行：{family:?}");
        }
        for action_card_type in ActionCardType::ALL {
            assert!(
                row_of_action_card_type(action_card_type).is_some(),
                "缺行动牌类型行：{action_card_type:?}"
            );
        }
        for status_type in StatusType::ALL {
            assert!(
                row_of_status_type(status_type).is_some(),
                "缺状态类型行：{status_type:?}"
            );
        }
        for row in rows().filter(|row| row.layer() == Layer::Family) {
            assert!(row.family.is_some(), "牌族行必须带牌族枚举：{}", row.name_zh());
            assert_eq!(row.action_card_type, None);
            assert_eq!(row.status_type, None);
        }
        for row in rows().filter(|row| row.layer() == Layer::CardType) {
            assert!(
                row.action_card_type.is_some() || row.status_type.is_some(),
                "类型行必须带类型枚举：{}",
                row.name_zh()
            );
            assert!(row.family.is_none());
        }
    }

    /// 数字语义：上限 / 次数 / 门槛都必须 > 0，避免 0 被当「不限」误用。
    #[test]
    fn numeric_rules_are_positive() {
        for row in rows() {
            for rule in row.nums() {
                assert!(
                    rule.value > Nat::ZERO,
                    "规则数字不应为 0：{} / {}",
                    row.name_zh(),
                    rule.what
                );
            }
            if let Some(cap) = row.support_cap() {
                assert!(cap > Nat::ZERO);
            }
        }
    }

    /// 查询策略与既有实现（query.rs）同口径。
    #[test]
    fn query_policy_rows_match_query_module() {
        let undiscoverable = row_of_tag(crate::tags::action_card::common::undiscoverable::data())
            .expect("不可发现标签必须在表里");
        assert_eq!(
            undiscoverable.query_policy(),
            Some(ops::query::UNDISCOVERABLE)
        );
        let scrapped = row_of_tag(crate::tags::action_card::common::scrapped::data())
            .expect("弃用标签必须在表里");
        assert_eq!(scrapped.query_policy(), Some(ops::query::SCRAPPED));
        for policy in [ops::query::UNDISCOVERABLE, ops::query::SCRAPPED] {
            assert!(!policy.discoverable);
            assert!(policy.explicitly_queryable);
        }

        struct Meta(&'static [&'static TagText]);
        impl CardMeta for Meta {
            fn ty(&self, _card: crate::base::CardId) -> Option<ActionCardType> {
                None
            }
            fn tags(&self, _card: crate::base::CardId) -> &'static [&'static TagText] {
                self.0
            }
        }
        let tags: &'static [&'static TagText] =
            Box::leak(Box::new([crate::tags::action_card::common::undiscoverable::data()]));
        let meta = Meta(tags);
        assert!(!is_discoverable(&meta, crate::base::CardId::new(331004)));
    }

    /// 开局钩子只挂在「该进开局」的行上。
    #[test]
    fn opening_hooks_are_attached_to_expected_rows() {
        let with_opening: Vec<&'static str> = rows()
            .filter(|row| row.opening().is_some())
            .map(|row| row.name_zh())
            .collect();
        for expected in ["行动牌", "角色", "支援牌", "事件牌", "元素幻变", "秘传"] {
            assert!(with_opening.contains(&expected), "缺开局钩子：{expected}");
        }
        for name in ["装备牌", "料理", "元素共鸣", "武器", "圣遗物"] {
            let row = rows().find(|row| row.name_zh() == name).expect("行存在");
            assert_eq!(row.opening(), None, "{name} 不该进开局流程");
        }
    }
}