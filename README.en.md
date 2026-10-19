# genius-invokation-simulation

**English** | [简体中文](README.md)

A fan-made simulation for **Genius Invokation TCG** — rules engine, match simulation, and **exact** probability analysis. Written in Rust.

Randomness is kept as **branches with probabilities**, so probabilities are **exact** (first-turn-kill is just one use case).

## Design philosophy

> **Whoever wants a feature gets a function interface from the framework — and implements the feature themselves.**

This outranks every concrete design decision:

1. **The framework does no central bookkeeping, no central collection, no built-in behaviour tables** — interfaces and primitives only.
2. **Whoever needs a feature implements it**: cards, card families and concrete tags write their own logic.
3. **Same-kind behaviour is wrapped in one place** (e.g. everything "skill-like" is wrapped inside the character family).
4. **The framework answers "can it be done"; cards answer "what it does".**

In other words: the framework hands the **judgement** back to the concrete cards and families, and only takes care of the infrastructure — state, randomness, choices, loading, text.

Same family of commitments: zero game semantics in the engine; all randomness and choices go through a single enumeration entry (hence exact probabilities); **different histories never share a hash**; **anything not revealed is treated as unknown**.

## What it does

| Capability | Description |
|---|---|
| **Rules engine** | Resolves the full ruleset; every card effect lives in the `definitions` layer |
| **Match simulation** | Plays complete matches under given policies — card testing, regression, replays |
| **Multi-turn decisions** | Expectimax over the game tree: MAX (us) / MIN (opponent) / CHANCE (random) |
| **Exact probability** | Exact enumeration with multiset folding; results are exact and reproducible |

## Core design

### The engine is pure data

The engine knows only three things:

1. **Entities** (`EntityId`) — an opaque numeric handle
2. **Containers** — a multi-entity container and a single-entity container (= a variable). An area is **not an id**: it is a variable the upper layer owns, addressed directly after compilation
3. **Random / select** — two pure functions (`enumerate_random` / `enumerate_select`); all randomness and all choices must go through them

The engine has **no idea** what a summon, a support card, a discard, damage, a reaction or a cost is.

### Everything is an entity

A die, a card, a summon, a support, a character — all entities. Areas are open too:

```
hand / pile / support zone / summon zone / field characters / dice pool / per-character equipment slots
```

### Cards are Rust code

A card is a Rust function: one language, one type system, one debugger.

### The type tree is a folder tree

`ActionCard → Support / Equipment / Event` maps to directories, with full paths (`行动牌.支援.场地`) and inherited tags.

### Sides are two root areas, not a parameter

The engine knows neither sides nor area ids: an area is just a variable the upper layer owns. A "player" is
**an area that contains areas**. The base layer declares only the two root areas; **every entity area inside them is declared by the module that owns it** (hand / pile / dice pool /
summon zone / each character's equipment slot …), and the "opponent" root area does the same:

```text
root_me  = group area ── hand_me / pile_me / dice_me / summons_me / …
root_opp = group area ── hand_opp / pile_opp / dice_opp / summons_opp / …
```

Callers do not pass a side; they call **paired entry points** — `hand_me()` / `hand_opp()`, `draw_me()` / `draw_opp()` — each forwarding to one shared implementation, so the rules exist once.

## Features

### Pile and randomness

- **Segmented pile**: random segments (unordered, O(1) draw) and fixed segments (ordered); an index-linked segment list; no capacity cap
- **Same-source random pools**: split random segments share one pool **by index** (pointer-like); drawing removes the card from the pool **once**
- **Discard from any position at random** — the whole pile is a candidate set
- Draw modes: by segment / topmost of a type / by card id / draw N (fewer if short)
- Hand overflow: the **just-drawn** card is discarded

### Dice

- **Each die is its own entity** (with a face); the pool is an entity area; 0~16 total
- **Creation**: `DiceGenSpec { count, distinct, base, element }`
- **Payment**: uses the elemental dice cost format (same colour / any colour / specific colour)

### Cost and payment

- `cost` is only the **spending property** (what must be paid)
- **Current state and payment** live with the **thing being spent**: dice pool, arcane edict, energy

### Support / summon / status

- Supports: max 4, **ordered**; when full the **player chooses** which to discard; a support value is **either a bool or a count** (mutually exclusive)
- Summons: max 4; when full the summon fails but the action still counts; uses / effect type / effect points are **all optional**; same-kind merges with **max of uses and points, latest effect type**
- Statuses: combat-status or character attachment; **no stacking** — higher layers overwrite lower ones

### Characters and equipment

- 2~4 characters per side (opponent unbounded), at most 4 on field
- Damage operations belong to the **character** type
- Equipment is a **special action card**; **each on-field character owns an entity area** (its equipment slot)

### Resolution and reactions

- **Resolution order**: each type registers before/after; the total order is resolved at **compile time** (no built-in table, no fixed slots)
- **Elemental reactions**: one definition each (trigger + effect + priority), **overridable** (A triggers, B applies)

### Exact solving

- Randomness is kept as **branches with probabilities** ⇒ results are exact and reproducible
- Node kinds: **MAX** (us) / **MIN** (opponent) / **CHANCE** (random)
- Memoization, transposition table, pruning; the key is the **complete state** — **different histories never share a hash** (no cross-path merging)
- Simulation and search are **decoupled**

### Extensible and reproducible

- Cards loaded as **dlls**; one dll by default (splitting supported)
- Each card dll records the framework dll **sha256** and an **ABI** (rustc release + commit); mismatches refuse to load
- **Two-phase loading**: register → seal → query; stats recomputed after every dll
- RNG lives in the state and travels in/out of calls ⇒ reproducible
- Bilingual terms (zh/en); fallback is reported **explicitly**

### Performance

- Saturating `Nat(u32)`, zero-copy (`&T` / `&str` / `&[T]`), `Rc` structural sharing, bit packing
- Zobrist hashing and a transposition table to follow

## Cross-platform

Windows and Linux; CI cross-compiles on Linux (MinGW) and runs the tests on a Windows runner.

## Status

- ✅ Design specification complete (engine boundary, signatures, per-module specs)
- 🚧 Implementation starting with the skeleton: `core` / `engine` / `definitions`
- ⏳ Next: definitions layer → solver → local server & frontend → all cards → optimisation

## Disclaimer

This is a fan-made, unofficial project for educational and research purposes only. It is **not** affiliated with, endorsed by, or sponsored by HoYoverse or miHoYo.

“Genshin Impact”, “Genius Invokation TCG”, all character names, card names and related terminology are trademarks or copyrighted material of HoYoverse (miHoYo). All rights reserved by their respective owners.

This repository contains **no official game assets** (no art, audio, models or card images) — only rules, names and numerical data. The project is non-commercial. Should the rights holder request it, we will delete this repository immediately (**excluding historical snapshots already archived**).


### Image assets

Official images bundled in this repository (card art, card screenshots, icons — under `assets/`) are **NOT** covered by the Apache-2.0 license:

- They are © **miHoYo / HoYoverse** (official Genshin Impact / Genius Invokation material);
- Used only for documentation, development reference and non-commercial fan content;
- **Every** image must be registered with its source and copyright holder in [`assets/SOURCES.md`](assets/SOURCES.md); unregistered images must not be committed;
- Full policy: [`ASSETS.md`](ASSETS.md).

## License

Licensed under the **Apache License 2.0** — see [LICENSE](LICENSE).