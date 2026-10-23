# 静态计划之后：节点存储与 DOM 元数据优化

本轮保留手写 Rust 静态计划、全部语义节点和两份独立 Memory，优化节点存储与 DOM 登记。没有改变三框架 benchmark 的应用、布局、九项操作或测量容差，没有引入 DSL、模板 VM、vendor 或第三方依赖修改。

## 定位结论

静态计划已经把每行的物理创建合并为一次 `cloneNode(true)`，但 Glory 仍需为每个语义节点建立完整可写的模型、维护运行时身份，并生成 DOM 元数据。当前 benchmark 每行 8 个语义节点，千行为 8,010 个活节点，万行为 80,010 个；千行对应 6,003 个响应图节点和 2,013 个 Owner。数据节点没有被隐藏、虚拟化或合并。

这里存在三个高频的完整 NodeId 哈希表：App 的 staged Memory、HTML Mapper 的 committed Memory、浏览器的 Entry 表。原 DOM 还为每个身份保存单独的 token 字符串索引和 parent 哈希表。静态计划可以批量克隆物理树，却不能自动消除这些逐节点登记和清理。

### 两份 Memory 为什么仍然存在

UI 的 `Present.tree` 在宿主提交之前验证排队中的新状态；HTML Mapper 的树独立验证适配器直接输入，并记录已接受操作的前缀。查询水位和宿主拒绝后的状态依赖这个区别。把二者简单替换成同一个可变树，会使适配器读到尚未提交的未来状态，或者失去直接 apply 的独立校验。

本轮没有整树克隆，也没有将整份 Descriptor 改成共享写时克隆。`CreatePlan` 仍在两份 Memory 中分别实例化可写 Descriptor；`prepare_patch` 的字段级校验、回滚和提交边界不变。改进集中在两份树都能使用的存储层。

### CPU 证据

单独进行无 DOM 包装的 CPU 采样，每个版本、操作各 3 次；这些采样不混入主计时。完整基线为 before / Leptos / Dioxus 共 36 份，Memory-only / after 共 24 份。首次目录扩容单独保留在 repeat 0；不把首次大 Node 哈希表 rehash 的冷启动成本，全部算成热身后 benchmark 的收益。

| 万行创建的采样类别，ms | before | Memory-only | after |
|---|---:|---:|---:|
| 哈希函数 self samples | 68.08 | 38.89 | 17.66 |
| staged Memory apply inclusive | 39.80 | 31.08 | 25.72 |
| committed Memory apply inclusive | 46.12 | 37.85 | 30.11 |
| setAttribute self samples | 39.40 | 41.75 | 39.01 |
| cloneNode self samples | 26.08 | 27.83 | 28.37 |

数值为 3 次采样的类别中位数；inclusive 类别会重叠，不能相加当成精确的 CPU 账单。内联也会改变归属，不能把未出现的函数解释为零成本。分配器采样并没有一致下降，本轮不宣称总分配量或 RSS 已降低。

销毁时，Owner 清理与 `removeChild` 仍在：after 的千行清理采样分别约 5.98 / 4.79 ms。逐节点身份分配器本身没有成为主要采样热点；更有证据的成本是身份被反复用作多个哈希表的 key，以及随之产生的元数据生命周期。

## 实现

### 分页活节点索引

`glory-render::NodeMap<T>` 以 slot 定位 32 项一页的固定 payload。增长时仅扩展页目录，不搬移已有的大 Node。常见 slot 通过直接目录访问；目录最多 4,096 页，超出范围使用稀疏页表，外部 `u32::MAX` 身份不会导致按最大 slot 分配数组。

每次读、写、移除都比较完整的 runtime / slot / generation。Memory 原有 generation 水位表仍保留，创建与复用规则没有交给裸槽位索引代替。不同活身份不能覆盖同一槽；过期身份不能读取或删除新一代节点。容量跟随数值身份的分配峰值，空槽不持有 Descriptor、业务行键、事件捕获或访问历史。页容量不是 RSS。

代价是每个实际使用的稀疏页仍分配 32 槽，极度分散的外部 slot 可能比原 HashMap 使用更多空间。空页在该索引生命周期内保留容量，整份 Dom/Memory 释放时归还；当前没有声称稀疏工作负载或空闲 RSS 最优。测试明确覆盖最高 slot、回收后的 payload 释放和目录扩容时地址稳定。

App / Mapper 的 Memory 和 DOM Entry 统一使用这份索引。原有 Children 顺序、Move、Suspend/Resume、成功前缀、事件与查询检查不变。

### DOM 元数据归并

父身份改为 Entry 内的 `Cell<Option<NodeId>>`，删除独立 parent 表。正常 Place 在物理移动成功后更新它；hydration 从已验证语义树恢复；离线计划按内部父索引一次初始化。销毁 Entry 时父信息随之释放。

删除 token → NodeId 的第二份哈希表以及 Entry 中重复保存的 token String。事件解析只接受规范的十进制三段文本，再从 live slot 核对完整身份。保留 runtime 的全部 64 位和 slot / generation 的全部 32 位，没有经过 JavaScript Number。前导零、正负号、空段、额外段、溢出、外来 runtime 与退休身份都不会进入业务回调。

静态计划将身份直接格式化到一份 NUL 分隔缓冲区，代替 `Vec<String>`、join 及多份 token 克隆；离线节点构造成功后，在一次 Entry 表借用中登记实例。实际 DOM 上的 `data-gly-node` 属性仍然完整写入；没有通过删属性、删注释或缩减查询目标来获得收益。

千行创建的最终 DOM census 进一步区分了物理内容和框架元数据：

| 框架 | Element | Text | Comment | 属性总数 |
|---|---:|---:|---:|---:|
| Glory before / after | 8,008 | 3,006 | 4 | 32,037 |
| Leptos | 8,008 | 3,006 | 1 | 12,009 |
| Dioxus | 8,008 | 3,006 | 0 | 14,015 |

Glory 仍保留 `data-gly-node`、`data-gly-kind`、交互状态及语义 role。这个计数是最终属性数量，不是每次新建的 setAttribute 调用次数：静态属性已由原型克隆复制。本轮优化 Rust 侧重复登记，不会消除身份属性写入本身，采样中 setAttribute 的时间也没有明显改善。

### 与 Dioxus 的差别

本次锁定 Dioxus 0.7.9。`dioxus-web/src/mutations.rs::load_template` 保存静态原型；`dioxus-interpreter-js/src/unified_bindings.rs::load_template` 深克隆原型并登记 JS nodes 数组，`assign_id` 通过静态路径定位动态节点。`dioxus-core/src/diff/node.rs` 的挂载表分别按模板根、动态 node_paths 和 attr_paths 分配，静态布局后代不需要像 Glory 一样都建立完整语义 Descriptor。

因此，Dioxus 的快路径不仅来自深克隆；它还减少了静态后代的可变运行时记录，并使用数值索引。本轮借鉴后者，保留 Glory 每个语义节点独立查询、事件、可变属性和跨适配器预检的能力。不能把 Dioxus 的挂载/动态部件身份和 Glory 的全部语义身份视为完全相同的工作量，也不能从这个差别直接推断必须引入模板解释器。

此外，当前 Glory 每行创建 seed/label、selected 派生与 Binding，并维护 row-owned label 注册和 cleanup；Dioxus 示例主要更新列表与 selected 两个 Signal，再由列表 diff 处理后代。Leptos 示例则为各行 label 使用 ArcRwSignal，并在 For 内建立选中状态和文本订阅。三者输出与操作契约一致，但响应模型并不相同。要解释剩余的全部差距，还需分别剥离模型构造、响应图、Descriptor 初始化和宿主桥接；本轮没有把总差距全部归给双份 Memory。

## 独立构建与计时

证据根：`D:/BuildCache/glory-node-storage-20261024`。before 冻结 260 项输入；Memory-only 仅修改 render 的 4 个文件（含新增 NodeMap 与测试）；after 再叠加 web 的 6 个文件。全部 benchmark 输入逐字节相同。同期其他功能修改没有叠加进其中任一对比版本。

五个应用各自使用全新 D: target，严格 WASM Clippy 后独立 release 构建：before Glory、Memory-only Glory、after Glory、Leptos 0.8.19、Dioxus 0.7.9。release opt-level=3 / LTO / 单 codegen unit，wasm-bindgen 0.2.129，无 wasm-opt；Chrome 155.0.8059.84，1280×900，DPR 1，同一 P 核亲和性。Cargo metadata 核对实际 target 在 D:。

首批 formal 正反两轮，每操作 3 次热身、12 个样本，共 1,080 个。发现 after 千行创建、Memory-only 千行清理有明显轮次波动后，预先决定再完整跑五版、相反起始顺序的 confirm 两轮，保留所有样本，不挑选有利轮次。CPU 采样、截图与诊断在正式计时之外运行；本任务的编译和回归在计时结束后启动。

四轮合计 **2,160 个正式样本**，每版本每操作 48 个；下表为所有样本的 ScriptDuration 中位数，单位 ms，越低越好。

| 操作 | Glory before | 仅 Memory | Glory after | Leptos | Dioxus | before→after 降幅 |
|---|---:|---:|---:|---:|---:|---:|
| 创建千行 | 27.87 | 25.24 | 23.90 | 14.79 | 4.48 | +14.2% |
| 替换千行 | 46.21 | 41.17 | 36.87 | 21.63 | 7.88 | +20.2% |
| 更新每十行 | 0.65 | 0.57 | 0.58 | 0.36 | 0.90 | +11.1% |
| 选行 | 0.77 | 0.83 | 0.89 | 171.47 | 0.74 | -15.9% |
| 交换两行 | 0.59 | 0.59 | 0.61 | 0.17 | 0.88 | -3.6% |
| 删除一行 | 0.61 | 0.62 | 0.62 | 0.27 | 0.84 | -2.6% |
| 追加千行 | 30.85 | 27.05 | 24.02 | 16.49 | 5.64 | +22.1% |
| 创建万行 | 343.40 | 291.90 | 256.21 | 183.78 | 49.81 | +25.4% |
| 清空千行 | 24.57 | 22.73 | 16.86 | 16.71 | 2.70 | +31.4% |

对应的总耗时中位数，ms：

| 操作 | Glory before | Glory after | Leptos | Dioxus |
|---|---:|---:|---:|---:|
| 创建千行 | 53.85 | 52.50 | 40.40 | 30.00 |
| 替换千行 | 74.15 | 66.20 | 45.60 | 32.80 |
| 创建万行 | 602.65 | 532.50 | 466.80 | 306.50 |
| 清空千行 | 27.20 | 20.45 | 17.75 | 9.85 |

千行创建的 Script 从 27.87 降至 23.90 ms，但总耗时只从 53.85 降至 52.50 ms，不能描述为端到端大幅提升。千行替换、万行创建和清空的 Script 分别降低 20.2%、25.4%、31.4%。小操作没有全面获益：选行 Script 增加约 15.9%（0.77→0.89 ms），交换与删除也略慢；这些结果完整保留。

相对 Leptos，after 的创建千行 / 替换 / 创建万行 Script 仍约为 **1.62 / 1.70 / 1.39 倍**，清空约为 1.01 倍。相对 Dioxus，上述四项仍约为 **5.34 / 4.68 / 5.14 / 6.25 倍**。这仍是明显差距，本轮没有追平批量构造。

轮次波动与补跑：

| 批次 | before 创建千行 | after 创建千行 | before 清空 | 仅 Memory 清空 | after 清空 |
|---|---:|---:|---:|---:|---:|
| formal | 28.48 | 26.47 | 24.45 | 30.88 | 16.94 |
| confirm | 27.14 | 23.34 | 24.71 | 21.96 | 16.72 |

formal 的 after 创建千行两轮 Script 中位数为 36.10 / 22.98 ms，Memory-only 清空为 44.74 / 22.74 ms。confirm 没有复现这两个量级的慢轮次；本次没有精确定位其外部扰动或 GC 成因，因此既不归因给某个具体因素，也不删除样本。合并表和两个完整批次均可复核。

主表保留 Leptos 的混合工作负载选行结果。另用新页面、分别经过 1 / 30 次创建替换后，各测 12 个独立选行样本，共 72 个，不混入主表：

| 框架 | 1 次创建后的选行 Script | 30 次后的选行 Script |
|---|---:|---:|
| glory | 0.97 | 0.93 |
| leptos | 1.25 | 6.43 |
| dioxus | 0.71 | 0.75 |

这说明主表选行有明显的运行历史效应；不能用这一格宣称 Leptos 普遍慢一两百倍。本轮没有为任一框架改变选行实现。

未压缩 wasm 字节数：before 1,005,668；memory 1,007,422；after 1,007,436；leptos 278,497；dioxus 457,457。体积不是运行时内存；本轮没有测量总 RSS。

click → 两次前台 RAF 的总耗时不是显示完成或精确 paint latency；ScriptDuration 来自 CDP，不能把总耗时减去 Script 当成绘制耗时。测试机器仍有其他应用，本表是同机局部证据，不是官方 HTML-table 排名或统计显著性结论。

## 验证与边界

完整回归使用 `validation/source`，其全部 crates 输入与实测 after 逐字节相同，另记录脚本、文档与浏览器夹具，共 372 项哈希。性能对照和验收都针对这一份冻结实现；工作区同期的其他功能改动不算成本轮优化收益。

已完成：

- 五版独立深度检查：数据哈希、保留的 Element / Text 对象、Glory 完整语义身份、万行所有行的几何摘要、旧子树断开及退休后的状态回收一致。两批主计时共 60 张 PNG 在对应检查点完全一致。
- DOM census：Glory 千行创建/替换的 8,008 个 Element、3,006 个 Text、4 个 Comment、32,037 个属性及 8,008 个 `data-gly-node` 均保持不变；万行为 80,008 / 30,006 / 4 / 320,037 / 80,008。清空后为 10 个语义节点、3 个响应图节点、13 个 Owner，三版 Glory 一致。
- Windows / Linux 各 **470 项**工作区测试，包括新索引的随机 HashMap 差分、payload 释放、地址稳定、稀疏最高 slot、完整位宽与退休身份测试；两平台各重新验证固定 engine forks 的 **289 项**测试。
- 两平台 workspace fmt、严格 all-features / all-targets Clippy；WASM workspace 和夹具严格 Clippy；Linux native no-default / paint 特征检查；两平台 CPU raster。
- Windows 真实窗口、AccessKit 窗口和 UIA 客户端验收；Linux X11 / Wayland 各运行 shell 与 AccessKit 真实窗口验收。两平台夹具各 6 项 Rust 测试通过。
- 浏览器夹具使用新的独立 `validation/web-build/target` 构建，最终 **27 个**套件全部通过，覆盖计划、事件、焦点、输入/选区、hydration、资源、CSS、布局、滚动与流式交接。新增 identity 套件实际发送 10 类非法 token 和退休 runtime 的 InputEvent，业务状态不变，恢复规范身份后正常编辑。
- 64 / 1024 测量缓存各运行 **70 次真实小数行高变化**，各 **73 张**截图；测量数量不超过预算，锚点增长前后均为 **−15.125 px**，存续节点身份、prepend 后身份、最终物理/逻辑滚动位置与视口像素一致。保留原行高和原断言，没有放宽容差。
- 实际 HTTP stream 检查通过 Shell / Patch / Patch / End、2 项资源交接、取消及慢消费者背压；已停止本任务的 28955 / 28956 / 28957 服务。

另对含同期表单、文档与 toggle 修改的工作树冻结了 **271 项**输入，作为独立的 `integration/source`，Windows **493 项** workspace 测试和 native / WASM 严格 all-targets Clippy 通过。这批新增功能没有参与性能表，也没有替换上面的 470/470 + 27 套件候选回归。

合并检查中只整理了两个 Clippy 问题：HTML helpers 移到测试模块前，hydration 中等价的嵌套条件改为 let-chain。初次直接检查工作树时，其他修改在编译过程中增加 pressed 字段，造成短暂的跨 crate 类型不一致，随后改用冻结快照。快照还包含较早的 Toggle/AX 测试选择器：按钮文字导出为独立 Label 子节点，不能按按钮原始 label 查找；同步当前工作树已经采用的 Button role 选择器后，全量通过。该同步只改测试，并以 `integration/test-selector.patch` 和原输入清单留档，生产源码不变。初次失败日志保留，最终结论使用带 final 的测试日志及冻结快照的 Clippy 日志。

这些验收不扩大 Android、OS IME、屏幕阅读器或其他尚未完成宿主的支持承诺。

原始源码、5 份独立 target、metadata、WASM/JS、2,160 个样本、72 个选行诊断样本、60 份完整 CPU 采样、浏览器截图与日志均位于证据根。`verified-evidence.json` 是交叉校验汇总，`verify-evidence.py` 校验源码差异、构建 fresh=false、数据/几何/图像哈希和回归结果；`implementation.patch` 只包含实测 before→after 的 10 个文件。早期因 Leptos 尚未构建完成而中断的 `profile-before` 留档但不参与任何性能表或采样结论，实际使用完整的 `profile-baseline` / `profile-after`。

重建时选择证据中相应的冻结源码，并指定一个尚未使用的 D: 构建目录，例如：

```powershell
./benchmarks/build.ps1 -Apps glory -SourceRoot D:/BuildCache/glory-node-storage-20261024/after/source -BuildRoot D:/BuildCache/glory-node-storage-replay -Bindgen D:/BuildCache/glory-framework-compare/tools/bin/wasm-bindgen.exe
```

计时使用证据内的 `driver/compare.cjs`，formal 参数为 `--variants before,memory,after,leptos,dioxus --rounds 2 --no-probe`，confirm 为相反起始顺序；两次均通过 `--base` 指向只读产物服务、`--out` 指定新的 D: 目录。不能让编译、CPU 采样或另一轮浏览器计时与主计时同时运行。`summarize.py` 保留两个 cohort，再汇总全部 48 个样本/格。

## 后续优先级

1. 先分离静态语义数据与实例可变字段，研究只共享静态声明、保持字段级 prepared patch 的实现。两份树仍保留独立可观察状态；需要有同样的直接 apply、失败前缀和查询水位测试，不能直接共享一个可变 Memory。
2. 继续采样 generation 水位、临时 subtree 集合及 Owner/Binding 清理。数值水位可以研究轻量分页账本，但必须保留退休水位、稀疏外来 slot 和永不回绕规则。当前数据还不能保证这些改动一定加速。
3. 研究批量 DOM 登记与释放中的 Rust/JS 引用管理。即使更少 wasm 边界能加速，也不应删除完整身份、控件状态和事件校验。原型深克隆已经存在，重复实现它不会消除剩余 Descriptor、响应图或元数据成本。

本轮没有实现这三项更大改造，也没有以牺牲节点身份或最终画面作为优化条件。
