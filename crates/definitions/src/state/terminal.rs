//! **终局判定**（规则书·胜利条件 / 对局回合上限 / 击倒 / 免于被击倒）。
//!
//! ## 依据（唯一权威：规则书 S1–S8 常规模式 60 条）
//!
//! - **胜利条件**（S1.4）：「击倒对方牌手的所有『角色牌』，即可获得胜利！」
//! - **对局回合上限**（S7.7）：「第15回合开始时，如果仍未分出胜负，则双方都会输掉这场对局。」
//! - **击倒**（S7.3）：「当角色的生命值被降为0，则在技能或效果结算完成后，角色就会被『击倒』。」
//! - **免于被击倒**（S7.4）：「此时，角色不视为被『击倒』过。」
//! - **双方同时全灭 = 双方都输**（规则书 S1.4 / S7.7 都没写，**用户 D267 明确拍板**）：
//!   用户原话：「不是一步内结算顺序，而是一步内有人全阵亡不下一步，但是本步执行完」；
//!   实例（用户 / D267）：芙宁娜（白芙）天赋牌 + 普通攻击 可造成双负 → 见「判定时机」。
//!
//! 结算顺序参照工作区 `结算顺序.md` §三（暂定，与规则书 S7.3 / S7.4 一致）：
//! 受到伤害 -> 护盾 / 减伤 -> 扣血 -> 生命值降到 0 时**中断** -> 免于击倒判定 ->
//! 治疗后从 cursor 继续；确实被击倒则丢弃剩余步骤。
//!
//! ## 判定时机：**步边界**（本模块只读，不推进结算）
//!
//! - 本模块是**纯函数**：吃 `&DefState`、返回判定结果，入参一个字节都不改；
//! - **判定时机 = 步边界**（D204「步是原子单位」+ 用户 D267）：
//!   **一步内即使某一方角色区已经被打空，也不在步内提前终止 / 提前判定；本步全部执行完，
//!   在步边界判一次**。步内出现的「只有一方空」只是中间态，**不代表终局**；
//! - **必须在「技能 / 效果结算完成」之后调用**：HP 归 0 本身不是击倒（S7.3），
//!   击倒清理由 `state::flows::defeat`（把角色移出角色区）执行；
//! - 因此「全灭」的判据是**角色区为空**（所有角色牌都已被击倒清理移出），
//!   **不是**「当前 HP > 0 的角色数」。免于击倒窗口里 HP 为 0、角色仍留在角色区 ->
//!   本模块判 [`Outcome::Ongoing`]（角色不视为被击倒过，S7.4）；
//! - **步边界上的三种结果**：
//!   1. 只有一方角色区为空 -> 该方负、对方胜（S1.4）；
//!   2. **双方角色区都为空 -> 双方都输** [`Outcome::BothLose`]（D267；**不是平局**）；
//!   3. 双方都不为空 -> 未分胜负（再按回合上限判，S7.7）；
//! - **双负不是特例**：它只是「步边界判定 + 步内不提前终止」的自然结果 —— 同一步内两边都被
//!   清空，本步执行完在步边界一看：双方角色区都空 -> 双方都输（D267 / D268）。
//!   用户实例：芙宁娜（白芙）天赋牌 + 普通攻击 = 普通攻击加伤打死对面最后一个角色，
//!   同一步内还会扣我方角色 1 点血、打死我方最后一个角色（不需要任何专门的双负判定分支，
//!   见回归测试 `same_step_enemy_damage_plus_self_damage_ends_in_both_lose`）；
//! - **击倒区不参与**：击倒清理会把角色留一份内部记录在 `PlayerDefs::defeated`（供复苏），
//!   本模块只读**角色区**；角色复苏回到角色区后才会影响判定（见下方「复苏」条）。
//!
//! ## 回合计数
//!
//! - `DefState::round()`：0 = 第一回合还没开始；1、2、…；
//! - [`ROUND_LIMIT`] = 15（S7.7「第15回合开始」）；
//! - 推进点唯一：`state::flows::round_begin::begin_round`（调 [`next_round`]，
//!   到 15 封顶）；
//! - 第 15 回合开始仍未分胜负 -> [`Outcome::RoundLimit`]（**双方都输**，S7.7）。
//!   引擎接口 `GameResult::RoundLimit` 表达的就是这个结果。
//!
//! ## 规则书没写清 / 待实测（不自行规定）
//!
//! - **双方同时全灭的胜负**：规则书 S1.4 / S7.7 确实都没写；但**这不是待实测项** ——
//!   用户已明确裁定为**双方都输**（D267，见上「依据」），所以 [`Outcome::BothLose`] 是
//!   按用户口径落地，不是自行规定；
//! - **复苏 / 复活**：复苏操作已实现（`state::flows::revive`：把击倒区的角色放回角色区）。
//!   本模块仍只按**角色区**判：击倒清理移出的角色不在角色区，所以「结算完成 -> 击倒清理 ->
//!   再判」的顺序不会把已经复活 / 还会复活的人误判成「已全灭」；
//!   但**终局判定前是否要先检查击倒区里还有可复苏的角色**，规则书没写（复苏是主动打出的
//!   料理，全灭后对局已经结束，似乎不该有「死后复苏」窗口）-> **待游戏实测**
//!   （留空 #41 / #65）；本模块不自行加检查，复苏后角色回到角色区才会影响判定；
//! - 标准模式是否可能因牌库耗尽等其它原因提前结束：规则书没有这类条款，本模块不判。
//!
//! ## 引擎接口适配（`feature = "rules"`）
//!
//! `engine::rules::GameRules::terminal` 要求 `Option<GameResult<Player>>`。适配层
//! [`DefRules`] 把本模块的 [`Outcome`] 映射过去；`to_move` 读
//! `DefState::first_player`（本回合先手牌手，规则书·宣布结束 / 结算顺序「当前牌手优先」），
//! 先手未知时返回我方占位；`actions` / `evaluate` 仍是求解器接口的**占位**
//! （各归其它块），见各自 TODO。

use crate::state::def_state::DefState;
use gi_genius_invokation_engine::{nadd, Nat};

/// **对局回合上限**：第 15 回合（规则书·其他规则 对局回合上限）。
///
/// 规则书原文：「第15回合开始时，如果仍未分出胜负，则双方都会输掉这场对局。」
pub const ROUND_LIMIT: Nat = Nat::new(15);

/// 回合数是否已经到达 / 超过上限（第 15 回合开始及其后）。
pub fn at_round_limit(round: Nat) -> bool {
    round >= ROUND_LIMIT
}

/// **推进一个回合**（回合开始用）：+1；到 [`ROUND_LIMIT`] 后**不再增长**。
///
/// 为什么封顶：第 15 回合开始时游戏就结束了（S7.7），不存在第 16 回合；封顶是对
/// 「调用了不该再调用的推进」的边界处理，不改变任何规则语义。
pub fn next_round(round: Nat) -> Nat {
    if round < ROUND_LIMIT {
        nadd!(round, Nat::new(1))
    } else {
        round
    }
}

/// **终局判定的结果**（纯判定值；引擎侧映射见 [`DefRules`]）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Outcome {
    /// 对局继续（还没开始第一回合、双方都有角色且未到回合上限）。
    Ongoing,
    /// 我方获胜：敌方所有角色牌都已被击倒（S1.4）。
    MeWins,
    /// 敌方获胜：我方所有角色牌都已被击倒（S1.4）。
    OppWins,
    /// 第 15 回合开始仍未分胜负：**双方都输**（S7.7）。引擎侧 = `GameResult::RoundLimit`。
    RoundLimit,
    /// **双方同时全灭 = 双方都输**（D267 / D268 用户明确：「一步内有人全阵亡不下一步，但是本步执行完」）。
    ///
    /// 不是特例：同一步内双方各自被打空，步执行完在步边界判 -> 自然就是双负。
    ///
    /// - 步边界上双方角色区都为空 -> 双负；**不是平局**，也不是任何一方胜；
    /// - 规则书 S1.4 / S7.7 没有这一条，这是**用户裁定**（不是本模块自编）；
    /// - 引擎侧 = `GameResult::BothLose`。
    BothLose,
}

impl Outcome {
    /// 是否已经终局（[`Outcome::Ongoing`] 之外都是）。
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Outcome::Ongoing)
    }
}

/// **终局判定**（纯函数：只读 `&DefState`，不改任何区）。
///
/// 判定顺序（语义即规则书条文顺序 + D267）：
/// 1. 回合数为 0（第一回合还没开始 / 空建局）-> [`Outcome::Ongoing`]；
/// 2. 双方角色区都为空 -> [`Outcome::BothLose`]（D267：双方都输）；
/// 3. 某一方角色区为空 -> 对方获胜（S1.4）；
/// 4. 双方都有角色且回合数到 15 -> [`Outcome::RoundLimit`]（S7.7「仍未分出胜负」）；
/// 5. 其余 -> [`Outcome::Ongoing`]。
///
/// **调用时点 = 步边界**（D267）：一步全部执行完才调；步内「只有一方空」的中间态不判终局。
pub fn terminal(state: &DefState) -> Outcome {
    // 还没开始第一回合：没有可判的终局（建局 / 开局准备阶段）。
    if state.round() == Nat::ZERO {
        return Outcome::Ongoing;
    }
    let me_wiped = state.me().characters().is_empty();
    let opp_wiped = state.opp().characters().is_empty();
    match (me_wiped, opp_wiped) {
        // 步边界上双方同时全灭 -> 双方都输（D267 用户明确；不是平局，也不是任何一方胜）。
        (true, true) => Outcome::BothLose,
        (true, false) => Outcome::OppWins,
        (false, true) => Outcome::MeWins,
        // 先判全灭（S1.4），再判回合上限 —— S7.7 的前提是「仍未分出胜负」。
        (false, false) => {
            if at_round_limit(state.round()) {
                Outcome::RoundLimit
            } else {
                Outcome::Ongoing
            }
        }
    }
}

// ======================================================================
// 引擎接口适配（D24：engine 的 rules 默认不编译，本 crate 的 feature "rules" 打开）
// ======================================================================

/// 引擎 `GameRules` 的适配实现（仅在 `feature = "rules"` 时编译）。
#[cfg(feature = "rules")]
mod rules_impl {
    use super::{terminal, Outcome};
    use crate::base::Perspective;
    use crate::state::def_state::DefState;
    use gi_genius_invokation_engine::nat::ExactNat128;
    use gi_genius_invokation_engine::rules::{ActionList, GameResult, GameRules};

    /// 引擎接口里的「谁」：**两个具名实例**（我方 / 敌方），不是编号、不是判别值（D34）。
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct Seat {
        name: &'static str,
    }

    impl Seat {
        /// 这个实例的名字（日志 / 测试用；不参与任何运算）。
        pub const fn name(self) -> &'static str {
            self.name
        }
    }

    /// 我方（`DefState::me`）。
    pub const ME: Seat = Seat { name: "我方" };
    /// 敌方（`DefState::opp`）。
    pub const OPP: Seat = Seat { name: "敌方" };

    /// **定义层对求解接口的适配**：求解器只通过 `GameRules` 看局面。
    ///
    /// 本轮只把 `terminal` 接完整；其它三个函数是占位（见各自 TODO）。
    #[derive(Clone, Copy, Debug, Default)]
    pub struct DefRules;

    impl GameRules<DefState> for DefRules {
        type Player = Seat;

        /// **当前轮到谁行动**：读 [`DefState::first_player`]（本回合先手牌手）。
        ///
        /// - `Some(Me)` -> [`ME`]；`Some(Opp)` -> [`OPP`]（规则书·进行行动
        ///   战斗行动【宣布结束】：先宣布结束者成为先手牌手，下回合行动阶段优先行动）；
        /// - `None`（先手还没裁定）-> 返回 [`ME`] **占位**，**不代表规则**：
        ///   规则书没有规定第一回合先手归谁（待实测 / 由外部裁定），这里不编默认值。
        ///   **只有加载 / 开局流程会保证它被设置**；
        /// - 终局判定 [`terminal`] 不依赖先手，所以先手未设置时对局照样能判终局。
        fn to_move(&self, state: &DefState) -> Seat {
            match state.first_player() {
                Some(Perspective::Me) => ME,
                Some(Perspective::Opp) => OPP,
                // 未设置：占位（只有加载 / 开局流程会保证设置）。
                None => ME,
            }
        }

        /// 终局：映射自 [`terminal`]。
        ///
        /// - [`Outcome::MeWins`] -> `Win(ME)`、[`Outcome::OppWins`] ->
        ///   `Win(OPP)`（S1.4）；
        /// - [`Outcome::RoundLimit`] -> `GameResult::RoundLimit`（S7.7 双方都输）；
        /// - [`Outcome::BothLose`] -> `GameResult::BothLose`（D267：双方都输；
        ///   引擎侧由 engine-basics 添加的同一语义变体，不是平局）。
        fn terminal(&self, state: &DefState) -> Option<GameResult<Seat>> {
            match terminal(state) {
                Outcome::Ongoing => None,
                Outcome::MeWins => Some(GameResult::Win(ME)),
                Outcome::OppWins => Some(GameResult::Win(OPP)),
                Outcome::RoundLimit => Some(GameResult::RoundLimit),
                // D267：双方同时全灭 = 双方都输（不是平局）。
                Outcome::BothLose => Some(GameResult::BothLose),
            }
        }

        /// **占位**：每个动作的列表形状归各块自己的 `parse.rs`（D26），本轮不接。
        fn actions(&self, _state: &DefState, _who: Seat) -> Vec<ActionList> {
            Vec::new()
        }

        /// **占位**：非终局估值是求解器的事（分工计划 §4.5.5）；这里只保证接口可用。
        fn evaluate(&self, _state: &DefState) -> ExactNat128 {
            ExactNat128::new(0)
        }
    }
}

#[cfg(feature = "rules")]
pub use rules_impl::{DefRules, Seat, ME, OPP};

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::growth::GrowthList;
    use crate::families::character::trigger::{avoid_defeat_with_triggers, CharacterTriggers};
    use crate::families::character::zones::equipment::EquipmentTriggers;
    use crate::families::character::zones::field::{
        CharacterSlot, CharacterZone, CharacterZoneTriggers,
    };
    use crate::state::flows::defeat::{defeat_me, defeat_opp, DefeatHpReset};
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{character, characters, entity, n};

    /// 一侧：给定角色实体号（空数组 = 全部角色都已被击倒清理移出）。
    fn side(values: &[u32]) -> PlayerDefs {
        PlayerDefs::new().with_characters(characters(values))
    }

    /// 已经开始的局面（回合数 > 0）。
    fn started(me: PlayerDefs, opp: PlayerDefs, round: u32) -> DefState {
        DefState::from_players(me, opp).with_round(n(round))
    }

    /// 我方角色区：只有 1 个角色，HP 已经归 0、但击倒清理**还没执行**（免于击倒窗口）。
    fn zone_with_hp_zero() -> CharacterZone {
        let dead = character().hp_sub(n(10));
        CharacterZone::try_from_slots(vec![CharacterSlot::new(entity(201), dead)])
            .expect("测试角色区合法")
    }

    #[test]
    fn unstarted_state_is_ongoing() {
        // 回合 0 = 第一回合还没开始（建局 / 开局准备）：两侧角色区都空也不判终局。
        assert_eq!(terminal(&DefState::new()), Outcome::Ongoing);
        assert!(!Outcome::Ongoing.is_terminal());
    }

    #[test]
    fn both_sides_alive_is_ongoing() {
        let state = started(side(&[201, 202]), side(&[401, 402]), 1);
        assert_eq!(terminal(&state), Outcome::Ongoing);
    }

    #[test]
    fn wiping_all_opponent_characters_wins() {
        // S1.4：击倒对方牌手的所有角色牌即获胜。
        let state = started(side(&[201]), side(&[]), 5);
        assert_eq!(terminal(&state), Outcome::MeWins);
        assert!(terminal(&state).is_terminal());
    }

    #[test]
    fn losing_all_characters_is_a_loss() {
        let state = started(side(&[]), side(&[401]), 5);
        assert_eq!(terminal(&state), Outcome::OppWins);
    }

    #[test]
    fn both_sides_wiped_at_the_step_boundary_means_both_lose() {
        // D267（用户明确）：一步内双方角色都被打空 -> 本步执行完再判，**双方都输**（不是平局）。
        let state = started(side(&[]), side(&[]), 5);
        assert_eq!(terminal(&state), Outcome::BothLose);
        assert!(terminal(&state).is_terminal());
        // 钉住语义：不是任何一方获胜（也不是旧的「无法判定」占位）。
        assert_ne!(terminal(&state), Outcome::MeWins);
        assert_ne!(terminal(&state), Outcome::OppWins);
    }

    #[test]
    fn one_side_wiped_at_one_boundary_and_the_other_later_gives_single_side_results() {
        // 两个**不同的步边界**局面：各自只有一方为空 -> 各自的单方胜负（不是双负）。
        let after_opponent_wiped = started(side(&[201]), side(&[]), 5);
        assert_eq!(terminal(&after_opponent_wiped), Outcome::MeWins);
        let after_me_wiped = started(side(&[]), side(&[401]), 5);
        assert_eq!(terminal(&after_me_wiped), Outcome::OppWins);
        // 对照：同一步边界上双方都空才是双负。
        assert_eq!(
            terminal(&started(side(&[]), side(&[]), 5)),
            Outcome::BothLose
        );
    }

    #[test]
    fn step_is_atomic_so_the_mid_step_single_side_state_is_not_judged() {
        // D267：终局在**步边界**判。一步内双方先后被打空时，中间态（只有一方空）只是中间态；
        // 若在步内提前判会得到单方胜负；正确做法是本步全部执行完、步边界再判 -> 双负。
        let step_start = started(side(&[201]), side(&[401]), 5);
        assert_eq!(terminal(&step_start), Outcome::Ongoing, "步开始");
        // 步内中间态（我方已被清空、敌方还没）：这个返回值只是「如果在这里判」的结果，
        // 调用方**不得**在步内判（D204 一步原子 + D267 步边界判定）。
        let mid_step = started(side(&[]), side(&[401]), 5);
        assert_eq!(
            terminal(&mid_step),
            Outcome::OppWins,
            "误在步内判 = 单方胜负（错）"
        );
        // 本步执行完（双方都清空）-> 步边界判定 = 双方都输。
        let step_end = started(side(&[]), side(&[]), 5);
        assert_eq!(terminal(&step_end), Outcome::BothLose);
    }

    /// **回归用例（D267 / D268）**：同一步内先对敌方出战角色造成伤害（打死对面最后一个角色），
    /// 再对我方出战角色造成 1 点自伤（打死我方最后一个角色）。**不需要任何双负特例分支**：
    /// 步内两次伤害都落地、本步执行完 -> 击倒清理后双方角色区都空 -> 步边界判定 [`Outcome::BothLose`]。
    ///
    /// 对应游戏实例：芙宁娜（白芙）天赋牌 + 普通攻击（加伤打死对面最后一个角色，同一步内还
    /// 会扣我方角色 1 点血）。本测试用现有结算 API（`keywords::Step` 一步原子）构造，
    /// 不依赖任何具体卡牌。
    #[test]
    fn same_step_enemy_damage_plus_self_damage_ends_in_both_lose() {
        use crate::base::{Element, Perspective};
        use crate::families::character::attachment::{AttachmentAllowSet, ReactionRegistry};
        use crate::families::character::character::CharacterState;
        use crate::families::character::damage::DamageEnv;
        use crate::families::character::trigger::CharacterTriggers;
        use crate::families::character::types::DamageType;
        use crate::keywords::effect::{EffectEnv, TriggerBook};
        use crate::keywords::op::{DamageArgs, DamageSourceKind, KeywordOp};
        use crate::keywords::step::{execute_step_me, Step};
        use crate::keywords::target::TargetSelector;

        static NO_ELEMENTS: [Element; 0] = [];

        // 双方都只剩 1 个角色、各 1 血：1 点伤害就会被打空。
        let one_hp = CharacterState::new(n(1), n(10), Nat::ZERO, n(2));
        let state = started(
            PlayerDefs::new().with_characters(
                CharacterZone::try_from_slots(vec![CharacterSlot::new(
                    entity(201),
                    one_hp.clone(),
                )])
                .expect("合法角色区"),
            ),
            PlayerDefs::new().with_characters(
                CharacterZone::try_from_slots(vec![CharacterSlot::new(entity(401), one_hp)])
                    .expect("合法角色区"),
            ),
            5,
        );

        // 现有结算 API：一步 = 原子单位，两个关键字操作按书写顺序执行完（D204）。
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let damage = DamageEnv {
            allow: AttachmentAllowSet::new(&NO_ELEMENTS),
            rules: &[],
            registry: &registry,
        };
        let mut book = TriggerBook::empty();
        book.me_characters = &me_triggers;
        book.opp_characters = &opp_triggers;
        let effect = EffectEnv::new(entity(201), Perspective::Me, damage).with_triggers(book);

        let step = Step::new(vec![
            // 1) 对敌加伤：打死对面最后一个角色。
            KeywordOp::Damage(DamageArgs::new(
                TargetSelector::opponent_active(),
                n(1),
                DamageType::Physical,
                DamageSourceKind::Other,
            )),
            // 2) 同一步内自伤：扣我方角色 1 血，打死我方最后一个角色。
            KeywordOp::Damage(DamageArgs::new(
                TargetSelector::own_active(),
                n(1),
                DamageType::Physical,
                DamageSourceKind::Other,
            )),
        ]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("确定性一步");

        // 步内：两次伤害**都落地**（步内不提前终止），双方 HP 都到 0，但角色都还在角色区。
        assert_eq!(
            branch
                .state
                .me()
                .characters()
                .slot_of(entity(201))
                .expect("还在")
                .state
                .hp(),
            Nat::ZERO
        );
        assert_eq!(
            branch
                .state
                .opp()
                .characters()
                .slot_of(entity(401))
                .expect("还在")
                .state
                .hp(),
            Nat::ZERO
        );
        // 步内不判终局：HP 0 但击倒清理未做 -> Ongoing（S7.3 结算完成后才击倒）。
        assert_eq!(terminal(&branch.state), Outcome::Ongoing);

        // 步边界：结算完成 -> 击倒清理（现有 defeat 流程；同一步的两次击倒都清理）。
        let growth = GrowthList::new();
        let hp = DefeatHpReset::new(n(10), &growth);
        let characters = CharacterZoneTriggers::new();
        let equipment = EquipmentTriggers::new();
        let after_me_defeated = defeat_me(&branch.state, entity(201), &hp, &characters, &equipment)
            .expect("角色在区里")
            .state;
        // 只清了一边就判（步内提前判）会得到单方胜负 —— 那是错的。
        assert_eq!(terminal(&after_me_defeated), Outcome::OppWins);
        // 两边都清理完 = 步边界：双方角色区都空 -> 双方都输（不是平局、不是任一方胜）。
        let after_both_defeated =
            defeat_opp(&after_me_defeated, entity(401), &hp, &characters, &equipment)
                .expect("角色在区里")
                .state;
        assert_eq!(terminal(&after_both_defeated), Outcome::BothLose);

        // 纯函数：入参局面不变（双方都还有角色）。
        assert_eq!(state.me().characters().len(), Nat::new(1));
        assert_eq!(state.opp().characters().len(), Nat::new(1));
    }

    #[test]
    fn fifteenth_round_start_is_the_round_limit() {
        // S7.7：第 15 回合开始时仍未分胜负 -> 双方都输。
        let before = started(side(&[201]), side(&[401]), 14);
        assert_eq!(terminal(&before), Outcome::Ongoing);
        for round in [15, 16, 20] {
            let at_limit = started(side(&[201]), side(&[401]), round);
            assert_eq!(terminal(&at_limit), Outcome::RoundLimit, "第 {round} 回合");
        }
    }

    #[test]
    fn defeat_takes_precedence_over_the_round_limit() {
        // S7.7 的前提是「仍未分出胜负」：已经全灭的一方按胜利条件判，不再套回合上限。
        let decided = started(side(&[201]), side(&[]), 15);
        assert_eq!(terminal(&decided), Outcome::MeWins);
    }

    #[test]
    fn hp_zero_is_not_a_loss_until_the_defeat_settlement_completes() {
        // 我方只有 1 个角色、HP 已 0，但击倒清理还没做（S7.3：结算完成后才击倒）。
        let state = started(
            PlayerDefs::new().with_characters(zone_with_hp_zero()),
            side(&[401]),
            5,
        );
        assert_eq!(terminal(&state), Outcome::Ongoing);

        // 免于被击倒：只标记「当前不视作被击倒」，不动角色区（S7.4）—— 不判负。
        let triggers = CharacterTriggers::new();
        let slot = state.me().characters().slot_of(n(201)).expect("角色在区里");
        let _ = avoid_defeat_with_triggers(&slot.state, n(201), &triggers);
        assert_eq!(terminal(&state), Outcome::Ongoing);

        // 确实被击倒：结算完成后走 defeat 流程（把角色移出角色区）-> 全灭 -> 判负。
        let growth = GrowthList::new();
        let hp = DefeatHpReset::new(n(10), &growth);
        let outcome = defeat_me(
            &state,
            n(201),
            &hp,
            &CharacterZoneTriggers::new(),
            &EquipmentTriggers::new(),
        )
        .expect("角色在区里");
        assert_eq!(terminal(&outcome.state), Outcome::OppWins);
    }

    #[test]
    fn next_round_counts_up_and_stops_at_the_limit() {
        assert_eq!(ROUND_LIMIT, n(15));
        assert_eq!(next_round(Nat::ZERO), n(1));
        assert_eq!(next_round(n(1)), n(2));
        assert_eq!(next_round(n(14)), n(15));
        // 边界：第 15 回合开始即终局，回合数不再增长。
        assert_eq!(next_round(n(15)), n(15));
        assert_eq!(next_round(n(20)), n(20));
        assert!(!at_round_limit(n(14)));
        assert!(at_round_limit(n(15)));
        assert!(at_round_limit(n(20)));
    }

    /// 引擎接口适配（feature = "rules"）：Outcome -> GameResult 的映射。
    #[cfg(feature = "rules")]
    #[test]
    fn engine_adapter_maps_outcomes_to_game_results() {
        use gi_genius_invokation_engine::rules::{GameResult, GameRules};

        let rules = DefRules;
        let alive = || side(&[201]);
        let wiped = || side(&[]);

        assert_eq!(rules.terminal(&started(alive(), alive(), 1)), None);
        assert_eq!(
            rules.terminal(&started(alive(), wiped(), 1)),
            Some(GameResult::Win(ME))
        );
        assert_eq!(
            rules.terminal(&started(wiped(), alive(), 1)),
            Some(GameResult::Win(OPP))
        );
        assert_eq!(
            rules.terminal(&started(alive(), alive(), 15)),
            Some(GameResult::RoundLimit)
        );
        // D267：双方同时全灭 = 双方都输（不是平局）。
        assert_eq!(
            rules.terminal(&started(wiped(), wiped(), 1)),
            Some(GameResult::BothLose)
        );
        // 两个具名实例（不是编号）：名字可读、互不相等。
        assert_eq!(ME.name(), "我方");
        assert_eq!(OPP.name(), "敌方");
        assert_ne!(ME, OPP);
    }

    /// to_move 跟随 DefState::first_player；未设置时保持我方占位（文档已说明不代表规则）。
    #[cfg(feature = "rules")]
    #[test]
    fn engine_to_move_follows_the_first_player() {
        use crate::base::Perspective;
        use gi_genius_invokation_engine::rules::GameRules;

        let rules = DefRules;
        let alive = || side(&[201]);

        // 未设置先手（第一回合还没裁定）：保持 ME 占位
        assert_eq!(rules.to_move(&started(alive(), alive(), 1)), ME);
        // 显式 Some(Me) / Some(Opp)
        let me_first = started(alive(), alive(), 1).with_first_player(Some(Perspective::Me));
        assert_eq!(rules.to_move(&me_first), ME);
        let opp_first = started(alive(), alive(), 1).with_first_player(Some(Perspective::Opp));
        assert_eq!(rules.to_move(&opp_first), OPP);
        // 清回未知 -> 回到占位
        assert_eq!(rules.to_move(&opp_first.with_first_player(None)), ME);
    }
}
