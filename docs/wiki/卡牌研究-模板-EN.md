# Card Research - <Name>

> [[卡牌研究-模板|中文]] · English

*Scope: <Name> - card text (Chinese / English originals + keyword expansion + related rulebook entries), implementation code (real source / planned code + missing work), test records, and notes.*

> [!NOTE]
> How to use: copy this page, rename it to `卡牌研究-<Name>-EN.md`, fill in the 4 sections, and add a row to section 4 of [[卡牌研究-EN]].

> [!IMPORTANT]
> Hard writing rules:
> 1. No data-file paths or source annotations; write the card-face text itself only.
> 2. No numeric card ids; use names (talent / status / character / event / summon names).
> 3. No "talent card" wording in `§1.1`.
> 4. Type / tags / cost and other metadata: one item per line.
> 5. When a capability is discussed, paste its real source immediately after; do not pile code at the page end and do not just link.
> 6. Card text verbatim, code in full; when unsure write "pending test", never invent.

---

## 1. Card Text

### 1.1 Card data

| Item | Chinese | English |
|---|---|---|
| Name | <Chinese name> | <English name> |
| Family | <character / action card / summon / status> | <family> |
| Type | <type, if any> | <type> |
| Tags | <tags one by one> | <tags> |
| Values | <HP / Energy / cost, one per line> | <same> |

### 1.2 Card text (Chinese / English verbatim)

> Chinese original:
> <paste line by line>

> English original:
> <paste line by line>

### 1.3 Keyword expansion

| K code | Name | Keyword description | Where it appears |
|---|---|---|---|
| K<code> | <name> | <description> | <position> |

Related entities (by name):

| Name | Description | Where it appears |
|---|---|---|
| <status / summon / character name> | <description> | <position> |

### 1.4 Related rulebook entries (excerpts)

> Rulebook, <item>:
> <quote>

## 2. Implementation Code

> [!IMPORTANT]
> Implemented: paste the **full** real source (verbatim, file path, no excerpts, including `use` / constants / tests and other boilerplate), and paste it **where the capability is discussed**.
> Unimplemented: paste the **planned full code**, mark `[!WARNING] not implemented` at the start of this section, and list what is missing by capability.

### 2.1 <capability / file>

Description: <what this capability is>.

```rust
// Paste the matching real source immediately after the description (or planned code).
```

### 2.2 <capability / file>

Description: <what this capability is>.

```rust
// Paste the matching real source immediately after the description.
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
| [[卡牌研究-EN]] | column index and writing requirements |

---

Last verified: YYYY-MM-DD (code: `new/crates/...:<symbol>`)
