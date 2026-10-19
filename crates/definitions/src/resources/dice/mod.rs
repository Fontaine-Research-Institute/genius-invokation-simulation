//! 【骰子】纯计数的骰池（不是实体，D47）：万能 / 折叠的同色组 / 已展开的具体色（D49）。
//!
//! 依据（决策日志）：D46（骰子用计数表示、按需展开）、D47（纯计数、上限 16、支付只扣计数）、
//! D48（非万能按颜色分组计数）、D49（三块表示 + 展开规则）、D91/D92（本块自己的选择列表 + 折叠函数，
//! 放 dice/parse.rs）、D117（调和 = 转化，不是获得；转化必须指定目标，未指定目标的内置算法留空）、
//! D121（消耗骰子的格式归骰子模块）、D139/D140（生成必须给 n；随机可限定基础 / 不限定）、
//! D141（指定内容 = 有序表达式，数量 = 长度，顺序当前无语义）、D143/D145（查询返回固定的“当前所有骰子”）、
//! D146/D148（查询时折叠组按**当前局面的真实条件概率**采样展开；快照保留折叠组）、
//! D149/D151/D156/D158/D159（概率由本块算、真实随机数由引擎 Rng 给；概率质量用 ExactNat128 / 精确分数，全程不经过浮点）。
//!
//! **不是实体**：骰池就是计数，不占实体区、不发号（D47）。
//! **上限 16**：生成时超出部分直接不生成（截断；D47）。
//! **三块表示**（D49）：
//! 1. omni：万能骰个数（单独计，不参与折叠）；
//! 2. folded：颜色未知的同色组，每组带**颜色概率质量**与**同源标记**（D148）；
//! 3. known：已展开的具体颜色计数；一旦展开、该色骰子还在，就不能再折叠。
//!
//! **生成与花费是两套表达**（D144）：
//! - 生成：随机（n + 是否限定基础 + 是否不同）或**有序内容表达式**（数量 = 长度，顺序当前无语义）；
//! - 花费：CostComponent 三类（① 指定元素 n / ② 同色 n / ③ 任意 n，D168/D169），本块只提供
//!   can_afford（可能就行）与 pay（确定性扣减，D47）。
//!
//! **转化**（D117）：把若干骰子转成目标颜色 / 类型；**不是获得**；目标必须由调用方指定，
//! 未指定目标时的内置选择算法**留空**（调用处 TODO）。
//!
//! **查询**（D143/D145/D146）：all_dice 返回一个**拥有所有权的固定快照**，并且按当前局面的
//! 真实条件概率**采样展开**折叠组（随机数由引擎给，D149）；快照保留折叠组（D148）。

use crate::base::{Element, ELEMENTS};
use gi_genius_invokation_engine::nat::ExactNat128;
use gi_genius_invokation_engine::prob::{self, Fraction};
use gi_genius_invokation_engine::rng::Rng;
use gi_genius_invokation_engine::{nadd, nsub, Nat};

mod parse;

/// 骰池上限（D47：0~16，超出生成不算）。
pub const DICE_CAP: Nat = Nat::new(16);

// ======================================================================
// 骰面
// ======================================================================

/// 骰面：8 种（7 元素 + 万能）；只是计数向量的下标，不是实体（D47 / 计划 §4.4.0）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DieFace {
    /// 火。
    Pyro,
    /// 水。
    Hydro,
    /// 雷。
    Electro,
    /// 草。
    Dendro,
    /// 冰。
    Cryo,
    /// 风。
    Anemo,
    /// 岩。
    Geo,
    /// 万能。
    Omni,
}

impl DieFace {
    /// 全部 8 种骰面（七元素 + 万能），顺序固定。
    pub const ALL: [DieFace; 8] = [
        DieFace::Pyro,
        DieFace::Hydro,
        DieFace::Electro,
        DieFace::Dendro,
        DieFace::Cryo,
        DieFace::Anemo,
        DieFace::Geo,
        DieFace::Omni,
    ];

    /// 七种基础元素骰（**不含**万能）。
    pub const BASIC: [DieFace; 7] = [
        DieFace::Pyro,
        DieFace::Hydro,
        DieFace::Electro,
        DieFace::Dendro,
        DieFace::Cryo,
        DieFace::Anemo,
        DieFace::Geo,
    ];

    /// 对应元素；万能 -> None。
    pub const fn element(self) -> Option<Element> {
        match self {
            DieFace::Omni => None,
            DieFace::Pyro => Some(Element::Pyro),
            DieFace::Hydro => Some(Element::Hydro),
            DieFace::Electro => Some(Element::Electro),
            DieFace::Dendro => Some(Element::Dendro),
            DieFace::Cryo => Some(Element::Cryo),
            DieFace::Anemo => Some(Element::Anemo),
            DieFace::Geo => Some(Element::Geo),
        }
    }

    /// 是否万能。
    pub const fn is_omni(self) -> bool {
        matches!(self, DieFace::Omni)
    }

    /// 元素 -> 骰面。
    pub const fn from_element(element: Element) -> DieFace {
        match element {
            Element::Pyro => DieFace::Pyro,
            Element::Hydro => DieFace::Hydro,
            Element::Electro => DieFace::Electro,
            Element::Dendro => DieFace::Dendro,
            Element::Cryo => DieFace::Cryo,
            Element::Anemo => DieFace::Anemo,
            Element::Geo => DieFace::Geo,
        }
    }
}

/// 元素在 [元素;7] 计数数组里的下标（= Element 的固定枚举顺序）。
const fn element_slot(element: Element) -> Nat {
    match element {
        Element::Pyro => Nat::ZERO,
        Element::Hydro => Nat::new(1),
        Element::Electro => Nat::new(2),
        Element::Dendro => Nat::new(3),
        Element::Cryo => Nat::new(4),
        Element::Anemo => Nat::new(5),
        Element::Geo => Nat::new(6),
    }
}

// ======================================================================
// 生成表达式（D139/D140/D141）
// ======================================================================

/// 生成「指定内容」的有序表达式里的一位（D141）。
///
/// 数量 = 表达式长度；顺序当前**没有语义**（骰池纯计数，[火, 任意, 火] 与 [火, 火, 任意]
/// 结果相同），保留位置以备将来逐颗结算。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiceSlot {
    /// 指定某一元素骰。
    Element(Element),
    /// 指定万能骰。
    Omni,
    /// 任意骰：生成时 = **随机颜色**（受 limited_to_basic 约束；D141 A2）。
    Any,
}

// ======================================================================
// 花费表达式（原定格式；D144：不要新造 DiceCostTarget）
// ======================================================================

/// 花费 / 消耗的一位（D168：只有三类，枚举顺序 = 加费/减费优先级）。
///
/// 骰子模块只提供底层能力：can_afford（可能就行）与 pay（确定性扣减），**不做支付策略**（D147）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CostComponent {
    /// ① 指定元素骰：n 个**同一种指定元素**（只能是一种，不传多元素列表）。
    SpecificElement(Element, Nat),
    /// ② 相同元素骰：n 个**同色**（颜色在支付时再定）。
    SameColor(Nat),
    /// ③ 任意元素骰：n 个任意颜色。
    AnyColor(Nat),
}

/// 费用增减（D170/D172）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CostDelta {
    /// 加费 k。
    Increase {
        /// 加费数量。
        amount: Nat,
    },
    /// 减费 k。
    Decrease {
        /// 减费数量。
        amount: Nat,
    },
}

/// 校验费用是否合法（D168/D171/D172）：同一份费用里不允许两个**不同元素**的 ①。
///
/// 违反 = 逻辑错误 -> **panic**（与精确族「溢出 / 除零 panic」同一口径）。
pub fn validate_cost(cost: &[CostComponent]) {
    let mut first: Option<Element> = None;
    for component in cost {
        if let CostComponent::SpecificElement(element, _) = *component {
            match first {
                Some(known) => {
                    assert!(
                        known == element,
                        "费用不合法：① 指定元素骰只能是一种元素（D168/D171/D172）"
                    );
                }
                None => first = Some(element),
            }
        }
    }
}

/// 对费用做加减（D170/D172/D178）：顺序**写死**为 ① SpecificElement → ② SameColor → ③ AnyColor。
///
/// - 加费 +k（D172：**整体加到一个类，不拆类、不带元素**）：
///   有 ① -> 全加 ①；否则有 ② -> 全加 ②；否则有 ③ -> 全加 ③；
///   三类都不存在（0 费费用）-> **新建 ② `SameColor(k)`**（D178：0 费加费按相同元素算）。
/// - 减费 -k（D170）：先减 ①（减到 0 为止），还有余量再减 ②，再减 ③；
///   减到总费用 0 后剩余作废（最低 0）。
/// - 同类有多个项时：加费只加**第一个**该类项；减费按出现顺序把该类减到 0。
pub fn apply_delta(cost: &[CostComponent], delta: CostDelta) -> Vec<CostComponent> {
    validate_cost(cost);
    match delta {
        CostDelta::Increase { amount } => increase_cost(cost, amount),
        CostDelta::Decrease { amount } => decrease_cost(cost, amount),
    }
}

/// 加费（D172/D178）：① 存在就全加 ①；否则 ②；否则 ③；都没有（0 费）就新建 ② SameColor。
fn increase_cost(cost: &[CostComponent], amount: Nat) -> Vec<CostComponent> {
    let mut next = cost.to_vec();
    if amount.get() == 0 {
        return next;
    }
    if let Some(position) = next
        .iter()
        .position(|component| matches!(*component, CostComponent::SpecificElement(_, _)))
    {
        add_component_amount(&mut next[position], amount);
        return next;
    }
    if let Some(position) = next
        .iter()
        .position(|component| matches!(*component, CostComponent::SameColor(_)))
    {
        add_component_amount(&mut next[position], amount);
        return next;
    }
    if let Some(position) = next
        .iter()
        .position(|component| matches!(*component, CostComponent::AnyColor(_)))
    {
        add_component_amount(&mut next[position], amount);
        return next;
    }
    next.push(CostComponent::SameColor(amount));
    next
}

/// 减费（D170）：① 减到 0 -> ② -> ③；剩余作废。
fn decrease_cost(cost: &[CostComponent], amount: Nat) -> Vec<CostComponent> {
    let mut next = cost.to_vec();
    let mut remaining = amount;
    remaining = drain_class(&mut next, remaining, |component| {
        matches!(*component, CostComponent::SpecificElement(_, _))
    });
    remaining = drain_class(&mut next, remaining, |component| {
        matches!(*component, CostComponent::SameColor(_))
    });
    drain_class(&mut next, remaining, |component| {
        matches!(*component, CostComponent::AnyColor(_))
    });
    next
}

/// 把某一类费用从出现的项里依次减到 0，返回没用完的余量。
fn drain_class(
    cost: &mut Vec<CostComponent>,
    mut remaining: Nat,
    belongs: impl Fn(&CostComponent) -> bool,
) -> Nat {
    while remaining.get() > 0 {
        let Some(position) = cost
            .iter()
            .position(|component| belongs(component) && component_count(component).get() > 0)
        else {
            break;
        };
        let count = component_count(&cost[position]);
        let take = min_nat(count, remaining);
        let left = nsub!(count, take);
        remaining = nsub!(remaining, take);
        if left.get() == 0 {
            cost.remove(position);
        } else {
            set_component_count(&mut cost[position], left);
        }
    }
    remaining
}

/// 某项费用的数量（三类都带一个 Nat）。
fn component_count(component: &CostComponent) -> Nat {
    match *component {
        CostComponent::SpecificElement(_, count)
        | CostComponent::SameColor(count)
        | CostComponent::AnyColor(count) => count,
    }
}

/// 改某项费用的数量。
fn set_component_count(component: &mut CostComponent, count: Nat) {
    match component {
        CostComponent::SpecificElement(_, value)
        | CostComponent::SameColor(value)
        | CostComponent::AnyColor(value) => *value = count,
    }
}

/// 给某项费用加数量。
fn add_component_amount(component: &mut CostComponent, amount: Nat) {
    let current = component_count(component);
    set_component_count(component, nadd!(current, amount));
}

// ======================================================================
// 随机源（D149/D151/D156：真实随机数归引擎，本块只用引擎的 Rng）
// ======================================================================
//
// 骰子模块只算概率（精确族 ExactNat128 / 引擎的 Fraction）；采样时把分布交给
// 引擎的 prob::pick_index + rng::Rng::next_u128() 落一单（D149/D160）。
// 采样调用点只有 sample_color 一处。

// ======================================================================
// 折叠组（D49 + D148）
// ======================================================================

/// 一个颜色未知的**同色组**（D49 第 2 块）。
///
/// - size：组内骰子数；
/// - color_mass：该组颜色是各元素的**未归一化概率质量**（7 个元素顺序，见 ELEMENTS）；
///   分布就是「到当前局面后的真实条件概率」，不是写死的 1/7（D146/D148）；
/// - source：同源标记。同一次随机产生的多个组共用同一个 source，
///   **必须一起展开、且保守地视为同一种颜色**（D148；联合分布若将来需要再升级）。
#[derive(Clone, PartialEq, Debug)]
pub struct FoldedGroup {
    /// 组内骰子数。
    pub size: Nat,
    /// 该组颜色是各元素的未归一化概率质量（7 元素顺序；精确族，D156/D159：概率量不用浮点）。
    pub color_mass: [ExactNat128; 7],
    /// 同源标记：同源组必须一起展开。
    pub source: Nat,
}

// ======================================================================
// 骰池
// ======================================================================

/// 骰池 = 三块（D49）：万能 / 折叠的同色组 / 已展开的具体色。
///
/// 字段私有：对外只有一个读取入口 all_dice（D143），返回固定快照（D145）。
#[derive(Clone, PartialEq, Debug)]
pub struct DicePool {
    /// 万能骰个数。
    omni: Nat,
    /// 颜色未知的同色组（规范序：按 source 再按 size）。
    folded: Vec<FoldedGroup>,
    /// 已展开的具体颜色计数（按 Element 序，无 0 项）。
    known: Vec<(Element, Nat)>,
}

/// 「当前所有骰子」的**固定快照**（D143/D145/D146/D148）。
///
/// - omni / known：确定的部分；
/// - folded：仍折叠的组（原样保留，用来看出哪些是折叠组，D148）；
/// - expanded：本次查询按真实条件概率**采样**展开出的折叠骰具体色（每次查询采样一次）。
///
/// 具体骰总数 = omni + known + expanded；folded 与 expanded 描述的是同一批折叠骰的两面。
/// 快照拥有全部数据，查询后骰池再变化也不会影响它（D145）。
#[derive(Clone, PartialEq, Debug)]
pub struct DiceSnapshot {
    /// 万能骰个数。
    pub omni: Nat,
    /// 已展开的具体颜色计数。
    pub known: Vec<(Element, Nat)>,
    /// 仍折叠的同色组（原样保留）。
    pub folded: Vec<FoldedGroup>,
    /// 本次查询采样展开出的具体颜色计数。
    pub expanded: Vec<(Element, Nat)>,
}

impl DicePool {
    /// 空骰池。
    pub const fn empty() -> DicePool {
        DicePool {
            omni: Nat::ZERO,
            folded: Vec::new(),
            known: Vec::new(),
        }
    }

    /// 由三块构造（外部 / 后续构造折叠组用；本块的随机生成只产出 known/omni）。
    ///
    /// 规范化：known 合并同色、去 0、按 Element 序；folded 去 0、按 (source, size) 排序，
    /// 并把同一 source 的所有组的 color_mass 统一成第一组的（保守：同源组同色同分布，D148）；
    /// 最后按「万能 -> 具体色（元素序）-> 折叠组（规范序）」顺序截断到上限 16（D47）。
    pub fn new(omni: Nat, folded: Vec<FoldedGroup>, known: Vec<(Element, Nat)>) -> DicePool {
        let mut known_norm: Vec<(Element, Nat)> = Vec::new();
        for (element, count) in known {
            if count.get() == 0 {
                continue;
            }
            add_known_entry(&mut known_norm, element, count);
        }
        known_norm.sort_by_key(|entry| entry.0);

        let mut folded_norm: Vec<FoldedGroup> = Vec::new();
        for group in folded {
            if group.size.get() == 0 {
                continue;
            }
            folded_norm.push(FoldedGroup {
                size: group.size,
                color_mass: normalize_mass(group.color_mass),
                source: group.source,
            });
        }
        folded_norm.sort_by_key(|group| (group.source, group.size));
        unify_source_masses(&mut folded_norm);

        let omni = min_nat(omni, DICE_CAP);
        let mut budget = nsub!(DICE_CAP, omni);
        let mut known_capped: Vec<(Element, Nat)> = Vec::new();
        for (element, count) in known_norm {
            if budget.get() == 0 {
                break;
            }
            let take = min_nat(count, budget);
            if take.get() > 0 {
                known_capped.push((element, take));
                budget = nsub!(budget, take);
            }
        }
        let mut folded_capped: Vec<FoldedGroup> = Vec::new();
        for group in folded_norm {
            if budget.get() == 0 {
                break;
            }
            let take = min_nat(group.size, budget);
            if take.get() > 0 {
                folded_capped.push(FoldedGroup {
                    size: take,
                    color_mass: group.color_mass,
                    source: group.source,
                });
                budget = nsub!(budget, take);
            }
        }

        DicePool {
            omni,
            folded: folded_capped,
            known: known_capped,
        }
    }

    /// 骰子总数（omni + 折叠组 + 具体色）；0~16。
    pub fn total(&self) -> Nat {
        let mut total = self.omni;
        for group in &self.folded {
            total = nadd!(total, group.size);
        }
        for (_, count) in &self.known {
            total = nadd!(total, *count);
        }
        total
    }

    /// 「当前所有骰子」：返回**固定快照**（D143/D145），折叠组按真实条件概率采样展开（D146）。
    ///
    /// 随机数由调用方传入引擎的 Rng（D149/D151：本块算概率，引擎给真实随机数）。
    pub fn all_dice(&self, rng: &mut Rng) -> DiceSnapshot {
        let mut expanded: Vec<(Element, Nat)> = Vec::new();
        for (_, mass, size) in self.folded_sources() {
            if let Some(element) = sample_color(&mass, rng) {
                add_known_entry(&mut expanded, element, size);
            }
        }
        expanded.sort_by_key(|entry| entry.0);
        DiceSnapshot {
            omni: self.omni,
            known: self.known.clone(),
            folded: self.folded.clone(),
            expanded,
        }
    }

    // ==================================================================
    // 增加 / 减少
    // ==================================================================

    /// 增加指定骰面的 count 颗（确定性；超出上限 16 的部分不生成，D47）。
    pub fn add(&self, face: DieFace, count: Nat) -> DicePool {
        let capacity = nsub!(DICE_CAP, self.total());
        let take = min_nat(count, capacity);
        let mut next = self.clone();
        if take.get() == 0 {
            return next;
        }
        match face.element() {
            Some(element) => add_known_entry(&mut next.known, element, take),
            None => next.omni = nadd!(next.omni, take),
        }
        next
    }

    /// 减少指定骰面的 count 颗（确定性；不够 -> None）。
    ///
    /// 只扣 known / omni；**折叠组不动**（要看颜色就必须先显式展开，D144）。
    pub fn remove(&self, face: DieFace, count: Nat) -> Option<DicePool> {
        if count.get() == 0 {
            return Some(self.clone());
        }
        if self.face_count(face) < count {
            return None;
        }
        let mut next = self.clone();
        match face.element() {
            Some(element) => subtract_known(&mut next.known, element, count),
            None => next.omni = nsub!(next.omni, count),
        }
        Some(next)
    }

    // ==================================================================
    // 转化（D117：不是获得；目标必须指定；未指定目标的内置算法留空）
    // ==================================================================

    /// 转化（D117）：把 count 颗来源骰转成目标骰面；**不是获得**，总数不变。
    ///
    /// - target 必须由调用方指定；未指定目标时的内置选择算法留空（调用处 TODO）。
    /// - source = Some(face)：只从该面的 known / omni 扣（折叠组要先展开）；
    /// - source = None（任意来源）：按固定顺序扣「折叠组 -> 具体色（元素序）-> 万能」，
    ///   不产生分支（D144：选哪颗骰的分支由上层做）。
    pub fn convert(&self, source: Option<DieFace>, count: Nat, target: DieFace) -> Option<DicePool> {
        if count.get() == 0 {
            return Some(self.clone());
        }
        if self.total() < count {
            return None;
        }
        let mut next = self.clone();
        match source {
            Some(face) => {
                if next.face_count(face) < count {
                    return None;
                }
                match face.element() {
                    Some(element) => subtract_known(&mut next.known, element, count),
                    None => next.omni = nsub!(next.omni, count),
                }
            }
            None => {
                take_any(&mut next, count)?;
            }
        }
        match target.element() {
            Some(element) => add_known_entry(&mut next.known, element, count),
            None => next.omni = nadd!(next.omni, count),
        }
        Some(next)
    }

    // ==================================================================
    // 展开（D49 + D146 + D148）
    // ==================================================================

    /// 展开一个**同源**的折叠组集合（D49/D146/D148）：按 color_mass 枚举 7 种颜色分支，
    /// 概率 = 归一化质量；整组一起变成该颜色（同源组一起展开、同色），并记进 known。
    ///
    /// 精确枚举（不采样）；只返回概率 > 0 的颜色分支。source 不存在 -> 空。
    pub fn expand_source(&self, source: Nat) -> Vec<(DicePool, Fraction)> {
        let Some(entry) = self.folded_sources().into_iter().find(|item| item.0 == source) else {
            return Vec::new();
        };
        let mass = entry.1;
        let size = entry.2;
        let total = mass_total(&mass);
        if total == ExactNat128::ZERO {
            return Vec::new();
        }
        let mut branches = Vec::new();
        for (slot, element) in ELEMENTS.iter().enumerate() {
            if mass[slot] == ExactNat128::ZERO {
                continue;
            }
            let probability = fraction(mass[slot], total);
            let mut next = self.clone();
            next.folded.retain(|group| group.source != source);
            add_known_entry(&mut next.known, *element, size);
            branches.push((next, probability));
        }
        branches
    }

    /// 条件化（D148）：按「具体基础元素计数」上的条件，枚举命中的联合颜色取值。
    ///
    /// 折叠组可能相关（同源组同色），所以保守实现把**命中的联合取值直接展开成具体色**，
    /// 返回 (展开后的骰池, 该取值的无条件概率)；概率和 = P(条件成立)。
    /// 需要条件概率的调用方自己归一化。
    pub fn condition(&self, predicate: &dyn Fn(&BasicColors) -> bool) -> Vec<(DicePool, Fraction)> {
        let sources = self.folded_sources();
        if sources.is_empty() {
            if predicate(&BasicColors::from_known(&self.known)) {
                return vec![(self.clone(), Fraction::ONE)];
            }
            return Vec::new();
        }
        let mut counts = [Nat::ZERO; 7];
        for (element, count) in &self.known {
            let slot = element_slot(*element).get() as usize; // bare-int-ok: 容器下标
            counts[slot] = nadd!(counts[slot], *count);
        }
        let mut assignments: Vec<([Nat; 7], Fraction)> = Vec::new();
        collect_assignments(&sources, &mut counts, Fraction::ONE, &mut assignments);
        let mut total = Nat::ZERO;
        let mut matched = Nat::ZERO;
        let mut chosen: Vec<([Nat; 7], Fraction)> = Vec::new();
        for (counts, probability) in assignments {
            total = nadd!(total, Nat::new(1));
            if predicate(&BasicColors { counts }) {
                matched = nadd!(matched, Nat::new(1));
                chosen.push((counts, probability));
            }
        }
        if matched.get() == 0 {
            return Vec::new();
        }
        if matched == total {
            // 条件与折叠颜色无关（恒真）：折叠组保持原样，概率 1。
            return vec![(self.clone(), Fraction::ONE)];
        }
        let mut branches = Vec::new();
        for (counts, probability) in chosen {
            let mut next = self.clone();
            next.folded.clear();
            next.known = counts_to_known(&counts);
            branches.push((next, probability));
        }
        branches
    }

    // ==================================================================
    // 够不够付 / 扣减（花费格式，D144/D147）
    // ==================================================================

    /// 够不够付（**可能就行**，D144）：折叠组按「可能是什么色」计入上界。
    ///
    /// 注意：pay 要求折叠组先展开，所以 can_afford = true 不保证 pay 直接成功。
    pub fn can_afford(&self, cost: &[CostComponent]) -> bool {
        validate_cost(cost);
        let mut demand = [Nat::ZERO; 7];
        let mut any = Nat::ZERO;
        let mut same_color_counts: Vec<Nat> = Vec::new();
        for component in cost {
            match *component {
                CostComponent::SpecificElement(element, count) => {
                    let slot = element_slot(element).get() as usize; // bare-int-ok: 容器下标
                    demand[slot] = nadd!(demand[slot], count);
                }
                CostComponent::AnyColor(count) => any = nadd!(any, count),
                CostComponent::SameColor(count) => same_color_counts.push(count),
            }
        }
        for count in same_color_counts {
            let mut best_slot = Nat::ZERO;
            let mut best_delta: Option<Nat> = None;
            for (slot, element) in ELEMENTS.iter().enumerate() {
                let known = known_count(&self.known, *element);
                let current = demand[slot];
                let before = shortfall(current, known);
                let after = shortfall(nadd!(current, count), known);
                let delta = nsub!(after, before);
                if best_delta.is_none_or(|best| delta < best) {
                    best_delta = Some(delta);
                    best_slot = Nat::new(slot as _);
                }
            }
            let slot = best_slot.get() as usize; // bare-int-ok: 容器下标
            demand[slot] = nadd!(demand[slot], count);
        }
        let mut demand_total = any;
        for count in demand.iter() {
            demand_total = nadd!(demand_total, *count);
        }
        if demand_total > self.total() {
            return false;
        }
        let flexible = nadd!(self.omni, folded_total(&self.folded));
        let mut shortfall_total = Nat::ZERO;
        for (slot, element) in ELEMENTS.iter().enumerate() {
            let known = known_count(&self.known, *element);
            shortfall_total = nadd!(shortfall_total, shortfall(demand[slot], known));
        }
        shortfall_total <= flexible
    }

    /// 扣减（确定性，不产生分支；D47/D144）。付不起 -> None。
    ///
    /// - 按 ① SpecificElement → ② SameColor → ③ AnyColor 顺序扣（D168/D169）；
    /// - SpecificElement：先用该色的 known，再用万能；
    /// - SameColor：付哪个颜色由调用方给（same_color）；没给 -> None；
    /// - AnyColor：按固定顺序扣「折叠组 -> 具体色（元素序）-> 万能」（不产生分支）。
    ///
    /// 折叠组未显式展开时不能用于指定色 / 同色（返回 None）。
    pub fn pay(&self, cost: &[CostComponent], same_color: Option<Element>) -> Option<DicePool> {
        validate_cost(cost);
        let mut next = self.clone();
        // ① 指定元素骰
        for component in cost {
            if let CostComponent::SpecificElement(element, count) = *component {
                next = next.pay_color(element, count)?;
            }
        }
        // ② 相同元素骰（颜色由调用方给）
        for component in cost {
            if let CostComponent::SameColor(count) = *component {
                let element = same_color?;
                next = next.pay_color(element, count)?;
            }
        }
        // ③ 任意元素骰
        for component in cost {
            if let CostComponent::AnyColor(count) = *component {
                next = next.pay_any(count)?;
            }
        }
        Some(next)
    }

    // ==================================================================
    // 内部
    // ==================================================================

    fn face_count(&self, face: DieFace) -> Nat {
        match face.element() {
            Some(element) => known_count(&self.known, element),
            None => self.omni,
        }
    }

    fn pay_color(&self, element: Element, count: Nat) -> Option<DicePool> {
        let known = known_count(&self.known, element);
        let from_known = min_nat(known, count);
        let from_omni = nsub!(count, from_known);
        if self.omni < from_omni {
            return None;
        }
        let mut next = self.clone();
        if from_known.get() > 0 {
            subtract_known(&mut next.known, element, from_known);
        }
        next.omni = nsub!(next.omni, from_omni);
        Some(next)
    }

    fn pay_any(&self, count: Nat) -> Option<DicePool> {
        if self.total() < count {
            return None;
        }
        let mut next = self.clone();
        take_any(&mut next, count)?;
        Some(next)
    }

    /// 折叠组的同源列表：(source, color_mass, 该源组内骰子总数)，按 source 排序。
    fn folded_sources(&self) -> Vec<(Nat, [ExactNat128; 7], Nat)> {
        let mut sources: Vec<(Nat, [ExactNat128; 7], Nat)> = Vec::new();
        for group in &self.folded {
            match sources.iter_mut().find(|source| source.0 == group.source) {
                Some(source) => source.2 = nadd!(source.2, group.size),
                None => sources.push((group.source, group.color_mass, group.size)),
            }
        }
        sources.sort_by_key(|source| source.0);
        sources
    }
}

// ======================================================================
// 条件化用的「具体基础元素计数」
// ======================================================================

/// 条件化用的**具体基础元素计数**（7 元素，不含万能）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BasicColors {
    counts: [Nat; 7],
}

impl BasicColors {
    /// 由 7 元素计数构造（ELEMENTS 顺序）。
    pub const fn from_counts(counts: [Nat; 7]) -> BasicColors {
        BasicColors { counts }
    }

    fn from_known(known: &[(Element, Nat)]) -> BasicColors {
        let mut colors = BasicColors { counts: [Nat::ZERO; 7] };
        for (element, count) in known {
            let slot = element_slot(*element).get() as usize; // bare-int-ok: 容器下标
            colors.counts[slot] = nadd!(colors.counts[slot], *count);
        }
        colors
    }

    /// 该元素的个数。
    pub fn count(&self, element: Element) -> Nat {
        self.counts[element_slot(element).get() as usize] // bare-int-ok: 容器下标
    }

    /// 有几「种」基础元素（计数 > 0 的元素个数）。
    pub fn distinct(&self) -> Nat {
        let mut result = Nat::ZERO;
        for count in self.counts.iter() {
            if count.get() > 0 {
                result = nadd!(result, Nat::new(1));
            }
        }
        result
    }

    /// 基础元素骰总数。
    pub fn total(&self) -> Nat {
        let mut total = Nat::ZERO;
        for count in self.counts.iter() {
            total = nadd!(total, *count);
        }
        total
    }
}

// ======================================================================
// 自由函数 / 小工具
// ======================================================================

/// 两个 Nat 取小。
fn min_nat(left: Nat, right: Nat) -> Nat {
    if left < right {
        left
    } else {
        right
    }
}

/// 构造并**约分**一个精确分数（Fraction::new 不约分；加/乘/除的结果才约分）。
fn fraction(num: ExactNat128, den: ExactNat128) -> Fraction {
    Fraction::new(num, den).exact_div(Fraction::ONE)
}

/// 颜色质量总和（精确族，D156）。
fn mass_total(mass: &[ExactNat128; 7]) -> ExactNat128 {
    let mut total = ExactNat128::ZERO;
    for value in mass.iter() {
        total = nadd!(total, *value);
    }
    total
}

/// 规范化颜色质量：总质量 0 时退化为 7 元素均匀（避免 0 概率组；D156：精确族）。
fn normalize_mass(mass: [ExactNat128; 7]) -> [ExactNat128; 7] {
    if mass_total(&mass) != ExactNat128::ZERO {
        return mass;
    }
    [ExactNat128::new(1); 7]
}

/// 同一 source 的组共享第一组的颜色质量（保守：同源组同色，D148）。
fn unify_source_masses(folded: &mut [FoldedGroup]) {
    let length = Nat::new(folded.len() as _);
    let mut index = Nat::ZERO;
    while index < length {
        let position = index.get() as usize; // bare-int-ok: 容器下标
        let source = folded[position].source;
        let mass = folded[position].color_mass;
        let mut probe = position;
        while probe < folded.len() && folded[probe].source == source {
            folded[probe].color_mass = mass;
            probe = probe.saturating_add(1);
        }
        index = Nat::new(probe as _);
    }
}

/// 往具体色计数里加：同色合并，0 忽略。
fn add_known_entry(known: &mut Vec<(Element, Nat)>, element: Element, count: Nat) {
    if count.get() == 0 {
        return;
    }
    if let Some(entry) = known.iter_mut().find(|entry| entry.0 == element) {
        entry.1 = nadd!(entry.1, count);
    } else {
        known.push((element, count));
    }
}

/// 从具体色计数里扣（调用方已保证够）。
fn subtract_known(known: &mut Vec<(Element, Nat)>, element: Element, count: Nat) {
    let Some(position) = known.iter().position(|entry| entry.0 == element) else {
        return;
    };
    let remaining = nsub!(known[position].1, count);
    if remaining.get() == 0 {
        known.remove(position);
    } else {
        known[position].1 = remaining;
    }
}

/// 某具体色的数量。
fn known_count(known: &[(Element, Nat)], element: Element) -> Nat {
    known
        .iter()
        .find(|entry| entry.0 == element)
        .map_or(Nat::ZERO, |entry| entry.1)
}

/// 折叠组骰子总数。
fn folded_total(folded: &[FoldedGroup]) -> Nat {
    let mut total = Nat::ZERO;
    for group in folded {
        total = nadd!(total, group.size);
    }
    total
}

/// 缺口 = max(0, demand - known)。
fn shortfall(demand: Nat, known: Nat) -> Nat {
    if demand > known {
        nsub!(demand, known)
    } else {
        Nat::ZERO
    }
}

/// 计数数组 -> 具体色列表（元素序）。
fn counts_to_known(counts: &[Nat; 7]) -> Vec<(Element, Nat)> {
    let mut known = Vec::new();
    for (slot, element) in ELEMENTS.iter().enumerate() {
        if counts[slot].get() > 0 {
            known.push((*element, counts[slot]));
        }
    }
    known
}

/// 按固定顺序扣 count 颗：折叠组 -> 具体色（元素序）-> 万能；不够 -> None（D144）。
fn take_any(pool: &mut DicePool, count: Nat) -> Option<()> {
    if pool.total() < count {
        return None;
    }
    let mut remaining = count;
    let mut index = Nat::ZERO;
    while (index.get() as usize) < pool.folded.len() && remaining.get() > 0 { // bare-int-ok: 容器下标
        let position = index.get() as usize; // bare-int-ok: 容器下标
        let size = pool.folded[position].size;
        let take = min_nat(size, remaining);
        if take == size {
            pool.folded.remove(position);
        } else {
            pool.folded[position].size = nsub!(size, take);
            index = nadd!(index, Nat::new(1));
        }
        remaining = nsub!(remaining, take);
    }
    for element in ELEMENTS {
        if remaining.get() == 0 {
            break;
        }
        let available = known_count(&pool.known, element);
        let take = min_nat(available, remaining);
        if take.get() > 0 {
            subtract_known(&mut pool.known, element, take);
            remaining = nsub!(remaining, take);
        }
    }
    let take = min_nat(pool.omni, remaining);
    pool.omni = nsub!(pool.omni, take);
    remaining = nsub!(remaining, take);
    if remaining.get() == 0 {
        Some(())
    } else {
        None
    }
}

/// 采样调用点（D149）：把「每种颜色一份精确质量」交给引擎，落一单。
///
/// 这是本块**唯一**消费真实随机数的地方（D149/D151）：
/// 引擎的 prob::pick_index 按 Fraction 占比选下标，随机数由 Rng::next_u128 取。
fn sample_color(mass: &[ExactNat128; 7], rng: &mut Rng) -> Option<Element> {
    let total = mass_total(mass);
    if total == ExactNat128::ZERO {
        return ELEMENTS.first().copied();
    }
    let fractions: Vec<Fraction> = mass
        .iter()
        .map(|value| Fraction::new(*value, total))
        .collect();
    let index = prob::pick_index(&fractions, rng.next_u128())?;
    ELEMENTS.get(index).copied()
}

/// 条件化递归：枚举每个同源组的颜色，收集「具体基础元素计数 -> 无条件概率」（D148 保守版）。
///
/// 概率是精确 Fraction（D156/D158）。
fn collect_assignments(
    remaining_sources: &[(Nat, [ExactNat128; 7], Nat)],
    counts: &mut [Nat; 7],
    probability: Fraction,
    out: &mut Vec<([Nat; 7], Fraction)>,
) {
    let Some((_, mass, size)) = remaining_sources.first() else {
        out.push((*counts, probability));
        return;
    };
    let total = mass_total(mass);
    if total == ExactNat128::ZERO {
        return;
    }
    let rest = &remaining_sources[1..];
    for (slot, _) in ELEMENTS.iter().enumerate() {
        if mass[slot] == ExactNat128::ZERO {
            continue;
        }
        let branch_probability = Fraction::new(mass[slot], total);
        counts[slot] = nadd!(counts[slot], *size);
        collect_assignments(
            rest,
            counts,
            probability.exact_mul(branch_probability),
            out,
        );
        counts[slot] = nsub!(counts[slot], *size);
    }
}

// ======================================================================
// 生成入口（随机 / 指定内容；枚举与折叠在 dice/parse.rs）
// ======================================================================

impl DicePool {
    /// 随机生成 n 颗骰（D139/D140/D148），返回折叠后的分支。
    ///
    /// - count 必须 >= 1（0 视为非法，返回空）；
    /// - limited_to_basic = true：只在七种基础元素里随机（不含万能）；false：可以出万能（D140）；
    /// - distinct = true：n 颗互不相同；仅基础时 n > 7 非法、含万能时 n > 8 非法（先检测再枚举，D148）；
    /// - 上限 16：只生成放得下的 k = min(n, 16 - 当前总数) 颗；池已满 -> 1 支「池不变、概率 1」。
    pub fn generate_random(
        &self,
        count: Nat,
        limited_to_basic: bool,
        distinct: bool,
    ) -> Vec<(DicePool, Fraction)> {
        parse::generate_random(self, count, limited_to_basic, distinct)
    }

    /// 按**有序内容表达式**生成（D141）：数量 = 表达式长度，顺序当前无语义。
    ///
    /// - Element(e)：确定性生成一颗该元素骰；Omni：确定性生成一颗万能骰；
    /// - Any：随机一颗（颜色范围受 limited_to_basic 约束，D141 A2）；
    /// - 上限 16：按书写顺序取前 k 项生成，池已满 -> 1 支「池不变、概率 1」；空表达式非法 -> 空。
    pub fn generate(&self, content: &[DiceSlot], limited_to_basic: bool) -> Vec<(DicePool, Fraction)> {
        parse::generate_content(self, content, limited_to_basic)
    }
}

// ======================================================================
// 测试
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn folded(size: Nat, source: Nat) -> FoldedGroup {
        FoldedGroup {
            size,
            color_mass: [ExactNat128::new(1); 7],
            source,
        }
    }

    fn snapshot(pool: &DicePool, seed: Nat) -> DiceSnapshot {
        pool.all_dice(&mut Rng::from_seed(seed.get() as _))
    }

    fn frac(num: Nat, den: Nat) -> Fraction {
        fraction(
            ExactNat128::new(num.get() as _),
            ExactNat128::new(den.get() as _),
        )
    }

    fn probability_total(branches: &[(DicePool, Fraction)]) -> Fraction {
        branches.iter().map(|(_, probability)| *probability).sum()
    }

    #[test]
    fn add_stops_at_cap() {
        let pool = DicePool::empty().add(DieFace::Pyro, Nat::new(18));
        assert_eq!(pool.total(), DICE_CAP);
        let view = snapshot(&pool, Nat::ZERO);
        assert_eq!(view.known, vec![(Element::Pyro, Nat::new(16))]);
        assert!(view.folded.is_empty());
    }

    #[test]
    fn add_keeps_known_and_omni_separate() {
        let pool = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(2))
            .add(DieFace::Omni, Nat::new(3))
            .add(DieFace::Pyro, Nat::new(1));
        let view = snapshot(&pool, Nat::ZERO);
        assert_eq!(pool.total(), Nat::new(6));
        assert_eq!(view.known, vec![(Element::Pyro, Nat::new(3))]);
        assert_eq!(view.omni, Nat::new(3));
        assert!(view.expanded.is_empty());
    }

    #[test]
    fn remove_requires_enough_of_that_face() {
        let pool = DicePool::empty().add(DieFace::Pyro, Nat::new(2));
        assert!(pool.remove(DieFace::Pyro, Nat::new(3)).is_none());
        let after = pool.remove(DieFace::Pyro, Nat::new(2)).expect("够扣");
        assert_eq!(after.total(), Nat::ZERO);
        assert!(after.remove(DieFace::Omni, Nat::new(1)).is_none());
    }

    #[test]
    fn constructor_normalizes_and_caps() {
        let pool = DicePool::new(
            Nat::new(20),
            vec![folded(Nat::new(2), Nat::new(1)), folded(Nat::new(0), Nat::new(1))],
            vec![
                (Element::Hydro, Nat::new(2)),
                (Element::Pyro, Nat::new(1)),
                (Element::Hydro, Nat::new(1)),
            ],
        );
        assert_eq!(pool.total(), DICE_CAP);
        let view = snapshot(&pool, Nat::ZERO);
        assert_eq!(view.omni, Nat::new(16));
        assert!(view.known.is_empty());
        assert!(view.folded.is_empty());
    }

    #[test]
    fn convert_moves_count_without_changing_total() {
        let pool = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(2))
            .add(DieFace::Omni, Nat::new(1));
        let converted = pool
            .convert(Some(DieFace::Pyro), Nat::new(1), DieFace::Omni)
            .expect("够转");
        assert_eq!(converted.total(), pool.total());
        let view = snapshot(&converted, Nat::ZERO);
        assert_eq!(view.omni, Nat::new(2));
        assert_eq!(view.known, vec![(Element::Pyro, Nat::new(1))]);
        assert!(pool
            .convert(Some(DieFace::Cryo), Nat::new(1), DieFace::Omni)
            .is_none());
    }

    #[test]
    fn convert_any_source_uses_fixed_order() {
        // 任意来源固定顺序：折叠组 -> 具体色（元素序）-> 万能（D144 文档里写死）。
        let pool = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(1))
            .add(DieFace::Hydro, Nat::new(1))
            .add(DieFace::Omni, Nat::new(1));
        let converted = pool.convert(None, Nat::new(1), DieFace::Cryo).expect("够转");
        let view = snapshot(&converted, Nat::ZERO);
        assert_eq!(
            view.known,
            vec![
                (Element::Hydro, Nat::new(1)),
                (Element::Cryo, Nat::new(1)),
            ]
        );
        assert_eq!(view.omni, Nat::new(1));
        assert_eq!(converted.total(), pool.total());
    }

    #[test]
    fn convert_does_not_use_folded_groups() {
        let pool = DicePool::new(
            Nat::ZERO,
            vec![folded(Nat::new(2), Nat::new(1))],
            Vec::new(),
        );
        assert!(pool
            .convert(Some(DieFace::Pyro), Nat::new(1), DieFace::Omni)
            .is_none());
    }

    #[test]
    fn expand_source_splits_into_seven_colors() {
        let pool = DicePool::new(
            Nat::ZERO,
            vec![folded(Nat::new(2), Nat::new(1))],
            Vec::new(),
        );
        let branches = pool.expand_source(Nat::new(1));
        assert_eq!(branches.len(), 7);
        assert_eq!(probability_total(&branches), Fraction::ONE);
        for (branch, probability) in &branches {
            assert_eq!(*probability, frac(Nat::new(1), Nat::new(7)));
            assert_eq!(branch.total(), Nat::new(2));
            let view = snapshot(branch, Nat::ZERO);
            assert!(view.folded.is_empty());
            assert_eq!(view.known.len(), 1);
            assert_eq!(view.known.first().expect("一组").1, Nat::new(2));
        }
    }

    #[test]
    fn expand_source_moves_same_source_groups_together() {
        let pool = DicePool::new(
            Nat::ZERO,
            vec![
                folded(Nat::new(1), Nat::new(9)),
                folded(Nat::new(2), Nat::new(9)),
            ],
            Vec::new(),
        );
        let branches = pool.expand_source(Nat::new(9));
        assert_eq!(branches.len(), 7);
        for (branch, _) in &branches {
            let view = snapshot(branch, Nat::ZERO);
            assert_eq!(view.known.len(), 1);
            assert_eq!(view.known.first().expect("一组").1, Nat::new(3));
        }
    }

    #[test]
    fn expand_source_uses_stored_mass_not_uniform() {
        let mut group = folded(Nat::new(1), Nat::ZERO);
        group.color_mass = [
            ExactNat128::new(3),
            ExactNat128::new(1),
            ExactNat128::new(1),
            ExactNat128::new(1),
            ExactNat128::new(1),
            ExactNat128::new(1),
            ExactNat128::new(1),
        ];
        let pool = DicePool::new(Nat::ZERO, vec![group], Vec::new());
        let branches = pool.expand_source(Nat::ZERO);
        let pyro = branches
            .iter()
            .find(|(branch, _)| {
                snapshot(branch, Nat::ZERO)
                    .known
                    .first()
                    .is_some_and(|entry| entry.0 == Element::Pyro)
            })
            .expect("有火支");
        assert_eq!(pyro.1, frac(Nat::new(1), Nat::new(3)));
        assert_eq!(probability_total(&branches), Fraction::ONE);
    }

    #[test]
    fn expand_source_merges_with_existing_known_color() {
        let pool = DicePool::new(
            Nat::ZERO,
            vec![folded(Nat::new(2), Nat::new(1))],
            vec![(Element::Pyro, Nat::new(1))],
        );
        let branches = pool.expand_source(Nat::new(1));
        assert_eq!(branches.len(), 7);
        for (branch, _) in &branches {
            let view = snapshot(branch, Nat::ZERO);
            let pyro = view
                .known
                .iter()
                .find(|entry| entry.0 == Element::Pyro)
                .expect("火骰一直在");
            if view.known.len() == 1 {
                assert_eq!(pyro.1, Nat::new(3));
            } else {
                assert_eq!(view.known.len(), 2);
                assert_eq!(pyro.1, Nat::new(1));
            }
        }
    }

    #[test]
    fn expand_unknown_source_is_empty() {
        let pool = DicePool::new(Nat::ZERO, vec![folded(Nat::new(1), Nat::new(1))], Vec::new());
        assert!(pool.expand_source(Nat::new(2)).is_empty());
    }

    #[test]
    fn condition_enumerates_hidden_joint_colors() {
        let pool = DicePool::new(
            Nat::ZERO,
            vec![folded(Nat::new(2), Nat::new(1))],
            vec![(Element::Pyro, Nat::new(1))],
        );
        let branches = pool.condition(&|colors: &BasicColors| colors.distinct() >= Nat::new(2));
        assert_eq!(branches.len(), 6);
        assert_eq!(probability_total(&branches), frac(Nat::new(6), Nat::new(7)));
        let pyro_branch =
            pool.condition(&|colors: &BasicColors| colors.count(Element::Pyro) >= Nat::new(3));
        assert_eq!(pyro_branch.len(), 1);
        assert_eq!(pyro_branch.first().expect("一支").1, frac(Nat::new(1), Nat::new(7)));
        assert!(pyro_branch.first().expect("一支").0.folded.is_empty());
    }

    #[test]
    fn condition_without_folded_is_deterministic() {
        let pool = DicePool::empty().add(DieFace::Pyro, Nat::new(2));
        let hit = pool.condition(&|colors: &BasicColors| colors.count(Element::Pyro) >= Nat::new(2));
        assert_eq!(hit.len(), 1);
        assert_eq!(hit.first().expect("一支").1, Fraction::ONE);
        let miss = pool.condition(&|colors: &BasicColors| colors.count(Element::Pyro) >= Nat::new(3));
        assert!(miss.is_empty());
    }

    #[test]
    fn condition_always_true_keeps_folded_groups() {
        let pool = DicePool::new(Nat::ZERO, vec![folded(Nat::new(2), Nat::new(1))], Vec::new());
        // 条件只看基础元素总数，与折叠组颜色无关（恒真）-> 折叠组保留、概率 1。
        let branches = pool.condition(&|colors: &BasicColors| colors.total() >= Nat::new(2));
        assert_eq!(branches.len(), 1);
        assert_eq!(branches.first().expect("一支").1, Fraction::ONE);
        assert_eq!(branches.first().expect("一支").0, pool);
    }

    #[test]
    fn can_afford_uses_omni_and_possible_folded() {
        let with_omni = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(1))
            .add(DieFace::Omni, Nat::new(1));
        assert!(with_omni.can_afford(&[CostComponent::SpecificElement(Element::Pyro, Nat::new(2))]));
        let folded_only = DicePool::new(
            Nat::ZERO,
            vec![folded(Nat::new(2), Nat::new(1))],
            Vec::new(),
        );
        assert!(folded_only.can_afford(&[CostComponent::SpecificElement(Element::Pyro, Nat::new(2))]));
        assert!(folded_only.can_afford(&[CostComponent::SameColor(Nat::new(2))]));
        assert!(!folded_only.can_afford(&[CostComponent::AnyColor(Nat::new(3))]));
    }

    #[test]
    fn pay_is_deterministic_and_needs_same_color() {
        let pool = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(1))
            .add(DieFace::Omni, Nat::new(1));
        let paid = pool
            .pay(&[CostComponent::SpecificElement(Element::Pyro, Nat::new(2))], None)
            .expect("够付");
        assert_eq!(paid.total(), Nat::ZERO);
        assert!(pool
            .pay(&[CostComponent::SameColor(Nat::new(1))], None)
            .is_none());
        let paid = pool
            .pay(&[CostComponent::SameColor(Nat::new(1))], Some(Element::Pyro))
            .expect("够付");
        assert_eq!(paid.total(), Nat::new(1));
    }

    #[test]
    fn pay_does_not_use_unexpanded_folded_groups() {
        let pool = DicePool::new(Nat::ZERO, vec![folded(Nat::new(2), Nat::new(1))], Vec::new());
        assert!(pool.can_afford(&[CostComponent::SpecificElement(Element::Pyro, Nat::new(2))]));
        assert!(pool
            .pay(&[CostComponent::SpecificElement(Element::Pyro, Nat::new(2))], None)
            .is_none());
    }

    #[test]
    fn pay_any_color_may_consume_folded_groups() {
        let pool = DicePool::new(Nat::ZERO, vec![folded(Nat::new(2), Nat::new(1))], Vec::new());
        let paid = pool.pay(&[CostComponent::AnyColor(Nat::new(1))], None).expect("够付");
        let view = snapshot(&paid, Nat::ZERO);
        assert_eq!(paid.total(), Nat::new(1));
        assert_eq!(view.folded.first().expect("还剩一组").size, Nat::new(1));
    }

    #[test]
    fn all_dice_returns_owned_snapshot() {
        let pool = DicePool::new(Nat::ZERO, vec![folded(Nat::new(2), Nat::new(1))], Vec::new());
        let first = snapshot(&pool, Nat::ZERO);
        let same_seed = snapshot(&pool, Nat::ZERO);
        let other_seed = snapshot(&pool, Nat::new(1));
        assert_eq!(first, same_seed);
        // 快照保留折叠组（D148），并给出这次查询采样展开出的具体色（D146）。
        assert_eq!(first.folded.len(), 1);
        assert_eq!(first.expanded.len(), 1);
        let expanded = first.expanded.first().expect("展开有值");
        assert_eq!(expanded.1, Nat::new(2));
        assert!(ELEMENTS.contains(&expanded.0));
        assert_eq!(other_seed.expanded.len(), 1);
        assert_eq!(first.expanded, same_seed.expanded);
    }

    fn spec(element: Element, count: Nat) -> CostComponent {
        CostComponent::SpecificElement(element, count)
    }

    #[test]
    fn delta_increase_prefers_specific_element() {
        // D172：有 ① ② ③ 时 +1 -> 只加 ①。
        let cost = [
            spec(Element::Pyro, Nat::new(1)),
            CostComponent::SameColor(Nat::new(2)),
            CostComponent::AnyColor(Nat::new(1)),
        ];
        let after = apply_delta(&cost, CostDelta::Increase { amount: Nat::new(1) });
        assert_eq!(
            after,
            vec![
                spec(Element::Pyro, Nat::new(2)),
                CostComponent::SameColor(Nat::new(2)),
                CostComponent::AnyColor(Nat::new(1)),
            ]
        );
    }

    #[test]
    fn delta_increase_falls_back_to_same_color() {
        // D172：没有 ① 但有 ② -> +1 全加 ②。
        let cost = [CostComponent::SameColor(Nat::new(2)), CostComponent::AnyColor(Nat::new(1))];
        let after = apply_delta(&cost, CostDelta::Increase { amount: Nat::new(1) });
        assert_eq!(
            after,
            vec![CostComponent::SameColor(Nat::new(3)), CostComponent::AnyColor(Nat::new(1))]
        );
    }

    #[test]
    fn delta_increase_falls_back_to_any_color() {
        // D172：只有 ③ -> +1 全加 ③。
        let cost = [CostComponent::AnyColor(Nat::new(2))];
        let after = apply_delta(&cost, CostDelta::Increase { amount: Nat::new(1) });
        assert_eq!(after, vec![CostComponent::AnyColor(Nat::new(3))]);
    }

    #[test]
    fn delta_increase_on_empty_cost_creates_same_color() {
        // D178：0 费费用 +1 -> 新建 ② SameColor(1)。
        let after = apply_delta(&[], CostDelta::Increase { amount: Nat::new(1) });
        assert_eq!(after, vec![CostComponent::SameColor(Nat::new(1))]);
    }

    #[test]
    fn delta_increase_on_empty_cost_creates_same_color_two() {
        // D178：0 费费用 +2 -> 新建 ② SameColor(2)。
        let after = apply_delta(&[], CostDelta::Increase { amount: Nat::new(2) });
        assert_eq!(after, vec![CostComponent::SameColor(Nat::new(2))]);
    }

    #[test]
    fn delta_decrease_starts_with_specific_element() {
        // D170：有 ① 时 -1 只减 ①。
        let cost = [
            spec(Element::Pyro, Nat::new(2)),
            CostComponent::SameColor(Nat::new(1)),
            CostComponent::AnyColor(Nat::new(1)),
        ];
        let after = apply_delta(&cost, CostDelta::Decrease { amount: Nat::new(1) });
        assert_eq!(
            after,
            vec![
                spec(Element::Pyro, Nat::new(1)),
                CostComponent::SameColor(Nat::new(1)),
                CostComponent::AnyColor(Nat::new(1)),
            ]
        );
    }

    #[test]
    fn delta_decrease_crosses_classes_in_order() {
        // D170：① 减到 0 后再减 ②。
        let cost = [
            spec(Element::Pyro, Nat::new(1)),
            CostComponent::SameColor(Nat::new(2)),
            CostComponent::AnyColor(Nat::new(1)),
        ];
        let after = apply_delta(&cost, CostDelta::Decrease { amount: Nat::new(2) });
        assert_eq!(
            after,
            vec![CostComponent::SameColor(Nat::new(1)), CostComponent::AnyColor(Nat::new(1))]
        );
    }

    #[test]
    fn delta_decrease_reaches_any_color() {
        // D170：② 减到 0 后再减 ③。
        let cost = [CostComponent::SameColor(Nat::new(1)), CostComponent::AnyColor(Nat::new(2))];
        let after = apply_delta(&cost, CostDelta::Decrease { amount: Nat::new(2) });
        assert_eq!(after, vec![CostComponent::AnyColor(Nat::new(1))]);
    }

    #[test]
    fn delta_decrease_clamps_to_zero() {
        let after = apply_delta(
            &[CostComponent::AnyColor(Nat::new(1))],
            CostDelta::Decrease { amount: Nat::new(5) },
        );
        assert!(after.is_empty());
    }

    #[test]
    fn delta_zero_is_noop() {
        let cost = [CostComponent::AnyColor(Nat::new(1))];
        assert_eq!(
            apply_delta(&cost, CostDelta::Increase { amount: Nat::ZERO }),
            cost.to_vec()
        );
        assert_eq!(
            apply_delta(&cost, CostDelta::Decrease { amount: Nat::ZERO }),
            cost.to_vec()
        );
    }

    #[test]
    #[should_panic(expected = "① 指定元素骰只能是一种元素")]
    fn validate_cost_rejects_two_different_specific_elements() {
        validate_cost(&[
            spec(Element::Pyro, Nat::new(1)),
            spec(Element::Hydro, Nat::new(1)),
        ]);
    }

    #[test]
    fn pay_three_classes() {
        let specific = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(2))
            .pay(&[spec(Element::Pyro, Nat::new(2))], None)
            .expect("指定元素够付");
        assert_eq!(specific.total(), Nat::ZERO);
        let same = DicePool::empty()
            .add(DieFace::Hydro, Nat::new(2))
            .pay(&[CostComponent::SameColor(Nat::new(2))], Some(Element::Hydro))
            .expect("同色够付");
        assert_eq!(same.total(), Nat::ZERO);
        let any = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(1))
            .add(DieFace::Hydro, Nat::new(1))
            .pay(&[CostComponent::AnyColor(Nat::new(2))], None)
            .expect("任意够付");
        assert_eq!(any.total(), Nat::ZERO);
    }

    #[test]
    fn pay_uses_class_order_not_input_order() {
        // 池：火 2 水 1；费用**输入顺序**写成 [③ 任意1, ① 火2]。
        // 正确顺序（①→③）先付火 2、再任意付水 1 -> 成功且池空；
        // 若跟随输入顺序先任意（会花火）就会失败。
        let pool = DicePool::empty()
            .add(DieFace::Pyro, Nat::new(2))
            .add(DieFace::Hydro, Nat::new(1));
        let cost = [CostComponent::AnyColor(Nat::new(1)), spec(Element::Pyro, Nat::new(2))];
        let paid = pool.pay(&cost, None).expect("按类顺序应够付");
        assert_eq!(paid.total(), Nat::ZERO);
    }

    #[test]
    fn pay_fails_when_insufficient() {
        let pool = DicePool::empty().add(DieFace::Pyro, Nat::new(1));
        assert!(pool
            .pay(&[spec(Element::Pyro, Nat::new(2))], None)
            .is_none());
        assert!(pool.pay(&[CostComponent::AnyColor(Nat::new(2))], None).is_none());
        assert!(pool
            .pay(&[CostComponent::SameColor(Nat::new(1))], None)
            .is_none());
    }
}