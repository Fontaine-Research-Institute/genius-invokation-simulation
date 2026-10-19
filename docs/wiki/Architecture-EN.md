# Architecture Overview

> [[Architecture|中文]] · English

*Scope: dependency direction of the six crates, repository layout, the data flow of a match, how "the framework only provides interfaces" lands, and known documentation deviations.*

---

## 1. Crates and Dependency Direction

```text
richtext (zero-dependency: parsing only)
     ^
     |
cards --> definitions --> engine (mechanism primitives, zero deps by default)
  |            |
  |            +--> richtext
  |
simulation (facade) --> engine / definitions / richtext (+ optional cards)
compute (compute bins) --> no deps by default; a task adds its own
```

| Crate (package name) | Directory | Depends on | Responsibility |
|---|---|---|---|
| `gi-genius-invokation-engine` | `new/crates/engine` | none (`libloading` only under feature `dll`) | Mechanism primitives: numbers, ids, entities, areas, randomness and choice, exact fractions, statistics |
| `gi-genius-invokation-richtext` | `new/crates/richtext` | none | Rich-text parsing: raw markup -> neutral segment sequence |
| `gi-genius-invokation-definitions` | `new/crates/definitions` | engine, richtext | Game definition layer (families, types, tags, resources, rules, state, flow, keywords, dispatch, records) |
| `gi-genius-invokation-cards` | `new/crates/cards` | definitions, engine | Concrete cards: each card's data plus effect logic |
| `gi-genius-invokation-simulation` | `new/crates/simulation` | the four above (cards optional) | The single public facade: re-exports only, no logic |
| `gi-genius-invokation-compute` | `new/crates/compute` | none by default | Compute toolchain: one computation = one bin folder |

> [!IMPORTANT]
> Dependency direction is enforced by Cargo: `definitions` never depends on `cards` (a guard test checks it); `richtext` depends on no business crate; `engine` has no upper-layer semantics.
> External users depend on the `simulation` facade only; cross-crate imports go through `gi_genius_invokation_simulation::{engine, definitions, richtext}`.

## 2. Repository Layout

```text
new/
├─ Cargo.toml              # workspace: members = crates/*
├─ rust-toolchain.toml     # stable (offline: no concrete version)
├─ docs/wiki/              # this wiki
├─ crates/{engine,richtext,definitions,cards,simulation,compute}/
├─ tools/                  # run-compute.py, three scanners, COMPUTE.md
└─ .github/workflows/      # ci.yml, compute.yml
```

Inside `definitions`: `base/` (L1 families + ids + elements + perspective), `families/` (per-family type enums + definition carriers + areas + operations), `tags/` (L3 tags, a top-level directory), `resources/` (dice pool), `rules/` (unified three-level base operations), `timeline/` (trigger registry + dispatch + settlement frames), `reactions/` (elemental reactions), `records/` (record API), `source/` (source embedding), `version/` (version sync), `generated/` (generated data + rendering), `state/` (`DefState` + chained flows), `flow/` (end-to-end actions), `keywords/` (keyword functions + steps). See [[Crate-Definitions-EN]].

## 3. Data Flow of a Match

| Layer | Location | Role |
|---|---|---|
| State | `definitions/src/state/` | `DefState`: two named instances `me` / `opp` + `round` + `first_player`; every area owns its state (D42 / D182 / D183) |
| Flow | `definitions/src/flow/` | Action entry points over `&DefState`: skill use, card play, switching, tuning, techniques, round skeleton; return a new state plus pending work, or `Err` |
| Effect composition | `definitions/src/keywords/` | Keyword functions + a Step (D204): in-step order as written, no insertion |
| Dispatch | `definitions/src/timeline/` | Registry + ordered dispatch (sorting only, never executing, D186) + settlement frames (insertion-based, D196) |
| Base operations | `definitions/src/rules/` | Pure functions for unified three-level base operations (slots, caps, opening, queries, order, ...) |
| Reactions | `definitions/src/reactions/` | Reaction identity -> effect function pointer; the damage modifier chain lives in `families/character/damage_chain/` |
| Records | `definitions/src/records/` | `Recorder<T>`: step number + arbitrary content (D223 / D224) |

**Example: using a skill** (`definitions/src/flow/skill_use.rs`):

1. Paired entry points `use_skill_me` / `use_skill_opp` (one shared implementation, D29 / D34).
2. Can-use checks -> payment (dice / energy) -> dispatch "before skill use" (sorting only).
3. Execute the effect body: `keywords::Step`, calling keyword functions in order.
4. Flush at step end: pending work collected from areas is handed to the caller (`flow/pending.rs`); reaction sub-settlements are inserted at step boundaries.
5. Dispatch "after skill use" and use `timeline::Resolution::insert` at the current settlement position (D196).
6. Per-branch game-over check (`state/terminal.rs`; mutual defeat is `BothLose`, D267).

**Text data flow**: `generated/data/*.json` -> `build.rs` (generates static tables into `OUT_DIR` at compile time) -> `generated` accessors -> `richtext::parse` (strict) -> `generated::text::render` (sources -> final string).

### 3.1 Interface Source Excerpts
**Root state `DefState`**

`new/crates/definitions/src/state/def_state.rs:58-82`

```rust
/// **一局对局的完整状态**：我方全部区 + 敌方全部区（两个具名实例，D34）+ 回合数。
///
/// 容器没有身份（D42）：它就是「这一局」这个变量本身，不出现对局号、不登记。
#[derive(Clone, Debug, PartialEq)]
pub struct DefState {
    /// 我方的全部区。
    me: PlayerDefs,
    /// 敌方的全部区。
    opp: PlayerDefs,
    /// 回合数（0 = 还没开始第一回合；每回合开始 +1；上限见
    /// `crate::state::terminal::ROUND_LIMIT`）。
    ///
    /// 依据：规则书·其他规则 对局回合上限「第15回合开始时，如果仍未分出胜负，则双方都会输掉
    /// 这场对局」—— 判定需要知道「现在是第几回合」，所以回合数由根状态持有。
    round: Nat,
    /// **本回合的先手牌手**（行动阶段 / 结束阶段的处理起始方；[`None`] = 还没裁定）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】「先『宣布回合结束』的牌手会成为
    /// 『先手牌手』，并在下回合的行动阶段优先行动」；其他规则·结算顺序第 1 条
    /// 「先结算当前牌手的所有卡牌效果」。
    ///
    /// 写入方：开局 / 加载流程（第一回合由外部裁定；之后由上一回合的先宣布者接任）。
    /// 读取方：引擎求解接口 `state::terminal::DefRules::to_move`（当前轮到谁行动）。
    first_player: Option<Perspective>,
}
```

**Action entry `use_skill_me`**

`new/crates/definitions/src/flow/skill_use.rs:430-446`

```rust
/// **使用技能**（我方使用；下落标记读根状态）。
///
/// - 成功 -> 结算分支（确定性技能恰好一支）；settlement 会被**插入**「使用技能后」的子结算
///   （D196：插到当前 cursor 位置 = 步边界；调用方之后用 pending::execute_frames 执行完）；
/// - 失败 -> Err，state / settlement 一个字节都不改；
/// - 下落攻击标记（S8.10）：读 `state.me().plunging()` 判生效形态；战斗行动结束时按
///   `battle_action_end_me` 清除并写回根状态。需要显式传标记时用
///   [use_skill_me_with_plunging]。
pub fn use_skill_me(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
) -> Result<SkillOutcome, SkillUseError> {
    // 标记的持有区在根状态（PlayerDefs::plunging）：旧入口读使用方当前标记。
    use_skill_me_with_plunging(state, env, request, settlement, state.me().plunging())
}
```

**Step `Step`**

`new/crates/definitions/src/keywords/step.rs:44-48`

```rust
/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}
```

**Dispatch `dispatch`**

`new/crates/definitions/src/timeline/dispatch.rs:60-96`

```rust
/// **单侧派发**：从一个牌手的注册表里取出「这个时机、条件成立」的登记项，按结算顺序排好。
///
/// 排序 = `结算顺序.md` §一 第 2~4 条：来源类别 -> 进场先后 -> 登记顺序（稳定排序）。
/// 第 1 条「当前牌手优先」不在本函数里：它由 [`resolve`] 用「两个注册表的先后」表达。
///
/// 只读 + 不执行：不调用任何效果函数指针，也不改注册表。
#[must_use]
pub fn dispatch<'e, K, E>(
    registry: &TriggerRegistry<K, E>,
    timing: K,
    event: &'e Event,
) -> Vec<Pending<'e, E>>
where
    K: Copy + PartialEq,
    E: Copy,
{
    // 1) 只留「时机相等 + 条件成立」的登记项。
    let mut matched: Vec<Trigger<K, E>> = registry
        .entries()
        .iter()
        .copied()
        .filter(|trigger| trigger.applies(timing, event))
        .collect();
    // 2) 按结算顺序排（稳定排序：完全同序时保持登记顺序）。
    matched.sort_by(by_resolution_order);
    // 3) 产出「函数指针 + 事件」的待执行列表（不执行）。
    matched
        .into_iter()
        .map(|trigger| Pending {
            effect: trigger.effect,
            event,
            owner: trigger.owner,
            source_kind: trigger.source_kind,
            entry: trigger.entry,
        })
        .collect()
}
```

**Insert sub-settlement `Resolution::insert`**

`new/crates/definitions/src/timeline/suspend.rs:168-178`

```rust
    /// **插入子结算**：在当前帧当前位置压入新帧（父帧的 `cursor` 不动）。
    ///
    /// - 子结算执行完后**回到插入点**，继续父帧剩余步骤；
    /// - 不重排父帧已有步骤（不是 LIFO 触发栈）；
    /// - 空插入 = 不做任何事。
    pub fn insert(&mut self, steps: Vec<Queued<E>>) {
        if steps.is_empty() {
            return;
        }
        self.frames.push(Frame::new(steps));
    }
```

**Settlement snapshot `Resolution::snapshot`**

`new/crates/definitions/src/timeline/suspend.rs:202-217`

```rust
    /// **只读快照**：当前结算嵌套状态（当前帧 / 剩余步骤 / 插入点）。
    ///
    /// 不改状态、不含函数指针；供打印 / 导出对照游戏结算顺序。
    pub fn snapshot(&self) -> ResolutionSnapshot {
        let current = match self.frames.len() {
            0 => None,
            len => Some(nsub!(Nat::new(len as _), Nat::new(1))),
        };
        let frames = self
            .frames
            .iter()
            .enumerate()
            .map(|(depth, frame)| frame.snapshot(Nat::new(depth as _)))
            .collect();
        ResolutionSnapshot { current, frames }
    }
```

**Record entry `Recorder::push`**

`new/crates/definitions/src/records/mod.rs:88-91`

```rust
    /// **唯一写入入口**：把一条内容记到某一步。
    pub fn push(&mut self, step: Nat, content: T) {
        self.entries.push(Record::new(step, content));
    }
```

**Fold back `fold_states`**

`new/crates/definitions/src/flow/mod.rs:130-147`

```rust
/// **折叠回**（D191）：把局面完全相同的分支合并，概率**精确相加**。
///
/// 观测点展开出的分支是临时的（D191：展开 / 回收成对）：一旦不再需要区分「怎么走到这个局面」，
/// 就把等价分支折叠回一支。比较口径 = DefState 的 PartialEq（各区自己的等价语义，
/// 统计不参与，D187）；顺序保持首次出现顺序（可复现）。
///
/// 注意：只比较**局面**，不比较待执行（Queued 没有 PartialEq）；需要连待执行一起折叠时，
/// 由调用方先执行完待执行再调本函数（skill_use / play_card 内部就是这么用的）。
pub fn fold_states(branches: Vec<(DefState, Fraction)>) -> Vec<(DefState, Fraction)> {
    let mut folded: Vec<(DefState, Fraction)> = Vec::new();
    for (state, probability) in branches {
        match folded.iter_mut().find(|(existing, _)| *existing == state) {
            Some(entry) => entry.1 = entry.1.exact_add(probability),
            None => folded.push((state, probability)),
        }
    }
    folded
}
```


**Keyword function: damage**

`new/crates/definitions/src/keywords/keyword.rs:241-292`

```rust
/// **造成伤害**：对选择器解析出的每个角色走伤修链（加伤 -> 护盾 -> 减伤 -> 扣血 / 附着 / 反应）。
///
/// 每次伤害都派发「生命值变动」待执行（不执行，步末统一做，D204）。
pub fn cause_damage(state: &DefState, env: &EffectEnv<'_>, args: &DamageArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    fan_out(state, env, candidates, args.pick, |current, side, target| {
        let target_state = character_state(current, side, target)?;
        let player = player_of(current, side);
        // 乘算 / 除算修饰随参数进来（卡面「此次伤害翻倍」等）；默认 NONE。
        let chain_env = DamageChainEnv::new(player.combat_status(), &target_state.states)
            .with_modifiers(args.modifiers);
        let chained = damage_with_chain(
            &target_state,
            env.subject(),
            target,
            args.amount,
            args.ty,
            &ChainDamageEnv {
                damage: &env.damage,
                chain: &chain_env,
                bonus: DamageBonus {
                    increases: env.increases,
                    mastery: env.mastery,
                    reaction_bonus: Nat::ZERO,
                    reacted: false,
                },
                context: env.effect_context,
                triggers: env.character_triggers(side),
            },
        );
        let next = with_character(current, side, target, chained.outcome.state);
        let mut pending = PendingSet::new();
        for item in chained.pending {
            push_pending(
                &mut pending,
                side,
                FlowPending::Character {
                    character: target,
                    pending: item,
                },
            );
        }
        // R4：伤害路径判出的反应不在这里执行（D204 步内不插）；记成反应子结算，
        // 由步执行器在步边界插入执行（做完回到原步骤）。
        let mut reactions = Vec::new();
        if let Some(hit) = chained.reaction {
            reactions.push(ReactionRequest::new(hit, env.viewer(), side, false));
        }
        Some((next, pending, reactions))
    })
}
```

**Rules ops: settlement order predicate**

`new/crates/definitions/src/rules/ops/order.rs:38-59`

```rust
/// 同一时间点、同一牌手内：first 是否先于 second 结算（规则书第 2 条）。
///
/// 同一类别 -> false（同类按进场先后，规则书第 3 条，见 same_source_uses_entry_order）。
pub fn resolves_before(first: EffectSource, second: EffectSource) -> bool {
    // 同一类别不比先后（同类按进场先后，规则书第 3 条）。
    if first == second {
        return false;
    }
    let mut seen_first = false;
    for source in SOURCE_ORDER {
        if source == first {
            if seen_first {
                return false;
            }
            seen_first = true;
        }
        if source == second {
            return seen_first;
        }
    }
    false
}
```


## 4. How "The Framework Only Provides Interfaces" Lands

> [!NOTE]
> This is the highest principle (D20; `new/README.md` "project philosophy"; `分工计划.md` section 8.5) and overrides concrete designs.

| Feature | What the framework provides (mechanism) | Who implements the business |
|---|---|---|
| Randomness / choice | `enumerate_random` / `enumerate_random_weighted` / `enumerate_select` + exact fractions | Each block's own `parse.rs` for the select list and dedup function (D25 / D26) |
| Trigger dispatch | `timeline::TriggerRegistry` + `dispatch` (sorting only) | Families/types define timing keys and dispatch proactively; cards register effect function pointers (D185 / D186) |
| Settlement insertion | `timeline::Resolution` frames + `cursor` | The effect layer decides what to insert and where (D196) |
| Areas / state | Areas own their state + `with_*` pure functions; `state` only composes | Each area itself (D42 / D182 / D183) |
| Three-level base ops | `rules/` static rule table + `rules::ops` pure functions | Cards call them as needed; no central tables (D176-D179) |
| Payment | Dice pool / energy / arcane edict provide `can_afford` / `pay` | Cards only declare cost data (D121 / D133 / D144) |
| Text | `generated` accessors + `richtext` parser + `render` | Cards write `Localized` or reference generated data (D56 / D62 / D75) |
| Records | `records::Recorder`, a thin generic container | Each module defines its record content type and calls `push` (D222-D225) |

> [!WARNING]
> Forbidden (no exceptions): central statistics, central collection, built-in behavior tables, a unified cost-reduction calculator, attribute statistics tables, behavior switch tables, built-in buff tables.

## 5. Time and Uncertainty

| Topic | Rule | Basis |
|---|---|---|
| Settlement order | Within one timing, the workspace-root `结算顺序.md` is the single authority; order constants exist only in `rules::ops::order` | D197; `结算顺序.md` |
| Insertion-based settlement | Frames + `cursor`; a sub-settlement returns to the insertion point; it is not a LIFO stack | D196 |
| Step atomicity | In-step order as written, no insertion inside; insertion only between steps | D204 |
| Uncertainty | Do not materialize what is undetermined; on observation expand into concrete branches with exact probabilities, then fold back | D190 / D191 |
| Zero floating point | Probabilities, weights, and randomness are all integers (`Fraction` = `ExactNat128` numerator/denominator; `Rng` exposes only `u128`) | D158-D160 |

## 6. Known Documentation Deviations

> [!WARNING]
> The following older documents disagree with the current code or decisions. This wiki documents the code; the old documents are for the maintainers to update.

| Old document | Deviation |
|---|---|
| `new/README.md` / `README.en.md` | "Each die is an entity", "compile-time settlement order", type-tree inheritance, dll / two-phase loading, "core / engine / definitions, three crates", `Rc` structural sharing |
| `结构.md` / `分工计划.md` | Old layout and signatures: f64 probabilities, old `DefState` fields, `CardType` / `CardTag` in base, "card code stores no text" |
| `definitions/src/lib.rs` | Module docs still say resources / timeline / loader come later; they already exist |
| `definitions/src/families/mod.rs` | Still says families "only hold type enums"; they also hold definition carriers, areas, and operations |
| `definitions/src/records/mod.rs` | Still says damage_chain / area records are "pending"; several `record.rs` files already use `Recorder` |
| `definitions/build.rs` | Hand-written JSON parser (offline zero-dependency), which conflicts with D74 ("switch to a JSON crate") |

## 7. Related Pages

| Page | Content |
|---|---|
| [[Conventions-EN]] | design conventions quick reference |
| [[Crate-Engine-EN]] / [[Crate-Richtext-EN]] / [[Crate-Definitions-EN]] / [[Crate-Cards-EN]] / [[Crate-Simulation-EN]] / [[Crate-Compute-EN]] | crate details |
| [[定义层-State-EN]] / [[定义层-Flow-EN]] / [[定义层-Keywords-EN]] / [[定义层-Timeline-EN]] | runtime layers |

---

Last reviewed: 2026-10-19 (code: `new/crates/definitions/src/lib.rs:pub mod`)
