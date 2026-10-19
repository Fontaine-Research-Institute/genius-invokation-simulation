# Card Research - <Name>

> [[卡牌定义-模板|中文]] · English

*Scope: <Name> - card text (Chinese / English + keyword expansion + related rulebook entries), implementation code (real source / planned code + missing work), test records, and notes.*

> [!NOTE]
> How to use: copy this page, rename it to `卡牌定义-<Name>-EN.md`, fill in the 4 sections, and add a row to section 4 of [[卡牌定义-EN]].

> [!IMPORTANT]
> Hard writing rules:
> 1. No external data paths or source annotations; write the card-face text itself only.
> 2. No numeric card ids; use names (talent / status / character / event / summon names).
> 3. No keyword ids (they are ids too); use the keyword name or a one-line description.
> 4. Do not merge the two forms into one table: give 始基力：荒性形态 and 始基力：芒性形态 separate tables and write the full form names.
> 5. Passive skills have no cost field: do not list a cost row at all.
> 6. No wording the user explicitly asked to remove in `§1.1`.
> 7. Metadata one item per line: type / tags / cost, each on its own line.
> 8. Half-width punctuation only (the full-width colon inside the form names 始基力：荒性形态 / 始基力：芒性形态 is the only exception).
> 9. Copy the card-face text and keep the original punctuation; quoted blocks are the only place that keeps full-width punctuation (our own prose stays half-width).
> 10. Paste the matching real source immediately after each capability description; do not pile code at the page end and do not just link.
> 11. Paste code in full; when unsure write "pending test", never invent.
> 12. No tables for skills: each skill is three lines: skill name / skill type (Normal Attack / Elemental Skill / Elemental Burst / Passive Skill / Talent, no prefix) / ">" the card-face text copied exactly; passive skills list no cost, cost numbers go on their own line; modes use the same three-line form.
> 13. Replace keyword ids in code-block comments with names and add a one-line note above the block; code lines stay as in the repository.
> 14. Full cost wording: no shorthand; write "x Hydro Elemental Dice + n Any Elemental Dice / n Same Elemental Dice"; dice cost and Energy on separate lines; write Energy as "x Energy"; omit Energy when none; omit both lines for passive skills.

---

## 1. Card Text

### 1.1 Card data

| Item | Chinese | English |
|---|---|---|
| Name | <Chinese name> | <English name> |
| Family | <character / action card / summon / status> | <family> |
| Type | <type, if any> | <type> |
| Tags | <tags one by one> | <tags> |
| Values | <HP / Energy / cost, one per line; passive skills: no cost> | <same> |

### 1.2 Description (Chinese / English)

Skills and modes are written line by line in a fixed order (no tables); dice cost is one line and Energy another; omit the line when there is no Energy or for passive skills.

<skill name>
<skill type: Normal Attack / Elemental Skill / Elemental Burst / Passive Skill / Talent>
<dice cost: x Hydro Elemental Dice + n Any Elemental Dice / n Same Elemental Dice>
<x Energy, omit this line when none>
> <card-face text copied exactly, original punctuation kept>

<mode name>
<role and condition>
> <card-face text copied exactly, original punctuation kept>

### 1.3 Keyword expansion

| Keyword | Description | Where it appears |
|---|---|---|
| <keyword name> | <description> | <position> |

Related entities (by name):

| Name | Description | Where it appears |
|---|---|---|
| <status / summon / character name> | <description> | <position> |

### 1.4 Related rulebook entries (excerpts)

> Rulebook, <item>:
> <quote, half-width punctuation>

## 2. Implementation Code

> [!IMPORTANT]
> Implemented: paste the full real source (file path, no excerpts, including `use` / constants / tests and other boilerplate), and paste it where the capability is discussed.
> Unimplemented: paste the planned full code, mark `[!WARNING] not implemented` at the start of this section, and list what is missing by capability.

### 2.1 <capability / file>

Description: <what this capability is>.

```rust
// Paste the matching real source immediately after the description above (or the planned code).
```

### 2.2 <capability / file>

Description: <what this capability is>.

```rust
// Paste the matching real source immediately after the description above.
```

### 2.x Not implemented & what is missing (required when unimplemented)

> [!WARNING]
> Not implemented.

| Capability | Status | Missing |
|---|---|---|
| <capability> | not implemented / partial | <specific interface / data> |

## 3. Tests

| Item | Position / operation | In-game result | Our result | Conclusion |
|---|---|---|---|---|
| <item> | <concrete position + reproducible operation> | <measured / pending test> | <our result / test name> | match / mismatch / pending test |

**Reproducible verification steps**:

1. <step 1: build the position, perform the operation, observe the result>
2. <step 2>

## 4. Notes

### 4.1 Pitfalls

| Pitfall | Detail |
|---|---|
| <pitfall> | <wrong spelling -> why wrong -> correct spelling> |

### 4.2 Related D numbers

| No. | Conclusion |
|---|---|
| D<number> | <conclusion> |

### 4.3 Related pages

| Page | Content |
|---|---|
| [[卡牌定义-EN]] | column index and writing requirements |

---

Last verified: YYYY-MM-DD (code: `new/crates/...:<symbol>`)
