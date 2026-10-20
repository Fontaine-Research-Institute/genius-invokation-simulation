# Card Template: Character (example: Furina)

> [[卡牌定义-模板|中文]] · English

*Scope: writing template for cards. This page is the character template (including multi-form characters); other card types have their own template pages.*

> [!IMPORTANT]
> General hard rules:
>
> 1. No card ids, no keyword ids, no data-file paths;
> 2. Our own prose uses half-width punctuation and double quotes; copy card-face text and keep its original punctuation;
> 3. Keep a description as one paragraph; only metadata (tags / caps / cost / Energy) gets separate lines, with an empty `>` between items;
> 4. **Omit any cost that is zero**: 0 Same Elemental Dice / 0 Any Elemental Dice / 0 Energy are all omitted; a whole zero-cost line is omitted;
> 5. Fence code as ```rust (no space); source must match the repository exactly; if unimplemented, paste the planned code and mark "not implemented";
> 6. Passive skills list no cost and no Energy;
> 7. Multi-form characters: the form name goes above the character name; write the character name and every skill once per form, even when names repeat;
> 8. Link keywords: `[[Keyword-<Name>|<Name>]]`; never write bare keyword ids.

---

## 始基力：荒性形态

## Furina

> Hydro, Sword, Fontaine, 始基力：荒性
>
> Max HP 12
>
> Max [[Keyword-Energy|Energy]] 2

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Furina (始基力：荒性形态) character definition code
```

### Soloist's Solicitation

> Normal Attack
>
> Cost 1 Hydro Elemental Die + 2 Any Elemental Dice

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Soloist's Solicitation definition code
```

> Deals 2 Physical DMG.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Soloist's Solicitation implementation code
```

### Salon Solitaire

> Elemental Skill
>
> Cost 3 Hydro Elemental Dice

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Salon Solitaire definition code
```

> Deal 2 Hydro DMG with 1 stack of Agile Switch created. If you have a card with Empowerment in your Hand, deal 3 Hydro DMG instead.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Salon Solitaire implementation code
```

### Let the People Rejoice

> Elemental Burst
>
> Cost 4 Hydro Elemental Dice
>
> 2 [[Keyword-Energy|Energy]]

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Let the People Rejoice definition code
```

> Deals 2 Hydro DMG, creates 1 Universal Revelry.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Let the People Rejoice implementation code
```

### Arkhe: Seats Sacred and Secular

> Passive Skill

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Arkhe: Seats Sacred and Secular definition code
```

> Passive description that switches forms.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Arkhe: Seats Sacred and Secular implementation code
```

---

## 始基力：芒性形态

## Furina

> Hydro, Sword, Fontaine, 始基力：芒性
>
> Max HP 12
>
> Max [[Keyword-Energy|Energy]] 2

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Furina (始基力：芒性形态) character definition code
```

### Soloist's Solicitation

> Normal Attack
>
> Cost 1 Hydro Elemental Die + 2 Any Elemental Dice

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Soloist's Solicitation definition code
```

> Deals 2 Physical DMG.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Soloist's Solicitation implementation code
```

### Salon Solitaire

> Elemental Skill
>
> Cost 3 Hydro Elemental Dice

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Salon Solitaire definition code
```

> Deal 2 Hydro DMG with 1 stack of Agile Switch created. If you have a card with Empowerment in your Hand, deal 3 Hydro DMG instead.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Salon Solitaire implementation code
```

### Let the People Rejoice

> Elemental Burst
>
> Cost 4 Hydro Elemental Dice
>
> 2 [[Keyword-Energy|Energy]]

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Let the People Rejoice definition code
```

> Deals 2 Hydro DMG, creates 1 Universal Revelry.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Let the People Rejoice implementation code
```

### Arkhe: Seats Sacred and Secular

> Passive Skill

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Arkhe: Seats Sacred and Secular definition code
```

> Passive description that switches forms.

File: crates/cards/src/cards/character/furina/mod.rs

```rust
// Arkhe: Seats Sacred and Secular implementation code
```

## Summary

The character template writes the character name and every skill once per form, with metadata and descriptions in quote blocks. Keep definition code and implementation code separate, each block labeled with its file path.

---

## Writing Notes

> [!WARNING]
> Consecutive `>` lines merge into one paragraph on GitHub, so keep an empty `>` line between metadata items.
>
> Fence code as ```rust; adding a space breaks highlighting.
>
> Multi-form characters: write the form name first, then the character name; repeat the character name and every skill for each form.
>
> Omit zero costs: write "1 Hydro Elemental Die + 2 Any Elemental Dice" and do not list 0 Same Elemental Dice.
