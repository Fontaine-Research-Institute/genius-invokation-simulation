//! 爱诺（角色牌 1216）——「第一张卡」样板。
//!
//! **数据口径**：逐字取游戏内文本描述（本地中文数据，已核对）。
//! - id `1216`；生命上限 `hp = 10`；充能上限 `maxenergy = 2`（数据里只有 `maxenergy`，没有 `energy` 字段）；
//! - 标签：水元素 / 双手剑 / 挪德卡莱；
//! - 4 个技能：12161 敲打修理法（普攻）、12162 妙思捕手（战技）、
//!   12163 精密水冷仪（爆发）、12164 模块式高效运作（被动）。
//!
//! **相关但本轮不实现**（对应定义块还没建，只在注释里列出）：
//! - 召唤物 `112161 冷静一下鸭`：结束阶段造成 2 点水元素伤害，可用次数 2
//!   （游戏内文本描述）；
//! - 天赋牌 `212161 天才之为构造之责任`：装备牌 / 战斗行动，费用 3 水 + 2 充能
//!   （游戏内文本描述）；
//! - 状态「敏捷切换」（12162 生成 1 层）；
//! - 关键词「赋能」（K3006，12162 条件 / 12164 触发）。

// 卡牌「资源与形状」（id / 牌族 / 类型 / 标签 / 定义载体 / 费用组件）在 definitions（D241），
// 本 crate 只放这张卡的具体数据；导入路径因此都走 gi-genius-invokation-definitions。
use gi_genius_invokation_definitions::base::{CardId, Element, SkillId};
use gi_genius_invokation_definitions::families::character::definition::CharacterDef;
use gi_genius_invokation_definitions::families::character::skill::{SkillCost, SkillDef};
use gi_genius_invokation_definitions::families::character::types::SkillKind;
use gi_genius_invokation_definitions::generated::Localized;
use gi_genius_invokation_definitions::resources::dice::CostComponent;
use gi_genius_invokation_definitions::tags::action_card::equipment::claymore::Claymore;
use gi_genius_invokation_definitions::tags::character::element::hydro::Hydro;
use gi_genius_invokation_definitions::tags::character::nation::nodkrai::NodKrai;
use gi_genius_invokation_engine::Nat;

/// 爱诺的卡牌 id（角色牌族）。
pub const ID: CardId = CardId::new(1216);

// ======================================================================
// 游戏版本同步（D244 / D245 / D246）：本卡已同步（核对）到的最新游戏版本
// ======================================================================
//
// 口径（用户原话）：「不写最新版本号，只写同步了的最新版本」。
// - 这个值 = 本卡**已同步（核对）**到的最新游戏版本（现在是 6.2）；
// - 「已同步到最新版本」的唯一表示 = 把宏里的数字改成最新版本号；
// - **新版本没调整这张卡时，只改这里的数字**（6, 2 -> 6, 3），卡牌内容不动、同样算同步；
// - 宏是**通用定义**，留在 gi-genius-invokation-definitions（D242），本 crate 直接调用。
gi_genius_invokation_definitions::card_game_version!(6, 2);

// ======================================================================
// 编译期源码自声明（D214 / D215 / D216）：本文件源码归属角色牌 1216
// ======================================================================
//
// - 宏是**通用定义**，留在 gi-genius-invokation-definitions（D242）；本 crate 直接调用它；
// - 路径是分段列表 ["character", "aino"]（实际代表 character/aino，D216）；
// - include_str! 以**本 crate** 的 CARGO_MANIFEST_DIR 为根 → crates/cards/src/cards/character/aino/mod.rs，
//   但 path 字段仍是 "src/cards/character/aino/mod.rs"（结构不变）；
// - 不写 id：宏自动绑定本模块上面的 pub const ID: CardId（D215）；
// - 宏展开出 pub static SOURCE：原文 + 编译期解析的 use 依赖。
gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);

/// 生命上限（数据 `hp`）。
pub const MAX_HP: Nat = Nat::new(10);

/// 充能上限（数据 `maxenergy`）。
pub const MAX_ENERGY: Nat = Nat::new(2);

/// 标签集合（**类型级零大小元组**）：水元素 / 双手剑 / 挪德卡莱。
///
/// 标签就是类型本身（D54），所以这个元组零大小、无分配。
pub type Tags = (Hydro, Claymore, NodKrai);

/// 标签集合的常量值（三个零大小标签；运行期不存在）。
pub const TAGS: Tags = (Hydro, Claymore, NodKrai);

// ======================================================================
// 技能 id：角色 id × 10 + 序号（见 base::id 的派生口径）
// ======================================================================

/// 12161 敲打修理法（普通攻击）。
pub const SKILL_ID_NORMAL_ATTACK: SkillId = SkillId::new(12161);
/// 12162 妙思捕手（元素战技）。
pub const SKILL_ID_ELEMENTAL_SKILL: SkillId = SkillId::new(12162);
/// 12163 精密水冷仪（元素爆发）。
pub const SKILL_ID_ELEMENTAL_BURST: SkillId = SkillId::new(12163);
/// 12164 模块式高效运作（被动技能）。
pub const SKILL_ID_PASSIVE: SkillId = SkillId::new(12164);

/// 只填中文的本地化文本（D120）。
///
/// 现成接口只有 `Localized::from_parts`（**没有** `from_static_zh`），
/// 所以样板用 `from_parts(Some(中文), None, None, None)` 表示「只有中文」。
const fn zh_only(text: &'static str) -> Localized {
    Localized::from_parts(Some(text), None, None, None)
}

// ======================================================================
// 效果体（全部 TODO：每一处都写明缺哪个接口，P0/P1 清单落点）
// ======================================================================

/// 12161 敲打修理法：造成 2 点物理伤害。
fn normal_attack_effect() {
    // 效果：造成 2 点物理伤害。
    // 缺：伤害结算入口 —— `DamageType::Physical` 只有类型（families/character/types.rs），
    //     `families/character/damage.rs` 的「造成伤害」入口尚未建立。
    todo!("缺：伤害结算入口（damage.rs 未建）")
}

/// 12162 妙思捕手：造成 2 点水元素伤害，生成 1 层敏捷切换；
/// 若手牌中有卡牌具有赋能，则改为造成 3 点水元素伤害。
fn elemental_skill_effect() {
    // 效果：造成 2 点水元素伤害，生成 1 层「敏捷切换」；
    //       如果手牌中有卡牌具有「赋能」，则改为造成 3 点水元素伤害。
    // 缺：伤害结算入口（damage.rs 未建）。
    // 缺：状态定义与增删接口（「敏捷切换」状态定义未建）。
    // 缺：手牌查询 + 关键词「赋能」（K3006）判定（手牌区 / 关键词查询未建）。
    todo!("缺：伤害结算入口 + 状态生成 + 手牌赋能判定")
}

/// 12163 精密水冷仪：造成 2 点水元素伤害，召唤冷静一下鸭。
fn elemental_burst_effect() {
    // 效果：造成 2 点水元素伤害，召唤「冷静一下鸭」。
    // 缺：伤害结算入口（damage.rs 未建）。
    // 缺：召唤物定义与召唤物区（冷静一下鸭 112161：SummonDef / zones/summon.rs 未建）。
    todo!("缺：伤害结算入口 + 召唤物区")
}

/// 12164 模块式高效运作（被动）。
fn passive_effect() {
    // 效果：【被动】我方卡牌被赋予「赋能」时：如果我方场上存在「冷静一下鸭」，
    //       则使其可用次数 +1，否则自身获得 1 点充能。（每回合 1 次）
    // 缺：被动 / 触发注册机制（"被赋予赋能时"的钩子未建）。
    // 缺：召唤物区查询（我方场上是否存在冷静一下鸭）。
    // 缺：回合时间线 + 每回合 1 次计数。
    // 注：自身获得 1 点充能这一步**底层已有**
    //     `CharacterState::energy_add`（families/character/character.rs），只差接线。
    todo!("缺：被动触发注册 + 召唤物区查询 + 每回合 1 次计数")
}

// ======================================================================
// 4 个技能定义
// ======================================================================

/// 技能 12161 敲打修理法（普通攻击）。
///
/// - 文案：造成 2 点物理伤害。
/// - 消耗：1 水元素骰 + 2 无色骰。
pub static SKILL_12161: SkillDef = SkillDef {
    id: SKILL_ID_NORMAL_ATTACK,
    name: zh_only("敲打修理法"),
    description: zh_only("造成2点物理伤害。"),
    kind: SkillKind::normal_attack(),
    cost: SkillCost::from_dice(&[
        CostComponent::SpecificElement(Element::Hydro, Nat::new(1)),
        CostComponent::AnyColor(Nat::new(2)),
    ]),
    effect: normal_attack_effect,
};

/// 技能 12162 妙思捕手（元素战技）。
///
/// - 文案：造成 2 点水元素伤害，生成 1 层敏捷切换。
///   如果手牌中有卡牌具有赋能，则改为造成 3 点水元素伤害。
/// - 消耗：3 水元素骰。
pub static SKILL_12162: SkillDef = SkillDef {
    id: SKILL_ID_ELEMENTAL_SKILL,
    name: zh_only("妙思捕手"),
    description: zh_only(
        "造成2点水元素伤害，生成1层敏捷切换。如果手牌中有卡牌具有赋能，则改为造成3点水元素伤害。",
    ),
    kind: SkillKind::ElementalSkill,
    cost: SkillCost::from_dice(&[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))]),
    effect: elemental_skill_effect,
};

/// 技能 12163 精密水冷仪（元素爆发）。
///
/// - 文案：造成 2 点水元素伤害，召唤冷静一下鸭。
/// - 消耗：3 水元素骰 + 2 充能。
pub static SKILL_12163: SkillDef = SkillDef {
    id: SKILL_ID_ELEMENTAL_BURST,
    name: zh_only("精密水冷仪"),
    description: zh_only("造成2点水元素伤害，召唤冷静一下鸭。"),
    kind: SkillKind::ElementalBurst,
    cost: SkillCost::from_dice_and_energy(
        &[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))],
        Nat::new(2),
    ),
    effect: elemental_burst_effect,
};

/// 技能 12164 模块式高效运作（被动技能）。
///
/// - 文案：【被动】我方卡牌被赋予赋能时：如果我方场上存在冷静一下鸭，
///   则使其可用次数+1，否则自身获得1点充能。（每回合1次）
/// - 消耗：无。
pub static SKILL_12164: SkillDef = SkillDef {
    id: SKILL_ID_PASSIVE,
    name: zh_only("模块式高效运作"),
    description: zh_only(
        "【被动】我方卡牌被赋予赋能时：如果我方场上存在冷静一下鸭，则使其可用次数+1，否则自身获得1点充能。（每回合1次）",
    ),
    kind: SkillKind::Passive,
    cost: SkillCost::FREE,
    effect: passive_effect,
};

// ======================================================================
// 角色牌定义
// ======================================================================

/// 爱诺的角色牌定义（第一张卡样板）。
///
/// 用 `static` 而不是 `const`：定义需要**稳定地址**（将来注册表 / 状态里存引用）。
pub static AINO: CharacterDef<Tags> = CharacterDef {
    id: ID,
    name: zh_only("爱诺"),
    tags: TAGS,
    max_hp: MAX_HP,
    max_energy: MAX_ENERGY,
    // 顺序固定：普攻 → 战技 → 爆发 → 被动（与游戏内文本描述的 skills 顺序一致）。
    skills: &[&SKILL_12161, &SKILL_12162, &SKILL_12163, &SKILL_12164],
};

#[cfg(test)]
mod tests {
    use super::*;
    use gi_genius_invokation_definitions::generated::{Lang, Resolved};

    // ------------------------------------------------------------------
    // D244 / D245 / D246：本卡声明的「已同步到」的游戏版本
    // ------------------------------------------------------------------

    #[test]
    fn game_version_matches_declared_sync_version() {
        assert_eq!(GAME_VERSION.major(), Nat::new(6));
        assert_eq!(GAME_VERSION.minor(), Nat::new(2));
        // Display 输出 major.minor。
        assert_eq!(format!("{GAME_VERSION}"), "6.2");
    }

    #[test]
    fn aino_basic_data_matches_in_game_text() {
        let aino = &AINO;
        assert_eq!(aino.id, ID);
        assert_eq!(aino.id, CardId::new(1216));
        assert_eq!(aino.name.get(Lang::ZhCn), Resolved::Exact("爱诺"));
        // 生命 / 充能上限来自数据 hp=10、maxenergy=2。
        assert_eq!(aino.max_hp, Nat::new(10));
        assert_eq!(aino.max_energy, Nat::new(2));
        // 4 个技能：普攻 / 战技 / 爆发 / 被动。
        assert_eq!(aino.skills.len(), 4);
    }

    #[test]
    fn aino_tags_are_three_zero_sized_tags() {
        // 标签 = 类型本身（D54）：三个零大小标签组合起来仍然零大小。
        assert_eq!(
            std::mem::size_of::<Tags>(),
            0,
            "标签集合必须是零大小类型（D54）"
        );
        let (element, weapon, nation) = AINO.tags;
        assert_eq!(element, Hydro);
        assert_eq!(weapon, Claymore);
        assert_eq!(nation, NodKrai);
    }

    #[test]
    fn aino_skills_match_in_game_text() {
        let skills = AINO.skills;
        let expected: [(SkillId, &str, SkillKind); 4] = [
            (
                SkillId::new(12161),
                "敲打修理法",
                SkillKind::normal_attack(),
            ),
            (SkillId::new(12162), "妙思捕手", SkillKind::ElementalSkill),
            (SkillId::new(12163), "精密水冷仪", SkillKind::ElementalBurst),
            (SkillId::new(12164), "模块式高效运作", SkillKind::Passive),
        ];
        for (skill, (id, name, kind)) in skills.iter().zip(expected) {
            assert_eq!(skill.id, id);
            assert_eq!(skill.name.get(Lang::ZhCn), Resolved::Exact(name));
            assert_eq!(skill.kind, kind);
        }

        // 消耗（数据 playcost）：12161 = 1 水 + 2 无色；12162 = 3 水；
        // 12163 = 3 水 + 2 充能；12164 = 无。
        let one_hydro_two_any: &[CostComponent] = &[
            CostComponent::SpecificElement(Element::Hydro, Nat::new(1)),
            CostComponent::AnyColor(Nat::new(2)),
        ];
        let three_hydro: &[CostComponent] =
            &[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))];
        assert_eq!(skills[0].cost.dice, one_hydro_two_any);
        assert_eq!(skills[0].cost.energy, Nat::ZERO);
        assert_eq!(skills[1].cost.dice, three_hydro);
        assert_eq!(skills[1].cost.energy, Nat::ZERO);
        assert_eq!(skills[2].cost.dice, three_hydro);
        assert_eq!(skills[2].cost.energy, Nat::new(2));
        assert!(skills[3].cost.dice.is_empty());
        assert_eq!(skills[3].cost.energy, Nat::ZERO);

        // 文案：只填中文（D120），取自游戏内文本描述的 description。
        assert_eq!(
            skills[0].description.get(Lang::ZhCn),
            Resolved::Exact("造成2点物理伤害。")
        );
        assert_eq!(
            skills[3].description.get(Lang::ZhCn),
            Resolved::Exact(
                "【被动】我方卡牌被赋予赋能时：如果我方场上存在冷静一下鸭，则使其可用次数+1，否则自身获得1点充能。（每回合1次）"
            )
        );
    }

    // ------------------------------------------------------------------
    // D214 / D215 / D216 / D242：本文件源码在编译期被嵌入 SOURCE（原文，不解析、不宏展开）。
    // 宏本身留在 definitions；本 crate 的 Cargo.toml 显式打开 embed-source，所以原文一定嵌入。
    // ------------------------------------------------------------------

    #[test]
    fn source_embeds_this_file_verbatim() {
        // 归属：宏没写 id，自动绑定本模块的 ID（D215）。
        assert_eq!(SOURCE.owner, crate::source::SourceOwner::Card(ID));
        // 路径：分段列表拼出相对 crate 根的真实路径（D216）。
        assert_eq!(SOURCE.segments, ["character", "aino"]);
        assert_eq!(SOURCE.path, "src/cards/character/aino/mod.rs");
        // content 首行 = 本文件首行注释。
        assert!(SOURCE.content.starts_with("//! 爱诺（角色牌 1216）"));
        // content 里能看到宏调用本身：证明是原文，没有被展开 / 清洗。
        assert!(
            SOURCE.content.contains(
                r#"gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);"#
            ),
            "原文里应保留宏调用"
        );
        // 与磁盘原文逐字节一致（仅测试期读盘对照；运行期零 IO）。
        let disk = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cards/character/aino/mod.rs"
        ))
        .expect("读本文件失败");
        assert_eq!(SOURCE.content, disk, "嵌入文本必须与文件原文一致");
    }

    #[test]
    fn source_parses_use_dependencies() {
        use crate::source::DependencyKind;
        let deps: Vec<_> = SOURCE.dependencies().iter().collect();
        assert!(!deps.is_empty(), "本文件的 use 应被解析成依赖");
        // 花括号依赖：gi_genius_invokation_definitions::base::{CardId, Element, SkillId}
        assert!(deps.iter().any(|dep| dep.kind() == DependencyKind::Brace
            && dep.path().collect::<Vec<_>>() == ["gi_genius_invokation_definitions", "base"]));
        // 单条依赖：gi_genius_invokation_definitions::generated::Localized
        assert!(deps.iter().any(|dep| dep.kind() == DependencyKind::Single
            && dep.path().collect::<Vec<_>>()
                == ["gi_genius_invokation_definitions", "generated", "Localized"]));
        // 通配（tests 里的 use super::*;）解析不了：保留原文、标未解析。
        assert!(deps
            .iter()
            .any(|dep| !dep.is_parsed() && dep.raw().is_some_and(|raw| raw.contains("super::*"))));
    }

    #[test]
    fn source_is_registered_in_source_registry() {
        let entry =
            crate::source::source_of(SOURCE.path).expect("本文件应已登记到 source::SOURCE_FILES");
        assert_eq!(entry.owner, crate::source::SourceOwner::Card(ID));
        assert_eq!(entry.content, SOURCE.content);
        assert!(
            std::ptr::addr_eq(entry, &SOURCE),
            "登记项应指向同一份静态 SOURCE"
        );
    }
}
