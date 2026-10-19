# 参与贡献

感谢你愿意参与这个项目。本文说明怎么构建、怎么改、怎么提交；动手之前请先读一遍。

## 1. 这个项目是什么

用 Rust 实现的粉丝自制七圣召唤规则引擎、对局模拟与精确概率求解器，采用 Apache License 2.0 许可。项目定位与核心设计见 [README.md](README.md)（英文见 [README.en.md](README.en.md)）。

## 2. 构建与测试

在仓库根目录下：

```bash
cargo build
cargo test
```

- 本机可能断网，依赖已缓存时建议加 `--offline`：

  ```bash
  cargo build --offline
  cargo test --offline
  ```

- CI 全程使用 `--locked`。如果改动了依赖，记得同步更新并提交 `Cargo.lock`，否则 `cargo check --locked` 会失败。
- Windows 上若链接报错，把 `TEMP` 与 `TMP` 指到工作区内的 `.tmp` 再构建：

  ```powershell
  New-Item -ItemType Directory -Force .tmp | Out-Null
  $env:TEMP = (Resolve-Path .tmp).Path
  $env:TMP  = (Resolve-Path .tmp).Path
  ```

  `.tmp/` 只在本机使用，不要提交。

## 3. feature 怎么开

| feature | 作用 | 说明 |
| --- | --- | --- |
| `rules` | 求解接口 `GameRules` | 默认不编译 |
| `dedup` | 概率去重 | 只为计算优化，不改分布 |
| `opt-all` | 全部计算优化 | 包含 `dedup` |
| `dll` | 机制 dll 加载 | **尚未实现**，当前打开会编译失败，不要用它构建或测试 |

示例：

```bash
cargo test --features rules,dedup,opt-all
```

## 4. 代码红线

以下几条是本项目正确性的前提，评审时会逐条看：

1. **数值只能用 `Nat`**（默认 `Nat32`），不要用裸整型。故意使用其它宽度（`Nat8/16/64/128`）必须在那一行写 `// nat-width-ok: 理由`，并登记到维护者的宽度统计里。
2. **算术只能用 `nadd!` / `nsub!` / `nmul!` / `ndiv!`**，不要手写 `+ - * /`；唯一实现处是 `crates/engine/src/nat.rs`。
3. **概率不许用 `f64` 累加**。概率只能来自 `enumerate_random` / `enumerate_select` 枚举出的分支结果；自己用 `f64` 累加或近似就失去了精确性。
4. **随机与选择必须走引擎的 `enumerate_random` / `enumerate_select`**，不许自建 RNG、不许用系统随机、不许自己绕开枚举抽牌或选目标。
5. **敌我不许用数字编码、不许传 `side` 参数**：要么建两个具名实例（如 `root_me` / `root_opp`），要么提供成对入口（如 `draw_me()` / `draw_opp()`），共用实现只写一份。
6. **卡牌代码只存识别信息与逻辑，不存文案**。卡牌里不写展示用字符串，文案归文案层。
7. **每张卡一个文件夹、自己注册，没有集中登记表**。新增卡牌 = 新建文件夹并在自己的入口注册，不要往任何集中列表里加一行。
8. **框架里不许做中枢统计或内建行为表**。框架只提供接口与原语；具体功能由卡牌/牌族自己实现。

改完请跑一遍两个扫描器，确认没有新增命中：

```bash
python tools/check_nat_width.py
python tools/check_bare_int.py
```

它们只打印提示、退出码恒为 0，但新命中必须在评审前清掉或按规则登记。

## 5. 该怎么写 / 不该怎么写

依据决策 D71：编码规范只认下面这些写法，每条附一句理由，示意为主、不要求逐字一致。

**1. 数据访问：字段，不是下标** —— 字段访问编译后是基址 + 固定偏移。
```text
✅ TAGS.location.name.zh_cn
❌ TAGS.fields[i] / texts[lang.index()]   // 靠下标 / 查表，运行时才定位
```

**2. 标签：一个标签一个零大小类型，数据只有一份**
```text
✅ pub struct Location;  pub fn data() -> &'static TagText { &TAGS.location }
❌ enum Tag { Location } / "location"；标签文件里再写一份名字字面量   // 第二份数据
```

**3. 数值与算术** —— 除零 panic（逻辑错误），溢出饱和（正常业务）。
```text
✅ Nat + nadd! / nsub! / nmul! / ndiv!
❌ u32 / i32 裸类型；手写 a + b；max/min 兜底
```

**4. 随机与选择** —— 枚举出全部分支，概率才精确。
```text
✅ enumerate_random / enumerate_select
❌ 自己调 RNG 采样；自己写「随机挑一个」
```

**5. 敌我：两个具名实例 + 成对入口** —— 规则只写一份。
```text
✅ hand_me() / hand_opp()；DefState { me, opp }
❌ 传 side 参数；用 0/1 或数组下标区分敌我
```

**6. 卡牌只存识别信息与逻辑** —— 文案归文案层。
```text
✅ family / id / type / tags + 效果逻辑；文案从 generated 取
❌ 卡牌代码里写死描述字符串；在逻辑里按名字找卡 / 比字符串
```

**7. 注册** —— 没有集中登记表，新增卡不碰公共文件。
```text
✅ 每张卡在自己文件里 register_card!，链接期收集
❌ 在某个 mod.rs 里列一长串卡
```

**8. 忽略清单**：`tools/ignored_index.txt` 每条写成 `<路径> :: <该文件里唯一的代码片段> # 理由`；片段出现 0 次 = 失效、>1 次 = 不成立，都必须处理。

## 6. 提交与合并

- `main` 受保护：只能通过 PR 合入，且需要 1 个 approval。
- 合并方式用 **squash merge**，保持线性历史；请把一个 PR 收敛为一组相关改动。
- CI 的 `ci-ok` 汇总 job 必须为绿（Linux 构建 + clippy、Windows 测试、两个扫描脚本）。
- 提交信息建议 `type(scope): 摘要`：
  - type：`feat` / `fix` / `refactor` / `docs` / `test` / `chore`
  - scope 示例：`engine`、`nat`、`random`、`tools`、`ci`、`docs`
  - 示例：`fix(nat): ndiv! 除零返回 ZERO`、`docs: 补充贡献指南`

## 7. 设计变更流程

设计文档由维护者保管、不在本仓库内，包括分工计划、结构文档与决策日志。流程是**先改设计，再改代码**：

1. 提出设计变更，由维护者评审并给出变更编号（Dxx）。
2. 按新决策改代码。
3. PR 描述里引用对应编号（例如「依据 D34」），方便回溯。

纯 bug 修复且不涉及设计取舍时，说明清楚即可；拿不准就先开 Issue 问。

## 8. 怎么问问题

- 用 Issue 模板：缺陷选 Bug 报告，想法选功能建议；请把模板字段填完整。
- 提问前先读 README 的「核心设计」一节，很多前提（引擎边界、实体与区、随机/选择入口、敌我不传参）都在那里。
- 安全漏洞**不要**公开提 Issue，请按 [SECURITY.md](SECURITY.md) 走私密上报。
