# 批量创建、替换和清空：热点与质量优化（2026-10-26）

本轮消除两处重复工作：四个浏览器控制器重复提取批次元数据，以及 Mapper
退休前预先遍历随后还要由 Memory 删除的同一子树。没有改 benchmark 业务、
feature、依赖版本或优化等级；保留双 Memory、完整身份、独立校验和类型安全。

同路径、四轮正反顺序、统一绑核的 864 个正式样本中，千行创建 / 替换 / 清空
Script 中位数改善 **4.5% / 5.4% / 2.8%**，万行创建改善 **6.7%**。
局部小操作仍有回退，不把这次改动描述为全面提速。Dioxus 的批量路径仍明显更快。

## 当前热点的证据

初始 534 个输入与上一轮已测构建逐文件一致。独立、不包装 DOM 的 CPU 采样
使用 100 µs 间隔，每个框架四项操作、每项三次。下表为 inclusive ms 的三次
中位数；路径有包含关系，不能相加，也不能当正式时间的精确占比。

| Glory 路径 | 创建千行 | 替换千行 | 清空千行 | 创建万行 |
|---|---:|---:|---:|---:|
| HTML Mapper.apply | 16.229 | 21.822 | 5.329 | 166.132 |
| gloryClonePlan | 7.245 | 6.061 | — | 68.115 |
| Memory 操作 | 3.224 | 4.977 | 1.098 | 44.236 |
| Owner.dispose | — | 4.407 | 3.308 | — |
| 原生 setAttribute | 4.407 | 2.122 | — | 40.546 |
| 原生 cloneNode | 2.187 | 3.791 | — | 20.477 |

Dioxus 的 JS run 对应为 5.306 / 7.231 / 2.757 / 57.826 ms。它的万行 cloneNode
为 21.304 ms，接近 Glory 的 20.477 ms；剩余差距不能再概括为 Glory 没有深克隆。
首次万行采样有堆增长，不能拿被它放大的 malloc 平均值代表稳定运行；原始三次
及新版本采样全部保留，未从正式样本中删除慢值。

### 1. 四个控制器重复解析同一批元数据

原先 `Layers::update`、`Sheets::update`、`Collapses::update` 各自遍历 mutations，
对所有 CreatePlan 身份查询 Memory，再发现绝大多数节点没有相关能力。
`update_translations` 还先将已有成员及所有新建身份并入 `BTreeSet`，再逐个查询。
普通千行实例便构造 8,000 个临时候选，万行为 80,000 个；这些行并没有 translation。

现在 `semantic_changes::collect` 在 Mapper 完成后提取一次借用视图：

- Plan 使用既有的不可变 Semantics 索引，普通无元数据声明不访问每个实例节点。
- 普通 Create 只提取带元数据的节点；Semantics patch 按最终已提交值合并。
- 保留 Clear 的 None，检查完整 NodeId 世代；拒绝后缀不能安装未提交的值。
- 四个控制器共用该视图；translation 不再构造覆盖所有新建节点的候选集合。
- 现有 live 成员的退休、结构/可见性/惯性更新，以及 sheet 的 Style 重装规则
  继续运行。没有能力开关，也没有跨批次历史缓存或业务值克隆。

独立旧路径 replay 测试验证最终 registry 一致；16,384 个普通 Plan 节点的
旧候选表为 16,384 项，新视图为 0 项。这是实际多余登记与查询的消除。

### 2. Mapper 预扫退休子树，Memory 随后再次遍历

旧 `Mapper.apply(Dispose)` 先读取整棵子树建立 ID 清单，随后调用 Memory 的
Dispose 再走一遍删除。现在 `Memory::dispose` 与普通 Dispose 共用退休函数，
在实际删除时返回同序身份清单，Mapper 和宿主用它释放索引。

没有合并两份 Memory 或删除语义节点；UI 自己的 Owner/查询清理仍独立执行。
测试对照普通 apply，覆盖后代顺序、挂起后代、存活兄弟、名称、FocusScope 计数、
旧世代拒绝、查询回复水位和失败时 receipt 不变。

## 为什么与 Dioxus 的差距仍然大

独立计数不参与性能表。千行创建：

| 实际工作 | Glory 前 | Glory 后 | Dioxus |
|---|---:|---:|---:|
| WASM→JS 导入调用 | 19,034 | 19,034 | 10 |
| TextDecoder.decode | 5,012 | 5,012 | 2 |
| 深克隆 | 1,000 | 1,000 | 1,000 |
| setAttribute | 9,000 | 9,000 | 3,000 |
| nodeValue 更新 | 2,000 | 2,000 | 0 |
| insertBefore | 1,000 | 1,000 | 0 |

Dioxus 的动态文本使用实际 Text 创建/替换，最终通过解释器批量接入，并非不做
文本或插入工作。调用数比也不是速度比。Glory 前后所有操作的 DOM 方法、属性名、
过桥、解码次数和字节数严格一致，证明没有少写属性或漏做节点工作。

Glory 当前已使用 each_seed；万行实测 **graph=40,003、owners=10,013、nodes=80,010**，
旧报告的 60,003 / 20,013 已不适用。每行仍有 label Source、class Derived、两个
Binding 和一个行 Owner，八个语义实例在 UI 与 Mapper 中分别维护。Web 还要取得
这一行十一份物理部件句柄。Dioxus 的 JS nodes 表约五个动态目标/行。

这些独立身份、事件、查询和生命周期能力不是重复逻辑，本轮没有为缩小差距删掉
它们，也没有将 Rust 侧 DOM 工作改写为通用 opcode VM。

### 清空的回收范围不相同

本轮重新做“创建万行 → 清空 → 显式 CDP GC → 三次小千行创建/清空”检查。
探针只保存 WASM exports 的 WeakRef，不把节点数组或解释器存进额外强引用。

| GC 后观察 | Glory | Dioxus |
|---|---:|---:|
| 文档中的行 | 0 | 0 |
| Rust externref 表直接保留的离树行根 | 0 | 0 |
| 活动解释器 JS nodes 表保留的离树行根 | 无该表 | 10,000 |
| 三次小循环后离树行根 | 0 | 仍为 10,000 |

实际找到的对象为 RawInterpreter，nodes 长度 50,009；Glory externref 容量
132,100 仍在，但不再引用这些离树行对象。容量与对象引用是两回事。
Dioxus 清空依然是有效成绩，但不代表完成了同等范围的主机对象引用释放。
该观察针对存活 App 的峰值保留，不证明无限增长、具体 RSS 或 App 卸载后的状态。

## 前后对照

同一绝对编译路径 `D:/BuildCache/glory-bulk-quality-20261026/source`，前后独立
target，Rust/Cargo 1.98.0、opt-level 3、LTO、CU1、bindgen 0.2.129。
Chrome 155.0.8059.84、i7-14700KF、1280×900、DPR1、en-US/light。
四轮 before/after、after/before 交替，每项每轮三次预热、十二次正式测量，
每格 48 个样本，合计 864。浏览器各页统一验证掩码 65535 / AboveNormal。
下表为 ScriptDuration 中位数 ms，不含探针、编译或初始化。

| 操作 | 前 | 后 | 耗时变化 |
|---|---:|---:|---:|
| 创建千行 | 19.050 | 18.196 | −4.5% |
| 替换千行 | 28.839 | 27.273 | −5.4% |
| 更新每十行 | 0.480 | 0.488 | +1.7% |
| 选行 | 0.629 | 0.638 | +1.5% |
| 交换行 | 0.374 | 0.372 | −0.4% |
| 删除行 | 0.349 | 0.361 | +3.6% |
| 追加千行 | 21.132 | 19.807 | −6.3% |
| 创建万行 | 231.100 | 215.501 | −6.7% |
| 清空千行 | 14.841 | 14.424 | −2.8% |

两项改动合并测量，未逐项消融，不能把上述百分比拆分归给单项。小操作回退保留。

首次未绑核的两轮 432 个样本也完整保留，未与以上样本混池。当时旧版自身两轮
万行 Script 为 493.84 / 278.74 ms；追加汇总回退 12.5%、万行回退 14.9%、清空
回退 21.9%。这种阶段漂移促使补做与三框架同条件的四轮绑核实验，而不是删除
不利样本。初次全九项、全部逐轮值和后续实验均在机器可读结果中。

## 优化后三框架重跑

三者从同一冻结输入各用全新 target 构建；release compiler artifacts
35 / 234 / 124 个均 fresh=false，各自 WASM Clippy 通过。
Glory 0.3.1、Leptos 0.8.22、Dioxus 0.7.10；仍为仓库锁定版本。
与前后对照分开，三框架正反两轮，各格 24 样本，共 648 个。
同一绑核、浏览器、视口及 release+bindgen 流程，数据、身份、几何与六处截图一致。

| Script 中位数 ms | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 18.351 | 15.202 | 4.472 |
| 替换千行 | 28.423 | 22.885 | 7.869 |
| 更新每十行 | 0.487 | 0.371 | 0.869 |
| 选行† | 0.650 | 174.781 | 0.748 |
| 交换行 | 0.378 | 0.165 | 0.884 |
| 删除行 | 0.352 | 0.333 | 0.796 |
| 追加千行 | 20.896 | 16.824 | 5.685 |
| 创建万行 | 220.445 | 185.063 | 49.464 |
| 清空千行 | 14.645 | 16.521 | 2.784 |

† Leptos 的连续流程选行异常再次出现，主值不替换。另 72 个独立诊断样本中，
同一千行实例交替选择 / 每次重建后选择分别为：Glory 0.672 / 0.615、Leptos
1.167 / 3.635、Dioxus 0.728 / 0.814 ms。不将 175 ms 当成常规选行成本。

万行 click→两个 RAF 的 wall 中位数为 497.700 / 448.000 / 303.600 ms；
Script 倍数不能当作整页呈现倍数或 FPS。完整 wall / Task / Layout / Style
分布、p10/p90 和轮间变化均已保留。

## 体积和正确性

同路径 Glory A/B，统一 Binaryen 132.0.0 −Oz 后 WASM
**1,411,183 → 1,405,443 B**（−5,740），Brotli q11
**376,465 → 373,566 B**（−2,899）。生成 JS 60,648 B，哈希不变。
最终三框架 −Oz WASM 为 Glory 1,405,443、Leptos 155,008、Dioxus 317,164 B。
主计时仍使用 release+bindgen，统一 −Oz 的六项真实浏览器验收另外通过。

- 原有所有字段完整 Shape 差分、共享 render/UI/core/html/SSR 全 feature 测试和
  Clippy，以及 Sapid 默认/全 feature 测试与 Clippy 通过；新增 metadata replay
  与退休对照测试通过，WASM 严格 Clippy 通过。
- Glory 27 组基础浏览器验收通过，包括实际流式 SSR、CSR/hydration、Plan、输入、
  selection、query、Owner 及节点身份。Sapid translation、collapse-motion、sheet、
  overlay、focus-scope、portal、layer-document 七组通过，覆盖主题、CSR/hydration、
  reduced-motion、中断、隐藏/恢复、流式接管、输入身份及清理。
- Sapid 首次 translation/collapse 测试因遗漏生成的流页面失败，原始失败保留；
  补齐真实 SSR producer 产物和 stream.js 后，两组及 sheet 重跑通过。
- 前后及三框架全部计时样本的数据哈希一致。所有万行矩形一致，存续 Element/Text
  和 Glory 完整语义身份保持；六处截图逐字节一致。没有修改容差或 workload。

## 源码边界和证据

[机器可读汇总](../benchmarks/bulk-quality-results-2026-10-26.json)保存三批独立计时、
采样、操作计数、GC 观察、版本、构建与测试证据哈希。全部证据根目录为
`D:/BuildCache/glory-bulk-quality-20261026`：`inputs-before/`、`inputs-after/`、
`before/`、`after/`、`pair/`、`pair-pinned/`、`profile-before/`、`profile-after/`、
`operation-counts-final/`、`retained-host-objects/`，以及浏览器和 native 检查日志。

初始 HEAD 为 `617471f`，本轮仅修改 11 个库/测试输入，业务 main.rs、data.rs、
harness、CSS、清单和锁文件不变。冻结源码逐文件哈希未变化。本轮期间另一个工作
修改了旧 `core/src/truck.rs`、`scope.rs` 及旧路由/示例/API 文档；这些改动完整保留，
没有并入冻结源码。实际 Cargo dep-info 证明两个 core 参考文件未参与此 release；
参与编译的当前源码和本轮修改仍与冻结输入一致。所有构建、缓存、临时文件都在 D 盘。
