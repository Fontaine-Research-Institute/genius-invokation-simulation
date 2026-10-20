# Card Template: Action Card · Event

> [[卡牌定义-模板-行动牌-事件|中文]] · English

*Scope: writing template for action card · event; mirrors the character template [[卡牌定义-模板]] (Chinese reference).*

> [!IMPORTANT]
> General hard rules:
>
> 1. No card ids, no keyword ids, no external data paths;
> 2. Our own prose uses half-width punctuation and double quotes; copy card-face text and keep its original punctuation;
> 3. Keep a description as one paragraph; put metadata on separate quote paragraphs with an empty `>` line between them;
> 4. Omit any cost that is zero; omit the Energy line when there is no Energy;
> 5. Fence code as ```rust and keep definition code and implementation code separate;
> 6. Put the card image under the card name and the two fixed credit lines under the image; do not add skill icons;
> 7. For multi-mode cards give each mode its own `### Mode name` subsection with description and implementation.
> 8. Link keywords: `[[Keyword-<Name>|<Name>]]`; never write bare keyword ids.

---

## Example Event

![Example Event](assets/action_card/<id>.jpg)

> 图片来源：米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有。
> Image source: official Genshin Impact artwork by HoYoverse. All rights belong to miHoYo / HoYoverse.

> Event Card
>
> Tags: Food / Arcane Legend / Elemental Resonance / Combat Action
>
> Cost: 1 Hydro Elemental Die

```rust
// Example Event definition code
```

### Mode One

> <mode One description here>

```rust
// Mode One implementation code
```

### Mode Two

> <mode Two description here>

```rust
// Mode Two implementation code
```

---

## Writing Notes

> [!WARNING]
> Consecutive `>` lines merge into one paragraph on GitHub, so keep an empty `>` line between metadata items.
>
> Fence code as ```rust with no space; a space breaks highlighting.
>
> Omit zero costs entirely; do not write 0 Hydro Elemental Dice or 0 Energy.
>
> Card image path: `assets/action_card/<id>.jpg`; register each new image in `new/assets/SOURCES.md`.
>
> Do not use skill icons.

---

Last verified: 2026-10-20 (doc: `new/docs/wiki/卡牌定义-模板-行动牌-事件-EN.md`)
