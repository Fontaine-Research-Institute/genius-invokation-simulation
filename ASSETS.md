# 素材与许可（Assets and Licensing）

> 中文为主，英文摘要见文末。

## 一、总原则

1. **代码**：本仓库源码以 **Apache-2.0** 授权（见 `LICENSE`）。
2. **图片素材**（卡图、卡面截图、图标等）：**不属于 Apache-2.0 授权范围**。
   - 版权归 **miHoYo / HoYoverse**（《原神》及《七圣召唤》官方素材）所有；
   - 仅用于文档说明、开发对照与非商业同人用途；
   - 不得单独再分发、不得用于商业用途。
3. **每张**放入仓库的官方素材图片，都必须在 `assets/SOURCES.md` 登记一行：文件路径、来源、版权归属。
4. 未登记来源的图片不得提交进仓库。
5. 除文档说明用途外，框架与卡牌数据代码**不内嵌**任何官方美术资源。

## 二、目录约定

| 路径 | 用途 | 是否受 Apache-2.0 |
|---|---|---|
| `assets/cards/` | 卡牌图片（卡面 / 图标），供 wiki 与文档引用 | 否，版权归 miHoYo / HoYoverse |
| `assets/diagrams/` | 我们自绘的架构图、流程图 | 是 |
| `assets/SOURCES.md` | 素材来源清单（图片 → 来源 → 版权） | 是（清单文本自身） |

## 三、引用图片时必须写的版权行（固定文本，不得改写）

**凡是在文档 / Wiki / README / 页面里引用官方素材图片，图片下方必须原样写这两行：**

```text
图片来源：米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有。
Image source: official Genshin Impact artwork by HoYoverse. All rights belong to miHoYo / HoYoverse.
```

Markdown 写法（两行共用一段小字）：

```html
<p><sub>图片来源：米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有。<br>Image source: official Genshin Impact artwork by HoYoverse. All rights belong to miHoYo / HoYoverse.</sub></p>
```

- 多张图放在一起时，可共用同一段版权行（放在图片组下方）。
- **不得删减、不得只写其中一行、不得改写措辞。**

## 四、登记格式

`assets/SOURCES.md` 每行一条：

```text
| 文件 | 来源 | 版权 |
|---|---|---|
| assets/cards/example.png | <来源 URL 或来源说明> | 米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有 |
```

新增图片时：

1. 下载到 `assets/cards/`（或 `assets/diagrams/`）；
2. 在 `assets/SOURCES.md` 追加一行；
3. 在引用该图片的文档里粘贴 §三 的两行版权文本。

## 五、仓库根的其他声明

- `NOTICE.md`：项目层面的归属与商标声明；
- `LICENSE`：Apache-2.0 全文。

## 六、English summary

- Source code is licensed under **Apache-2.0**.
- **Image assets** (card art, screenshots, icons) are **NOT** covered by Apache-2.0. They are © **miHoYo / HoYoverse**, used only for documentation, development reference and non-commercial fan content.
- Every official image must be registered in `assets/SOURCES.md` with its file path, source and copyright holder.
- Whenever an official image is used in any document, the two-line credit block in section 3 must be reproduced verbatim beneath the image.
- Images without a registered source must not be committed.
