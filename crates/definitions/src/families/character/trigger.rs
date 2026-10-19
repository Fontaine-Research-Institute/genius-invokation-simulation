//! 角色族的**第一份真实时机键 + 触发式数值操作**（D185）。
//!
//! 用户口径（D185）：「调度具体，由对应的 families(types) 定义，比如生命值变动，就是角色家庭去
//! 主动调用生命值变动触发的效果」。所以：
//! - **时机键由角色族自己定义**（`CharacterTiming`；通用层 `timeline` 不认识任何具体时机）；
//! - 角色族在**自己的数值操作**里**主动派发**（`damage_with_triggers` 在 `damage.rs`；
//!   `hp_sub_with_triggers` / `hp_add_with_triggers` / `heal_with_triggers` /
//!   `defeat_with_triggers` / `avoid_defeat_with_triggers` 在本文件）；
//! - **派发只排序、不执行**（D186）：产出待执行列表，由调用方执行。
//!
//! 数值语义**完全复用** `character.rs`（`hp_add` / `hp_sub` / `hp_set`；本文件不改它的语义）。
//!
//! ## 六个时机键（依据：规则书 / 卡面 / 权威关键字）
//!
//! 前四个由本文件的触发式数值操作主动派发；`SkillUsed`（使用技能时 / 后）的派发入口在
//! `families::character::skill_flow::timing::dispatch_skill`（三档作用域各派发一次、再按结算顺序归并）。
//!
//! - `CharacterTiming::HpChanged` **生命值变动**：规则书·其他规则 击倒「当角色的生命值被降为0，
//!   则在技能或效果结算完成后，角色就会被『击倒』」；特殊词条 护盾「『护盾』所保护的角色受到
//!   伤害时，将会消耗『护盾值』来抵消伤害」；卡面「受到伤害时」「受到伤害后」「治疗后」。
//! - `CharacterTiming::Healed` **治疗后**：规则书未单列条目；卡面「治疗后 / 受治疗后」。
//! - `CharacterTiming::Defeated` **被击倒时**：规则书·其他规则 击倒「此时，角色身上所附属的
//!   装备和状态都会被弃置，充能会被清空」。**击倒后的清理归角色块**（Lead 拍板）：本层只派发时机。
//! - `CharacterTiming::AvoidedDefeat` **免于被击倒**：规则书·其他规则 免于被击倒「此时，角色
//!   不视为被『击倒』过（因此，角色所附属的装备和状态不会被弃置，充能也不会被清空）」。
//!   用户口径：「免于击倒就是当前不视作被击倒，但由于血量不 >1 还会继续尝试被击倒，所以会治疗一点」；
//!   所以**「免于击倒」与「治疗」是两个操作**（Lead 拍板）：本时机只表示「当前不视作被击倒」，
//!   **不内置治疗、不派发 Healed**；治疗由卡牌效果自己组合 `heal_with_triggers`（例如补到 1 点血）。
//!
//! - `CharacterTiming::SkillUsed { phase, scope }` **使用技能时 / 后**：规则书·进行行动 战斗行动
//!   【使用技能】；卡面「使用技能时」（52 处，多为「少花费 1 个元素骰」这类费用修正的时点）
//!   与「使用技能后」（119 处 `generated/data/descriptions.json` 统计）。`phase` = 使用技能时 / 后；
//!   `scope` = 任意技能 / 某一类技能（「使用元素爆发后」10 处、「使用普通攻击后」1 处）/
//!   某个具体技能（「使用 `$[S…]` 后」10 处）。
//!   **特技（K58）与准备技能（K53）的使用不触发这个时机**（规则书 / 关键字原文）。
//!
//! - `CharacterTiming::FormTransformed` **转换形态**：规则书·其他规则 转换形态「转换为其他形态后，
//!   角色的名称、元素类型、标签、技能等都有可能发生改变」；权威关键字 K1013（「女士」-> 「焚尽的
//!   炽炎魔女」：元素 冰 -> 火、只能用转换后技能）、K1014（「失能形态」：不能用原本技能）、
//!   K1012（若陀龙王按汲取元素获得技能）。派发入口在 `families::character::form`
//!   （`transform_with_triggers` / `restore_with_triggers`）：**转换完成后**派发一次，
//!   事件 `element` = 转换后的元素类型。
//!
//! 「生命值变动」与「被击倒」是**两个时机**：规则书原文规定击倒发生在「技能或效果结算完成后」，
//! 所以扣血先派发「生命值变动」，击倒判定（`defeat_with_triggers`）由调用方在结算完成后调用。
//!
//! ## 效果函数与实例过滤（Lead 拍板）
//!
//! - 效果签名 `CharacterEffect` = `fn(&mut CharacterState, &Event, EntityId)`：
//!   前两个参数是家族上下文（角色状态 + 事件），**第三个参数 = `Pending.owner`**；
//! - 条件函数仍是 `fn(&Event) -> bool`，**不带 owner**（D186 固定）；
//! - 同一张卡的多实例都登记进**同一个** `CharacterTriggers`；实例判断放在**效果函数**里：
//!   用 owner 判断「这条触发是不是属于本次事件的主体」（如 `owner == event.who`），不是就直接返回。
//! - 执行由调用方做：`(item.effect)(state, &item.event, item.owner)`；`entry` 由调用方用来定位实例
//!   （如扣可用次数 / 弃置）。便捷执行入口见 `execute_character`。
//!
//! ## 不确定性（D190）
//!
//! 本文件的数值操作与派发都是**确定性**的（生命值变动 / 治疗后 / 被击倒 / 免于被击倒是确定的
//! 结算事实）：不采样、不提前展开不确定量。若某个卡效果需要随机值（随机元素 / 随机目标等），
//! 由该效果在**观测点**展开成「分支 + `Fraction` 概率」再交给调用方，不在注册 / 派发阶段决定；
//! 既有折叠语义（骰子 / 抽牌）复用各自区域，不在本层重造。
//!
//! **观测展开是临时的（D191）**：本文件不把展开结果写进 CharacterState；待执行列表执行完即丢弃；
//! 若某个效果需要在观测点展开成具体分支，应在不再影响其它计算后折叠回紧凑表示（展开 / 回收成对），
//! 不让状态永久膨胀。
//!
//! ## 插入式结算 + 中断 / 恢复（D194 / D196）
//!
//! 用户口径（D196）：「不是栈，是有些效果会先插入一次别的结算再继续原来的结算」。
//!
//! 角色侧约定（机制在 `timeline::Resolution` / `timeline::Frame`；本层**不实现**具体治疗 / 判定；
//! 顺序以 `结算顺序.md` §三「生命值降到 0」那一行为准）：
//! 1. 当前结算用 `CharacterFrames`（= `Resolution<CharacterEffect>`）表示：一帧步骤 + `cursor`；
//! 2. 击倒点 = **中断**：保留帧与 `cursor`（`CharacterFrames` 值本身），不是终止；
//! 3. 击倒那一刻**不插入**「受伤时治疗 1」这类可救回的效果：角色已被击倒，治疗类效果的
//!    **目标已不存在 → 无法执行**（**不是**触发被屏蔽：触发条件仍然成立，只是没有可操作对象）；
//! 4. 若触发**免于被击倒**：先 `avoid_defeat_with_triggers`（只标记「当前不视作被击倒」，
//!    角色重新成为有效目标），**之后**再用卡牌效果自己组合的治疗（如 `heal_with_triggers(补到 1 点)`），
//!    然后 `CharacterFrames::insert(免于击倒 + 治疗的步骤)` 插入子结算，做完**回到 `cursor` 继续**；
//! 5. 若最终确实被击倒：`discard()` 丢弃帧，后续真正终止；
//! 6. debug：`CharacterFrames::snapshot()` 返回只读嵌套状态（当前帧 / 剩余步骤 / 插入点），可打印 / 导出。
//!
//! ## 卡牌怎么登记（示例）
//!
//! ```text
//! // 定义侧：静态登记表（时机 -> 条件函数指针 + 执行函数指针），数据只有一份
//! static TRIGGERS: [CharacterTrigger; 1] = [
//!     CharacterTrigger::new(CharacterTiming::HpChanged, owner, EffectSource::CharacterEquipmentOrStatus,
//!                           entry, on_hp_changed),
//! ];
//! // 初始化：登记进所属家族的注册表（每个牌手一个实例，D34）
//! registry.register_all(&TRIGGERS);
//! ```

use crate::base::{Element, SkillId};
use crate::families::character::character::CharacterState;
use crate::families::character::types::SkillKind;
use crate::timeline::{dispatch, Event, Queued, Resolution, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::{EntityId, Nat};

/// 技能使用的**阶段**（`CharacterTiming::SkillUsed` 的第一个字段）。
///
/// 依据：卡面「使用技能时」（52 处，多为「少花费 1 个元素骰」这类费用修正的时点）与
/// 「使用技能后」（119 处）；规则书·进行行动 战斗行动【使用技能】。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SkillPhase {
    /// 使用技能时（= 使用技能前）：技能效果**结算之前**。
    Before,
    /// 使用技能后：技能效果**结算之后**。
    After,
}

/// 技能时机的**作用域**：这个时机在哪一档技能上成立（`CharacterTiming::SkillUsed` 的第二个字段）。
///
/// 依据（卡面措辞 / `generated/data/descriptions.json` 统计）：任意技能（「使用技能后 / 时」）、
/// 某一类技能（「使用元素爆发后」10 处、「使用普通攻击后」1 处；普通攻击含重击 / 下落攻击，D124）、
/// 某个具体技能（「所附属角色使用 `$[S…]` 后」10 处）。
///
/// 为什么放进时机键：`timeline::Event` 是通用五字段（不含技能 id），触发条件又固定
/// `fn(&Event) -> bool`（D186）—— 技能身份只能由时机键表达；派发时三档都查一遍再归并
/// （`families::character::skill_flow::timing::dispatch_skill`）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SkillScope {
    /// 任意技能（卡面「使用技能后」「使用技能时」）。
    Any,
    /// 某一类技能（卡面「使用元素爆发后」「使用普通攻击后」）。
    Kind(SkillKind),
    /// 某个具体技能（卡面「所附属角色使用 `$[S…]` 后」）。
    Skill(SkillId),
}

/// 角色族的**时机键**（D185：由角色族自己定义；通用层不认识它）。
///
/// 每个值都是「角色族的一类事件」，派发时与登记项里的时机键做相等比较；
/// `SkillUsed` 额外带「阶段 × 作用域」数据（卡面措辞的三档粒度），其余四个是零大小变体。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CharacterTiming {
    /// 生命值变动（受伤 / 治疗 / 直接加减血都算）。
    ///
    /// 依据：规则书·其他规则 击倒「当角色的生命值被降为0，则在技能或效果结算完成后，角色就会被
    /// 『击倒』」；特殊词条 护盾「受到伤害时，将会消耗『护盾值』来抵消伤害」；
    /// 卡面「受到伤害时」「受到伤害后」「治疗后」。
    ///
    /// 方向（掉血 / 回血）由**派发入口**决定：伤害走 `damage_with_triggers`、治疗走
    /// `heal_with_triggers`、裸加减血走 `hp_sub_with_triggers` / `hp_add_with_triggers`。
    HpChanged,
    /// 治疗后（治疗结算完成）。
    ///
    /// 依据：规则书未单列条目；卡面「治疗后 / 受治疗后」。
    Healed,
    /// 被击倒时（生命值 0 且**技能 / 效果结算完成后**）。
    ///
    /// 依据：规则书·其他规则 击倒「当角色的生命值被降为0，则在技能或效果结算完成后，角色就会被
    /// 『击倒』。此时，角色身上所附属的装备和状态都会被弃置，充能会被清空」。
    Defeated,
    /// 免于被击倒（**当前不视作被击倒**；不含治疗 —— 治疗是另一个操作，Lead 拍板）。
    ///
    /// 依据：规则书·其他规则 免于被击倒「此时，角色不视为被『击倒』过（因此，角色所附属的装备和
    /// 状态不会被弃置，充能也不会被清空）」。
    /// 用户口径：「免于击倒就是当前不视作被击倒，但由于血量不 >1 还会继续尝试被击倒，所以会治疗一点」。
    /// 所以本时机**只标记「当前不视作被击倒」**：不内置治疗、不派发 `Healed`；
    /// 卡牌效果自己组合 `heal_with_triggers`（例如补到 1 点血）。
    AvoidedDefeat,
    /// **使用技能时 / 后**（卡面「使用技能时」「使用技能后」；规则书·进行行动 战斗行动【使用技能】）。
    ///
    /// - `phase`：使用技能时（技能效果结算之前，费用修正类效果的时点）/ 使用技能后（技能效果结算完成后）；
    /// - `scope`：任意技能 / 某一类技能 / 某个具体技能（卡面措辞的三档粒度）。
    ///
    /// 为什么带数据而不是拆成几个零大小变体：卡面同时存在「使用技能后」「使用元素爆发后」
    /// 「使用 `$[S…]` 后」，抽象成「阶段 × 作用域」后**一个变体**就能表达；派发时按三档作用域
    /// 各查一次，再按结算顺序归并（`families::character::skill_flow::timing::dispatch_skill`）。
    ///
    /// **特技（K58）与准备技能（K53）的使用不触发这个时机**：
    /// 特殊词条 特技「角色使用『特技』时不视为使用技能，无法触发『使用技能后』……」；
    /// 权威关键字 K53 准备技能「此类需要准备的技能，无法触发『使用技能后』……」。
    SkillUsed {
        /// 阶段（使用技能时 / 后）。
        phase: SkillPhase,
        /// 作用域（任意 / 某一类 / 某个具体技能）。
        scope: SkillScope,
    },
    /// **转换形态**（规则书·其他规则 转换形态；权威关键字 K1013 / K1014 / K1012）。
    ///
    /// 依据：规则书·其他规则 转换形态「部分角色可以通过一些效果『转换形态』。转换为其他形态后，
    /// 角色的名称、元素类型、标签、技能等都有可能发生改变」；
    /// 权威关键字 K1013「『焚尽的炽炎魔女』」（女士 永久转换；元素 冰 -> 火；只能用转换后技能）、
    /// K1014「『失能形态』」（不能用原本技能）、K1012「汲取对应元素的力量」（按汲取元素获得技能）。
    ///
    /// 派发入口在 `families::character::form`：`transform_with_triggers` /
    /// `restore_with_triggers`（**转换完成后**派发一次；事件带转换后的元素类型）。
    /// 形态效果要读写当前形态，所以登记在 `form::FormTriggers`
    /// （效果签名 `fn(&mut FormState, &Event, EntityId)`），与 `CharacterTriggers` 分开
    /// （待接入：统一「角色状态 + 形态」上下文）。
    FormTransformed,
}

impl CharacterTiming {
    /// 「使用技能时 / 后」的时机键（= `SkillUsed { phase, scope }`）；便于定义侧写常量。
    pub const fn skill_used(phase: SkillPhase, scope: SkillScope) -> CharacterTiming {
        CharacterTiming::SkillUsed { phase, scope }
    }
}

/// 角色族效果函数指针：`fn(&mut 角色状态, &Event, owner)`。
///
/// - 前两个参数 = 家族上下文（本事件主体的角色状态 + 事件）；
/// - 第三个参数 = `Pending.owner`（这条触发挂在哪个实例上），用于**实例级过滤**：
///   用 owner 判断「这条触发是不是属于本次事件的主体」，不是就直接返回。
pub type CharacterEffect = fn(&mut CharacterState, &Event, EntityId);

/// 角色族的触发登记项（时机键 + 条件 + 效果 + 排序标识；D185 / D186）。
pub type CharacterTrigger = Trigger<CharacterTiming, CharacterEffect>;

/// 角色族触发注册表（每个牌手一个**具名实例**，D34；同一张卡的多实例都登记在同一张表里）。
pub type CharacterTriggers = TriggerRegistry<CharacterTiming, CharacterEffect>;

/// 一条**待执行**（owned 版本；事件按值带走，便于从纯数值操作里返回）。由调用方执行。
///
/// = `timeline::Queued<CharacterEffect>`（数据只有一份；字段：effect / event / owner /
/// source_kind / entry）。
pub type CharacterPending = Queued<CharacterEffect>;

/// 角色族的**结算帧栈**（D194 / D196：插入式结算；中断 = 保留帧与 cursor）。
///
/// = `timeline::Resolution<CharacterEffect>`：`insert` 插入子结算（做完回到插入点），
/// `next_step` 逐步推进，`discard` 丢弃（确实被击倒），`snapshot` 导出只读嵌套状态。
pub type CharacterFrames = Resolution<CharacterEffect>;

/// 主动派发：从角色族注册表里取出「这个时机、条件成立」的登记项，按结算顺序排成**待执行**。
///
/// 排序 / 条件 / 稳定顺序全部复用 `timeline::dispatch`（规则书·其他规则·结算顺序）；
/// 本函数只把借用的 `Pending` 转成 owned（事件按值带走），**不执行效果**。
pub fn dispatch_character(
    triggers: &CharacterTriggers,
    timing: CharacterTiming,
    event: &Event,
) -> Vec<CharacterPending> {
    dispatch(triggers, timing, event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}

/// 执行一条待执行列表（**调用方显式调用**；`timeline` / 本模块的派发永不执行）。
///
/// `state` = 本次事件主体的角色状态；效果函数自己用 `owner` 判断是否真的生效
/// （同一张卡多实例时只有属于主体的那一条会生效）。
pub fn execute_character(pending: &[CharacterPending], state: &mut CharacterState) {
    for item in pending {
        (item.effect)(state, &item.event, item.owner);
    }
}

// ======================================================================
// 事件构造（统一口径，避免各卡自己拼字段）
// ======================================================================

/// 「生命值变动」事件：`who` / `target` 都是这个角色，`amount` = 变动量，`source` = 来源。
///
/// 元素只在伤害导致的变动里给（附着 / 反应类触发要用）；纯数值加减血为 `None`。
pub fn hp_changed_event(
    owner: EntityId,
    amount: Nat,
    element: Option<Element>,
    source: Option<EntityId>,
) -> Event {
    Event::new(Some(owner), Some(owner), amount, element, source)
}

/// 「治疗后」事件：`who` / `target` 都是被治疗的角色，`amount` = 治疗量。
pub fn healed_event(owner: EntityId, amount: Nat, source: Option<EntityId>) -> Event {
    Event::new(Some(owner), Some(owner), amount, None, source)
}

/// 「被击倒时」事件：谁被击倒。
pub fn defeated_event(owner: EntityId) -> Event {
    Event::of(owner)
}

/// 「免于被击倒」事件：谁「当前不视作被击倒」。
///
/// 不带数值：治疗是另一个操作（Lead 拍板），不在本事件里表达。
pub fn avoided_defeat_event(owner: EntityId) -> Event {
    Event::of(owner)
}

// ======================================================================
// 触发式数值操作（数值语义复用 character.rs，本文件只加派发）
// ======================================================================

/// 一次带触发的角色数值操作结果。
#[derive(Clone, Debug)]
pub struct CharacterResolution {
    /// 操作后的角色状态（与直接调 `character.rs` 的纯函数结果一致）。
    pub state: CharacterState,
    /// 派发出来的待执行（由调用方执行；本层不执行）。
    pub pending: Vec<CharacterPending>,
}

/// 生命值**减少**（`CharacterState::hp_sub` 的数值语义）+ 派发「生命值变动」。
pub fn hp_sub_with_triggers(
    state: &CharacterState,
    owner: EntityId,
    amount: Nat,
    source: Option<EntityId>,
    triggers: &CharacterTriggers,
) -> CharacterResolution {
    let next = state.hp_sub(amount);
    let event = hp_changed_event(owner, amount, None, source);
    let pending = dispatch_character(triggers, CharacterTiming::HpChanged, &event);
    CharacterResolution {
        state: next,
        pending,
    }
}

/// 生命值**增加**（`CharacterState::hp_add` 的数值语义）+ 派发「生命值变动」。
pub fn hp_add_with_triggers(
    state: &CharacterState,
    owner: EntityId,
    amount: Nat,
    source: Option<EntityId>,
    triggers: &CharacterTriggers,
) -> CharacterResolution {
    let next = state.hp_add(amount);
    let event = hp_changed_event(owner, amount, None, source);
    let pending = dispatch_character(triggers, CharacterTiming::HpChanged, &event);
    CharacterResolution {
        state: next,
        pending,
    }
}

/// **治疗**（`CharacterState::hp_add` 的数值语义）+ 依次派发「生命值变动」-> 「治疗后」。
///
/// 顺序依据：`结算顺序.md` §三「生命值降到 0」——免于击倒使角色重新成为有效目标后，才治疗，
/// 然后从 cursor 继续（与 `avoid_defeat_with_triggers` / `CharacterFrames` 组合使用）。
///
/// 顺序约定：事实（生命值变动）先于结果（治疗后）；规则书未规定两个时机之间的先后。
pub fn heal_with_triggers(
    state: &CharacterState,
    owner: EntityId,
    amount: Nat,
    source: Option<EntityId>,
    triggers: &CharacterTriggers,
) -> CharacterResolution {
    let next = state.hp_add(amount);
    let changed = hp_changed_event(owner, amount, None, source);
    let healed = healed_event(owner, amount, source);
    let mut pending = dispatch_character(triggers, CharacterTiming::HpChanged, &changed);
    pending.extend(dispatch_character(
        triggers,
        CharacterTiming::Healed,
        &healed,
    ));
    CharacterResolution {
        state: next,
        pending,
    }
}

/// **被击倒**判定（生命值 0 且技能 / 效果结算完成时由调用方调用）+ 派发「被击倒时」。
///
/// 生命值不为 0 -> `None`（不派发）；为 0 -> 返回待执行（**不改状态**：装备 / 状态的弃置与
/// 充能清空**归角色块**（Lead 拍板），本层只派发时机）。
///
/// **插入点**（`结算顺序.md` §三「生命值降到 0」）：中断当前结算 -> 免于击倒判定 -> 治疗后
/// 从 cursor 继续；确实被击倒 -> 丢弃帧。
///
/// **D194 / D195 / D196**：击倒点不会插入「受伤时治疗 1」这类可救回的效果 —— 角色已被击倒，
/// 治疗类效果的**目标已不存在 → 无法执行**（不是触发被屏蔽，条件仍成立）。
/// 调用方应把当前结算帧栈（`CharacterFrames`）保留下来（中断，帧与 `cursor` 不动）：
/// 有免于被击倒则 `insert(免于击倒 + 治疗)`（角色重新成为有效目标后才补治疗），
/// 做完回到 `cursor` 继续；否则 `discard()`（确实被击倒，终止后续）。
pub fn defeat_with_triggers(
    state: &CharacterState,
    owner: EntityId,
    triggers: &CharacterTriggers,
) -> Option<CharacterResolution> {
    if state.hp() != Nat::ZERO {
        return None;
    }
    let event = defeated_event(owner);
    let pending = dispatch_character(triggers, CharacterTiming::Defeated, &event);
    Some(CharacterResolution {
        state: state.clone(),
        pending,
    })
}

/// **免于被击倒**：只做「**当前不视作被击倒**」并派发 `AvoidedDefeat`（Lead 拍板：与治疗拆开）。
///
/// 插入点见 `结算顺序.md` §三「生命值降到 0」：中断 -> 免于击倒判定 -> 治疗 -> 从 cursor 继续。
///
/// 用户口径：「免于击倒就是当前不视作被击倒，但由于血量不 >1 还会继续尝试被击倒，所以会治疗一点」；
/// 「我们把这两个操作拆开」。所以：
/// - 本函数**不改状态**（生命值仍为 0，之后还会继续尝试被击倒）；
/// - **不内置治疗、不派发 `Healed`**：卡牌效果自己组合 `heal_with_triggers`（例如补到 1 点血）。
///
/// 依据：规则书·其他规则 免于被击倒「角色不视为被『击倒』过（因此装备和状态不会被弃置，
/// 充能也不会被清空）」。
pub fn avoid_defeat_with_triggers(
    state: &CharacterState,
    owner: EntityId,
    triggers: &CharacterTriggers,
) -> CharacterResolution {
    let event = avoided_defeat_event(owner);
    let pending = dispatch_character(triggers, CharacterTiming::AvoidedDefeat, &event);
    CharacterResolution {
        state: state.clone(),
        pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::attachment::{AttachmentAllowSet, ReactionRegistry};
    use crate::families::character::damage::{damage_with_triggers, DamageEnv, ReactionRule};
    use crate::families::character::types::DamageType;
    use crate::rules::ops::order::EffectSource;

    /// 建角色状态（当前生命 / 生命上限；充能 0/2）。
    fn character(hp: Nat, max_hp: Nat) -> CharacterState {
        CharacterState::new(hp, max_hp, Nat::ZERO, Nat::new(2))
    }

    /// 示例效果：**只回应属于本次事件主体的实例** —— 用 owner 判断（Lead 拍板的实例过滤约定）。
    fn heal_one_if_subject(state: &mut CharacterState, event: &Event, owner: EntityId) {
        if event.who == Some(owner) {
            let next = state.hp_add(Nat::new(1));
            *state = next;
        }
    }

    /// 顺序测试用：把生命值置为 1。
    fn set_hp_one(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let next = state.hp_set(Nat::new(1));
        *state = next;
    }

    /// 顺序测试用：生命值 +10。
    fn add_hp_ten(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let next = state.hp_add(Nat::new(10));
        *state = next;
    }

    /// 条件：数值 > 2。
    fn amount_above_two(event: &Event) -> bool {
        event.amount > Nat::new(2)
    }

    /// 注册一条「生命值变动」触发。
    fn registry_with(effect: CharacterEffect, owner: EntityId) -> CharacterTriggers {
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            owner,
            EffectSource::CharacterCard,
            Nat::ZERO,
            effect,
        ));
        triggers
    }

    /// 注册一条指定时机的触发。
    fn registry_for(
        timing: CharacterTiming,
        effect: CharacterEffect,
        owner: EntityId,
    ) -> CharacterTriggers {
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            timing,
            owner,
            EffectSource::CharacterEquipmentOrStatus,
            Nat::ZERO,
            effect,
        ));
        triggers
    }

    /// 空的伤害环境（无反应规则 / 无反应注册表）。
    fn empty_damage_env<'a>(
        rules: &'a [ReactionRule],
        registry: &'a ReactionRegistry,
    ) -> DamageEnv<'a> {
        DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules,
            registry,
        }
    }

    #[test]
    fn hp_sub_with_triggers_keeps_numeric_semantics_and_dispatches() {
        let state = character(Nat::new(10), Nat::new(10));
        let triggers = registry_with(heal_one_if_subject, Nat::new(1));
        let resolution = hp_sub_with_triggers(&state, Nat::new(1), Nat::new(3), None, &triggers);
        // 数值语义与 character.rs 完全一致（本文件不改它的语义）。
        assert_eq!(resolution.state, state.hp_sub(Nat::new(3)));
        assert_eq!(resolution.state.hp(), Nat::new(7));
        // 派发只产出待执行，不执行。
        assert_eq!(resolution.pending.len(), 1);
        assert_eq!(resolution.pending[0].owner, Nat::new(1));
        // 执行由调用方做。
        let mut after = resolution.state.clone();
        execute_character(&resolution.pending, &mut after);
        assert_eq!(after.hp(), Nat::new(8));
    }

    #[test]
    fn hp_add_with_triggers_keeps_numeric_semantics_and_dispatches() {
        let state = character(Nat::new(3), Nat::new(10));
        let triggers = registry_with(heal_one_if_subject, Nat::new(1));
        let resolution = hp_add_with_triggers(&state, Nat::new(1), Nat::new(4), None, &triggers);
        assert_eq!(resolution.state, state.hp_add(Nat::new(4)));
        assert_eq!(resolution.state.hp(), Nat::new(7));
        let mut after = resolution.state.clone();
        execute_character(&resolution.pending, &mut after);
        assert_eq!(after.hp(), Nat::new(8));
    }

    #[test]
    fn heal_dispatches_hp_changed_before_healed() {
        // 治疗本身：hp_add（上限 20）。派发顺序约定：生命值变动 -> 治疗后。
        let state = character(Nat::new(10), Nat::new(20));
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::new(1),
            set_hp_one,
        ));
        triggers.register(Trigger::new(
            CharacterTiming::Healed,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            add_hp_ten,
        ));
        let resolution = heal_with_triggers(&state, Nat::new(1), Nat::new(5), None, &triggers);
        assert_eq!(resolution.state.hp(), Nat::new(15));
        assert_eq!(resolution.pending.len(), 2);
        let mut after = resolution.state.clone();
        execute_character(&resolution.pending, &mut after);
        // 先 HpChanged（置 1）再 Healed（+10）-> 11；反序会得到 1。
        assert_eq!(after.hp(), Nat::new(11));
    }

    #[test]
    fn heal_keeps_the_hp_cap_semantics() {
        // 治疗仍是 hp_add：超过上限钳到上限（character.rs 语义不变）。
        let state = character(Nat::new(8), Nat::new(10));
        let triggers = CharacterTriggers::new();
        let resolution = heal_with_triggers(&state, Nat::new(1), Nat::new(5), None, &triggers);
        assert_eq!(resolution.state, state.hp_add(Nat::new(5)));
        assert_eq!(resolution.state.hp(), Nat::new(10));
    }

    #[test]
    fn defeat_only_dispatches_when_hp_is_zero() {
        let triggers = registry_for(CharacterTiming::Defeated, heal_one_if_subject, Nat::new(1));
        let alive = character(Nat::new(3), Nat::new(10));
        assert!(defeat_with_triggers(&alive, Nat::new(1), &triggers).is_none());

        let down = character(Nat::ZERO, Nat::new(10));
        let resolution =
            defeat_with_triggers(&down, Nat::new(1), &triggers).expect("生命值 0 -> 被击倒时机");
        assert_eq!(resolution.pending.len(), 1);
        // 击倒事件不该改数值（装备/状态/充能的处理归角色区 / 状态层）。
        assert_eq!(resolution.state, down);
        let mut after = resolution.state.clone();
        execute_character(&resolution.pending, &mut after);
        assert_eq!(after.hp(), Nat::new(1));
    }

    #[test]
    fn avoid_defeat_only_marks_not_defeated_and_dispatches() {
        let down = character(Nat::ZERO, Nat::new(10));
        let triggers = registry_for(
            CharacterTiming::AvoidedDefeat,
            heal_one_if_subject,
            Nat::new(1),
        );
        let resolution = avoid_defeat_with_triggers(&down, Nat::new(1), &triggers);
        // 只做「当前不视作被击倒」：血量不变（0 血之后还会继续尝试被击倒）。
        assert_eq!(resolution.state, down);
        assert_eq!(resolution.state.hp(), Nat::ZERO);
        assert_eq!(resolution.pending.len(), 1);
        assert_eq!(resolution.pending[0].event.amount, Nat::ZERO);
        // 待执行里若有效果，那是效果自己做的（不是本操作内置的治疗）。
        let mut after = resolution.state.clone();
        execute_character(&resolution.pending, &mut after);
        assert_eq!(after.hp(), Nat::new(1));
    }

    #[test]
    fn avoid_defeat_does_not_dispatch_healed() {
        // Lead 拍板：AvoidedDefeat 不顺带派发 Healed；治疗是另一个操作。
        let down = character(Nat::ZERO, Nat::new(10));
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::Healed,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            heal_one_if_subject,
        ));
        let resolution = avoid_defeat_with_triggers(&down, Nat::new(1), &triggers);
        assert!(resolution.pending.is_empty());
        assert_eq!(resolution.state, down);
    }

    #[test]
    fn card_composes_avoid_defeat_then_heal_to_one() {
        // 用户口径：「还会继续尝试被击倒，所以会治疗一点」——两个操作由卡牌效果自己组合。
        let down = character(Nat::ZERO, Nat::new(10));
        let triggers = CharacterTriggers::new();
        let resolution = avoid_defeat_with_triggers(&down, Nat::new(1), &triggers);
        // 卡牌效果自己组合「治疗 1 点」：补到 1 血（治疗走独立的 heal_with_triggers）。
        let resolution =
            heal_with_triggers(&resolution.state, Nat::new(1), Nat::new(1), None, &triggers);
        assert_eq!(resolution.state.hp(), Nat::new(1));
        assert!(resolution.pending.is_empty());
    }

    #[test]
    fn same_card_two_instances_only_the_owner_responds() {
        // Lead 拍板的实例过滤：同一张卡两个实例都登记进**同一个**注册表；
        // 条件不带 owner（D186），实例判断在效果函数里用 Pending.owner 做。
        let state = character(Nat::new(10), Nat::new(10));
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            Nat::new(1),
            EffectSource::Summon,
            Nat::ZERO,
            heal_one_if_subject,
        ));
        triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            Nat::new(2),
            EffectSource::Summon,
            Nat::new(1),
            heal_one_if_subject,
        ));
        // 事件主体是 1 号实例：两条登记都命中（时机相同、条件恒真）。
        let resolution = hp_sub_with_triggers(&state, Nat::new(1), Nat::new(2), None, &triggers);
        assert_eq!(resolution.state.hp(), Nat::new(8));
        assert_eq!(resolution.pending.len(), 2, "过滤不在条件里做");
        let mut after = resolution.state.clone();
        execute_character(&resolution.pending, &mut after);
        // 只有 1 号实例真正响应（+1）；两个都响应会是 +2。
        assert_eq!(after.hp(), Nat::new(9));
    }

    #[test]
    fn empty_registry_returns_no_pending_and_keeps_semantics() {
        let state = character(Nat::new(10), Nat::new(10));
        let triggers = CharacterTriggers::new();
        let resolution = hp_sub_with_triggers(&state, Nat::new(1), Nat::new(4), None, &triggers);
        assert!(resolution.pending.is_empty());
        assert_eq!(resolution.state, state.hp_sub(Nat::new(4)));
    }

    #[test]
    fn condition_filters_at_dispatch() {
        let state = character(Nat::new(10), Nat::new(10));
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::with_condition(
            CharacterTiming::HpChanged,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            amount_above_two,
            heal_one_if_subject,
        ));
        // 数值 1 -> 条件不成立，无待执行。
        let small = hp_sub_with_triggers(&state, Nat::new(1), Nat::new(1), None, &triggers);
        assert!(small.pending.is_empty());
        // 数值 3 -> 条件成立。
        let big = hp_sub_with_triggers(&state, Nat::new(1), Nat::new(3), None, &triggers);
        assert_eq!(big.pending.len(), 1);
    }

    #[test]
    fn damage_with_triggers_dispatches_hp_changed_and_keeps_damage_semantics() {
        let reactions = ReactionRegistry::new();
        let rules: [ReactionRule; 0] = [];
        let env = empty_damage_env(&rules, &reactions);
        let state = character(Nat::new(10), Nat::new(10));
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            Nat::new(2),
            EffectSource::CharacterCard,
            Nat::ZERO,
            heal_one_if_subject,
        ));
        // damage.rs 的带触发入口：物理伤害 3（来源 1 -> 目标 2）。
        let result = damage_with_triggers(
            &state,
            Nat::new(1),
            Nat::new(2),
            Nat::new(3),
            DamageType::Physical,
            &env,
            &triggers,
        );
        // 底层 damage 语义完全一致：扣血 3、物理不附着、不反应。
        assert_eq!(result.outcome.state.hp(), Nat::new(7));
        assert!(!result.outcome.attachment_landed);
        assert!(result.outcome.reaction.is_none());
        // 派发「生命值变动」：事件带谁/目标/数值/元素/来源。
        assert_eq!(result.pending.len(), 1);
        let item = result.pending[0];
        assert_eq!(item.event.who, Some(Nat::new(2)));
        assert_eq!(item.event.target, Some(Nat::new(2)));
        assert_eq!(item.event.amount, Nat::new(3));
        assert_eq!(item.event.element, None);
        assert_eq!(item.event.source, Some(Nat::new(1)));
        // 由调用方执行。
        let mut after = result.outcome.state.clone();
        execute_character(&result.pending, &mut after);
        assert_eq!(after.hp(), Nat::new(8));
    }

    #[test]
    fn defeat_point_inserts_avoid_defeat_and_heal_then_resumes() {
        // D196：中断 = 保留结算帧与 cursor；恢复 = 插入「免于击倒 + 治疗」子结算，
        // 做完回到插入点继续原结算。
        let down = character(Nat::ZERO, Nat::new(10));
        let mut triggers = CharacterTriggers::new();
        // 免于被击倒的触发（owner 3）与治疗后的触发（owner 4）各一条示例效果。
        triggers.register(Trigger::new(
            CharacterTiming::AvoidedDefeat,
            Nat::new(3),
            EffectSource::CharacterEquipmentOrStatus,
            Nat::ZERO,
            heal_one_if_subject,
        ));
        triggers.register(Trigger::new(
            CharacterTiming::Healed,
            Nat::new(4),
            EffectSource::CharacterEquipmentOrStatus,
            Nat::new(1),
            heal_one_if_subject,
        ));
        // 原结算：两条待执行步骤（owner 1 / 2，各自事件主体 = 自己）。
        let mut frames = CharacterFrames::new(vec![
            CharacterPending {
                effect: heal_one_if_subject,
                event: Event::of(Nat::new(1)),
                owner: Nat::new(1),
                source_kind: EffectSource::Summon,
                entry: Nat::ZERO,
            },
            CharacterPending {
                effect: heal_one_if_subject,
                event: Event::of(Nat::new(2)),
                owner: Nat::new(2),
                source_kind: EffectSource::Summon,
                entry: Nat::new(1),
            },
        ]);
        // 击倒中断：保留帧与 cursor（原结算还没执行，cursor 仍是 0）。
        assert_eq!(
            frames.current().map(|frame| frame.cursor()),
            Some(Nat::ZERO)
        );
        // 1) 免于击倒：只标记「当前不视作被击倒」（不治疗）。
        let avoid = avoid_defeat_with_triggers(&down, Nat::new(3), &triggers);
        assert_eq!(avoid.pending.len(), 1);
        // 2) 治疗是另一个操作（卡牌效果自己组合）：补到 1 点血。
        let heal = heal_with_triggers(&avoid.state, Nat::new(4), Nat::new(1), None, &triggers);
        // 3) 恢复：把「免于击倒 + 治疗」作为子结算插入，然后回到 cursor 继续原结算。
        let mut before = avoid.pending;
        before.extend(heal.pending);
        frames.insert(before);
        // 4) 逐步推进：子结算先执行，然后回到原帧 cursor（owner 1 / 2 按原顺序）。
        let mut owners = Vec::new();
        let mut steps = Vec::new();
        while let Some(step) = frames.next_step() {
            owners.push(step.owner);
            steps.push(step);
        }
        assert_eq!(
            owners,
            vec![Nat::new(3), Nat::new(4), Nat::new(1), Nat::new(2)],
            "顺序：免于击倒 -> 治疗 -> 回到 cursor 后的原剩余"
        );
        // 执行：从 0 血救回（治疗 +1），再继续原流程（每条效果 +1）。
        let mut state = heal.state;
        execute_character(&steps, &mut state);
        assert_eq!(state.hp(), Nat::new(5));
        assert!(frames.is_finished());
    }

    #[test]
    fn defeat_point_without_avoid_defeat_terminates() {
        // D196：没有免于被击倒 -> 丢弃结算帧（终止，不是继续）。
        let frames = CharacterFrames::new(vec![CharacterPending {
            effect: heal_one_if_subject,
            event: Event::of(Nat::new(1)),
            owner: Nat::new(1),
            source_kind: EffectSource::Summon,
            entry: Nat::ZERO,
        }]);
        assert_eq!(frames.depth(), Nat::new(1));
        assert!(!frames.is_finished());
        frames.discard();
        // 确实被击倒：defeat_with_triggers 照常给出「被击倒时」待执行（由角色块处理清理）。
        let down = character(Nat::ZERO, Nat::new(10));
        let triggers = CharacterTriggers::new();
        assert!(defeat_with_triggers(&down, Nat::new(1), &triggers).is_some());
    }

    #[test]
    fn static_character_trigger_table_works() {
        // 「静态数据 + 函数指针表」：角色族的定义侧静态登记表。
        static TABLE: [CharacterTrigger; 1] = [CharacterTrigger::new(
            CharacterTiming::HpChanged,
            Nat::new(1),
            EffectSource::CharacterCard,
            Nat::ZERO,
            heal_one_if_subject,
        )];
        let mut triggers = CharacterTriggers::new();
        triggers.register_all(&TABLE);
        assert_eq!(triggers.len(), Nat::new(1));
        let event = hp_changed_event(Nat::new(1), Nat::new(2), None, None);
        let pending = dispatch_character(&triggers, CharacterTiming::HpChanged, &event);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].owner, Nat::new(1));
        assert_eq!(pending[0].event, event);
    }
}
