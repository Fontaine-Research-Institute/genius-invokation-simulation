# Card Research

> [[卡牌研究|中文]] · English

*Scope: what this column is (card text + implementation code + test records), writing requirements, the page template, studied entries, basis, and maintenance conventions.*

---

## 1. What This Column Is

**Card Research** = **card text + implementation code (+ test records)**: for one concrete card, put the card-face text, our implementation code, and test records on the same page for review and comparison.

| Item | Detail |
|---|---|
| Target | one concrete card (character / action card / summon / status, etc.) |
| Content | card text (Chinese / English + keyword expansion) -> full implementation code (real source) -> test records -> notes |
| Out of scope | no data-file paths or source annotations, no numeric card ids, no long "interaction-chain step sequence" |
| Unimplemented cards | the implementation section shows **the planned full code** with `[!WARNING] not implemented` and lists what is missing by capability |

> [!IMPORTANT]
> Card text is copied verbatim; implementation code is pasted in full (including `use`, constants, tests and other boilerplate), never key-line excerpts.

## 2. Writing Requirements

### 2.1 Fixed 4 sections

| Section | What must be written |
|---|---|
| `## 1. Card text` | Chinese / English originals verbatim; keyword expansion (K code -> description); related rulebook excerpts |
| `## 2. Implementation code` | the **full** implementation code (real source verbatim, file path, no excerpts); if unimplemented, planned full code + `[!WARNING] not implemented` + missing work by capability |
| `## 3. Tests` | table: `Item / Position + operation / In-game result / Our result / Conclusion`; untested items say "pending test" with reproducible steps |
| `## 4. Notes` (optional) | pitfalls / related D numbers / related pages |

### 2.2 Hard writing rules

| No. | Rule |
|---|---|
| 1 | **No data-file paths or source annotations**: no disk paths, JSON file names, or generated-data directory names; write the card-face text itself only |
| 2 | **No numeric card ids**: no numeric ids for talents / statuses / characters / events / summons; use **names** |
| 3 | **No "talent card" wording in `§1.1`** |
| 4 | **Metadata one item per line**: type, tags, cost, etc. each on its own line (or list row); never crammed into one line |
| 5 | **Paste implementation code in place**: when a capability is discussed, paste that real source immediately after; do not pile it at the page end / last section, and do not just link |
| 6 | No emoji; tables with alignment rows; critical notes use `[!NOTE]` / `[!IMPORTANT]` / `[!WARNING]` |
| 7 | Fence code as `rust` (commands `bash`, config `toml`); source must match the current repository verbatim and be re-pasted when interfaces change |
| 8 | Footer: `---` + `Last verified: YYYY-MM-DD (code: <file>:<symbol>)` |

Evidence order: rulebook > card text / authoritative keywords > reference implementation (technique only; `RULES.md` section 3, items 2 and 3).

## 3. Page Template

Copy [[卡牌研究-模板-EN]] for a new card; add a row to section 4 of this page.

## 4. Studied Entries

| Name | Type | Status | Page |
|---|---|---|---|
| Aino | character card | implemented (four effect bodies are still `todo!()` placeholders) | [[卡牌研究-爱诺-EN]] |
| Lost Legacies in the Sand | event card, Arcane Legend | card not implemented (pile / hand / Arcane bookkeeping primitives exist) | [[卡牌研究-沙中遗事-EN]] |
| Furina | character card (multi-card) | being rewritten to this structure by defs-char | [[卡牌研究-芙宁娜]] |

## 5. Basis and Related Pages

| Topic | Page / file |
|---|---|
| Settlement and insertion order (authority for order conclusions) | workspace-root `结算顺序.md` (sections 1 / 3) |
| Rulebook text | in-game rules text (Game Areas / Taking Actions / Special Terms / Other Rules) |
| Authoritative keyword data | in-game keyword text (142 K codes x 4 languages) |
| Decision records | workspace-root `决策日志.md` (append-only) |
| Pending measurement / integration list | workspace-root `留空.md` |
| Concrete-card crate | [[Crate-Cards-EN]] |
| Action cards / pile / hand | [[定义层-Action-Card-EN]]、[[定义层-State-EN]] |
| Character family | [[定义层-Character-EN]] |
| Dice and cost | [[定义层-Resources-EN]] |

## 6. Maintenance Conventions

- **Re-paste code when it changes**: section-2 source is verbatim from the repository; re-paste and update the footer.
- **New cards**: template + one row in section 4; Chinese page `卡牌研究-<name>.md`, English page `卡牌研究-<name>-EN.md`.
- **Test results** only use: match / mismatch / pending test; a mismatch states the exact difference (punctuation, number, etc.).
- **Order conclusions**: one sentence + reproducible steps in section 3, D numbers in 4.2; no long interaction chains.
- `_Sidebar.md` / `_Footer.md` are maintained centrally.

---

Last verified: 2026-10-19 (code: `new/crates/cards/src/cards/character/aino/mod.rs:AINO`)
