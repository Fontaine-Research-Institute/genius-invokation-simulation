# 七圣召唤模拟 · 开发 Wiki

> 中文 · [[Home-EN|English]]

*本页范围: 仓库定位, 文档读法, 验证命令, 全部页面索引.*

---

## 1. 仓库是什么

| 项 | 内容 |
|---|---|
| 项目 | 粉丝自制的<七圣召唤> (Genius Invokation TCG) 模拟: 规则引擎, 对局模拟, 精确概率求解, Rust 实现 |
| 许可与声明 | Apache-2.0; 与 HoYoverse / miHoYo 无关联; 仓库不含官方美术 / 音频 / 模型 / 卡图 |
| crate 组成 | `engine` (机制原语) , `richtext` (富文本解析) , `definitions` (游戏定义层) , `cards` (具体卡牌) , `simulation` (对外门面) , `compute` (算力 bin)  |
| 当前进度 | 定义层基础部分已收口 (D265 / D266 / D270) ; 具体卡牌只有爱诺样板 (`cards/character/aino`) , 效果体仍是占位 |
| 求解器 | `solver` 尚未建 (见 `new/Cargo.toml` 注释)  |

### 1.1 四条核心坚持 (`new/README.md` 与 D20) 

1. 框架不做中枢统计, 不做中枢收集, 不做内建行为表, 只给接口与原语.
2. 具体功能谁要谁实现 (卡牌 / 牌族 / 具体 tag 自己写) .
3. 同类功能归同一个"统一包装"层 (例如技能类行为统一包装在角色牌族里) .
4. 框架负责"能不能", 卡牌负责"做什么".

### 1.2 精确概率

随机与选择不采样, 保留成"分支 + 精确分数" (D158 / D159) ; 同一局面可复现.

---

## 2. 怎么读这套文档

| 顺序 | 页面 | 用途 |
|---|---|---|
| 1 | [[Architecture]] | crate 依赖方向, 一局对局的数据流, "框架只给接口"怎么落地 |
| 2 | [[Conventions]] | 设计约定速查: 身份, 数据唯一, 纯函数, 零浮点, Nat 与 ExactNat, 一步, 记录 |
| 3 | [[Crate-Engine]] → [[Crate-Richtext]] → [[Crate-Definitions]] → [[Crate-Cards]] | 按层读 crate |
| 4 | 定义层各页 | 需要具体模块时再进 |

### 2.1 依据与记录

- **依据优先级** (`RULES.md` 第三节) : 规则书 > 卡面 / 权威关键字 > 参考实现 (参考实现只看实现方法) .
- **待实测 / 待接入**: 统一记在工作区根 `留空.md`, 本 Wiki 用 `#N` 引用.
- **结算顺序**: 唯一权威是工作区根 `结算顺序.md` (标注"暂定") .
- **决策编号**: `DNNN` 见工作区根 `决策日志.md` (只追加) ; 本 Wiki 只引用编号与结论.

---

## 3. 验证命令

```bash
cd new
cargo test --offline --workspace
cargo clippy --offline --workspace --all-targets -- -D warnings
python tools/run-compute.py --list
```

> [!NOTE]
> 本机一律离线 (`--offline`) .CI (`new/.github/workflows/ci.yml`) 用 `--locked`, 并额外跑三个源文本扫描脚本 (只提示, 不阻断) .
> Windows 若链接报错, 把 `TEMP` / `TMP` 指到工作区 `.tmp` (见 `RULES.md` 第二节第七条) .

---

## 4. 页面索引

### 4.1 入门

| 页面 | 内容 |
|---|---|
| [[Home]] | 本页 |
| [[Architecture]] | 依赖方向, 数据流, 设计哲学的落地方式 |
| [[Conventions]] | 设计约定速查 |

### 4.2 基础 crate

| 页面 | 内容 |
|---|---|
| [[Crate-Engine]] | 数值 / 实体与区 / 随机与选择 / 分数 |
| [[Crate-Richtext]] | 富文本解析与渲染 |
| [[Crate-Definitions]] | 游戏定义层 (除具体卡牌外)  |
| [[Crate-Simulation]] | 对外门面 |
| [[Crate-Cards]] | 具体卡牌 |
| [[Crate-Compute]] | 算力任务 |

### 4.3 定义层 · 基础设施

| 页面 | 内容 |
|---|---|
| [[定义层-Base]] | 元素 / 牌族 / 编号 / 视角 |
| [[定义层-Resources]] | 骰子与费用 |
| [[定义层-Rules]] | 三级统一规则表与 ops |
| [[定义层-Tags]] | 标签体系 |
| [[定义层-Timeline]] | 时机键, 触发器, 插入式结算 |
| [[定义层-Reactions]] | 元素反应与效果执行 |
| [[定义层-Records]] | 记录 API |
| [[定义层-Source-Version-Generated]] | 源码嵌入, 版本, 生成数据 |

### 4.4 定义层 · 运行时

| 页面 | 内容 |
|---|---|
| [[定义层-State]] | 对局状态, 区域承载, 终局判定 |
| [[定义层-State-Flows]] | 区流程纯函数 |
| [[定义层-Flow]] | 端到端流程 (技能 / 打牌 / 回合)  |
| [[定义层-Keywords]] | 关键字层与"一步" |

### 4.5 定义层 · 牌族

| 页面 | 内容 |
|---|---|
| [[定义层-Families]] | 五牌族总览 |
| [[定义层-Character]] | 角色牌族 |
| [[定义层-Action-Card]] | 行动牌 |
| [[定义层-Summon-Enemy-Status]] | 召唤物 / 敌人 / 状态 |

### 卡牌定义

| 页面 | 内容 |
|---|---|
| [[卡牌定义]] | 栏目说明, 模板, 已研究卡列表 |
| [[卡牌定义-爱诺]] | 爱诺: 完整代码 + 逐段解释为什么这么写 |
| [[卡牌定义-芙宁娜]] | 芙宁娜: 天赋, 万众瞩目, 形态切换的设计研究 (未实现)  |
| [[卡牌定义-模板]] | 新增卡牌定义页的模板 |

---

## 5. 按任务找入口

| 想做什么 | 先看 |
|---|---|
| 改数值原语, 实体与区, 随机与选择, 分数 | [[Crate-Engine]] |
| 改文本解析, 加占位符白名单 | [[Crate-Richtext]] |
| 加一张具体卡 | [[Crate-Cards]] + [[定义层-Character]] / [[定义层-Action-Card]] |
| 改一局怎么走 (技能 / 打牌 / 切人 / 回合)  | [[定义层-Flow]] + [[定义层-State-Flows]] |
| 加关键字函数或"一步" | [[定义层-Keywords]] |
| 加触发时机或结算插入 | [[定义层-Timeline]] (配合 `结算顺序.md`)  |
| 加一个算力任务 | [[Crate-Compute]] |

---

## 6. 文档维护约定

- 页面放在 `new/docs/wiki/`, **文件名即页面名**; 页面间只用 wiki 链接 (`[[页面名]]`) , 不用相对路径.
- `_Sidebar.md` / `_Footer.md` 由维护者统一维护, 普通页面不写.
- 每个模块页固定 8 节: 职责 / 关键概念与类型 / 公开入口 / 不变量与禁止 / 依据 (规则书与 D 编号) / 常见误用 / 测试 / 待接入与待实测.
- 样式规范: 标题带序号, 表格带对齐行, 关键提醒用 GitHub 警告块 (`[!NOTE]` / `[!IMPORTANT]` / `[!WARNING]`) , **不使用表情符号**.
- 页面引用官方素材图片时, 图片下方必须原样写这两行 (见 `ASSETS.md` §三) : `图片来源：米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有。<br>Image source: official Genshin Impact artwork by HoYoverse. All rights belong to miHoYo / HoYoverse.`.
- 本 Wiki 写的是**代码现状**.与旧文档 (`new/README.md`, `结构.md`, `分工计划.md`) 冲突时, 以**代码 + 决策日志**为准; 已知偏差见 [[Architecture]] 末节.

---

最后核对: 2026-10-19 (代码: `new/Cargo.toml:members`) 
