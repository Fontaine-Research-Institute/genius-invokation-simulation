# Card Template: Status

> [[卡牌定义-模板-状态|中文]] · English

*Scope: writing template for status; mirrors the character template [[卡牌定义-模板-EN]].*

> [!IMPORTANT]
> General hard rules:
>
> 1. No card ids, no keyword ids, no external data paths;
> 2. Our own prose uses half-width punctuation and double quotes; copy card-face text and keep its original punctuation;
> 3. Keep a description as one paragraph; put metadata on separate quote paragraphs with an empty `>` line between them;
> 4. Omit any cost that is zero; omit the Energy line when there is no Energy;
> 5. Fence code as ```rust and keep definition code and implementation code separate;
> 6. Put the card image under the card name (except for status pages) and the two fixed credit lines under the image; do not add skill icons;
> 7. Status pages use no card image and are not registered in the image sources list; the status type is Character Status or Combat Status.
> 8. Link keywords: `[[Keyword-<Name>|<Name>]]`; never write bare keyword ids.

---

## Example Status

> <Character Status / Combat Status>
>
> [[Keyword-Usage|Usage(s)]]: 2
>
> [[Keyword-Duration|Duration (Rounds)]]: 1 Round

File: <file path>

```rust
// Example Status definition code
```

> <card-face description here>

File: <file path>

```rust
// Example Status implementation code
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
> Status pages use no card image and are not registered in the image sources list.
>
> Do not use skill icons.

## Summary

The status template mirrors the summon template but uses Character Status or Combat Status. Status pages have no card image and are not registered in the image sources list; write Usages and Duration only when present.

---

Last verified: 2026-10-20 (doc: `new/docs/wiki/卡牌定义-模板-状态-EN.md`)
