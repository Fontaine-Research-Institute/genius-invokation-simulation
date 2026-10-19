//! **damage_chain 的记录内容类型 + 统一记录器接线**（D221 / D223 / D224 / D225）。
//!
//! 记录的**唯一入口**是 [crate::records::Recorder]：极薄容器，只提供「属于哪一步 + 任意内容」
//! （D223 / D224）。内容类型由本模块自己定（D225：内容类型单独一个文件），显示由
//! [ShowRecord] 决定 —— 记录器不预设任何种类枚举 / 字段。
//!
//! ## 口径（与本模块的结算语义分离）
//!
//! - **只写观测、不改结算**：记录在「干活处」直接 push 真实数值（加伤各项 / 护盾吸收 /
//!   减伤合计 / 最终扣血），**不靠前后局面 diff 推导** —— diff 看不到护盾吸收与加伤明细，
//!   keywords::trace 里 shield_used / bonus 恒 0 就是这个原因；
//! - **步号**：链内每个阶段各有固定步号（[STAGE_IMMUNITY] / [STAGE_BONUS] / [STAGE_SCALE] /
//!   [STAGE_SHIELD] / [STAGE_REDUCTION] / [STAGE_TO_HP]）；外层（`damage_with_chain_recorded`）
//!   会把它们**重盖**成调用方给的
//!   结算步号（[write_records]），所以一条伤害在调用方记录器里都属于同一步；
//! - **可整体裁剪**（D223）：关闭 feature 时 [crate::records::Recorder] 是零大小类型、
//!   push 为空操作；链里的记录点用 cfg 分支包住
//!   （`damage_chain::chain` 里的 `record` 辅助函数），所以**关闭时既不分配也不克隆**，
//!   结算结果完全不变。
//!
//! ## 记录什么（Lead 要求）
//!
//! | 记录 | 内容 |
//! |---|---|
//! | [DamageChainRecord::Hit] | 本次伤害的基础值 + 类型 + 来源实体 + 来源属性（调用方给的标签）+ 目标 |
//! | [DamageChainRecord::TypeOverride] | 伤害类型被覆盖：原始类型 -> 实际类型（覆盖先于反应判定） |
//! | [DamageChainRecord::Increase] | 加伤明细：每一条通用增伤（下标 + 数值） |
//! | [DamageChainRecord::Mastery] | 加伤明细：元素精通（点数 × 每点值 = 实际加上） |
//! | [DamageChainRecord::ReactionBonus] | 加伤明细：反应加伤（「本伤害 +N」） |
//! | [DamageChainRecord::BonusTotal] | 加伤合计 |
//! | [DamageChainRecord::PiercingBypass] | 穿透：不吃加伤 / 护盾 / 减伤 / 免疫（记下被忽略的量） |
//! | [DamageChainRecord::Immune] | 伤害免疫：本次伤害归 0（记下被免疫掉的数值） |
//! | [DamageChainRecord::Scale] | 乘算 / 除算：独立乘法、倍数区、除数、取整、区外加减、最终值 |
//! | [DamageChainRecord::Shield] | 护盾可吸收值、实际吸收、吸收后剩余伤害 |
//! | [DamageChainRecord::Reduction] | 减伤各条、请求合计、实际扣掉、减后剩余 |
//! | [DamageChainRecord::ToHp] | 最终扣到生命值的伤害 |
//! | [DamageChainRecord::Reaction] | 命中的元素反应（身份名 + 已覆盖元素 + 本次攻击元素） |
//!
//! ## 依据
//!
//! D221（插入式结算回放）、D223 / D224（记录 API 极薄：属于哪一步 + 任意内容；内容类型自己定义
//! 显示）、D225（内容类型单独文件）、D196（伤修链是插入式子结算，每个阶段各算一步）。

use crate::base::Element;
use crate::families::character::damage_chain::bonus::Rounding;
use crate::families::character::types::DamageType;
use crate::records::{Recorder, ShowRecord};
use gi_genius_invokation_engine::{EntityId, Nat};

/// 链内的**阶段步号**：伤害免疫（本次伤害归 0）。
pub const STAGE_IMMUNITY: Nat = Nat::ZERO;

/// 链内的**阶段步号**：伤害类型覆盖（进入计算前；与免疫同一步号 0）。
pub const STAGE_TYPE: Nat = Nat::ZERO;

/// 链内的**阶段步号**：加伤。
pub const STAGE_BONUS: Nat = Nat::new(1);

/// 链内的**阶段步号**：乘算 / 除算（分工计划 §4.4.5 四段模型）。
pub const STAGE_SCALE: Nat = Nat::new(2);

/// 链内的**阶段步号**：护盾抵消。
pub const STAGE_SHIELD: Nat = Nat::new(3);

/// 链内的**阶段步号**：减伤扣减。
pub const STAGE_REDUCTION: Nat = Nat::new(4);

/// 链内的**阶段步号**：扣血数值。
pub const STAGE_TO_HP: Nat = Nat::new(5);

/// **本模块自己的记录器类型**（D225：内容类型由本模块定；容器是公用 [Recorder]）。
pub type DamageChainRecorder = Recorder<DamageChainRecord>;

/// 一条伤害链记录的内容（**纯数据**；显示见 [ShowRecord] 实现）。
///
/// 数值全部是**计算处直接给的**（不是前后 diff）：护盾吸收 / 加伤明细 / 减伤合计 /
/// 最终扣血都能与 `chain::ChainOutcome` 逐项对上。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DamageChainRecord {
    /// 本次伤害的**基础值与类型 + 来源信息**（由 `damage_with_chain_recorded` 在入口处记）。
    Hit {
        /// 基础伤害（技能 / 卡面给的原始数值）。
        base: Nat,
        /// 伤害类型（元素 / 物理 / 穿透）。
        ty: DamageType,
        /// 来源实体（谁造成这次伤害；不知道 -> None）。
        source: Option<EntityId>,
        /// 目标实体（这次伤害落在谁身上；不知道 -> None）。
        target: Option<EntityId>,
        /// 来源属性标签（由调用方给，例如「普通攻击」「元素战技」「反应」；
        /// 空串 = 未标注 —— damage_chain 不认识关键字层的来源类别枚举）。
        source_label: &'static str,
    },
    /// **伤害类型被覆盖**（卡面「使角色造成的物理伤害变为水元素伤害」类效果）。
    ///
    /// 覆盖发生在进入计算前：上一条 [Self::Hit] 记原始类型，本条记实际类型；
    /// 之后的穿透 / 免疫 / 乘算 / 反应判定都按实际类型。
    TypeOverride {
        /// 原始类型（调用方传入）。
        original: DamageType,
        /// 实际生效类型（覆盖后）。
        actual: DamageType,
    },
    /// 加伤明细：第 `index` 条通用增伤。
    Increase {
        /// 第几条（0 起；顺序 = 调用方给的登记顺序，结算顺序 §一 第 2/3 条）。
        index: Nat,
        /// 这条增伤的数值。
        amount: Nat,
    },
    /// 加伤明细：元素精通（只在触发元素反应时生效）。
    Mastery {
        /// 触发方元素精通点数。
        mastery: Nat,
        /// 每点精通的加伤值（= `bonus::MASTERY_DAMAGE_PER_POINT`）。
        per_point: Nat,
        /// 实际加上的点数（= 点数 × 每点值；不反应时为 0）。
        added: Nat,
    },
    /// 加伤明细：反应加伤（规则书「本伤害 +N」）。
    ReactionBonus {
        /// 加上的点数。
        amount: Nat,
    },
    /// 加伤合计（= 各项增伤 + 反应加伤 + 元素精通；穿透恒为 0）。
    BonusTotal {
        /// 合计点数。
        total: Nat,
    },
    /// 穿透伤害：**不吃**加伤 / 乘算 / 护盾 / 减伤 / 免疫（规则书·特殊词条 穿透伤害 + K5），
    /// 记下被忽略的量。
    PiercingBypass {
        /// 本可以加上的伤害加成（被忽略）。
        ignored_bonus: Nat,
        /// 场上护盾点数（被忽略）。
        ignored_shield: Nat,
        /// 各条减伤合计（被忽略）。
        ignored_reduction: Nat,
        /// 目标身上有没有伤害免疫（穿透不吃免疫，但如实记录它存在）。
        ignored_immunity: bool,
    },
    /// **伤害免疫**：本次伤害归 0（不结算），记下被免疫掉的数值。
    Immune {
        /// 被免疫掉的基础伤害。
        ignored: Nat,
    },
    /// **乘算 / 除算**（分工计划 §4.4.5 第 2 / 3 / 4 段）：各步真实值。
    Scale {
        /// 独立乘法的乘积（没有 -> 1）。
        standalone: Nat,
        /// 倍数区乘数之和（没有 -> 0；`0` 表示整段跳过）。
        zone_mul: Nat,
        /// 除数的乘积（没有 -> 1）。
        divisor: Nat,
        /// 取整方向（`None` = 没走除法）。
        rounding: Option<Rounding>,
        /// 乘除区之外加上的点数。
        added: Nat,
        /// 乘除区之外减掉的点数。
        reduced: Nat,
        /// 乘算 / 除算之后的最终值。
        total: Nat,
    },
    /// 护盾：可吸收值 / 实际吸收 / 吸收后剩余伤害。
    Shield {
        /// 读到的护盾总点数（出战状态区 + 角色状态列表）。
        available: Nat,
        /// 实际被护盾吸掉的点数。
        absorbed: Nat,
        /// 吸收后剩余的伤害值（还没走减伤）。
        remaining: Nat,
    },
    /// 减伤：各条数值 / 请求合计 / 实际扣掉 / 减后剩余。
    Reduction {
        /// 各条减伤的数值（顺序 = 角色状态 -> 出战状态，同类按进场先后）。
        items: Vec<Nat>,
        /// 请求合计（各条相加；可能大于剩余伤害）。
        requested: Nat,
        /// 实际扣掉的点数（不会超过剩余伤害）。
        removed: Nat,
        /// 减后剩余的伤害值。
        remaining: Nat,
    },
    /// 最终扣到生命值的伤害。
    ToHp {
        /// 扣血数值（= 链的最终结果）。
        amount: Nat,
    },
    /// 命中的元素反应（没有反应就不记这一条）。
    Reaction {
        /// 反应身份名（中文；例如「融化」「结晶」）。
        name: &'static str,
        /// 命中的已覆盖元素。
        covered: Element,
        /// 本次攻击元素。
        attack: Element,
    },
}

/// 元素的**显示名**（中文；只用于记录显示，不参与任何运算）。
pub fn element_text(element: Element) -> &'static str {
    match element {
        Element::Pyro => "火",
        Element::Hydro => "水",
        Element::Electro => "雷",
        Element::Dendro => "草",
        Element::Cryo => "冰",
        Element::Anemo => "风",
        Element::Geo => "岩",
    }
}

/// 伤害类型的**显示名**（中文；只用于记录显示，不参与任何运算）。
pub fn damage_type_text(ty: DamageType) -> String {
    match ty {
        DamageType::Elemental(element) => format!("{}元素伤害", element_text(element)),
        DamageType::Physical => "物理伤害".to_string(),
        DamageType::Piercing => "穿透伤害".to_string(),
    }
}

impl ShowRecord for DamageChainRecord {
    fn show(&self, out: &mut String) {
        match self {
            DamageChainRecord::Hit {
                base,
                ty,
                source,
                target,
                source_label,
            } => {
                if source_label.is_empty() {
                    out.push_str("造成伤害");
                } else {
                    out.push_str(source_label);
                }
                out.push_str(&format!(
                    "：基础 {}（{}）",
                    base.get(),
                    damage_type_text(*ty)
                ));
                if let Some(source) = source {
                    out.push_str(&format!("，来源 {}", source.get()));
                }
                if let Some(target) = target {
                    out.push_str(&format!("，目标 {}", target.get()));
                }
            }
            DamageChainRecord::TypeOverride { original, actual } => {
                out.push_str(&format!(
                    "伤害类型转换：{} -> {}",
                    damage_type_text(*original),
                    damage_type_text(*actual)
                ));
            }
            DamageChainRecord::Increase { index, amount } => {
                out.push_str(&format!("加伤 #{}：+{}", index.get(), amount.get()));
            }
            DamageChainRecord::Mastery {
                mastery,
                per_point,
                added,
            } => {
                out.push_str(&format!(
                    "元素精通：{} 点 × 每点 {} = +{}",
                    mastery.get(),
                    per_point.get(),
                    added.get()
                ));
            }
            DamageChainRecord::ReactionBonus { amount } => {
                out.push_str(&format!("反应加伤：+{}", amount.get()));
            }
            DamageChainRecord::BonusTotal { total } => {
                out.push_str(&format!("加伤合计：+{}", total.get()));
            }
            DamageChainRecord::PiercingBypass {
                ignored_bonus,
                ignored_shield,
                ignored_reduction,
                ignored_immunity,
            } => {
                out.push_str(&format!(
                    "穿透伤害：不吃加伤（忽略 +{}）/ 护盾（忽略 {}）/ 减伤（忽略 {}）/ 免疫（{}）",
                    ignored_bonus.get(),
                    ignored_shield.get(),
                    ignored_reduction.get(),
                    if *ignored_immunity { "有但不吃" } else { "无" }
                ));
            }
            DamageChainRecord::Immune { ignored } => {
                out.push_str(&format!("伤害免疫：本次伤害归 0（被免疫 {}）", ignored.get()));
            }
            DamageChainRecord::Scale {
                standalone,
                zone_mul,
                divisor,
                rounding,
                added,
                reduced,
                total,
            } => {
                out.push_str(&format!(
                    "乘算：独立 ×{}，倍数区 ×{}，除数 {}，取整 {}，区外加 {} / 减 {} -> {}",
                    standalone.get(),
                    zone_mul.get(),
                    divisor.get(),
                    match rounding {
                        None => "无",
                        Some(Rounding::Up) => "上取整",
                        Some(Rounding::Down) => "下取整",
                    },
                    added.get(),
                    reduced.get(),
                    total.get()
                ));
            }
            DamageChainRecord::Shield {
                available,
                absorbed,
                remaining,
            } => {
                out.push_str(&format!(
                    "护盾：可吸收 {}，实际吸收 {}，剩余伤害 {}",
                    available.get(),
                    absorbed.get(),
                    remaining.get()
                ));
            }
            DamageChainRecord::Reduction {
                items,
                requested,
                removed,
                remaining,
            } => {
                out.push_str("减伤：各条 [");
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&format!("{}", item.get()));
                }
                out.push_str(&format!(
                    "]，请求合计 {}，实际扣 {}，减后 {}",
                    requested.get(),
                    removed.get(),
                    remaining.get()
                ));
            }
            DamageChainRecord::ToHp { amount } => {
                out.push_str(&format!("最终扣血：{}", amount.get()));
            }
            DamageChainRecord::Reaction {
                name,
                covered,
                attack,
            } => {
                out.push_str(&format!(
                    "元素反应：{}（已覆盖 {} + 本次 {}）",
                    name,
                    element_text(*covered),
                    element_text(*attack)
                ));
            }
        }
    }
}

/// **记录写入口**：步号 + 记录器（D223：属于哪一步由调用方给）。
///
/// `damage_with_chain_recorded` 用它把一次伤害的全部记录写到**同一个结算步号**下
/// （链内阶段步号会被重盖，见 [write_records]）。
pub struct RecordedStep<'a> {
    /// 结算步号（外层给的）。
    pub step: Nat,
    /// 来源属性标签（空串 = 未标注；由调用方给，例如「元素战技」）。
    pub source_label: &'static str,
    /// 记录器（只借，不拥有）。
    pub recorder: &'a mut DamageChainRecorder,
}

impl<'a> RecordedStep<'a> {
    /// 建一个步号 + 记录器的写入口（不标注来源属性）。
    pub fn new(step: Nat, recorder: &'a mut DamageChainRecorder) -> RecordedStep<'a> {
        RecordedStep {
            step,
            source_label: "",
            recorder,
        }
    }

    /// 补上来源属性标签（链式）。
    pub fn with_source_label(mut self, label: &'static str) -> RecordedStep<'a> {
        self.source_label = label;
        self
    }
}

/// **把一份链记录转写到调用方记录器**：步号统一重盖为 `step`（D223：一次伤害属于同一个结算步）。
///
/// - 关闭 feature 时源记录器恒为空 -> 循环不执行（零开销）；
/// - 只读源记录器、只追加到目标，不改任何结算结果。
pub fn write_records(source: &DamageChainRecorder, step: Nat, sink: &mut DamageChainRecorder) {
    for record in source.entries() {
        sink.push(step, record.content.clone());
    }
}

// ======================================================================
// 测试（D221 / D223 / D224 / D225）
// ======================================================================

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::base::{CardId, Element};
    use crate::families::action_card::status::StateList;
    use crate::families::character::attachment::{
        Attachment, AttachmentAllowSet, ReactionRegistry,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::{
        damage_with_chain, damage_with_chain_recorded, ChainDamageEnv, DamageEnv,
    };
    use crate::families::character::damage_chain::bonus::{AmountOp, DamageBonus, DamageModifiers};
    use crate::families::character::damage_chain::chain::{
        schedule_records, settle, settle_records, DamageChainEnv, DamageInput,
    };
    use crate::families::character::damage_chain::statuses::{
        DAMAGE_IMMUNITY, DAMAGE_NEGATION, SHIELD_CHARACTER, SHIELD_COMBAT,
    };
    use crate::families::character::trigger::CharacterTriggers;
    use crate::families::character::types::DamageType;
    use crate::families::character::zones::combat_status::{
        CombatStatusTriggers, CombatStatusZone,
    };
    use crate::reactions::effect::EffectContext;
    use gi_genius_invokation_engine::{EntityId, Nat};

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 出战状态区：给 `owner` 挂护盾 `points` 点。
    fn combat_status_with_shield(owner: EntityId, points: u32) -> CombatStatusZone {
        CombatStatusZone::new()
            .attach_with_triggers(
                owner,
                &SHIELD_COMBAT,
                Some(n(points)),
                None,
                &CombatStatusTriggers::new(),
            )
            .zone
    }

    /// 角色状态列表：护盾 + 减伤。
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

    /// 取记录里的某种内容（只读；按写入顺序）。
    fn records_of<T>(
        recorder: &DamageChainRecorder,
        pick: impl Fn(&DamageChainRecord) -> Option<T>,
    ) -> Vec<T> {
        recorder
            .entries()
            .iter()
            .filter_map(|record| pick(&record.content))
            .collect()
    }

    fn to_hp_records(recorder: &DamageChainRecorder) -> Vec<Nat> {
        records_of(recorder, |content| match content {
            DamageChainRecord::ToHp { amount } => Some(*amount),
            _ => None,
        })
    }

    fn shield_records(recorder: &DamageChainRecorder) -> Vec<(Nat, Nat, Nat)> {
        records_of(recorder, |content| match content {
            DamageChainRecord::Shield {
                available,
                absorbed,
                remaining,
            } => Some((*available, *absorbed, *remaining)),
            _ => None,
        })
    }

    fn bonus_total_records(recorder: &DamageChainRecorder) -> Vec<Nat> {
        records_of(recorder, |content| match content {
            DamageChainRecord::BonusTotal { total } => Some(*total),
            _ => None,
        })
    }

    #[test]
    fn shield_absorption_records_real_numbers() {
        // 5 点伤害：盾 2 吸 2 -> 3；减伤 1 -> 扣血 2。
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = states(None, Some(1));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let (outcome, recorder) = settle_records(
            DamageInput::new(n(5), DamageType::elemental(Element::Pyro)),
            &env,
            &DamageBonus::NONE,
        );
        assert_eq!(outcome.to_hp, n(2));
        // 护盾：可吸收 2、实际吸收 2、剩余 3（不是 diff 出来的）。
        assert_eq!(shield_records(&recorder), vec![(n(2), n(2), n(3))]);
        // 减伤：各条 [1]、请求合计 1、实际扣 1、减后 2。
        let reductions: Vec<(Vec<Nat>, Nat, Nat, Nat)> =
            records_of(&recorder, |content| match content {
                DamageChainRecord::Reduction {
                    items,
                    requested,
                    removed,
                    remaining,
                } => Some((items.clone(), *requested, *removed, *remaining)),
                _ => None,
            });
        assert_eq!(reductions, vec![(vec![n(1)], n(1), n(1), n(2))]);
        // 最终扣血。
        assert_eq!(to_hp_records(&recorder), vec![n(2)]);
    }

    #[test]
    fn shield_over_absorption_records_zero_to_hp() {
        // 2 点伤害、盾 6：全部吸掉、扣血 0，减伤没有东西可减。
        let combat_status = combat_status_with_shield(n(1), 6);
        let character_states = states(None, Some(2));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let (outcome, recorder) = settle_records(
            DamageInput::new(n(2), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        );
        assert_eq!(outcome.to_hp, Nat::ZERO);
        assert_eq!(shield_records(&recorder), vec![(n(6), n(2), Nat::ZERO)]);
        let reductions: Vec<Nat> = records_of(&recorder, |content| match content {
            DamageChainRecord::Reduction { removed, .. } => Some(*removed),
            _ => None,
        });
        assert_eq!(reductions, vec![Nat::ZERO]);
        assert_eq!(to_hp_records(&recorder), vec![Nat::ZERO]);
    }

    #[test]
    fn piercing_bypass_records_what_is_ignored() {
        // 穿透 3 点：加伤 9 / 盾 10 / 减伤 2 全部被忽略，只扣 3。
        let combat_status = combat_status_with_shield(n(1), 10);
        let character_states = states(None, Some(2));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let bonus = DamageBonus {
            increases: &[n(4)],
            mastery: n(3),
            reaction_bonus: n(2),
            reacted: true,
        };
        let (outcome, recorder) =
            settle_records(DamageInput::new(n(3), DamageType::Piercing), &env, &bonus);
        assert!(outcome.bypassed);
        assert_eq!(outcome.to_hp, n(3));
        let bypass: Vec<(Nat, Nat, Nat, bool)> = records_of(&recorder, |content| match content {
            DamageChainRecord::PiercingBypass {
                ignored_bonus,
                ignored_shield,
                ignored_reduction,
                ignored_immunity,
            } => Some((
                *ignored_bonus,
                *ignored_shield,
                *ignored_reduction,
                *ignored_immunity,
            )),
            _ => None,
        });
        assert_eq!(bypass, vec![(n(9), n(10), n(2), false)]);
        // 穿透下护盾 / 减伤实际生效值为 0。
        assert_eq!(shield_records(&recorder), vec![(n(10), Nat::ZERO, n(3))]);
        let removed: Vec<Nat> = records_of(&recorder, |content| match content {
            DamageChainRecord::Reduction { removed, .. } => Some(*removed),
            _ => None,
        });
        assert_eq!(removed, vec![Nat::ZERO]);
        assert_eq!(to_hp_records(&recorder), vec![n(3)]);
    }

    #[test]
    fn bonus_chain_records_each_item_and_total_in_order() {
        // 增伤 [1, 2] + 反应加伤 2 + 精通 3 点（每点 +1）-> 合计 8。
        let combat_status = CombatStatusZone::new();
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let bonus = DamageBonus {
            increases: &[n(1), n(2)],
            mastery: n(3),
            reaction_bonus: n(2),
            reacted: true,
        };
        let (outcome, recorder) = settle_records(
            DamageInput::new(n(4), DamageType::elemental(Element::Cryo)),
            &env,
            &bonus,
        );
        assert_eq!(outcome.bonus_total, n(8));
        assert_eq!(outcome.damage_after_bonus, n(12));
        assert_eq!(outcome.to_hp, n(12));
        // 明细顺序（只看加伤那一步 STAGE_BONUS）：增伤 #0、#1 -> 反应加伤 -> 精通 -> 合计。
        let kinds: Vec<&'static str> = recorder
            .step_entries(STAGE_BONUS)
            .map(|content| match content {
                DamageChainRecord::Increase { .. } => "增伤",
                DamageChainRecord::ReactionBonus { .. } => "反应加伤",
                DamageChainRecord::Mastery { .. } => "精通",
                DamageChainRecord::BonusTotal { .. } => "合计",
                _ => "其它",
            })
            .collect();
        assert_eq!(kinds, vec!["增伤", "增伤", "反应加伤", "精通", "合计"]);
        let increases: Vec<(Nat, Nat)> = records_of(&recorder, |content| match content {
            DamageChainRecord::Increase { index, amount } => Some((*index, *amount)),
            _ => None,
        });
        assert_eq!(increases, vec![(Nat::ZERO, n(1)), (n(1), n(2))]);
        let mastery: Vec<(Nat, Nat, Nat)> = records_of(&recorder, |content| match content {
            DamageChainRecord::Mastery {
                mastery,
                per_point,
                added,
            } => Some((*mastery, *per_point, *added)),
            _ => None,
        });
        assert_eq!(mastery, vec![(n(3), n(1), n(3))]);
        assert_eq!(bonus_total_records(&recorder), vec![n(8)]);
        assert_eq!(to_hp_records(&recorder), vec![n(12)]);
    }

    #[test]
    fn recorded_damage_entry_carries_source_label_and_reaction() {
        // 冰附着 + 本次火 2 点：融化（本伤害 +2）；入口记录带来源属性 / 来源 / 目标 / 反应。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let triggers = CharacterTriggers::new();
        let combat_status = CombatStatusZone::new();
        let character_states = StateList::new();
        let chain_env = DamageChainEnv::new(&combat_status, &character_states);
        let damage_env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let mut state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        state.attachment = Attachment::with(Element::Cryo);
        let env = ChainDamageEnv {
            damage: &damage_env,
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let mut recorder = DamageChainRecorder::new();
        let mut recorded = RecordedStep::new(n(7), &mut recorder).with_source_label("元素战技");
        let result = damage_with_chain_recorded(
            &state,
            n(1),
            n(2),
            n(2),
            DamageType::elemental(Element::Pyro),
            &env,
            &mut recorded,
        );
        assert_eq!(result.chain.to_hp, n(4));
        // 全部记录都落在调用方给的结算步号 7 下（链内阶段步号被重盖）。
        assert!(recorder.entries().iter().all(|record| record.step == n(7)));
        let hits: Vec<(Nat, &'static str, Option<EntityId>, Option<EntityId>)> =
            records_of(&recorder, |content| match content {
                DamageChainRecord::Hit {
                    base,
                    source,
                    target,
                    source_label,
                    ..
                } => Some((*base, *source_label, *source, *target)),
                _ => None,
            });
        assert_eq!(hits, vec![(n(2), "元素战技", Some(n(1)), Some(n(2)))]);
        let reactions: Vec<&'static str> = records_of(&recorder, |content| match content {
            DamageChainRecord::Reaction { name, .. } => Some(*name),
            _ => None,
        });
        assert_eq!(reactions, vec!["融化"]);
        // 反应加伤明细 + 合计 + 最终扣血都在。
        assert_eq!(
            records_of(&recorder, |content| match content {
                DamageChainRecord::ReactionBonus { amount } => Some(*amount),
                _ => None,
            }),
            vec![n(2)]
        );
        assert_eq!(bonus_total_records(&recorder), vec![n(2)]);
        assert_eq!(to_hp_records(&recorder), vec![n(4)]);
        // 文本导出可读（内容类型自己定义显示）。
        let text = recorder.render();
        assert!(text.contains("元素战技：基础 2（火元素伤害）"));
        assert!(text.contains("元素反应：融化"));
        assert!(text.contains("最终扣血：4"));
    }

    #[test]
    fn recording_does_not_change_the_outcome() {
        // 同一输入：带记录 / 不带记录的结果必须完全一致。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let triggers = CharacterTriggers::new();
        let combat_status = combat_status_with_shield(n(10), 3);
        let character_states = states(None, Some(1));
        let chain_env = DamageChainEnv::new(&combat_status, &character_states);
        let damage_env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let env = ChainDamageEnv {
            damage: &damage_env,
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let plain = damage_with_chain(&state, n(1), n(2), n(5), DamageType::Physical, &env);
        let mut recorder = DamageChainRecorder::new();
        let mut recorded = RecordedStep::new(n(0), &mut recorder);
        let with_record = damage_with_chain_recorded(
            &state,
            n(1),
            n(2),
            n(5),
            DamageType::Physical,
            &env,
            &mut recorded,
        );
        assert_eq!(plain.chain, with_record.chain);
        assert_eq!(plain.outcome.state, with_record.outcome.state);
        assert_eq!(plain.outcome.record, with_record.outcome.record);
        assert_eq!(plain.pending.len(), with_record.pending.len());
        assert!(!recorder.is_empty());
    }

    #[test]
    fn plain_chain_settle_matches_the_collecting_entry() {
        // 普通入口不产生记录（热路径不额外分配）；记录版入口才写记录。
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let plain = settle(
            DamageInput::new(n(3), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        );
        let (recorded, recorder) = schedule_records(
            DamageInput::new(n(3), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        )
        .run_recorded();
        assert_eq!(plain, recorded);
        assert!(!recorder.is_empty());
    }

    #[test]
    fn write_records_restamps_the_step_number() {
        // 链内阶段步号 0..=3 -> 调用方步号 9。
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = states(None, Some(1));
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let (_, chain_records) = settle_records(
            DamageInput::new(n(5), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        );
        assert!(chain_records
            .entries()
            .iter()
            .any(|record| record.step == STAGE_BONUS));
        let mut sink = DamageChainRecorder::new();
        write_records(&chain_records, n(9), &mut sink);
        assert_eq!(sink.len(), chain_records.len());
        assert!(sink.entries().iter().all(|record| record.step == n(9)));
        // 内容不变。
        assert_eq!(
            sink.entries()[0].content,
            chain_records.entries()[0].content
        );
    }

    #[test]
    fn show_record_uses_element_names() {
        assert_eq!(element_text(Element::Pyro), "火");
        assert_eq!(element_text(Element::Geo), "岩");
        assert_eq!(damage_type_text(DamageType::Physical), "物理伤害");
        assert_eq!(
            damage_type_text(DamageType::elemental(Element::Cryo)),
            "冰元素伤害"
        );
        assert_eq!(damage_type_text(DamageType::Piercing), "穿透伤害");
        let mut text = String::new();
        DamageChainRecord::Reaction {
            name: "结晶",
            covered: Element::Hydro,
            attack: Element::Geo,
        }
        .show(&mut text);
        assert_eq!(text, "元素反应：结晶（已覆盖 水 + 本次 岩）");
    }

    #[test]
    fn immunity_records_the_nullified_damage() {
        // 出战中挂「伤害免疫」：伤害归 0，记录里有 Immune（被免疫的数值是真实值）。
        let combat_status = CombatStatusZone::new()
            .attach_with_triggers(
                n(1),
                &DAMAGE_IMMUNITY,
                None,
                None,
                &CombatStatusTriggers::new(),
            )
            .zone;
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let (outcome, recorder) = settle_records(
            DamageInput::new(n(5), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        );
        assert!(outcome.immune);
        let immunities: Vec<Nat> = records_of(&recorder, |content| match content {
            DamageChainRecord::Immune { ignored } => Some(*ignored),
            _ => None,
        });
        assert_eq!(immunities, vec![n(5)]);
        assert_eq!(to_hp_records(&recorder), vec![Nat::ZERO]);
        assert!(recorder
            .render()
            .contains("伤害免疫：本次伤害归 0（被免疫 5）"));
    }

    #[test]
    fn multiplier_records_the_four_stage_numbers() {
        // 基础 3，倍数区 ×2 -> 6，除 2 -> 3，区外 +2 -> 5。
        let combat_status = CombatStatusZone::new();
        let character_states = StateList::new();
        let ops = [
            AmountOp::mul_zone(n(2)),
            AmountOp::div(n(2)),
            AmountOp::add_after_mul(n(2)),
        ];
        let env = DamageChainEnv::new(&combat_status, &character_states).with_modifiers(
            DamageModifiers {
                ops: &ops,
                rounding: None,
            },
        );
        let (outcome, recorder) = settle_records(
            DamageInput::new(n(3), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        );
        assert_eq!(outcome.to_hp, n(5));
        let scales: Vec<(Nat, Nat, Nat)> = records_of(&recorder, |content| match content {
            DamageChainRecord::Scale {
                standalone,
                zone_mul,
                divisor,
                ..
            } => Some((*standalone, *zone_mul, *divisor)),
            _ => None,
        });
        // standalone 1 / zone 2 / divisor 2。
        assert_eq!(scales, vec![(n(1), n(2), n(2))]);
        assert_eq!(to_hp_records(&recorder), vec![n(5)]);
        assert!(recorder.render().contains("乘算：独立 ×1"));
    }

    #[test]
    fn record_content_is_pinned_by_card_id_free_display() {
        // 记录显示不依赖卡 id（反应只记名字）；这里钉住 CardId 的 import 不过期。
        let _ = CardId::new(203);
    }
}

/// **关闭 records feature 时**：记录器零大小、链不分配任何内容（D223）。
///
/// 这一组测试只在去掉 records feature 时编译与运行：记录入口仍然可用（签名不变），
/// 但记录器恒为空、结算结果与带记录时一致。
#[cfg(all(test, not(feature = "records")))]
mod tests_without_records {
    use super::*;
    use crate::base::Element;
    use crate::families::action_card::status::StateList;
    use crate::families::character::attachment::{
        Attachment, AttachmentAllowSet, ReactionRegistry,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::{
        damage_with_chain, damage_with_chain_recorded, ChainDamageEnv, DamageEnv,
    };
    use crate::families::character::damage_chain::bonus::DamageBonus;
    use crate::families::character::damage_chain::chain::{
        settle, settle_records, DamageChainEnv, DamageInput,
    };
    use crate::families::character::damage_chain::statuses::SHIELD_COMBAT;
    use crate::families::character::trigger::CharacterTriggers;
    use crate::families::character::types::DamageType;
    use crate::families::character::zones::combat_status::{
        CombatStatusTriggers, CombatStatusZone,
    };
    use crate::reactions::effect::EffectContext;
    use gi_genius_invokation_engine::{EntityId, Nat};

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    fn combat_status_with_shield(owner: EntityId, points: u32) -> CombatStatusZone {
        CombatStatusZone::new()
            .attach_with_triggers(
                owner,
                &SHIELD_COMBAT,
                Some(n(points)),
                None,
                &CombatStatusTriggers::new(),
            )
            .zone
    }

    #[test]
    fn records_feature_off_keeps_result_and_records_nothing() {
        let combat_status = combat_status_with_shield(n(1), 2);
        let character_states = StateList::new();
        let env = DamageChainEnv::new(&combat_status, &character_states);
        let plain = settle(
            DamageInput::new(n(5), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        );
        let (recorded, recorder) = settle_records(
            DamageInput::new(n(5), DamageType::Physical),
            &env,
            &DamageBonus::NONE,
        );
        assert_eq!(plain, recorded);
        assert!(recorder.is_empty(), "关闭 records 时必须没有记录");
    }

    #[test]
    fn recorded_damage_entry_still_works_without_records() {
        let registry = ReactionRegistry::new();
        let triggers = CharacterTriggers::new();
        let rules: [crate::families::character::damage::ReactionRule; 0] = [];
        let combat_status = CombatStatusZone::new();
        let character_states = StateList::new();
        let chain_env = DamageChainEnv::new(&combat_status, &character_states);
        let damage_env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let mut state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        state.attachment = Attachment::with(Element::Cryo);
        let env = ChainDamageEnv {
            damage: &damage_env,
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let mut recorder = DamageChainRecorder::new();
        let mut recorded = RecordedStep::new(n(0), &mut recorder);
        let with_record = damage_with_chain_recorded(
            &state,
            n(1),
            n(2),
            n(2),
            DamageType::Physical,
            &env,
            &mut recorded,
        );
        let plain = damage_with_chain(&state, n(1), n(2), n(2), DamageType::Physical, &env);
        assert_eq!(with_record.chain, plain.chain);
        assert!(recorder.is_empty());
        assert!(recorder.render().is_empty());
    }
}
