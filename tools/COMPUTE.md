# 算力工具链：按「计算名」跑仓库里的 Rust 计算程序（D231 - D236）

一句话：想算什么，就把那个计算写成一个名字固定的 Rust 程序放进仓库；
之后只报名字就能编译并运行它，再按目录里的配置自动跑多组用例、比对答案文件里的正确答案。
**脚本本身不接收计算参数**；参数组合预先写在仓库的配置里。

## 1. 一个计算 = 一个文件夹（D232）

计算名不是一个 `.rs` 文件，而是**文件夹**——因为一个计算可能写多个 rs。

```
new/
├── Cargo.toml                        # workspace 成员里有 crates/compute
├── crates/compute/
│   ├── Cargo.toml                    # package: gi-genius-invokation-compute
│   └── src/bin/
│       └── ftk_state_count/          # ← 计算名 = 文件夹名
│           ├── compute.json          # 可选：用例列表 / 超时
│           ├── main.rs               # 入口（名字固定）
│           ├── helper.rs             # 该计算的子模块，可有任意多个
│           └── expected/             # 正确答案文件（只在配置里给地址，D236）
│               ├── default.json
│               └── big.json
├── tools/run-compute.py              # 按名字找到它、跑用例、比对答案、汇总
└── .github/workflows/compute.yml     # 手动触发；不同计算各自一个 runner 并行跑
```

规则：

- **计算名就是文件夹名**：只用小写字母、数字、`-`、`_`（kebab / snake 都行）；
  不能含路径分隔符或 `..`；精确匹配、区分大小写。
- **入口固定叫 main.rs**；同目录其它 `.rs` 用 `mod xxx;` 引进来当子模块。
- Cargo 会自动把 `src/bin/<名字>/main.rs` 识别成名为 `<名字>` 的 bin target，
  **不用改 Cargo.toml 的 [[bin]]**，加文件夹就完事。
- **结果写 stdout**（有正确答案时必须是一个合法 JSON 值；方便 CI 直接当 artifact）；
  **日志 / 进度写 stderr**，别污染 stdout。

## 2. 怎么加一个新计算

以计算名 `my_compute` 为例（kebab `my-compute` 也一样，文件夹名就是计算名）：

1. 建文件夹 `crates/compute/src/bin/my_compute/`。
2. 写入口 `crates/compute/src/bin/my_compute/main.rs`：

   ```rust
   //! 计算 my_compute：一句话说明算什么、输出什么。
   mod helper; // 需要多文件时这样引入同目录的 helper.rs

   fn main() {
       // 参数由 compute.json 用例的 cmd 传进来（cargo run 的 -- 之后）；
       // 脚本不解释它们的语义。
       let input = std::env::args().skip(1).next().unwrap_or_default();
       let answer = helper::compute(&input);
       // stdout 只放结果。
       println!("{{\"compute\":\"my_compute\",\"answer\":{answer}}}");
   }
   ```

3. 需要多个文件就在同目录加 `helper.rs` / `xxx.rs`，在 `main.rs` 里 `mod` 进来；
   想分层可以再建子目录（`mod sub;` -> `sub/mod.rs` 或 `sub.rs`，Rust 普通模块规则）。
4. 需要用到 engine / definitions / simulation（或第三方库）时，
   在 `crates/compute/Cargo.toml` 的 `[dependencies]` 里加；这会编译进全部计算，依赖越少越好。
5. 可选：写 `compute.json` 声明用例与答案文件地址（见第 3 节）。
6. 本地跑：`python tools/run-compute.py my_compute`；
   `python tools/run-compute.py --list` 里能看到它。

**不需要改**：脚本 run-compute.py、CI 的 compute.yml、根 Cargo.toml（crates/compute 已是 workspace 成员）。

## 3. compute.json：用例列表（D235 / D236）

每个计算目录可放一个 `compute.json`（可选）。**配置只放地址，不直接填答案内容**：
正确答案写在文件里，配置用 `expected_file` 指向它。

```json
{
  "cases": [
    { "name": "默认", "cmd": [],
      "expected_file": "expected/default.json" },
    { "name": "大样本", "cmd": ["--samples", "4000000"],
      "expected_file": "expected/big.json" },
    { "name": "只跑（无正确答案）", "cmd": ["--samples", "1234"] }
  ],
  "timeout_minutes": 10
}
```

字段：

| 字段 | 必填 | 含义 |
| --- | --- | --- |
| `cases` | 是 | 用例数组（至少一条）。同一计算内**串行**执行 |
| `cases[].name` | 否 | 用例名，默认「用例N」；只用于日志 / 汇总 |
| `cases[].cmd` | 否 | 该用例的执行参数（字符串数组，默认 `[]`）；原样拼在 `cargo run ... --` 之后传给该计算的 bin |
| `cases[].expected_file` | 否 | **正确答案文件的相对路径**（相对该计算目录，可带子目录，用 `/` 分隔）|
| `cases[].timeout_minutes` | 否 | 该用例「编译 + 运行」的超时（正数，分钟）；缺省用全局值 |
| `timeout_minutes` | 否 | 全局超时（正数，分钟）；用例自己的优先 |

### 3.1 正确答案只用文件（D236）

- 配置里**只允许** `expected_file`，不支持内联 `expected` 写内容；写了内联 `expected` 会直接报配置错误。
- `expected_file` 相对该计算目录：`"expected/default.json"` 可以，`"/abs/path.json"`、
  `"../other.json"`、反斜杠路径都不行。这样配置跟着计算目录走，换 OS / 换 runner 都能找到。
- 答案文件内容就是**程序 stdout 应有的 JSON**，脚本会做深度比较。
- **答案文件不存在不会静默通过**：打印明确警告，按「无正确答案」处理（只运行、输出、视作绿），
  并在用例汇总里标出「无(警告:答案文件缺失)」。--list 也会显示缺失条数。

### 3.2 运行行为

1. `python tools/run-compute.py <名字>` 会按 `cases` 顺序**串行**跑完所有用例；
2. 有 `expected_file` 且文件存在：运行程序 → 把 stdout 解析成 JSON → 与答案文件**深度比较**：
   - 一致 → 该用例通过；不一致 / stdout 不是合法 JSON → 该用例失败，打印 diff 摘要
     （第一处差异 + 期望与实际各前 40 行）；
   深度比较规则：对象按字段比（不要求字段顺序）；数组按元素比；数字按数值比（`1` 与 `1.0` 相等）；
   `true` 与 `1` **不**相等（Python 里 `True == 1` 为真，脚本已排除）。
3. 没有 `expected_file` / 答案文件缺失：**只运行并输出结果，视作成功**（stdout 照样透传）；
4. 某条用例失败后**继续跑完剩余用例**，最后打印汇总：
   每条用例的「名称 / 命令 / 是否有答案 / 通过与否 / 耗时」，以及绿 / 红 / 警告条数与总耗时；
5. 编译失败 / 运行非零退出 / 超时：该用例失败；所有用例都通过，整体退出码才是 0。

注意：`timeout_minutes` 覆盖**编译 + 运行**整个 `cargo run`；第一次 release 编译可能就要几十秒。

### 3.3 退出码

| 退出码 | 含义 |
| --- | --- |
| 0 | 全部用例通过（含「只运行、无正确答案」的视作成功） |
| 1 | 计算名非法 / 不存在 / 配置非法；`--all` 有红项时也返回 1 |
| 3 | stdout 不是合法 JSON，或与答案文件不一致 |
| 124 | 超过用例 / 全局 `timeout_minutes` |
| 其它 | 第一条失败的编译 / 运行退出码原样返回（常见 101）；127 = 找不到 cargo |

## 4. 为什么不传参数

用户口径：算东西的脚本「不实际负责传入算东西的参数，而是按照传入的算名字，
去仓库里找对应的算的东西的 rust 程序，编译然后算」。

落到这套工具链上：

- `run-compute.py` 的命令行只接受**一个计算名**；多写参数直接报错退出；
- 一个计算要跑哪些参数组合，**预先写在仓库里**（`compute.json` 的 `cases[].cmd`），
  脚本只负责把 `cmd` 拼在 `cargo run ... --` 之后传给 bin，不解释语义；
- 所以「算东西的参数」不是在触发时临时传的，而是随代码一起版本化的测试用例；
- 好处：CI 只有 `name` 一个可选输入；本地和 CI 跑的是同一组用例；
  正确答案固化在文件里，能当回归测试；参数拼错会变成配置 review 的问题，而不是运行时事故。

## 5. 本地怎么跑

```bash
cd new

# 列出全部计算名，并标出「有配置/无配置、用例条数、有答案/无答案条数、答案缺失条数」
python tools/run-compute.py --list

# 只列出带 compute.json 的计算名（每行一个；CI 的 discover 就调它）
python tools/run-compute.py --configs

# 按用例顺序跑一个计算：逐条运行、比对、最后打印用例汇总（stdout 透传）
python tools/run-compute.py ftk_state_count

# 依次跑所有带 compute.json 的计算，最后打绿/红汇总
python tools/run-compute.py --all
```

PowerShell 下同样写法（`python` 或 `python3` 都行）：

```powershell
cd new
python tools/run-compute.py --list
python tools/run-compute.py ftk_state_count
```

脚本实际执行的命令（每条用例一次）：

```
cargo run --offline --release -p gi-genius-invokation-compute --bin <计算名> -- <用例的 cmd...>
```

工作目录固定是仓库根（`new/`），从哪个目录调用脚本都行。
脚本默认设 `CARGO_INCREMENTAL=0`（调用方显式设置时以调用方为准）。

小提示：示例 `ftk_state_count` 的「默认」用例 `cmd` 为空，程序会退回读环境变量
`FTK_STATE_COUNT_SAMPLES`；改它会改 stdout，从而与该用例的答案文件比对失败。
要换输入，请改 `compute.json` 的用例（加 `cmd` 参数）并同步答案文件。

## 6. CI 怎么用（D234：每个计算一个独立 runner 并行跑）

工作流文件：`.github/workflows/compute.yml`，只能手动触发。

触发：仓库 Actions -> **Compute** -> **Run workflow**：

- `name`（可选）：只跑指定计算；**留空 = 按目录扫描，跑所有带 `compute.json` 的计算**；
- `timeout-minutes`（可选，默认 30）：每个 matrix job 的超时；
  单次编译+运行的细粒度超时写在 `compute.json` 里。

job 结构：

1. **discover**（1 个 job）：checkout 后扫 `crates/compute/src/bin/*/compute.json`
   （调用 `python3 tools/run-compute.py --configs`），输出 JSON 数组 `names`；
   空数组直接失败。**CI 里没有硬编码计算名单。**
2. **compute**（N 个并行 job）：
   `strategy.matrix.compute: fromJSON(needs.discover.outputs.names)`、`fail-fast: false`，
   **每个计算一个独立 runner**；每个 job 自己跑 `python3 tools/run-compute.py <名字>`、
   自己上传 artifact，互不影响（一个红不会拖停其它）。

并发口径：**不同计算之间并行**（matrix）；**同一计算内的多条用例串行**（脚本里一条接一条，
不允许同一计算的多条用例并发抢同一份产物）。

每个计算的 artifact：名字 `compute-output-<计算名>`，内含：

- `<名字>.stdout.txt` —— 该计算所有用例的 stdout 依次拼接，**结果都在这里**；
  用例边界、命令、比对摘要看 stderr；
- `<名字>.stderr.txt` —— 编译 / 运行 / 比对日志与用例汇总（失败时看它；job 日志里也打最后 200 行）；
- `<名字>.exit-code.txt` —— 该计算的退出码。

名字非法或不存在时，该 matrix job 变红，job 日志里脚本会列出全部可用计算名。
CI 里名字只通过环境变量传，不拼进 shell 命令；artifact 名字里的非法字符会被替换成 `-`。

**新增计算不需要改 workflow**：放好文件夹和 `compute.json`，下次手动触发时 discover 自动发现。

## 7. 约定与注意

- **离线**：脚本固定 `cargo run --offline`；计算用到的依赖必须已在 Cargo.lock 里、
  且本机 / runner 缓存里能离线解析（本仓库目前零第三方依赖）。
- **stdout 只放结果**；调试信息走 stderr，否则比对会失败、artifact 会被日志污染。
- **计算名精确匹配**：文件夹叫 `my_compute`，命令就得写 `my_compute`，写 `my-compute` 不算。
- **同一计算内的用例不要并发**；要并发请拆成不同计算（不同文件夹），CI 会给它们不同 runner。
- `target/` 是构建产物，不入库。
- crates/compute 只放「驱动 + 参数装配 + 输出格式」，真正的算法应留在 engine / definitions 里复用；
  不要让计算程序复制游戏逻辑。
- 目前接入的示例计算：`ftk_state_count`（占位，不真算；3 条用例演示「有答案文件 / 只运行」两种模式）。
