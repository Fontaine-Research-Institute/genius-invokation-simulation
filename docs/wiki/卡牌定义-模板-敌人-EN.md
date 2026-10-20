# Card Template: Enemy

> [[卡牌定义-模板-敌人|中文]] · English

*Scope: writing template for enemy; mirrors the character template [[卡牌定义-模板]] (Chinese reference).*

> [!IMPORTANT]
> General hard rules:
>
> 1. No card ids, no keyword ids, no external data paths;
> 2. Our own prose uses half-width punctuation and double quotes; copy card-face text and keep its original punctuation;
> 3. Keep a description as one paragraph; put metadata on separate quote paragraphs with an empty `>` line between them;
> 4. Omit any cost that is zero; omit the Energy line when there is no Energy;
> 5. Fence code as ```rust and keep definition code and implementation code separate;
> 6. Put the card image under the card name and the two fixed credit lines under the image; do not add skill icons;
> 7. Enemy skills mirror character skills; put the enemy card description last.
> 8. Link keywords: `[[Keyword-<Name>|<Name>]]`; never write bare keyword ids.

---

## Example Enemy

![Example Enemy](assets/enemy/<id>.jpg)

> 图片来源：米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有。
> Image source: official Genshin Impact artwork by HoYoverse. All rights belong to miHoYo / HoYoverse.

> Enemy

```rust
// Example Enemy definition code
```

### Example Skill One

> Normal Attack
>
> Cost: 1 Hydro Elemental Die

```rust
// Skill definition code
```

> <skill description here>

```rust
// Skill implementation code
```

### Example Skill Two

> Elemental Burst
>
> Cost: 4 Hydro Elemental Dice
>
> 2 [[Keyword-Energy|Energy]]

```rust
// Elemental Burst definition code
```

> <Elemental Burst description here>

```rust
// Elemental Burst implementation code
```

### Example Passive Skill

> Passive Skill

```rust
// Passive skill definition code
```

> <passive description here>

```rust
// Passive skill implementation code
```

> <enemy card description here>

---

## Writing Notes

> [!WARNING]
> Consecutive `>` lines merge into one paragraph on GitHub, so keep an empty `>` line between metadata items.
>
> Fence code as ```rust with no space; a space breaks highlighting.
>
> Omit zero costs entirely; do not write 0 Hydro Elemental Dice or 0 Energy.
>
> Card image path: `assets/enemy/<id>.jpg`; register each new image in `new/assets/SOURCES.md`.
>
> Do not use skill icons.

---

Last verified: 2026-10-20 (doc: `new/docs/wiki/卡牌定义-模板-敌人-EN.md`)
