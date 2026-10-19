# tools —— 代码扫描脚本

本目录放 CI 里跑的三个「代码风格扫描器」。它们只打印普通提示日志，退出码恒为 0，
不会让 CI 变红，也不会产生任何 GitHub 注解（warning / notice 之类）。

三个扫描脚本均为「只读」：文件用共享只读方式打开（不加锁、不独占，读取期间别人可以照常编辑），
不创建 / 写入任何文件（不产出报告文件），结果只打 stdout。扫描期间文件被改 / 删除 / 读不出来时，
由脚本自己记入「读取失败」并跳过该文件，继续扫其余文件，不中断 job（D73）。

`ignored_index.txt` 是 `check_no_index.py` 的下标忽略清单（格式见第 3 节）。

扫描范围排除的基础库目录（D75）：`crates/richtext/` —— 富文本解析器，
按「位置 / 长度 + 字节级解析」实现，用 `usize` / `u32` 与内部 `x[i]` 按位置切字符串是合理的；
它不属于 definitions / solver / app 这类业务层。以后再有同类基础库，加进两个脚本的 `EXCLUDED_DIRS`。

## 1. check_nat_width.py —— 非 32 位 Nat 用法

查什么（依据：分工计划.md §8.3、决策日志 D37）：

- 递归扫描 crates/ 下所有 *.rs，找 `Nat8` / `Nat16` / `Nat64` / `Nat128` 这些标识符（词边界匹配）；
- 跳过 crates/engine/src/nat.rs（那里是一族宽度的定义与自身测试）；
- 排除基础库目录 `crates/richtext/`（D75：富文本解析器，位置 / 长度 + 字节级解析，
  用 `usize` / `u32`、内部按位置切片是合理的；不属于 Nat 宽度的业务层）。
- 同一行带 `// nat-width-ok: <理由>` 的算「已登记」，其余算未登记；
- 注释 / 字符串 / 字符字面量里的提及不算「用法」，会单独列在「仅供参考」一节。

背景：默认一律用 `Nat`（= `Nat32`）；故意用别的宽度必须登记到 git 之外的不入库/nat-宽度统计.md，
并在代码该行写 `// nat-width-ok: <理由>` 作为白名单标记。

输出：只打 stdout 的中文人读日志（不产出任何报告文件）；扫描期间读不出的文件会列在
「读取失败」一节，最后统计「读取失败 N 个」。

本地跑（在 `new/` 里）：

    python tools/check_nat_width.py
    python3 tools/check_nat_width.py

可选参数：`--root` 指定扫描根目录（默认 `crates/`）。脚本不写文件，所以没有 `--report`。

## 2. check_bare_int.py —— 裸整型 / 裸算术

查什么（依据：分工计划.md §8.3，只扫上层 crate）：

- 扫描 crates/definitions、crates/cards（具体卡牌，D240）、crates/solver、crates/app 下的 *.rs；
  目录不存在时打印「尚未出现，跳过」并正常结束（definitions / cards 已出现后会真的扫描它们）；
- 先去掉注释与字符串字面量，再找两类东西：
  1. 裸整型类型名：`u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize`；
  2. 手写的二元 `+ - * /`；
- 排除非算术用法：解引用 `*p`、glob `use ...::*;`、指针类型 `*const` / `*mut`、
  trait bound（`T: A + B` / `dyn` / `impl`）、一元负号 `-x`、箭头 `->`；
- 白名单：`crates/definitions/src/base/`（历史写法 `definitions/base/` 也认）下的 id 类型声明（整目录）；
  `#[repr(u8)]` 这类属性行。`.len()` 之类的 `usize` 返回值不在白名单（按规格必须改用 Nat 或新类型）。
  路径匹配前会先规范化（反斜杠转 `/`、去重复斜杠、去前导 `./`），不依赖前导斜杠，
  避免 `crates/definitions/src/base/...` 这种相对路径匹配不上。
- 契约白名单：某行含裸整型、且行尾带 `// bare-int-ok: <理由>` 时，该行不计命中，
  改计入「契约白名单跳过」并逐行列出（便于审计，防止滥用）；用于引擎 API 契约
  （如 `SelectionSource` 的 `usize`）这类无法避免的地方。风格与 `nat-width-ok:` 一致。

输出：中文人读日志（stdout），只读；读取失败会列在「读取失败」一节，退出码恒 0。

本地跑（在 `new/` 里）：

    python tools/check_bare_int.py
    python3 tools/check_bare_int.py

也可以临时指定要扫的目录（自测用，默认会替换掉三个默认目标）：

    python tools/check_bare_int.py path/to/definitions

自测（验证路径规范化后的 definitions/src/base 白名单没失效）：

    python tools/check_bare_int.py --self-test

纯内存 fixture，不写磁盘：先用多组路径（相对路径 / 反斜杠 / 绝对路径 / 历史写法 `definitions/base/`）
断言 `is_base_file` 判定正确，再对 `definitions/src/base/id.rs`（`u32` 必须被豁免）、
`definitions/src/foo.rs`（裸整型 / 裸算术必须命中）、以及契约白名单（带 `// bare-int-ok:`
标记的行不计命中、不带的照常命中）做扫描断言；打印「通过 / 失败」，退出码恒 0。

## 3. check_no_index.py —— 取值式下标（全树）

目的（用户要求 / D69、D70）：标签与文本数据必须字段访问（`TAGS.location`），
不许下标取数（`x[x]`）。

- 扫描范围：`crates/` 下所有 `*.rs`（含 tests），全树，不限于 tags/、generated/；
  但排除基础库目录 `crates/richtext/`（D75：富文本解析器，位置 / 长度 + 字节级解析，
  用 `usize` / `u32`、内部 `x[i]` 按位置切字符串是合理的，不属于业务层规则）；
- 判定：找「取值式下标」——表达式后紧跟 `[`（标识符 `[`、`)[`、`][`，如 `x[i]` / `foo() [i]` / `a[0][1]`）；
- 排除：属性 `#[...]` / 内部属性 `#![...]`、类型位置（`[T; N]` / `&[T]` / `Vec<[u8; 4]>` /
  `: [T]` / `&'a [T]` / `&mut [T]`）、字符串与字符字面量、注释、宏调用 `name![...]`；
- 忽略清单：`tools/ignored_index.txt`，每行 `<相对路径> :: <该处唯一代码片段>    # 理由`；
  匹配用「路径精确 + 片段在命中行内精确子串出现」，不用行号；
  片段在该文件里必须只出现 1 次：0 次 = 失效（提醒清理），>1 次 = 不成立
  （条目不生效，对应下标按未忽略处理；不影响其它条目、不中断扫描）；
- 输出：只打 stdout 的中文人读日志（不产出报告文件），分四块：
  未忽略命中 / 已失效条目 / 多条命中→忽略不成立 / 读取失败，外加统计；
- 忽略清单也只读，发现失效或不成立条目只报告、绝不自动回写；
- 退出码恒 0，不产生任何 GitHub 注解。

本地跑（在 `new/` 里）：

    python tools/check_no_index.py
    python3 tools/check_no_index.py

自测（纯内存 fixture）：

    python tools/check_no_index.py --self-test

覆盖：未忽略命中 / 已忽略命中 / 失效条目（片段 0 次）/ 不成立条目（片段 2 次，且不影响其它条目）/
读取失败时不误判失效 / 排除项（`#[derive]`、`&[T]`、`[T; 4]`、字符串里的 `[`、`name![…]`）。

## 4. CI 在哪跑

`.github/workflows/ci.yml` 的两个 job 都会跑这三个脚本：

- `build-linux`（ubuntu-latest）：cargo check / build / clippy 全部带 `--locked` + `python3` 跑三个扫描；
- `test-windows`（windows-latest）：cargo test 带 `--locked` + `python` 跑三个扫描；
- `ci-ok`：汇总 job，分支保护只勾这一个。

扫描脚本只提示、只读、只打 stdout，不上传 artifact；job 状态只由 cargo check / build / clippy / test 决定。
workflow 顶层设了 `PYTHONUTF8: 1`，脚本自身也会强制 stdout/stderr 用 UTF-8，
所以 Windows runner 的 cp1252 环境下中文日志不会抛 UnicodeEncodeError。

## 5. 已知局限

- 三个扫描都是「源文本级」启发式（宏展开之前），故意绕开总能绕开；
- 裸算术扫描会把跨行的二元表达式也算进去（按整份源码扫描），但 `a + b + c` 这种
  同一行多次命中只报一行；
- 注释 / 字符串剥离支持嵌套块注释、原始字符串（`r#"..."#` / `br"..."`）、字符字面量与生命周期区分；
- Nat 宽度扫描按「代码用法」统计；只在文档注释里提到 `Nat8/16/64/128` 不会造成未登记提示；
- 下标扫描会命中解析器内部按位置切片、测试断言等必要用法，这些登记在 tools/ignored_index.txt；
  日志里的「失效 / 不成立」两节就是用来防止清单腐烂的，请及时清理或把片段改得更特殊。
