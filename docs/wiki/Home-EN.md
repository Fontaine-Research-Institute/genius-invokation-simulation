# Genius Invokation Simulation - Development Wiki

> [[Home|中文]] · English

*Scope: project positioning, how to read this wiki, verification commands, and the full page index.*

---

## 1. What This Repository Is

- Project: A fan-made simulation for Genius Invokation TCG: rules engine, match simulation, and exact probability solving, written in Rust
- License and notice: Apache-2.0; not affiliated with HoYoverse / miHoYo; no official art, audio, models, or card images
- Crates: `engine` (mechanism primitives), `richtext` (rich-text parser), `definitions` (game definition layer), `cards` (concrete cards), `simulation` (public facade), `compute` (compute bins)
- Progress: Definition-layer basics are closed out (D265 / D266 / D270); the only concrete card is the Aino sample (`cards/character/aino`), whose effect bodies are still placeholders
- Solver: `solver` does not exist yet (see the comment in `new/Cargo.toml`)

### 1.1 Four Core Principles (`new/README.md` and D20)

1. The framework does no central statistics, no central collection, and no built-in behavior tables; it only provides interfaces and primitives.
2. Whoever needs a feature implements it (cards, families, and concrete tags).
3. Features of the same kind belong to one unified wrapper layer (for example, skill behavior is wrapped in the character family).
4. The framework decides "whether it is allowed"; cards decide "what it does".

### 1.2 Exact Probability

Randomness and choices are never sampled; they stay as branches with exact fractions (D158 / D159), so the same position is reproducible.

---

## 2. How to Read This Wiki

Read in this order:

- Step 1: [[Architecture-EN]] -- crate dependency direction, the data flow of a match, and how "interfaces only" is implemented
- Step 2: [[Conventions-EN]] -- quick reference: identity, single source of data, pure functions, zero floating point, Nat and ExactNat, step, records
- Step 3: [[Crate-Engine-EN]] -> [[Crate-Richtext-EN]] -> [[Crate-Definitions-EN]] -> [[Crate-Cards-EN]] -- read the crates layer by layer
- Step 4: definition-layer pages -- only when a concrete module is needed

### 2.1 Sources and Records

- **Priority of evidence** (`RULES.md` section 3): rulebook > card text / authoritative keywords > reference implementation (implementation technique only).
- **Pending measurement / integration**: recorded in the workspace-root `留空.md`; this wiki cites items as `#N`.
- **Settlement order**: the single authority is the workspace-root `结算顺序.md` (marked provisional).
- **Decision numbers**: `DNNN` refer to the workspace-root `决策日志.md` (append-only); this wiki cites numbers and conclusions only.

---

## 3. Verification Commands

The verification commands are:

```bash
cd new
cargo test --offline --workspace
cargo clippy --offline --workspace --all-targets -- -D warnings
python tools/run-compute.py --list
```

The commands above should all pass.

> [!NOTE]
> This machine is always offline (`--offline`). CI (`new/.github/workflows/ci.yml`) uses `--locked` and additionally runs three source-text scanners (advisory only).
> On Windows, point `TEMP` / `TMP` into the workspace `.tmp` if linking fails (see `RULES.md` section 2, item 7).

---

## 4. Page Index

### 4.1 Getting Started

- [[Home-EN]]: this page
- [[Architecture-EN]]: dependency direction, data flow, how the design philosophy lands
- [[Conventions-EN]]: quick reference for design conventions

### 4.2 Base Crates

- [[Crate-Engine-EN]]: numbers, entities and areas, randomness and choice, fractions
- [[Crate-Richtext-EN]]: rich-text parsing and rendering handoff
- [[Crate-Definitions-EN]]: game definition layer (everything except concrete cards)
- [[Crate-Simulation-EN]]: public facade
- [[Crate-Cards-EN]]: concrete cards
- [[Crate-Compute-EN]]: compute tasks

### 4.3 Definition Layer - Infrastructure

- [[定义层-Base-EN]]: elements, families, ids, perspective
- [[定义层-Resources-EN]]: dice and cost
- [[定义层-Rules-EN]]: unified three-level rule table and ops
- [[定义层-Tags-EN]]: tag system
- [[定义层-Timeline-EN]]: timing keys, triggers, insertion-based settlement
- [[定义层-Reactions-EN]]: elemental reactions and effect execution
- [[定义层-Records-EN]]: record API
- [[定义层-Source-Version-Generated-EN]]: source embedding, versions, generated data

### 4.4 Definition Layer - Runtime

- [[定义层-State-EN]]: match state, area carriers, game-over detection
- [[定义层-State-Flows-EN]]: pure functions for area flows
- [[定义层-Flow-EN]]: end-to-end flows (skills, card play, rounds)
- [[定义层-Keywords-EN]]: keyword layer and steps

### 4.5 Definition Layer - Families

- [[定义层-Families-EN]]: overview of the five families
- [[定义层-Character-EN]]: character family
- [[定义层-Action-Card-EN]]: action cards
- [[定义层-Summon-Enemy-Status-EN]]: summons, enemies, statuses

---

## 5. Find by Task

- Change numeric primitives, entities and areas, randomness, fractions: [[Crate-Engine-EN]]
- Change text parsing or placeholder whitelists: [[Crate-Richtext-EN]]
- Add a concrete card: [[Crate-Cards-EN]] + [[定义层-Character-EN]] / [[定义层-Action-Card-EN]]
- Change how a match proceeds (skills, card play, switching, rounds): [[定义层-Flow-EN]] + [[定义层-State-Flows-EN]]
- Add a keyword function or a step: [[定义层-Keywords-EN]]
- Add a trigger timing or a settlement insertion: [[定义层-Timeline-EN]] (with `结算顺序.md`)
- Add a compute task: [[Crate-Compute-EN]]

---

## 6. Wiki Maintenance Conventions

- Pages live in `new/docs/wiki/`; the file name is the page name; pages link only with wiki links (`[[PageName]]`), never relative paths.
- `_Sidebar.md` / `_Footer.md` are maintained by the maintainers; normal pages do not write them.
- Every module page has the same 8 sections: Responsibility / Key Concepts and Types / Public Entry Points / Invariants and Prohibitions / Basis (Rulebook and D Numbers) / Common Misuse / Tests / Pending Work and Field Verification.
- Style: numbered headings, tables with alignment rows, GitHub alert blocks for critical notes (`[!NOTE]` / `[!IMPORTANT]` / `[!WARNING]`), and no emoji.
- This wiki documents the current code. When old documents (`new/README.md`, `结构.md`, `分工计划.md`) disagree, code plus the decision log win; known deviations are listed at the end of [[Architecture-EN]].

### Card Research

- [[卡牌定义]]: Column overview, template, index of studied cards
- [[卡牌定义-爱诺]]: Aino: full code plus line-by-line rationale
- [[卡牌定义-芙宁娜]]: Furina: talent, Center of Attention, form switching (design study, not implemented)
- [[卡牌定义-模板]]: Template for new card research pages

---
---

## Summary

Read this wiki in the order engine -> richtext -> definitions -> cards. Every page tries to show real source and its basis so you can follow the code.

---

Last reviewed: 2026-10-19 (code: `new/Cargo.toml:members`)
