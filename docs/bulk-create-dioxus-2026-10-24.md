# 批量创建、替换与 Dioxus 设计借鉴 — 2026-10-24

> 2026-10-25 复核：下文保留当时的浅克隆实现、旧版本和历史测量；当前 Glory 已使用整行深克隆、共享声明和更少的 Owner。最新差距见[稳定版重跑](framework-benchmark-latest-stable-2026-10-25.md)，当前 Dioxus 0.7.10 的调用计数、实例粒度和清空后引用观察见[深入分析](dioxus-performance-analysis-2026-10-25.md)。不要把本文的旧热点直接当作当前待办。

本轮保留一个小范围的 Web 提交优化：把浅元素克隆、class/style 和身份/live 标记放进一次同步 JS 调用，属性名使用 JS 字面量。
独立构建、相同九项 benchmark 的两轮结果：千行创建 Script **57.33→55.61 ms（-3.0%）**，千行替换 **83.03→79.87 ms（-3.8%）**，万行创建 **702.08→654.75 ms（-6.7%）**。
这是有限收益，尚未解决与 Dioxus 的主要差距。两项实验没有进入最终实现：动态属性字符串打包、Memory Node 间接存储。

## 1. 本轮实现及边界

- [create.rs](../crates/web/src/browser/create.rs) 是固定参数的本地桥接，仅处理 class、style、data-gly-node 和可选 data-gly-live。缺失和空字符串分别传递，不序列化对象、不拼 HTML、不使用 eval 或模板解释器、不延后提交。
- [templates.rs](../crates/web/src/browser/templates.rs) 继续使用文档拥有、最多 64 个的有限浅原型。满足字段条件时走桥接；其他属性、未知原型或容量耗尽时保留通用有序路径。原型不保存 class/style、label、URL、身份或业务数据。
- [dom.rs](../crates/web/src/browser/dom.rs) 仍为每个语义节点分配全新 DOM 节点并注册完整身份。原生 value/checked、composition、输入版本和属性屏障保持原路径。DOM 异常同步返回，注册和成功水位仍在原来的操作边界。
- wasm-bindgen 会为桥接生成 `snippets/glory-web-<hash>/inlineN.js`。[流式测试服务器](../examples/semantic-web/src/bin/stream_fixture.rs) 已补齐这些模块的服务，只接受固定 namespace/hash/数字文件名，未开放任意路径。部署需要服务完整的 wasm-bindgen 输出目录；现有两个构建脚本本来就将其保存在输出目录。
- [创建验收](../examples/semantic-web/create-acceptance.js) 增加真实浏览器对照：Unicode/NUL、缺失与空属性、属性顺序、完整 u64 身份字符串、host 异常和原型隔离。没有修改业务 benchmark、数据生成器、计时方法或 CSS。

默认依赖图、Rust builder/Cage/Bond/Lotus、SSR/stream v3 格式均未改变，没有新增依赖、第三方修改或 vendor。

## 2. 同一 benchmark 的实际前后结果

以下采用所有样本的中位数，没有剔除不利结果。总耗时为点击到两次前台 RAF；Script 为相同区间的 CDP ScriptDuration 增量，包含公共 harness/绑定桥开销。
短操作总耗时包含 RAF 等待，微小差异不能当作稳定排名，也不代表实际显示完成或 FPS。

| 操作 | 总耗时前→后 ms | Script 前→后 ms | Script 降幅 |
|---|---:|---:|---:|
| 创建千行 | 85.40→82.60 | 57.33→55.61 | 3.0% |
| 替换千行 | 111.45→105.60 | 83.03→79.87 | 3.8% |
| 更新每十行 | 10.75→12.05 | 0.65→0.67 | -3.1% |
| 选行（混合流程） | 8.15→8.40 | 1.06→1.06 | 0.2% |
| 交换两行 | 11.30→9.10 | 0.58→0.59 | -1.2% |
| 删除一行 | 11.55→9.75 | 0.62→0.67 | -8.2% |
| 追加千行 | 98.35→94.30 | 65.08→61.66 | 5.3% |
| 创建万行 | 970.95→909.45 | 702.08→654.75 | 6.7% |
| 清空千行 | 34.85→34.25 | 30.81→30.98 | -0.6% |

创建/替换在两轮均改善。关键 Script 中位数：千行创建 **57.07→56.69 / 57.76→55.13**，千行替换 **84.91→80.16 / 82.78→79.44**，万行创建 **695.28→661.16 / 704.07→651.48 ms**。
清空 Script **30.81→30.98 ms**，本轮没有销毁提速的证据；小更新的约 20 微秒差值也不推导稳定退化。

当前三个框架的总耗时，ms：

| 操作 | Glory 优化后 | Leptos 0.8.19 | Dioxus 0.7.9 |
|---|---:|---:|---:|
| 创建千行 | 82.60 | 39.55 | 30.15 |
| 替换千行 | 105.60 | 46.75 | 33.00 |
| 更新每十行 | 12.05 | 11.90 | 11.80 |
| 选行（混合流程） | 8.40 | 176.30 | 8.35 |
| 交换两行 | 9.10 | 9.85 | 9.15 |
| 删除一行 | 9.75 | 10.80 | 10.10 |
| 追加千行 | 94.30 | 48.55 | 35.80 |
| 创建万行 | 909.45 | 465.10 | 313.70 |
| 清空千行 | 34.25 | 19.80 | 9.00 |

相同区间 ScriptDuration，ms：

| 操作 | Glory 优化后 | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 55.61 | 14.53 | 4.46 |
| 替换千行 | 79.87 | 22.04 | 8.00 |
| 更新每十行 | 0.67 | 0.35 | 0.88 |
| 选行（混合流程） | 1.06 | 174.79 | 0.75 |
| 交换两行 | 0.59 | 0.17 | 0.90 |
| 删除一行 | 0.67 | 0.28 | 0.82 |
| 追加千行 | 61.66 | 17.40 | 5.76 |
| 创建万行 | 654.75 | 201.62 | 52.36 |
| 清空千行 | 30.98 | 17.81 | 2.92 |

Glory 万行总耗时仍为 Leptos 的约 **1.96 倍**、Dioxus 的 **2.90 倍**；Script 则约为 Dioxus 的 **12.50 倍**。
这说明只继续压缩字符串转换，难以缩小主要差距。总耗时与 Script 之间还包括布局、调度及等待，不能简单相减后全称为绘制时间。

Leptos 混合流程选行保留实际异常值。额外新上下文、每格 3 热身/12 样本的独立诊断（72 样本，不进入主表）：首次千行列表 **11.75 ms / Script 1.23 ms**，连续 30 次建表（首次创建加 29 次替换）后 **11.45 / 6.27 ms**。
主表的 176.30 ms 不能泛化为通常选行成本；运行历史原因仍未进一步定位。Glory 对应 **11.80/1.31、11.70/1.28**，Dioxus **11.90/0.70、11.90/0.74**。

## 3. 剩余开销：调用计数与采样证据

计数探针与主计时分开。千行替换的调用如下，包含本次完整包装的 append/before/after/replaceWith/remove 和 textContent setter：

| 调用 | Glory 前 | Glory 后 | Leptos | Dioxus |
|---|---:|---:|---:|---:|
| UTF-8 decode | 34,006 | 20,006 | 23,000 | 2 |
| 被解码 UTF-8 bytes | 318,998 | 184,998 | 158,994 | 31,008 |
| 浅/深 cloneNode | 8,000 | 8,000 | 1,000 | 1,000 |
| createElement | 0 | 0 | 7,000 | 0 |
| createTextNode | 3,000 | 3,000 | 3,000 | 2,000 |
| setAttribute | 15,000 | 15,000 | 10,000 | 3,000 |
| insertBefore | 8,000 | 8,000 | 11,000 | 0 |
| replaceWith | 0 | 0 | 0 | 2,001 |
| removeChild / remove | 1,000 / 0 | 1,000 / 0 | 0 / 1,000 | 0 / 999 |

Dioxus 的 2,001 次 replaceWith 包括 2,000 个动态文本占位替换和一次批量插入新行；不是没有执行插入。
深克隆仍会创建全部后代，不能将 1,000 次 cloneNode 解释为只创建 1,000 个元素。
Glory 的解码次数减少约 **41%**，属性数没有减少，这也是收益有限的一个直接解释。

另做 80 个不包装 DOM 方法的替换 CPU profile，每个版本两轮、每轮 8 个样本，均排除在主计时之外。Glory 包含式采样中位数，ms：

| 函数族 | 前 | 后 |
|---|---:|---:|
| DOM Sink | 33.81 | 33.56 |
| UTF-8 decode | 9.02 | 7.58 |
| UI Element build | 14.85 | 14.51 |
| render Memory apply | 12.23 | 13.48 |
| HTML shape | 3.75 | 3.42 |
| Owner dispose | 12.15 | 11.88 |

这些区间有包含关系，不能相加；内联和采样误差也会影响归属。profile 中的 Script 中位数为 **90.20→87.94 ms**，高于主表，不能混用。
证据支持解码减少和整体的小幅收益，**不支持声称 DOM Sink、Memory 或 Owner 单独出现显著提速**。暖替换 profile 未采到 reserve_rehash，不表示冷创建没有扩容成本。

主要剩余工作分为三部分：

1. **细粒度运行时构造和退休。** 万行 Glory 挂载后仍有 80,010 个语义节点、60,003 个图节点、90,013 个 Owners。每行的 label Cage、选中态派生、Binding、组件/子节点 Scope 都有真实成本；替换还需退休旧树。Dioxus 的本 workload 是一个 App Signal<Vec<Row>> 加按 key 的 VNode，未为每个静态 DOM 元素建立同等 Owner。不能把这种差异简单称为“Rust 比 JS 慢”。
2. **两个语义树与逐节点 lowering。** UI 的 staged Memory 和 HTML Mapper 的独立 Memory 分别承担应用状态、直接 adapter 调用的校验；每个节点仍需校验、描述符构造/复制、shape 和索引维护。Dioxus 的 Web interpreter 接收的是较低层的已编码 DOM 指令。这两个接口承担的契约不同；不能删除 Glory 的直接 apply 校验来换分数。
3. **宿主创建粒度。** Glory 逐个语义节点浅克隆、写动态属性、保存 Part/Entry/token/parent，再插入；Dioxus 将整行静态结构作为一个深模板，只处理动态槽。最终三者仍是 80,008 Elements、30,006 Text，真实布局工作没有被虚拟化掉。万行最终属性数 Glory 320,037、Leptos 120,009、Dioxus 140,015；Glory 的额外语义元数据也有实际宿主成本。本轮保持它们，没有靠删属性换性能。

## 4. Dioxus 值得借鉴的具体设计

分析基于本次锁文件解析的 **0.7.9 发布源码**。本地关键文件的 SHA-256、发布 VCS 记录保存在 dioxus-analysis.json；发布记录带 dirty 标记，不能假设它与标签源码每个字节相同。下列官方源码用于核对设计和导航。

| 设计 | Dioxus 实际做法 | 对 Glory 的价值与限制 |
|---|---|---|
| 静态子树模板 | Web 首次建立静态 DOM 树并注册模板，后续深克隆；动态属性/文本通过路径与槽定位 | 优先借鉴结构复用。Glory 应由手写 Rust 构造有限静态计划，不引入 RSX、DSL 或模板 VM，也不能从首行抽取业务值当模板 |
| 编码提交与名字缓存 | Sledgehammer 将 mutation 写入缓冲，在 JS flush；属性名/事件名/namespace 使用独立缓存，业务值仍是本次更新的数据 | 借鉴有限词汇和有界提交。本轮固定桥接只是缩小版，尚不是跨节点批量解释器；后者必须解决成功前缀、查询和受控输入边界 |
| 稀疏动态句柄 | 静态根和需要更新/监听的节点注册 ElementId，JS 侧维护 nodes/stack/templates；静态后代不逐个拥有相同运行时对象 | 可以减少宿主桥接和索引成本，但 Glory 仍承诺任意语义节点身份、查询及生命周期，不能直接照搬省掉它们 |
| keyed 中间段 | 先匹配相同前后缀；中间段映射完整 key；无共享 key 时走整体创建/移除，否则 LIS 保留顺序 | 可用于审查 each 的重排与批量替换路径；不是当前创建差距的首要原因。Glory 现有交换 Script 约 0.59 ms，先做创建计划比再调 LIS 更有价值 |
| 构造/提交分离 | VNode/模板差分写入 WriteMutations，再由 interpreter 执行；嵌套销毁回收状态，物理删除交给根操作 | Glory 已有队列与父子退休合并，可继续借鉴分工；不要再回到重复物理删除后代，也不要让 DOM listener 同步执行业务逻辑 |

官方来源：[Web template/WriteMutations](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/web/src/mutations.rs)，[编码与深克隆操作](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/interpreter/src/unified_bindings.rs)，[JS 句柄与事件委托](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/interpreter/src/ts/core.ts)，[keyed diff](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/core/src/diff/iterator.rs)，[动态槽与退休](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/core/src/diff/node.rs)。

### 建议的改进顺序

**P1：手写 Rust 静态构造计划。** 先在 render/ui 中表达不可变的有限语义结构与显式动态槽，HTML adapter 再编译为文档内的 DOM 子树/Part 路径。静态文字/classes/style 只能来自明确的静态声明；label、URL、输入值、身份和 reactive 捕获属于实例。其他 renderer 可以展开成普通语义操作，不依赖 HTML 类型。缓存预算按计划数/节点数/字节控制，禁止保存访问过的行。

验收先覆盖完整新行构造、替换旧行断开，以及 Group、Input/Check、name/role 变化和 Suspend/Resume。每个实例仍取得新的 generational NodeId；受控值不能从已使用的 DOM 反向复制。第一目标是减少 8 次浅克隆和 8 次插入的宿主粒度，而非减少实际元素数或公开语义身份。

**P1：有界、可返回失败位置的 DOM 批量提交。** 在计划成功后再研究连续创建的编码桥；仅跨无查询/动作/输入屏障的区间合并。JS 需要返回已成功提交位置，Rust 只能推进相应 Receipt，并保持 raw Render::apply 的独立校验及成功前缀。不能先让 Mapper 接受整批，再在尾部发现 DOM 失败却保留错误水位；也不能用全树 clone 回滚。没有解决这些问题前，不把 Dioxus 的 flush 机制直接套进 Glory。

**P2：数字身份索引与构造分配。** 优先测量受 Runtime/generation 保护的紧凑或分页存储、有限 Part 索引、静态字段共享，以及已消费 builder 的空对象保留。保留完整 key 比较、代际耗尽、活节点清理和 graph borrow 外析构；不为用户数据改成不完整 key 或缓存历史。两个 Memory 的职责先保留，只共享真正不可变的静态载荷；不能用整 Descriptor 的共享写时全克隆，破坏现有 prepare_patch 仅触碰字段的收益。

**P3：Owner/Scope 的结构瘦身。** Dioxus 不为每个静态元素建立相同 Owner，值得研究。但 Glory 的 Scope 包含独立取消、cleanup、query requester 和 listener Instance 语义，应先让静态构造计划明确哪些节点没有独立生命周期，再减少对象。不可直接共享所有行的 Owner，也不更换 Cage/Bond/Lotus。必须用自定义组件析构、跨 Scope listener、owner-owned query/task 和重入测试作门槛。

上述是后续改造建议，未作为本轮已经实现的功能或性能承诺。优先级依据本轮成本证据，不根据理论上“更轻”就合入。

## 5. 未保留的实验

- **属性字符串打包**（candidate）：把一个元素的动态字段编码成带分隔符的临时 String，JS 一次解码后拆分。432 个早期对照样本中，万行 Script **696.29→786.50 ms**，尽管千行创建略快，仍放弃。该早期轮次有编译并行，仅作为候选探索，不替代最终控制对照。额外拼接、字符串切片和分配是合理怀疑，未用分配器剖析将损失精确归因。原始源码、构建和不利样本完整保留。
- **间接 Node 载荷**（boxed）：在轻量桥接之上，将 Memory 的 HashMap<NodeId, Node> 改为 Box<Node>。冷创建采样确实出现过大载荷 reserve_rehash，但最终九项测试中，间接存储的千行创建/替换/万行 Script **57.90/83.67/670.64 ms**，均不如轻量桥接的 **55.61/79.87/654.75 ms**。追加较快，不足以支持整体合入；增加逐节点分配/间接访问的方案没有进入工作树。

完整三版 Glory Script 对照：

| 操作 | 基线 | 保留的轻量桥接 | 未保留的间接存储 |
|---|---:|---:|---:|
| 创建千行 | 57.33 | 55.61 | 57.90 |
| 替换千行 | 83.03 | 79.87 | 83.67 |
| 更新每十行 | 0.65 | 0.67 | 0.71 |
| 选行（混合流程） | 1.06 | 1.06 | 1.08 |
| 交换两行 | 0.58 | 0.59 | 0.61 |
| 删除一行 | 0.62 | 0.67 | 0.65 |
| 追加千行 | 65.08 | 61.66 | 58.90 |
| 创建万行 | 702.08 | 654.75 | 670.64 |
| 清空千行 | 30.81 | 30.98 | 31.31 |

## 6. 构建、验证与可复现性

基线冻结于 **2026-10-23T15:56:01.422909+00:00**，251 个输入；最终库版本 typed 冻结于 **2026-10-23T16:04:55.253377+00:00**，252 个输入。
相对共同基线仅修改 dom.rs/templates.rs，新增 create.rs；boxed 另有一个 Memory 文件实验。Git 基点 `945c077b6a3f167c9e79cbc75f726b1d6316e34d`，包含当时工作树改动。15 个应用/共享输入在全部版本逐字节一致。

before/candidate/typed/boxed/Leptos/Dioxus 使用六个全新独立 **D:** target，metadata 验证目录，应用编译 artifact 均 fresh=false。release opt-level=3、LTO、一个 codegen unit；wasm-bindgen 库/CLI 0.2.129，无 wasm-opt。最终 WASM 原始大小 **930,502→932,662 bytes（+2,160）**，另有生成 JS 模块；未测 RSS，不声称内存字节下降。

Windows 11 / i7-14700KF / Rust 1.98.0 MSVC / Chrome 155.0.8059.84，1280×900、scale 1、en-US、light。所有主测上下文在计时外固定 P 核逻辑 CPU 0–15、AboveNormal 并读回验证。
顺序 **before→typed→boxed→Leptos→Dioxus**，第二轮反向。每格每轮 3 次热身、12 个正式样本，合计 **1,080 个主样本**、每格 24 个。正式计时前本任务的编译和完整回归已结束；机器仍非独占，宿主进程列表已记录。

本轮最终库版本的完整验收：

- 所有主样本行数/摘要一致；独立 seeded oracle 验证九项操作。补充深身份探针对五版检查存续 Elements/Text Nodes，Glory 同时检查 data-gly-node；替换旧后代全部断开，万行完整矩形摘要一致。五版各六个视口，共 **30 张 PNG** 同检查点逐字节一致。
- Windows/Linux 各 **452** 项工作区测试、各 **289** 项独立干净 fork 测试；fmt、严格原生/WASM all-target Clippy 通过。Linux no-default/paint 通过，双平台各两张 CPU raster PNG 成功。
- Windows shell/accesskit 真实窗口与双窗口实际 UIA；Linux X11/Wayland × shell/accesskit 四组实窗通过。没有把这些检查称为实际 OS IME、Unix AT-SPI 或读屏器验收。
- 浏览器 **25/25** 通过。64/1024 缓存各 70 次真实小数行高变化、73 张截图，保留原 0.1/0.15 容差；锚点均 **-15.125→-15.125 px**，节点身份、最终滚动/几何/画面一致。没有改成固定行高。
- 最终 SSR/stream fixture 双平台各 **5** 项测试；真实 Shell/Patch/End、两个资源交接、断开取消和慢消费者背压通过。生成 JS 模块修复后重跑全部 25 浏览器套件，未只补跑失败项。

失败与边界也已归档：早期 runner 的日志输出文件冲突及清理路径归一化错误、Linux wrapper 的 CRLF、流式服务器缺少生成模块、fixture 新测试的 Clippy 模块位置。修复后重跑相关完整检查，没有放宽产品断言。benchmark 对应 typed 冻结库，fixture 的两个测试/资源服务文件作为 validation overlay 单独记录；库文件与当前工作树哈希一致。

完整证据位于 [evidence-index.json](/D:/BuildCache/glory-bulk-bridge-20261024/evidence-index.json)。同目录 summarize.py 重算主表并验证独立构建、源码、样本和验收，verify.py 核对补充 profile/身份/计数/截图与全部归档哈希。driver、run-final.ps1/run-post.ps1、构建和双平台命令日志保留；重现必须使用新的 D: 输出目录。
