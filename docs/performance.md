# Performance Notes

## 跨 mutation 字段提交 — 2026-10-26

[字段提交报告](field-submit-plan-2026-10-26.md)记录 Text/Label/Classes 的
有界宿主提交、精确后缀恢复、屏障和成功前缀。千行创建 WASM→JS 从 5,034
降至 3,065，DOM 工作量保持不变；调用次数变化不代表同等比例提速。
同路径四轮 864 个正式样本及独立计数见
[结果汇总](../benchmarks/field-submit-results-2026-10-26.json)。

## Plan physical parts and seed rows — 2026-10-25

本轮保留显式 `each_seed` 和普通 Part 销毁/挂起减分配，撤回按需 JS Part 数组实验。
对 Dioxus 差距的判断仍以[源码和独立计数分析](dioxus-performance-analysis-2026-10-25.md)
为基础：两者已经整行深克隆，剩余差别主要是克隆后的实例登记、跨边界访问、
动态字段首写和响应图构造/退休。不能把调用次数比换算成耗时比，也不能通过保留
离树 DOM 引用来改善清空成绩。当前修改没有重新测量 Dioxus。

### 实现与语义边界

- 完整 Runtime/slot/generation、两份 Memory、HTML key 和全部语义 Entry 仍保留。
  控件 property、selection、composition 仍走原有受控路径。没有通用 opcode VM，
  没有延迟 host work，也没有改变每操作成功前缀/receipt/query 屏障。
- 普通 Part 的销毁/挂起不再建立单元素范围 Vec；销毁后的事件/布局待处理项通过
  代次校验的活 Entry 表过滤，省去每次退休临时 HashSet。Group 保持注释范围处理。
- `each_seed(items, key, factory)` 只把首次出现的 T 交给行工厂。相同 key 的新 T
  和新 index 不通知该行；移除后重新插入会建立新 Owner 和 seed。工厂及其隐式派生
  仍属于该行 Owner。普通 `each` 的 item/index 活更新语义保持不变。

标准 rows 应用本来就只对 item 做一次 `get_untracked` 且忽略 index，因此可以
明确迁移到 `each_seed`。label Cage、selected Bond、事件处理和数据生成算法保持不变。
这属于应用 API 迁移，必须和不改应用的内部优化分开比较。

### 保留版结果与验收

基线 `aecbe360bd0731f118af31e653f6df2b26ef7576`；保留版是 each_seed 应用迁移加
普通 Part 退休/挂起减分配，没有 JS Part 数组。两个独立 target，先冻结源码，再
构建；保留版因 Clippy 的 let-and-return 清理在自己的 target 内重编 web/app，
没有跨版本复制产物。最终 512 个输入与当前源码逐一匹配。
[构建审计](D:/BuildCache/glory-parts-20261025/kept-audit.json)。

每操作相邻交错 before/kept，轮换先后并在第二轮反转；每轮新 context，每项 3 次
热身、12 次测量，共 **432 样本**，保留所有慢样本。Chrome 155.0.8059.84，
1280×900 / DPR 1，独占本任务的浏览器进程，CDP 取得其进程 ID 后固定 affinity
65535 / AboveNormal。机器上有其他并行构建；本轮环境漂移明显，结果不代表无噪声
的绝对性能或统计显著性。未重新跑 Dioxus，不拼接旧时间计算新倍数。

ScriptDuration 中位数，ms；正百分比为变慢：

| 操作 | before | 保留版 | 变化 |
|---|---:|---:|---:|
| 创建千行 | 25.102 | 25.658 | +2.2% |
| 替换千行 | 42.821 | 41.169 | -3.9% |
| 更新每十行 | 0.797 | 0.746 | -6.3% |
| 选行 | 0.814 | 0.840 | +3.1% |
| 交换行 | 0.748 | 0.477 | -36.2% |
| 删除行 | 0.769 | 0.512 | -33.4% |
| 追加千行 | 24.787 | 24.616 | -0.7% |
| 创建万行 | 310.714 | 308.506 | -0.7% |
| 清空千行 | 19.044 | 17.267 | -9.3% |

交换两轮 before→kept 为 **0.732→0.447 / 0.878→0.550**，删除为
**0.710→0.463 / 0.892→0.558**，方向一致。清空两轮 **18.793→19.693 /
19.294→16.395**，不能声称稳定降低 9.3%。创建千行两轮都略快，但合并中位数反而
略慢，说明 pooled median 在分布漂移时不能替代相邻样本/逐轮观察。替换、局部更新、
追加也有方向混合；万行不足 1%，不宣称批量创建已经明显改善。

明确的结构收益是万行 graph **60,003→40,003**，Owner **20,013→10,013**；
完整语义节点仍为 80,010。清空后仍为 graph 3 / Owner 13 / UI nodes 10。
[最终原始样本](D:/BuildCache/glory-parts-20261025/kept-interleaved/evidence.json)、
[逐轮与 p10/p90](D:/BuildCache/glory-parts-20261025/kept-summary.json)。

UI / Render / HTML **442 项测试通过**，其中新增 seed 初值、重排保持本地状态、
移除重插、工厂构造上下文与资源退休、精确图节点/Owner 减量测试。最终 WASM
all-features/all-targets 严格 Clippy 与 workspace fmt 通过。最终浏览器 **27/27**
通过，包含 CSR、SSR、stream、hydrate、受控编辑、查询和 Owner 清理。额外覆盖完整的
存续 Element/Text/语义 ID、全部万行矩形及清空；Plan 增补部分退休、旧 ID 拒绝、
新实例隔离、属性 Clear、实际 Bounds 查询和挂起/恢复。测试这些仍有价值，虽 JS
Part 候选撤回也予以保留。没有在本轮重新宣称 Linux/Blitz/OS IME/屏幕阅读器验收。
[单元测试](D:/BuildCache/glory-parts-20261025/tests.txt)、
[严格 Clippy](D:/BuildCache/glory-parts-20261025/kept-clippy.txt)、
[完整身份和几何](D:/BuildCache/glory-parts-20261025/kept-integrity/evidence.json)、
[最终 Plan 浏览器验收](D:/BuildCache/glory-parts-20261025/kept-plan/evidence.json)、
[最终 27 组浏览器验收](D:/BuildCache/glory-parts-20261025/kept-browser/evidence.json)、
[相同数据与六处 PNG 哈希对照](D:/BuildCache/glory-parts-20261025/kept-pixels/evidence.json)。

### 撤回的 JS Part 实验

候选让每个 Plan 实例持有一个 JS Part 数组；文本/属性在 JS 中直接取 Part，
Rust 在结构操作、输入和查询时才取回 Node，偏移/范围在声明级共享。部分删除清空
自身单元，整实例退休一次清空数组。其功能与清引用验收通过，但减少 ABI 调用
没有带来稳定的时间收益，因此最终源码恢复原来的 Array.get / Parts 句柄路径。

以下是**撤回候选**的独立探针，不能当作保留实现的指标。千行创建：

| 指标 | before | JS Part 候选、原 Each 应用 | 候选再使用 each_seed |
|---|---:|---:|---:|
| WASM→JS 调用 | 19,034 | 8,034 | 8,034 |
| Array.get | 11,000 | 1,000 | 1,000 |
| JS→WASM externref allocator 调用 | 1,010 | 1,010 | 1,010 |
| JS→WASM malloc 调用 | 2 | 2 | 2 |
| 深克隆 / setAttribute / insertBefore | 1,000 / 9,000 / 1,000 | 相同 | 相同 |
| 直接 Node externref 句柄 | 11,054 | 1,054 | 1,054 |
| 活 Plan 数组 / 数组中的 Node 引用 | 0 / 0 | 1,000 / 11,000 | 1,000 / 11,000 |
| 万行 graph / Owner | 60,003 / 20,013 | 相同 | 40,003 / 10,013 |

句柄减少不是 DOM 对象减少，也不是总堆或 RSS 降幅。新路径仍通过 JS 数组持有全部
物理部件，并保留按需取得的 Node 引用。清空千行调用数 **2,034→3,034**，包括
每实例的一次数组清引用；不隐藏这一新增工作。

创建万行→清空→GC→再次创建/清空千行后，三版的表内离树行引用均为 0；活图回到 3，
UI 节点回到 10，Owner 回到 13。新路径活 Part 数组/数组内 Node 引用均回到 0。
这里只统计框架可达句柄及 Part 数组，不将 JS 引擎 GC 时点、容量或 RSS 混为一谈。
详见 [双向 ABI 与引用计数](D:/BuildCache/glory-parts-20261025/counts.json)。额外探针
刻意持有数组：部分删除的两个单元均为 null、三个剩余 Part 可用；最终三个实例
数组长度均为 0，见 [引用验收](D:/BuildCache/glory-parts-20261025/retention.json)。

长轮次初测存在明显漂移：before 千行创建两轮 20.122 / 39.450 ms，不采用合并
百分比作为保留依据。随后每个操作交错运行 before / 候选 / 候选+seed，每项三次
热身、每轮十二样本、两轮反转并轮换顺序，648 样本。原 sample/setup/action/settle/
snapshot/CDP 边界不改，只增加单操作入口，数据摘要逐样本一致。

仅 JS Part 候选的万行创建两轮为 **329.254 / 314.570 ms**，均慢于同轮 before
**322.787 / 303.584 ms**；局部文本更新 **0.725 / 0.718 ms**，也慢于 before
**0.633 / 0.652 ms**。其他场景混合且有漂移；不把较少的调用/句柄包装成已实现的
加速，也不将所有差值直接归因于 JS 数组。实验还包括 Each 新模式的内部枚举与
销毁减分配，原应用没有启用 seed；这些因素及共享机器噪声都存在。
[初测原始数据](D:/BuildCache/glory-parts-20261025/final/evidence.json)、
[交错复核](D:/BuildCache/glory-parts-20261025/interleaved/evidence.json)、
[逐轮分布](D:/BuildCache/glory-parts-20261025/interleaved-summary.json)。

### 尚未完成的批量路径

最终仍是一行一次 clone、逐 Part 返回和一次 insertBefore，没有实现
跨多实例的一次创建/挂接。完整实例登记、11 个 Part 的 JS 寻址、8 个语义身份属性、
两份 Memory 和动态字段首写仍有成本。下一步应在明确每个操作成功水位的前提下做
有限的多实例创建/挂接协议，不能在提交尾部统一执行所有 DOM 操作而丢失失败前缀。

Plan 初始动态字段一次初始化、class token/String/Edit 多层复制和发布临时 Vec
也没有在本轮消除。前者需要在真实 Binding 依赖上下文中采集初值，并让两份 Memory
独立验证；不能把 untracked 首读当成订阅，不能跨 query/action/input 屏障折叠。
本轮优先完成语义风险较小的 seed 入口，并用独立应用组识别其收益。

## Plan preparation and batch identity checks — 2026-10-25

最新同批改进见 [Plan 优化报告](plan-batch-preparation-optimization-2026-10-25.md)：
共享不可变声明事实，严格升/降序 slot 证明批内唯一性，乱序保留原校验。
千行创建 **20.349→18.827 ms（-7.5%）**，替换 **30.104→28.829 ms（-4.2%）**，
5,000 行看板初始化 **87.492→82.236 ms（-6.0%）**。万行轮间不稳定，
密集更新有 3–6% 回退；没有把这些结果统称为全面提速。
八份独立构建、最终 1,800 样本、完整身份/几何/42 张 PNG；Windows/Linux
908/907、27 浏览器及 64/1024 真实行高、实窗/UIA 回归通过。
下方旧版本数据保留为历史，当前跨框架版本对照另见[稳定版报告](framework-benchmark-latest-stable-2026-10-25.md)。

## Bulk creation and Dioxus design analysis — 2026-10-24

保留固定参数的同步 Web 创建桥接：原型克隆、class/style、身份/live 一次提交，其他属性与受控输入保留通用路径。
六个全新独立构建、相同九项 benchmark、正反顺序共 **1,080 样本**：Glory 千行创建 Script
**57.33→55.61 ms（-3.0%）**、千行替换 **83.03→79.87 ms（-3.8%）**、万行创建
**702.08→654.75 ms（-6.7%）**。属性打包与 Memory 间接存储实验均未保留。

当前万行总耗时 Glory **909.45 ms**、Leptos **465.10 ms**、Dioxus **313.70 ms**。
Dioxus 千行替换约 1,000 次深模板克隆/3,000 次属性写入/2 次 UTF-8 解码；Glory 为
8,000 次浅克隆/15,000 次属性写入/20,006 次解码。差距主要仍在结构构造、运行时对象和宿主提交粒度。
双平台各 452 工作区/289 fork、25 浏览器、64/1024 真实行高、实窗/UIA/stream 完整回归通过。
流式 fixture 已补齐生成 JS 模块服务。

完整九项前后/三框架表、80 个独立 CPU profile、失败实验、Dioxus 源码分析及后续优先级，见
[专项报告](bulk-create-dioxus-2026-10-24.md)。[原始证据](/D:/BuildCache/glory-bulk-bridge-20261024/evidence-index.json)。

## Three-framework rerun after DOM creation optimization — 2026-10-23

本轮重新独立构建 **Glory / Leptos 0.8.19 / Dioxus 0.7.9**，包括 Glory 上轮的
DOM 创建优化和本轮冻结时已有的 Layer/语义改动。源码冻结于
**2026-10-23T15:42:48.062060+00:00**，共 **251 个输入**，Git 基点
`945c077b6a3f167c9e79cbc75f726b1d6316e34d`（含未提交改动）。15 个 benchmark 应用/共享输入与上次
三框架比较逐字节一致，包括源码、锁文件、数据生成器、CSS 和计时 harness；另有
22 个公共库文件变化、8 个新增，完整差异保留在 source-history.json/.diff。
冻结后并行修改未混入二进制，summary.json 的 post_snapshot 字段记录其边界。

三个全新独立 D: target，cargo metadata 验证实际输出目录，编译 artifact 确认 fresh=false。
release opt-level=3、LTO、一个 codegen unit，wasm-bindgen 库/CLI 0.2.129，无 wasm-opt。
三个应用的严格 WASM all-target Clippy 和格式检查均通过。
环境 Windows 11、Rust 1.98.0/MSVC、i7-14700KF、Chrome **155.0.8059.84**，
1280×900、scale 1。P 核逻辑 CPU 0–15（mask 0xffff）、AboveNormal 对三个框架一致，
每个上下文在计时外设置并读回验证；CPU 拓扑和进程记录已归档。

两轮使用独立 Chrome、每框架新上下文，顺序 **Leptos→Glory→Dioxus**，再
**Dioxus→Glory→Leptos**。每项 3 次不计时热身、每轮 12 个样本，每格合计
**24 个，主表共 648 个**。本机并非独占，轮次开始的其他 Cargo/rustc 活动已记录；
固定核心不消除温度、频率、运行历史和系统负载的影响。

以下为**点击到两次前台 RAF 的中位耗时，ms，越低越好**。数据准备和验证在计时外；
全部行实际挂载，未使用虚拟列表。该指标不证明实际显示完成/FPS，也不是官方 HTML-table 分数。

| 操作 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 83.10 | 40.15 | 29.75 |
| 替换千行 | 110.65 | 43.70 | 32.30 |
| 更新每十行 | 8.85 | 11.25 | 10.80 |
| 选行（混合流程） | 7.75 | 176.40 | 9.25 |
| 交换两行 | 9.85 | 9.25 | 9.85 |
| 删除一行 | 10.00 | 8.95 | 11.60 |
| 追加千行 | 94.00 | 43.75 | 34.45 |
| 创建万行 | 955.10 | 439.20 | 297.35 |
| 清空千行 | 32.90 | 17.65 | 10.05 |

相同区间 CDP **ScriptDuration 增量中位数，ms**。包含公共 harness/绑定桥开销，
各包含式 CPU 指标不能相加；Layout/RecalcStyle/Task 的完整原始值与中位数亦已保留。

| 操作 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 57.26 | 14.58 | 4.43 |
| 替换千行 | 82.06 | 20.50 | 7.67 |
| 更新每十行 | 0.64 | 0.34 | 0.90 |
| 选行（混合流程） | 1.04 | 172.12 | 0.73 |
| 交换两行 | 0.56 | 0.17 | 0.91 |
| 删除一行 | 0.62 | 0.26 | 0.80 |
| 追加千行 | 62.30 | 15.75 | 5.60 |
| 创建万行 | 684.82 | 176.92 | 45.79 |
| 清空千行 | 30.49 | 15.98 | 2.75 |

Glory 的主要差距仍是批量操作。创建万行总耗时约为 Leptos 的 **2.17 倍**、
Dioxus 的 **3.21 倍**；替换千行为 **2.53/3.43 倍**。
局部操作总耗时需结合较小的 Script 成本与 RAF 等待理解，不从微小差值推导稳定排名。

逐轮总耗时/Script 中位数，ms：

| 框架/操作 | 第 1 轮 | 第 2 轮 |
|---|---:|---:|
| glory 创建万行 | 970.50 / 689.04 | 938.75 / 681.68 |
| leptos 创建万行 | 439.20 / 176.80 | 431.85 / 178.40 |
| dioxus 创建万行 | 302.30 / 48.36 | 296.85 / 45.79 |
| glory 替换千行 | 110.80 / 81.78 | 110.65 / 83.80 |
| leptos 替换千行 | 43.10 / 20.35 | 44.00 / 20.75 |
| dioxus 替换千行 | 31.60 / 7.64 | 33.20 / 8.14 |
| glory 清空千行 | 34.75 / 31.31 | 31.90 / 30.06 |
| leptos 清空千行 | 19.20 / 15.95 | 17.40 / 16.22 |
| dioxus 清空千行 | 9.60 / 2.78 | 10.20 / 2.66 |

选行主表保留完整混合流程的实际结果。另用新上下文测首次千行列表与连续 30 次建表
（首次创建加 29 次替换）后选行，
每格 3 次热身、12 个样本，以下 **72 个诊断样本不混入主表**：

| 框架 | 首次列表 总耗时 / Script | 连续 30 次建表后 总耗时 / Script |
|---|---:|---:|
| glory | 11.70 / 1.20 | 11.80 / 1.20 |
| leptos | 11.55 / 1.20 | 11.85 / 6.16 |
| dioxus | 11.90 / 0.69 | 11.85 / 0.76 |

本轮 Leptos 混合流程的 Script 中位数为 172.12 ms，独立首次列表仅 1.20 ms。这个显著差距说明运行历史敏感；不能把主表异常泛化为 Leptos 的普遍选行成本。本轮未进一步定位其内部原因。

验收结果：全部 648 样本行数/摘要跨框架跨轮一致；三者通过独立种子 oracle、九项操作、
keyed 交换/替换和清理。补充检查更新/选择/交换/删除/追加保留存续 Element 与 Text Node，
Glory 同时保留 data-gly-node 身份，替换后旧后代断开。10,000 行真实矩形摘要一致；
创建千行、替换千行、追加千行、创建万行、万行中部、清空六个视口检查点，三框架两轮
**36 张 PNG** 逐字节一致。

万行 DOM 计数：

| 框架 | Elements | Text | Comments | Attributes |
|---|---:|---:|---:|---:|
| glory | 80,008 | 30,006 | 4 | 320,037 |
| leptos | 80,008 | 30,006 | 1 | 120,009 |
| dioxus | 80,008 | 30,006 | 0 | 140,015 |

Glory 清理回到 0 行/label、10 语义节点、3 图节点、13 Owners。属性数与之前一致，
原型优化减少调用和字符串转换，保留最终属性；计数不表示 RSS。
本轮只重新构建和比较 benchmark，没有修改框架实现，也没有重复原生/窗口/64/1024/
流式全回归。Layer 代码进入此普通列表的执行路径不等于其交互功能已经得到验收。

与上一次三框架运行的 Glory 历史值对照，两个会话均固定 P 核，但源码与运行条件仍有变化，
**不能用这个跨会话差值替代单项优化的因果 A/B**：

| 操作 | 上次→本次总耗时 ms | 上次→本次 Script ms |
|---|---:|---:|
| 创建千行 | 90.40→83.10 | 64.49→57.26 |
| 替换千行 | 116.10→110.65 | 89.17→82.06 |
| 追加千行 | 98.05→94.00 | 66.47→62.30 |
| 创建万行 | 1012.15→955.10 | 741.00→684.82 |
| 清空千行 | 33.70→32.90 | 31.52→30.49 |

DOM 创建优化的独立 432 样本前后对照仍见[专项报告](#html-mapping-and-dom-creation--2026-10-23)。
完整本轮证据：[evidence-index.json](/D:/BuildCache/glory-framework-after-create-20261023/evidence-index.json)。
同目录 verify.py 重算中位数、核验冻结源码/独立构建、648 样本、身份/几何/36 张截图及 CPU 设置；
run.ps1 和 driver 记录复现顺序、辅助脚本及服务清理。重跑须换新 D: 输出目录，保留原始结果。

## HTML mapping and DOM creation — 2026-10-23

本轮针对批量创建/替换，保留 semantic-rows-v1 的业务实现、全部最终属性和节点身份。
最终独立构建对照共 **432 个样本**：Glory 千行创建 Script 中位数 **69.34→57.58 ms
（降低 17.0%）**，千行替换 **91.13→84.04 ms（降低 7.8%）**，万行创建
**764.40→677.15 ms（降低 11.4%）**。清空变化较小，不把这轮当作退休优化。

基线源码/诊断发现三个成本：

- shape 已有自己的 String，却对每个文本/属性/文本属性调用 normalize().into_owned()，
  规范字符串仍会重新分配。现在只在 normalize_text 返回 Owned 时替换，保留规范字符串
  的原分配；非规范 CR/CRLF/NUL 仍执行相同转换。HTML shape/序列化结果保持原契约。
- 每个活 DOM Entry 用 BTreeMap 保存有限的 Main/Control/Content 句柄。现在是内联
  `[Option<Node>; 3]`，保留原 Part 顺序/查询；adoption 的验证计划仍使用原映射，
  验证成功后转入内联句柄。没有删除 Entry、token 表、父表或代际校验。
- 每个新元素都重复写 kind、role、type 和 enabled/disabled 的有限静态属性，产生
  大量 WASM→JS 字符串解码。现在由文档拥有最多 **64 个**浅元素原型，键为五个
  u8 字段，只表示有限 tag/kind/role/type/状态，使用 cloneNode 创建全新节点。
  class、style、name、内容、值、URL、id、data-gly-node 和用户语义关系不进入原型。
  未知词汇/满表走原 createElement 路径，close/clear 释放原型。没有跨 App/文档共享。

新鲜原生控件的 disabled 属性已由 attribute 反射，创建时不再重复 Reflect.set。
value/checked、composition、属性 patch 和 adoption 的写屏障保持原样。SSR/流式 wire
格式、shape 的有序 BTreeMap 和所有最终 DOM 属性不变；CSS 状态标记独立于 feature。
本轮没有使用 innerHTML、字符串模板、业务行缓存或第三方 vendor。

单独计数探针验证千行操作的实际调用变化（探针开销排除在主计时之外）：

| 调用/计数 | 基线创建 | 优化创建（本探针首次计时） | 基线替换 | 优化替换 |
|---|---:|---:|---:|---:|
| createElement | 8,000 | 3 | 8,000 | 0 |
| cloneNode | 0 | 8,000 | 0 | 8,000 |
| setAttribute | 32,000 | 15,005 | 32,000 | 15,000 |
| UTF-8 decode | 78,006 | 34,019 | 78,006 | 34,006 |
| disabled Reflect.set | 2,000 | 0 | 2,000 | 0 |
| createTextNode | 3,000 | 3,000 | 3,000 | 3,000 |
| insertBefore | 8,000 | 8,000 | 8,000 | 8,000 |

该新页面已有外层 App 原型，创建案例还需建 3 个原型并写 5 个静态属性；这些少量冷成本
计入该案例，主 benchmark 的热身也保持原样。克隆仍然分配新 DOM 元素，不能把
createElement 次数减少解释为元素数量减少。万行最终仍为 **80,008 Elements、30,006
Text、4 Comments、320,037 Attributes**；live graph/Owner 数及清理基线也一致。

最终测量版本冻结于 **2026-10-23 22:47:22 +08:00**，248 个输入，Git 基点
`945c077b6a3f167c9e79cbc75f726b1d6316e34d`，包含当时未提交的并行语义工作。
为隔离优化，从最终公共源码派生 before-ready，仅撤回 mapping 规范化、内联 Part、
原型创建及其单元测试入口。sources/inputs.json 验证四个差异路径，其他代码和工作负载
逐字节一致；没有使用先前较老的 before 二进制作最终基线。

before-ready/final 使用两个全新独立 D: Cargo target，metadata 验证输出路径，编译日志
确认应用 artifact fresh=false。release opt-level=3、LTO、一个 codegen unit，
wasm-bindgen 库/CLI 0.2.129，无 wasm-opt；两个应用严格 WASM Clippy 通过。
Windows 11 / Rust 1.98.0 MSVC / i7-14700KF / Chrome 155.0.8059.84，
1280×900、scale 1；同一独立 Chrome、每版每轮新上下文，固定 P 核逻辑 CPU 0–15、
AboveNormal，进入回调前读回核对。顺序 before-ready→final，再 final→before-ready；
每项 3 次热身、12 个样本，每格合计 24 个。计时 harness/CDP meter 与此前相同，
准备、数据验证、截图、CPU 采样和 DOM 包装全部在主计时外。仍是本机观测，非独占实验室。

| 操作 | 总耗时前→后 ms | Script 前→后 ms | Script 降幅 |
|---|---:|---:|---:|
| 创建千行 | 96.70→84.85 | 69.34→57.58 | 17.0% |
| 替换千行 | 116.20→112.50 | 91.13→84.04 | 7.8% |
| 更新每十行 | 11.10→13.00 | 0.66→0.67 | -0.7% |
| 选行 | 8.05→8.40 | 1.06→1.05 | 1.1% |
| 交换两行 | 10.10→9.85 | 0.56→0.59 | -5.3% |
| 删除一行 | 10.70→11.65 | 0.60→0.65 | -6.8% |
| 追加千行 | 102.90→94.85 | 72.51→62.81 | 13.4% |
| 创建万行 | 1036.20→944.40 | 764.40→677.15 | 11.4% |
| 清空千行 | 34.65→34.40 | 31.97→30.96 | 3.2% |

总耗时为点击至两次前台 RAF，不证明真实显示完成/FPS。短操作 Script 仅有数十微秒差异，
总耗时受 RAF 等待影响，包含不利结果，不将这些小差值解释为稳定收益或退化。
两轮关键 Script 中位数分别为：创建千行 **68.22→58.52 / 69.95→55.34**，
替换 **92.97→87.53 / 89.79→81.32**，万行创建 **733.98→663.46 / 843.62→693.64**。
初版原型键尚未压缩的 432 个候选样本也保留在 paired，不与最终样本混合或择优。

另跑 **32 个未包装 DOM 方法的替换 CPU profile**，保持反向顺序，排除在主表之外。
包含式采样中位数：DOM Sink **43.36→35.84 ms**，UTF-8 decode **16.14→11.09 ms**，
Element build **14.92→15.77 ms**，Memory apply **13.55→13.34 ms**，
HTML mapping::shape **3.74→3.69 ms**，Owner dispose **12.37→12.49 ms**。
这些函数族相互包含，不能相加；profile 本身有开销，内联/未采到的帧不等于零成本。
这组证据支持 DOM 创建/桥接成本下降，不支持声称 HTML shape 或 Owner 单独明显变快。
本轮三项实现一起对照，没有把总收益任意分摊到每一项。

千行替换仍要退休旧 Owner、分配/构建完整新树、映射、创建 8,000 个全新元素并写
15,000 次动态属性，最终 Script 仍约 84 ms。后续优先研究动态元数据/类型化 DOM
提交减少字符串转换，其次才是 shape 的小集合表示和构造分配；保留可验证代际身份、
原生属性反射、前缀失败及 IME/选区契约。原型最多增加 64 个离线元素，WASM 原始体积
**868,675→872,899 bytes（+4,224）**，没有测量 RSS，不声称内存字节下降。

完整验证：

- 全部 432 样本行数/摘要一致；更新/选择/交换/删除/追加保留存续 Element、Text Node、
  data-gly-node；替换旧后代断开，万行矩形摘要与六个视口前后 **12 张 PNG** 一致。
- Windows/Linux 各 **448** 项工作区测试、各 **289** 项 fork 测试，fmt、严格原生/WASM
  Clippy 通过；Linux no-default/paint 通过，双平台各两张 CPU raster PNG 输出成功。
- Windows shell/accesskit 真实窗口、双窗口真实 UIA，以及 Linux X11/Wayland×
  shell/accesskit 四组真实窗口通过。此处不替代 Unix AT-SPI/屏幕阅读器验收。
- 浏览器 **25/25** 通过。新增创建隔离探针覆盖文本/属性/身份污染后重建、输入 value/
  purpose/disabled/checked/label 污染后的新行，18 个观察到的原型、161 次克隆，零泄漏。
  原有 24 个脚本与上轮全回归逐字节一致；SSR adoption/受控输入/选区/composition 均通过。
- 变高列表 **64/1024** 各 70 次真实小数行高变化、原有 0.1/0.15 容差和 **73 张截图**
  通过，锚点均 -15.125→-15.125，最终滚动位置/几何/画面一致。
- SSR fixture 四项测试、真实 Shell/Patch/End 和两个资源交接、断开取消/慢消费者背压通过。

验证快照首次遇到并行新增的 AccessKit 测试使用 NodeId 查询 FullNodeId 接口而编译失败；
仅同步工作树已有的测试修正后重跑。新隔离测试首次误把 outside-focus 哨兵输入算作行，
修正列表内选择器并另外验证哨兵后重跑全部 25 项，未放宽原断言。原文件和失败日志均归档。
期间另有 render::Memory 验名快路径及 HTML Role lowering 修正：integration/source 单独
冻结这四个变化文件，双平台各 448 工作区/289 fork、原生/WASM Clippy、Windows 窗口/UIA
与 Linux raster 补验通过。本轮性能和完整浏览器矩阵仍对应 final，未冒称并行改动已经重测性能。
收尾时工作树又出现 Layer/top-layer、described_by 等新实现，包含 Dom 的层管理钩子；
这些新增保持原样，未纳入上述冻结版本的性能/回归结论。最终源码差异和新增路径随证据记录，
不通过不断变更被测快照来替换已经完成的本轮结果。

完整证据：[evidence-index.json](/D:/BuildCache/glory-create-map-20261023/evidence-index.json)。
同目录 evidence.py 核验冻结输入/测试 overlay、独立构建、全部样本、身份/画面、CPU 设置及
回归日志；summarize-final.py 重算主表。重现需使用归档 driver/bulk-compare.cjs 与
pin-browser.ps1、D: TEMP/TMP，服务目录根，分别构建 before-ready/final，使用新的输出目录。
final-paired 是最终主表，final-integrity 是深身份探针，decode-profile/unwrapped 是独立诊断；
最早 before/baseline/release/paired 和中途 before-final 构建仅保留为过程记录，不充当最终基线。

## Three-framework rerun after teardown optimizations — 2026-10-23

本节保留 DOM 创建优化前的三框架运行；最新结果见[创建提交与 Dioxus 分析](#bulk-creation-and-dioxus-design-analysis--2026-10-24)。

本轮重新比较 Glory、**Leptos 0.8.19**、**Dioxus 0.7.9**。三者均从冻结源码在
D: 上使用新的独立 target 构建 release；opt-level=3、LTO、一个 codegen unit，
wasm-bindgen 库/CLI 0.2.129，不使用 wasm-opt。严格 WASM all-target Clippy 和
应用格式检查均通过。15 个 benchmark 应用/共享输入与上一轮三框架比较逐字节一致，
包括应用实现、锁文件、数据生成器、CSS 和计时脚本；本次只重新构建和测量。

冻结时间为 **2026-10-23 22:12:36 +08:00**，共 **243 个输入**，包含当时尚未提交的
Scope/unlink 优化和输入辅助/Check 修改，Git 基点为
`7874f2c39bd3e40ac1945ba34455192cc5511afc`。构建/测量期间工作树有并行提交和语义相关
修改，未混入本轮二进制；精确差异路径见 summary.json 的 post_snapshot 字段。
本节描述这份冻结版本，不把结果扩展为随后工作树的性能或回归证明。

环境为 Windows 11、Rust 1.98.0/MSVC、i7-14700KF、Chrome **155.0.8059.84**，
1280×900、scale 1。三者浏览器进程统一固定逻辑 CPU 0–15（P 核，mask 0xffff）、
AboveNormal 优先级；每个新上下文进入验收/热身前设置并读回核对，计时区间没有这些操作。
每轮独立 Chrome、每个框架独立上下文：**Leptos→Glory→Dioxus**，再
**Dioxus→Glory→Leptos**。每项 3 次不计时热身、每轮 12 个样本，合并后每格
**24 个样本，共 648 个**。本机另有项目编译，两轮开始时的 Cargo/rustc 活动已归档；
固定核心不代表独占机器，小幅差异仍需结合逐轮数据判断。

下面是点击到两次前台 RAF 回调的**中位耗时，ms，越低越好**。准备数据、数据摘要、
身份、几何和截图验证在计时区间外；完整 keyed 列表所有行均挂载，未使用虚拟列表。
该指标不等于实际显示完成、FPS 或官方 HTML-table 分数。

| 操作 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 90.40 | 39.55 | 30.05 |
| 替换千行 | 116.10 | 45.45 | 33.45 |
| 更新每十行 | 10.60 | 11.20 | 11.70 |
| 选行（混合流程） | 7.90 | 175.80 | 8.40 |
| 交换两行 | 9.15 | 10.15 | 9.75 |
| 删除一行 | 9.55 | 10.20 | 11.10 |
| 追加千行 | 98.05 | 48.10 | 36.50 |
| 创建万行 | 1012.15 | 451.30 | 306.95 |
| 清空千行 | 33.70 | 19.25 | 9.35 |

相同区间 CDP **ScriptDuration 增量中位数，ms**，包括共享 harness/绑定桥开销。
各包含式 CPU 指标不能直接相加；Layout/RecalcStyle/Task 原始值和中位数保留在证据中。

| 操作 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 64.49 | 14.43 | 4.51 |
| 替换千行 | 89.17 | 21.82 | 8.30 |
| 更新每十行 | 0.64 | 0.35 | 0.87 |
| 选行（混合流程） | 1.05 | 173.97 | 0.76 |
| 交换两行 | 0.55 | 0.18 | 0.97 |
| 删除一行 | 0.59 | 0.30 | 0.80 |
| 追加千行 | 66.47 | 16.91 | 5.62 |
| 创建万行 | 741.00 | 185.25 | 48.63 |
| 清空千行 | 31.52 | 16.17 | 3.05 |

Glory 的主要差距仍在批量创建、替换、追加、清空。万行创建的总耗时约为 Leptos
**2.24 倍**、Dioxus **3.30 倍**；千行替换约为 **2.55/3.47 倍**。
局部更新、交换和删除的总耗时接近，短操作受 RAF 等待影响，不能从零点几毫秒差值
推出稳定排名。小更新的 Script 更能区分计算成本：本轮 Leptos 较低，Glory 和
Dioxus 在不同操作上各有差异。

两轮关键操作的总耗时/Script 中位数如下，供检查合并中位数的稳定性：

| 框架/操作 | 第 1 轮 总耗时 / Script | 第 2 轮 总耗时 / Script |
|---|---:|---:|
| Glory 创建万行 | 1022.80 / 755.12 | 996.00 / 733.46 |
| Leptos 创建万行 | 451.30 / 186.77 | 445.50 / 185.25 |
| Dioxus 创建万行 | 306.95 / 48.98 | 300.55 / 48.44 |
| Glory 替换千行 | 115.15 / 90.17 | 116.25 / 88.95 |
| Leptos 替换千行 | 46.90 / 21.94 | 44.20 / 20.99 |
| Dioxus 替换千行 | 40.65 / 8.95 | 31.40 / 7.64 |
| Glory 清空千行 | 35.30 / 32.36 | 32.70 / 31.39 |
| Leptos 清空千行 | 19.55 / 17.01 | 17.00 / 15.78 |
| Dioxus 清空千行 | 9.45 / 3.24 | 9.20 / 2.90 |

**Leptos 选行仍明显受运行历史影响。**主流程保留原样，两个方向的 Script 分别为
172.40/175.19 ms。另用全新上下文比较首次千行列表与 30 次替换后选行，
每格 3 次热身和 12 个样本，72 个诊断样本不混入主表：

| 框架 | 首次千行列表 总耗时 / Script | 30 次替换后 总耗时 / Script |
|---|---:|---:|
| Glory | 11.75 / 1.25 | 11.80 / 1.25 |
| Leptos | 11.75 / 1.21 | 11.70 / 6.09 |
| Dioxus | 11.55 / 0.70 | 11.65 / 0.76 |

这说明主流程异常不能当作 Leptos 普遍的单次选行成本；单独的 30 次替换也没有
复现完整混合流程的幅度。本轮没有进一步定位其内部原因。

验收结果：

- 三者通过独立种子 oracle、九项操作、keyed 交换/替换和清理检查；全部 648 个样本
  的行数/数据摘要跨框架、跨轮一致。
- 补充身份探针确认更新、选择、交换、删除、追加保留存续行的 Element 和 Text Node；
  Glory 还核对 data-gly-node 身份。替换后旧后代全部断开。
- 全部 10,000 行有真实布局盒，矩形摘要一致；创建千行、替换千行、追加千行、
  创建万行、万行中部、清空六个视口检查点，三框架两轮共 **36 张 PNG** 逐字节一致。
- 万行 DOM 计数保持 Elements 80,008 / Text 30,006；Glory/Leptos/Dioxus 的
  Comments 为 4/1/0，Attributes 为 320,037/120,009/140,015。
  Glory 清空后回到 0 行/label、10 语义节点、3 图节点、13 Owners。
- 三个独立 WASM 应用的严格 Clippy/格式检查通过。本次没有重跑原生/窗口/流式或
  64/1024 变高列表全回归；这些不属于此完整 DOM 三框架 benchmark 的覆盖范围。

历史三框架表采用默认 CPU 调度，不能把其绝对差值归因于本轮或某一个优化。
销毁优化的因果对照仍见下方各自的 before/after 实验。当前 DOM 元数据计数较高是
后续创建/提交优化的调查方向，计数本身不证明耗时占比或 RSS。

完整证据保存在
[evidence-index.json](/D:/BuildCache/glory-framework-recompare-20261023/evidence-index.json)，
含冻结输入、独立 target 元数据、fresh 编译证明、构建/站点/驱动/样本/截图哈希、
逐轮结果、CPU 设置及独立选行诊断。同目录 verify.py 重算所有中位数并核验数据、身份、
几何、像素和来源；run.ps1 记录本轮服务、顺序和进程清理，重跑须使用新输出目录。
driver 下仅增加计时外验收/CPU 设置和归档；bench-harness.js/browser-run.js 保持原字节。

## Scope cleanup and sibling unlink — 2026-10-23

本轮继续优化 UI Scope 的逐节点清理及 `Memory::unlink`。普通 keyed `each`
按旧显示顺序退休：旧 `position` 通常首项即命中，但每次 `Vec::remove(0)` 都搬移
余下兄弟，连续清空形成二次方工作。另一个重复成本是 Scope 先收集完整子树再清理，
每个节点分别更新节点 Owner 和监听 Owner，空记录还需第二次哈希删除。

已实现：

- render 新增只读切片接口的 `Children`：前方删除推进游标，前插可复用空前缀；
  已移除的数值前缀超过 `live + 64` 才压缩，空序列立即清除。因此物理长度有界于
  `2 * live + 64`，连续前删摊销线性。Clone、迭代、Debug 仅暴露活跃身份。
  容量沿用峰值；不保留业务数据或所有访问过的行。任意中部删除仍为线性。
- `unlink` 直接判断首尾，并在修改前取得 next，修复前驱时省掉再次查询被移节点。
  Place-before-first 也走直接路径。根列表仍提供切片；公开 `Node.children`
  从 Vec 改为 Children，需要 Vec 快照的调用方使用 `to_vec()`。
- Scope 使用同一 DFS 栈遍历和清理，省去第二份完整身份数组；同一 Owner 的节点
  和监听兴趣一起更新，通过 occupied entry 移除空记录。其他 Scope 注册的监听、
  独立查询及活跃 Owner 边界保持原语义。
- 已提取整个 Owned 记录的当前 Scope 直接跳过对应索引项。Rust 1.98 的 std
  使用 hashbrown 0.17.1，缺失键的 `entry()` 会先 reserve(1)，即使不插入；
  代码避免这条无用路径。来源与哈希见 `stdlib-entry-evidence.json`。

用户回调顺序未调整，捕获依然在图/呈现借用外释放。算法回归用 Vec oracle 检查
混合前/中/尾编辑、32k 退休的压缩界和摊销搬移量，以及压缩后的移动/快照/代际复用。
新增 UI 回归覆盖本地和跨 Scope 监听注销、析构重入与存活外部节点身份。

按实现计算，每份 Memory 中连续前删的身份搬移数如下；它不是计时、RSS 或内存带宽测量：

| 行数 | 原 Vec 搬移身份数 | Children 搬移身份数 |
|---:|---:|---:|
| 1,000 | 499,500 | 737 |
| 10,000 | 49,995,000 | 9,529 |
| 32,768 | 536,854,528 | 32,183 |

基线 239 个、新版 240 个冻结输入；新增 Children，实现改动只在 render memory 和
UI app，其余差异是回归测试及一个 Vec 快照调用。两个版本在独立 D: target 全新
release 构建：opt-level=3、LTO、单 codegen unit、wasm-bindgen/CLI 0.2.129、
无 wasm-opt。应用、锁文件、种子、数据生成器、CSS、计时脚本均相同。
完整回归使用与最终被测构建逐字节相同的 240 个输入。

主对照仍是 `semantic-rows-v1`，完整挂载每行，三次预热、12 次采样、正反两轮，
每个版本/操作 24 样本，合计 **432 样本**。Chrome 155.0.8059.84，1280×900、scale=1。
默认调度出现较大波动且曾有并行编译，因此主对照将本轮 Chrome 进程固定在本机
已核实的 P 核（逻辑 0–15，mask 0xffff），优先级 AboveNormal；两版条件相同。
CPU 拓扑、每个 context 的进程设置和独立驱动均归档。没有更改其他任务的设置。
它与历史默认调度数字不可直接拼接；也不证明系统完全空闲。

中位 ms；Wall 为点击到两次 RAF，Script 为 CDP 脚本计数器差值，包含共同桥接开销。

| 操作 | Wall 前 → 后 | Script 前 → 后 | Script 降低 |
|---|---:|---:|---:|
| create_1k | 100.65 → 101.60 | 72.75 → 73.57 | -1.1% |
| replace_1k | 123.40 → 119.50 | 94.51 → 93.02 | 1.6% |
| update_10th | 12.20 → 10.75 | 0.70 → 0.66 | 6.0% |
| select_row | 8.05 → 8.05 | 1.04 → 1.03 | 1.3% |
| swap_rows | 10.80 → 11.10 | 0.60 → 0.57 | 4.7% |
| remove_row | 10.85 → 11.40 | 0.70 → 0.64 | 8.5% |
| append_1k | 106.80 → 104.75 | 76.21 → 73.22 | 3.9% |
| create_10k | 1395.00 → 1031.50 | 1027.60 → 752.14 | 26.8% |
| clear_1k | 44.85 → 33.50 | 43.06 → 31.24 | 27.5% |

清空千行的 Script 合并中位数 **43.06 → 31.24 ms（-27.5%）**；两个顺序分别
45.57 → 31.17、35.70 → 31.48 ms，方向一致但幅度有明显差异。
替换千行仅 **94.51 → 93.02 ms（-1.6%）**，两个顺序为 101.90 → 96.17、
91.42 → 91.18 ms，没有稳定的大幅收益。create_10k 的 runAll 分数含前序反复
创建/清空历史，不能仅凭此表把其差异归因于纯创建路径。

另行包装 DOM 方法/CPU 采样的六案例诊断不进入主计时：

| 诊断 | Busy sampled 前 → 后 | Owner 含回调前 → 后 | UI Scope 前 → 后 | Memory::apply 前 → 后 |
|---|---:|---:|---:|---:|
| replace_1k | 134.75 → 120.00 | 14.49 → 11.36 | 11.78 → 8.69 | 16.54 → 10.88 |
| clear_1k | 28.62 → 26.17 | 12.99 → 12.30 | 10.33 → 10.20 | 3.80 → 4.26 |
| clear_10k | 344.68 → 313.66 | 154.77 → 133.55 | 119.60 → 97.45 | 70.88 → 41.23 |
| clear_1k_after_10k | 32.65 → 33.05 | 13.61 → 13.26 | 11.46 → 9.00 | 3.32 → 5.19 |

万行清空的 Memory::apply 样本约 70.88 → 41.23 ms，Scope 约 119.60 → 97.45 ms。
旧 unlink 独立帧约 17.94 ms，新版未采到独立帧；内联影响归属，不能解释成零成本。
上述函数族是包含式、有重叠，不可相加，也不是重复计时中位数。

为定位替换，另外做了不包装 DOM 方法的正反两轮采样，16 个诊断/版本，
同样固定 P 核；这些也不混入主分数。包含式样本中位数：

| 替换诊断函数族 | 前 | 后 |
|---|---:|---:|
| Owner 含回调 | 14.32 | 12.55 |
| Scope 清理 | 10.97 | 9.22 |
| 独立 unlink 帧 | 0.53 | 0.53 |
| DOM sink | 49.86 | 47.32 |
| Element build | 17.15 | 17.32 |
| Memory apply | 17.82 | 13.80 |

替换使用全部新 key，仍需销毁旧行并创建 8,000 个元素、3,000 个文本节点、
写 32,000 次属性。诊断中 DOM sink 约 47–50 ms，Element build 约 17 ms，
Owner 含回调仅约 13–14 ms，独立 unlink 帧不到 1 ms。
因此本轮主要缩短的退休阶段只占替换的一部分；完整创建、映射、字符串转换和 DOM
提交继续占大头。函数族有包含关系，不能据此拆出相加的精确比例。
采样诊断本身也有开销，不能拿它的 Script 绝对值代替主对照。

附加保留了 1,296 个默认调度样本：初版候选、修正版、并行编译结束观察期各 432 个，
与受控主对照分开。初版候选替换明显变慢，后续发现并避开缺失 entry 容量准备；
但源码事实不等于证明它导致了全部回退，默认调度/热身/运行历史也存在明显波动。
这几组数据和全部不利结果均保留在 paired、final-paired、confirmed-paired，
主数据在 pinned-paired；不混合调度条件，也不只挑最短耗时。

验证结果：

- 每个计时样本核对数据摘要；更新/选择/交换/删除/追加保留 Element、Text Node 和
  语义身份；替换后旧后代断开；万行几何摘要和六个视口 PNG 检查点一致，含万行中部。
- 六个诊断案例的 DOM 方法/属性次数及图、Owner、节点数量一致；清空回到
  0 行/label、10 语义节点、3 图节点、13 Owners。
- Windows/Linux 各 **434** 项工作区测试、各 **289** 项 fork 测试通过；原生/wasm
  严格 Clippy、fmt、Linux 无默认 feature 和 paint 检查通过。
- Windows shell/accesskit 真实窗口及双窗口 UIA、Linux X11/Wayland × shell/accesskit
  四次窗口探针和 CPU raster 通过；这些不替代 Unix AT-SPI/屏幕阅读器验收。
- 浏览器 **24/24** 套件通过；64/1024 测量缓存各 70 次小数更新、真实行高、
  原有 0.1/0.15 容差、节点身份及各 73 张锚点截图通过，最终位置/画面一致。
- SSR fixture 四项测试、流式 Shell/Patch/End、资源交接、断连取消及慢消费者背压通过。

新增游标和已移除的数值前缀属于额外元数据，容量不主动收缩；以上节点数/身份/像素证明
不等于 RSS 降低。后续中间批量删除若成为主路径，需要另行评估有序结构/批量过滤；
本轮改善的是保持退休顺序的连续前删。替换的下一重点应是创建/HTML lowering/DOM
提交的分配与转换开销，并保持新 key 的身份和销毁契约。

冻结版本验收完成后，工作树另有并行新增的 input.rs 与 mod input 声明，提供两个输入辅助 API。
该独立新增未混入性能基线；已保留，并另冻结 241 输入的 integration/source，
补做 Windows 全工作区 434 项测试及原生/wasm 严格 Clippy。
随后新增的 widget_input.rs 两项测试也单独通过，未混入上述 434 项冻结套件。
原来的双平台/浏览器全套验收对应最终被测的 240 输入版本，不将它扩展为新增 API 的行为证明。

完整源码/构建哈希、四组原始分数、CPU profiles、DOM 计数、截图、进程设置和全回归：
`D:/BuildCache/glory-scope-unlink-20261023/evidence-index.json`。
`evidence.py` 重验所有来源、样本、身份/画面及回归；`profile-summary-pinned.py`
重算函数族。复现主对照用独立 before/final 构建，目录 HTTP 服务和已归档的
`bulk-pinned.cjs`/`pin-browser.ps1`；TEMP/TMP 设为该目录 pinned-tmp，计时使用原有
bench-harness.js/browser-run.js。其他完整验证入口为 run-windows.ps1、linux-base.sh、
linux-window-root.sh 和 scripts/test-semantic-browser.cjs。

## Owner and Binding retirement follow-up — 2026-10-23

本轮在 DOM 父表修复后冻结新基线，继续定位 Owner/Binding 销毁。清空万行的
基线 CPU 诊断中，`Owner::dispose` 含回调约 155.29 ms，其中 UI Scope 清理
约 127.71 ms；核心 `State::dispose_owner` 约 18.06 ms，`remove_node`
约 5.30 ms。后两者是包含关系，所有包含式统计均不可相加。
Owner 栈包含语义树、监听、查询和组件资源清理，不能全部归因于响应式图。

实际优化：

- `glory-core` 先用代际身份取出 Owner。重复退休直接返回，叶 Owner 不再先分配
  `vec![key]`，有孩子才扩展遍历栈。完整退休后执行回调、子先父后/反向注册顺序、
  借用外释放捕获，以及任务取消规则保持不变。
- `glory-ui` 将同一 Instance 的节点、监听、查询集中到一个 `Owned` 记录。
  最后一个兴趣退休时移除记录，后续 Scope 回调一次未命中即可返回。
  外部节点上的监听、外部目标的查询仍归注册 Scope，三类兴趣互不代替。
  `UiStats` 独立计数节点 Owner 和查询 Owner，不扫描所有 Owner 来计算。

Binding 仍从实际存活来源注销，立即取消调度成员，在图借用之外释放捕获。
此处没有全图 retain；双向边注销和用户析构是必要工作。队列代际校验、FIFO、
阶段顺序及 `2 * live + 64` 物理条目上限保持原契约。

两份冻结源码各 239 个输入，仅两个实现文件、两个回归测试文件不同。
各自从独立 D: target 全新构建，opt-level=3/LTO/单 codegen unit，
wasm-bindgen/CLI 0.2.129，无 wasm-opt。`semantic-rows-v1` 应用、锁文件、
生成器、种子、CSS、计时脚本完全相同，每行完整挂载。
Chrome 155.0.8059.84，1280×900，scale=1；正反顺序各一轮，独立 context，
每项三次预热、12 次采样：**432 样本，每个版本/操作 24 样本**。
回归冻结树与被测 release 的全部 239 个输入逐字节一致，无后续实现修补。

中位数，单位 ms。Wall 为点击到两次 RAF；Script 为 CDP 脚本计数器差值，
包含共用驱动开销。本机另有独立 Cargo 任务；本次对照不代表专用空闲机器。

| 操作 | Wall 前 → 后 | Script 前 → 后 | Script 降低 |
|---|---:|---:|---:|
| create_1k | 91.35 → 91.45 | 64.99 → 65.23 | -0.4% |
| replace_1k | 118.35 → 121.55 | 92.11 → 94.39 | -2.5% |
| update_10th | 11.75 → 12.35 | 0.74 → 0.68 | 8.7% |
| select_row | 8.50 → 10.05 | 1.13 → 1.07 | 5.4% |
| swap_rows | 10.90 → 9.60 | 0.58 → 0.56 | 4.1% |
| remove_row | 11.30 → 8.15 | 0.62 → 0.62 | 0.2% |
| append_1k | 100.35 → 100.80 | 71.88 → 70.11 | 2.5% |
| create_10k | 1061.25 → 1058.55 | 775.26 → 767.35 | 1.0% |
| clear_1k | 37.65 → 34.85 | 34.35 → 32.05 | 6.7% |

清空千行在两个顺序中均改善，分别约 10.4% 和 6.6%；合并后的 Script 中位数
**34.35 → 32.05 ms，降低 6.7%**。替换千行合并后慢 2.5%，第二轮 after
波动尤其大（116.04 ms，对照 93.00 ms），不能称为替换优化。
创建万行的 Script 差异约 1%，没有足够证据声称创建提速。
小操作的 RAF 差异也不作为架构收益。

另行包装 DOM 方法及 CPU 采样的诊断，不进入上述 432 个计时样本：

| 诊断 | Busy sampled 前 → 后 | Owner 含回调前 → 后 | UI Scope 清理前 → 后 |
|---|---:|---:|---:|
| replace_1k | 123.16 → 112.10 | 13.65 → 12.20 | 11.39 → 11.15 |
| clear_1k | 30.47 → 26.75 | 15.22 → 13.78 | 12.11 → 9.48 |
| clear_10k | 320.74 → 311.47 | 155.29 → 142.28 | 127.70 → 108.97 |
| clear_1k_after_10k | 34.07 → 33.03 | 14.36 → 14.64 | 10.11 → 10.88 |

万行 Owner 含回调样本降低约 8.4%，UI Scope 清理降低约 14.7%。
同次核心 `State::dispose_owner` 为 18.06 → 21.13 ms，`remove_node` 为
5.30 → 5.74 ms，核心部分未显示稳定降低。两项修改合并测量，未做单项消融，
不能给免分配快路径单独分配收益。编译内联/采样粒度影响函数归属，
诊断不能代替重复计时分布。

六个诊断案例的 DOM 方法、属性写入次数和节点/图统计完全一致。
每千行增加 8,000 语义节点、6,000 图节点、9,000 Owners；替换仍创建
8,000 个元素、写 32,000 次属性、删除 1,000 个行根。
清空回到 0 行/label、10 语义节点、3 图节点、13 Owners。
合并记录的单条载荷更大，活跃条目和节点数不等于 RSS；本轮不声称内存字节降低。

一致性与完整回归：

- 所有计时样本核对种子数据；更新、选择、交换、删除、追加保留存活 Element、
  Text Node 和语义身份，替换后旧后代断开，万行几何摘要一致。
- 六个视口 PNG 检查点逐字节相同，含万行中部；两份构建的 DOM 调用计数一致。
- Windows/Linux 各 430 项工作区、各 289 项 fork 测试通过；原生/wasm 严格
  Clippy、fmt、Linux 无默认 feature/paint 检查通过。
- Windows shell/accesskit 两次真实窗口探针和双窗口 UIA 通过；Linux
  X11/Wayland × shell/accesskit 四次真实窗口探针及 CPU raster 通过。
- Chromium 24/24 套件通过；64/1024 测量缓存各 70 次小数更新、真实行高、
  原有 0.1/0.15 容差、节点身份及各 73 张锚点截图通过，最终位置和画面一致。
- SSR fixture 四项测试及流式 Shell/Patch/End、资源交接、断连取消、
  慢消费者背压通过。
- 新增测试覆盖重复退休后的 Binding 槽位复用、回调顺序、捕获析构重入，
  以及目标退休后的独立查询完成、兴趣清空后的重新注册、退休请求不再回调。

剩余优先项是 UI Scope 中逐节点索引操作，以及语义 `Memory::unlink` 的
兄弟位置搜索/Vec 搬移；万行诊断后端 DOM sink 仍约 108 ms。
依赖边和捕获释放可继续细分，但当前纯核心占比不支持优先进行更大的生命周期重写。

原始样本、构建/源码哈希、CPU profiles、DOM 计数、截图和完整日志：
`D:/BuildCache/glory-owner-retirement-20261023/evidence-index.json`。
`evidence.py` 校验冻结源码、样本、像素、身份、回归及产物哈希；
`profile-summary.py` 重算包含式函数族时间。
复现使用该目录 before/release/source，以 `benchmarks/build.ps1` 指定独立
BuildRoot，启动目录 HTTP 服务，再执行 `bulk-compare.cjs`、`--integrity`
及 `summarize-bulk.py`。完整验证入口为已归档的 `run-windows.ps1`、
`linux-base.sh`、`linux-window-root.sh` 和 `scripts/test-semantic-browser.cjs`。

## DOM retirement follow-up — 2026-10-23

The parent lookup added for DOM keyboard/drag routing used `HashMap::retain`
for every disposed subtree. Clearing a keyed list disposes many row subtrees,
so each row scanned unrelated live parent entries and the table's retained
capacity. Repeated row teardown could approach quadratic work. A 10k run also
left a larger table allocation for later 1k clears; the main benchmark's setup
includes those expensive 10k clears outside the timed action interval.

`Dom::dispose` now removes parent and pending-size entries by each `NodeId`
in the mapper's complete semantic subtree. It includes parked descendants;
no source scan, new cache, ID lifetime change or capacity shrinking is involved.
The normal input/layout-queue filters and observer retirement remain in place.
The row workload does not subscribe to Resize, so the large regression here
comes from the parent lookup rather than a populated measurement cache.

The before/release snapshots differ only in `crates/web/src/browser/dom.rs`.
Both were freshly built in independent D: release targets, opt-level=3/LTO/one
codegen unit, wasm-bindgen/CLI 0.2.129 and no wasm-opt. The benchmark app, seed,
CSS, data generator and timing harness are byte-identical. Chrome 155.0.8059.84,
1280×900, scale 1; before→release then release→before, fresh contexts, three
warmups and 12 measured samples per operation/round: **432 samples**, 24 per
variant/operation. These are contemporaneous paired results, not reused values
from the earlier three-framework run.

Median ms. Wall is click-to-two-RAF; script is the CDP counter delta including
common bridge/harness work. This is a local full-DOM benchmark, not FPS,
presentation latency, RSS or an official HTML-table score.

| Operation | Wall before → after | Script before → after | Script reduction |
|---|---:|---:|---:|
| create_1k | 109.75 → 103.40 | 78.69 → 73.10 | 7.1% |
| replace_1k | 221.50 → 143.80 | 195.08 → 110.52 | 43.3% |
| update_10th | 11.20 → 11.10 | 0.68 → 0.70 | -3.0% |
| select_row | 8.05 → 8.10 | 1.10 → 1.12 | -2.0% |
| swap_rows | 10.85 → 11.10 | 0.59 → 0.64 | -8.1% |
| remove_row | 11.10 → 10.25 | 0.95 → 0.67 | 28.8% |
| append_1k | 113.50 → 110.15 | 73.07 → 77.07 | -5.5% |
| create_10k | 1131.60 → 1089.40 | 788.80 → 814.76 | -3.3% |
| clear_1k | 150.15 → 40.30 | 148.41 → 36.23 | 75.6% |

The improvement is concentrated in replacement and teardown. Creation,
append and tiny updates are essentially unchanged; do not treat their small
percentage differences as gains from this patch.

Separate diagnostic runs wrap DOM methods and use a sampling profiler. They
are excluded from the 432 timing samples and are not additional median scores:

| Diagnostic | Busy sampled ms before → after | DOM sink inclusive ms before → after |
|---|---:|---:|
| replace_1k | 206.09 → 141.09 | 141.75 → 77.74 |
| clear_1k | 103.15 → 26.16 | 80.23 → 8.01 |
| clear_10k | 10227.81 → 386.48 | 9528.39 → 119.39 |
| clear_1k_after_10k | 211.88 → 34.88 | 179.26 → 12.26 |

The 10k clear previously spent about 9.53 s inside the DOM sink; after the
change it spends about 119 ms there. Owner disposal is still present (about
177 ms inclusively in that optimized diagnostic) and is now a larger part of
the remaining cost. Inclusive families overlap and must not be added together.
The profiler resolves the adapter sink rather than a precise inlined retain
instruction; the paired source change and unchanged native-operation counts
are the evidence for the parent-table attribution.

All six diagnostic cases have identical native call/attribute counts and live
DOM/graph census across variants. Replacement creates 8,000 elements, writes
32,000 attributes and removes 1,000 row roots in both; clear-1k removes 1,000
roots, clear-10k removes 10,000. Speedup comes from less metadata work.

Correctness checks passed for both builds: seeded row data for all operations
and timing samples, surviving Element/Text Node/semantic IDs through update,
select, swap, remove and append, old descendants disconnected on replacement,
all 10,000 row rectangles equal, six viewport PNG checkpoints byte-identical
(including the middle of 10k rows), and cleanup back to zero rows/labels,
10 semantic nodes, 3 reactive nodes and 13 Owners.

Full regression on the frozen validation tree:

| Gate | Result |
|---|---|
| Windows workspace / fork tests | 428 / 289 passed |
| Linux workspace / fork tests | 428 / 289 passed |
| Browser | 24/24 suites passed |
| Windows real windows / UIA | shell + AccessKit windows and actual two-window UIA client passed |
| Linux real windows / paint | X11/Wayland × shell/AccessKit, CPU raster passed |
| Formatting / Clippy | validation workspace, native and WASM all targets, fixture passed |
| SSR / streaming | 4 fixture tests, actual progressive delivery, cancellation and slow-consumer backpressure passed |

The browser regression also reruns the real fractional-height lists with 64
and 1024 entries: both maintain zero anchor drift through 70 fractional changes,
live descendant/Text identities, equal settled PNGs and identical final logical
and physical Home positions. The original 0.1/0.15 px checks are unchanged.

The validation tree started as a copy of the measured release source. Seven
files were normalized with rustfmt. Full-feature Clippy then required equivalent
let-chains in existing native/Web drag and theme branches; no-shell Clippy
required gating private desktop drag state/initializers by `shell`. The fixes
are also in the working tree. `native-lint-fix.py`, `web-lint-amendments.json`,
`dom-lint.diff` and `drag-lint.diff` preserve the exact amendments. The initial
and second failed attempts remain in `windows-initial.txt`, `linux-initial.txt`,
`windows-second.txt` and `linux-second.txt`; only subsequent complete runs count
as accepted. Later parallel worktree edits are excluded.

After those gate fixes, `final/source` was built independently again. Its
benchmark/library input hashes exactly match the accepted validation tree.
The same workload passed another **108 samples**, six equal PNG checkpoints
and a full descendant/Text/semantic identity and 10k-geometry differential
against the measured release build. This is a final-source verification run,
separate from the 432-sample causal experiment above:

| Final-source verification | Wall median | Script median |
|---|---:|---:|
| replace_1k | 119.75 ms | 91.76 ms |
| clear_1k | 40.40 ms | 36.95 ms |
| create_10k | 1053.40 ms | 767.60 ms |


Everything is under `D:/BuildCache/glory-dom-retirement-20261023/`:
`before/source/inputs.json`, `release/source/inputs.json`, `validation/inputs.json`,
`paired/evidence.json`, `integrity/evidence.json`, `browser/evidence.json`,
`final/source/inputs.json`, `final-benchmark/evidence.json`, `summary.json`,
raw CPU profiles and final platform logs.
The [evidence index](/D:/BuildCache/glory-dom-retirement-20261023/evidence-index.json)
verifies sources, independent output directories, data/geometry/picture hashes,
method counts, accepted regressions and retained failed attempts.

## Current three-framework rerun — 2026-10-23

This section retains the run before the DOM/Owner/Scope retirement fixes. The
latest comparison is [the creation bridge/Dioxus analysis](#bulk-creation-and-dioxus-design-analysis--2026-10-24).

The current Glory snapshot was rebuilt alongside **Leptos 0.8.19** and
**Dioxus 0.7.9**, using fresh independent release targets on D:, opt-level=3,
LTO, one codegen unit and wasm-bindgen/CLI 0.2.129; no wasm-opt. All three
strict WASM Clippy and app formatting checks passed. The 15 shared/app build
inputs, including harness, data, CSS and lockfiles, are byte-identical to the
earlier semantic-rows-v1 comparison. Only Glory library sources differ.

Chrome **155.0.8059.84**, Windows/MSVC Rust 1.98.0, Intel Core i7-14700KF,
1280×900, scale 1. Two sequential rounds used fresh contexts in opposite order:
Leptos→Glory→Dioxus, then Dioxus→Glory→Leptos. Each operation warmed three
times and measured 12 samples per round: **24 samples per cell, 648 total**.
The table pools both rounds and reports medians. The browser had its own
process/profile; this is a local machine run, not an isolated lab measurement.
Sources were frozen before building. Later concurrent working-tree changes
did not enter these binaries; their paths are recorded in `validation.json`.

All three passed the independent seeded-data oracle, nine operations,
keyed swap/replacement checks, all 10,000 real row boxes and cleanup checks.
Every measured sample's final row hash/count agrees across frameworks and
rounds. All six create-1k checkpoint viewport PNGs are byte-identical.

Median **click to two foreground RAF callbacks**, ms, lower is better.
Setup/data validation is outside timing; this does not establish actual
display completion, FPS or an official HTML-table score.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 90.30 | 40.00 | 32.65 |
| replace_1k | 306.55 | 47.70 | 33.35 |
| update_10th | 14.35 | 15.40 | 11.25 |
| select_row | 9.10 | 267.70 | 8.10 |
| swap_rows | 10.60 | 10.65 | 9.70 |
| remove_row | 12.50 | 13.90 | 10.55 |
| append_1k | 177.95 | 89.65 | 37.95 |
| create_10k | 1625.25 | 739.20 | 341.85 |
| clear_1k | 152.65 | 21.60 | 9.10 |

Median CDP **ScriptDuration counter delta**, ms. This includes common
harness/binding-bridge work and is diagnostic; inclusive CPU metrics must not
be summed.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 63.93 | 14.87 | 4.75 |
| replace_1k | 266.76 | 23.06 | 8.09 |
| update_10th | 0.81 | 0.50 | 1.14 |
| select_row | 1.12 | 262.02 | 0.93 |
| swap_rows | 0.85 | 0.22 | 1.08 |
| remove_row | 1.18 | 0.41 | 1.07 |
| append_1k | 125.42 | 33.39 | 6.19 |
| create_10k | 1224.57 | 293.16 | 54.99 |
| clear_1k | 151.33 | 19.40 | 2.94 |

Per-round create-10k medians expose the repeatability limits:

| App | Round 1 wall / script ms | Round 2 wall / script ms |
|---|---:|---:|
| glory | 2067.90 / 1465.37 | 1519.95 / 1084.01 |
| leptos | 871.10 / 364.11 | 533.20 / 207.07 |
| dioxus | 353.60 / 57.01 | 321.55 / 52.91 |

Glory still trails on bulk creation/replacement/append/clear. Small updates,
reorders and removal are much closer in absolute cost; RAF waiting limits the
wall-time resolution for these short operations. There is no overall ranking.

Glory replacement varied substantially: 373.60 ms versus 209.50 ms
across the two rounds. Treat the pooled median as a local observation; do not
interpret small differences or historical deltas as a precise optimization effect.

Source inspection found a newly added cost in the current DOM adapter:
`Dom::dispose` calls `parents.retain(...)` for every disposed subtree, scanning
the whole live parent table. Disposing many row subtrees can therefore repeat
a full-table scan. This is a concrete profiling candidate for clear/replacement
and untimed setup cleanup; source inspection alone does not quantify its share
of the observed time. The frozen-source diff is retained as `parent-map-diff.txt`.
This rerun keeps that code intact; it does not substitute an older Glory binary.

The mixed-workload Leptos selection outlier is reported as observed. Separate
fresh-context diagnostics are excluded from the main 648 samples:

| App | Selection script, first 1k list | Selection script, after 30 replacements |
|---|---:|---:|
| glory | 1.19 ms | 1.54 ms |
| leptos | 1.27 ms | 6.78 ms |
| dioxus | 0.76 ms | 1.34 ms |

This establishes history sensitivity in the workload, not its root cause or
a general selection-performance ranking.

Ten-thousand-row census, outside timing:

| App | Elements | Text nodes | Comments | Attributes |
|---|---:|---:|---:|---:|
| glory | 80,008 | 30,006 | 4 | 320,037 |
| leptos | 80,008 | 30,006 | 1 | 120,009 |
| dioxus | 80,008 | 30,006 | 0 | 140,015 |

Glory's live 10k state: 80,010 semantic nodes, 60,003 reactive
nodes and 90,013 Owners. After clear it returns to the baseline
(10 semantic nodes, 3 reactive nodes, 13 Owners, zero rows/labels). Counts do
not measure RSS; no memory-byte claim is made.

Historical deltas describe different sessions, not a simultaneous causal A/B:

| Glory operation | Earlier wall → current ms | Earlier script → current ms |
|---|---:|---:|
| create_1k | 133.65 → 90.30 | 103.50 → 63.93 |
| replace_1k | 173.80 → 306.55 | 143.06 → 266.76 |
| append_1k | 169.55 → 177.95 | 129.07 → 125.42 |
| create_10k | 2809.90 → 1625.25 | 2156.60 → 1224.57 |
| clear_1k | 83.05 → 152.65 | 79.79 → 151.33 |

Use the earlier [paired bulk experiment](bulk-dom-optimization-2026-10-23.md)
for the controlled optimization comparison. The subsequent anchor correction
does not make this full-`each` workload a variable-height/virtual-list benchmark.

Frozen input manifest: `D:/BuildCache/glory-framework-rerun-20261023/source/inputs.json`.
Raw results: `round-1/evidence.json`, `round-2/evidence.json` under that build root.
The [evidence index](/D:/BuildCache/glory-framework-rerun-20261023/evidence-index.json)
contains all medians, per-round values, source/build/site/driver/PNG hashes,
independent target metadata and the separate selection diagnostic. No production
framework code was changed for this rerun. Runner changes add explicit ordering,
per-run screenshots/source preservation and an untimed DOM census.

## Fractional list anchor correction — 2026-10-23

The follow-up fixes the adaptive list's host scroll quantization residual.
Independently built before/final fixtures reproduce **0.375 → 0 px** drift with
both 64 and 1024 measurement entries, using actual fractional DOM row heights
and the unchanged 0.1 px anchor tolerance. Each configuration passed 70 repeated
fractional changes, live descendant/Text Node identity checks, 80 far seeks and
73 viewport PNG checkpoints. Corresponding settled images agree byte for byte;
both final logical and physical offsets are zero. All 24 browser suites pass.
This is correctness evidence, not a new bulk timing or presentation-latency run.
See the [fix, complete regression scope and frozen-source evidence](anchor-residual-fix-2026-10-23.md).

## Bulk DOM optimization — 2026-10-23

The paired follow-up kept the benchmark app/data/CSS/harness byte-identical and
used independently built before/after WASM, two reversed-order rounds and 432
samples. Median script time: create-10k **1127.54 → 730.45 ms (-35.2%)**,
replace-1k **129.69 → 88.48 ms (-31.8%)**, clear-1k **40.56 → 31.06 ms (-23.4%)**.
Ten-thousand-row Owners fell from 130,016 to 90,013, attributes from 560,061 to
320,037 and comments from 30,010 to 4; semantic nodes and bindings stayed live.
Six viewport PNG checkpoints and all 10,000 row rectangles agree, with surviving
row/descendant/text/semantic identities preserved across updates and moves.

See the [detailed implementation, results and evidence](bulk-dom-optimization-2026-10-23.md).
The full native workspace passed 419 tests and the actual Windows/UIA probes.
Browser suites: 23/24 passed; the adaptive suite's 0.375 px anchor discrepancy
also reproduces identically on the unoptimized snapshot. Do not describe that
historical run as green or combine this paired baseline with the historical timings below.
The subsequent anchor correction and its separate complete regression are linked above.

## Semantic browser comparison before bulk optimization — 2026-10-23

The three local apps have been migrated to **semantic-rows-v1**. Glory uses
`web`, `launch_with`, public handwritten semantic builders and the complete keyed
`each`; there is no `BrowserHolder`, old `Each`, template cloning, manual row DOM
patching, virtual scrolling or compatibility feature alias. Dioxus is pinned to
0.7.9 and Leptos to 0.8.19. All three use the same Rust input generator, seed,
visible element/text tree, CSS and nine operations. Framework attributes and
comment markers remain part of each implementation's measured cost.

This is a **local full-DOM workload, not an official js-framework-benchmark
table score**: the finite semantic API currently maps its table/row/cell roles to
divs, and all three adapters now use that same structure. The old official runner
fails explicitly instead of silently submitting a different DOM contract.

All three passed the independent seeded-data oracle, nine operation checks,
keyed-node preservation on swap, replacement/removal checks, 10,000 actual row
boxes and repeated clear/recreate checks. The viewport screenshots at the same
create-1k checkpoint are byte-identical. Every measured sample's final row-data
SHA-256 and count agree across frameworks. Glory returns to rows=0, labels=0,
10 semantic nodes, 3 live reactive nodes and 16 Owners after clear.

One round used Chrome **155.0.8059.84**, 1280×900 at scale 1, fresh contexts and
order Leptos→Glory→Dioxus. Each operation had 3 untimed warmups and 12 samples
(324 measured samples total). The browser process/profile was independent of
other tasks; temporary profiles and all build/test output were on D:.
Windows Rust 1.98.0/MSVC, three independent Cargo targets, opt-level=3, LTO and
one codegen unit; all use wasm-bindgen/CLI 0.2.129, with **no wasm-opt**. The
accepted frozen library/app sources are in `source-keyed/inputs.json`; later
working-tree edits cannot enter these binaries. All three WASM/all-targets
strict Clippy checks passed, as did formatting and runner syntax checks.
After the Dioxus key correction, Glory and Dioxus rebuilt. Leptos reused only
its own isolated target with byte-identical app/manifest/lock/shared-data inputs;
the compiler-artifact freshness flags and exact WASM hashes are preserved.

Median **click to two foreground RAF callbacks**, milliseconds, with setup and
correctness snapshots excluded. This includes waiting for frame callbacks; it
does not establish presentation latency or FPS:

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 133.65 | 43.35 | 49.80 |
| replace_1k | 173.80 | 49.75 | 47.30 |
| update_10th | 14.40 | 14.60 | 22.60 |
| select_row | 11.00 | 250.60 | 9.45 |
| swap_rows | 12.30 | 10.80 | 11.15 |
| remove_row | 14.65 | 12.80 | 15.95 |
| append_1k | 169.55 | 66.65 | 67.70 |
| create_10k | 2809.90 | 509.30 | 554.90 |
| clear_1k | 83.05 | 20.70 | 10.50 |

Median CDP **ScriptDuration counter delta**, milliseconds. The common harness
and binding bridge have some cost; these are diagnostics, not the official
tracing runner's isolated script metric:

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 103.50 | 16.17 | 7.25 |
| replace_1k | 143.06 | 24.29 | 10.53 |
| update_10th | 0.72 | 0.65 | 1.67 |
| select_row | 1.59 | 248.11 | 1.14 |
| swap_rows | 0.68 | 0.26 | 1.50 |
| remove_row | 0.81 | 0.42 | 1.34 |
| append_1k | 129.07 | 25.28 | 11.98 |
| create_10k | 2156.60 | 194.23 | 95.41 |
| clear_1k | 79.79 | 18.15 | 4.87 |

The main gap is bulk work. Glory create-10k is 2809.90 ms versus Leptos 509.30
and Dioxus 554.90 ms; its script counter is 2156.60 ms versus 194.23/95.41 ms.
Create/replace/append/clear also trail. Fine updates, swap and removal are much
closer in absolute time; frame-waiting limits the resolution of their wall time.
No overall score or statistical-significance claim is made from this one ordered
round on a machine running other work.

**Selection is history-sensitive.** The mixed sequence's Leptos select result
(250.60 ms wall / 248.11 ms script) is anomalous, not a general Leptos selection
baseline. A separate fresh-page probe with alternating selection on a fixed
1k list found the following script-counter medians; it is not pooled into the
main table and uses its own counter boundary:

| App | After 1 create | After 30 replacements |
+|---|---:|---:|
| glory | 2.259 | 2.165 |
| leptos | 2.204 | 11.110 |
| dioxus | 1.120 | 1.260 |

This confirms history sensitivity in this test but does not identify a leak or
its root cause. Do not use the mixed selection outlier to rank the frameworks.

A separate 10k census (outside timing) shows equal visible element/text counts,
but meaningful implementation overhead:

| App | Elements | Text nodes | Comments | Attributes |
+|---|---:|---:|---:|---:|
| glory | 80008 | 30006 | 30010 | 560061 |
| leptos | 80008 | 30006 | 1 | 120009 |
| dioxus | 80008 | 30006 | 0 | 140015 |

Glory's 10k state additionally reports 80,010 semantic nodes, 60,003 live graph
nodes and 130,016 Owners. These are live-count observations, not RSS or proof
of the timing root cause. They motivate profiling per-node construction,
Owner/Binding allocation, mapper work and DOM metadata/markers before changing
the contract. The earlier million/100-million-row virtual-list work mounted
only a bounded window and cannot establish full-`each` throughput.

Generated WASM bytes after bindgen (not compressed, and not whole download size):
Glory 817,189; Leptos 278,497; Dioxus 457,457. No cross-framework final bundle
size ranking is inferred without accounting for JS and Dioxus's snippets.

Initial conformance rejected Dioxus's expression-wrapped row loop because swap
replaced DOM nodes. The accepted version puts the keyed div directly in the
RSX loop. Earlier packaging assumed an underscore binary filename; the build
script now takes the WASM path from Cargo's compiler-artifact record. These
diagnostics and the failed pre-correction run are retained, never mixed into
the accepted timings. The shared MCP browser was occupied, so this run used a
separate Playwright-launched Chrome with D:-backed temporary profiles.

Raw records and the preserved measured driver sources:
`D:/BuildCache/glory-framework-compare/browser-keyed-round/evidence.json`.
Build/source/probe/cleanup hash index:
`D:/BuildCache/glory-framework-compare/comparison-evidence-2026-10-23.json`.
The new runner is `benchmarks/compare.ps1`; see the benchmark README for setup.
No production framework source was changed by this benchmark migration.

## Historical measurements (previous architecture)

The former command-backend measurements, runner instructions and data are
preserved in [pinned Git history](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/docs/performance.md#historical-measurements-previous-architecture).
Their benchmark programs and wrappers have been removed. Use the current
semantic benchmark commands and evidence above.
