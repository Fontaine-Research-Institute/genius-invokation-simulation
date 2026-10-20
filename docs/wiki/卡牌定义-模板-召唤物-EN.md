# Card Template: Summon

> [[卡牌定义-模板-召唤物|中文]] · English

*Scope: writing template for summon; mirrors the character template [[卡牌定义-模板]] (Chinese reference).*

> [!IMPORTANT]
> General hard rules:
>
> 1. No card ids, no keyword ids, no external data paths;
> 2. Our own prose uses half-width punctuation and double quotes; copy card-face text and keep its original punctuation;
> 3. Keep a description as one paragraph; put metadata on separate quote paragraphs with an empty `>` line between them;
> 4. Omit any cost that is zero; omit the Energy line when there is no Energy;
> 5. Fence code as ```rust and keep definition code and implementation code separate;
> 6. Put the card image under the card name and the two fixed credit lines under the image; do not add skill icons;
> 7. Write Usages / Duration only when present; otherwise omit.
> 8. Link keywords: `[[Keyword-<Name>|<Name>]]`; never write bare keyword ids.

---

## Example Summon

![Example Summon](assets/summon/<id>.jpg)

> 图片来源：米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有。
> Image source: official Genshin Impact artwork by HoYoverse. All rights belong to miHoYo / HoYoverse.

> Summon
>
> [[Keyword-Usage|Usage(s)]]: 2
>
> [[Keyword-Duration|Duration (Rounds)]]: 1 Round

```rust
// Example Summon definition code
```

> <card-face description here>

```rust
// Example Summon implementation code
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
> Card image path: `assets/summon/<id>.jpg`; register each new image in `new/assets/SOURCES.md`.
>
> Do not use skill icons.

---

Last verified: 2026-10-20 (doc: `new/docs/wiki/卡牌定义-模板-召唤物-EN.md`)
