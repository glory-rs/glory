# Dioxus 为什么在当前批量 benchmark 中快（2026-10-25）

结论：Dioxus 0.7.10 的优势是**静态描述共享、只登记动态目标、组件级响应式、紧凑指令批量过桥和批量挂接**共同产生的。当前 Glory 已经实现整行深克隆和静态声明共享，不能再用“Dioxus 有模板，Glory 逐节点创建”解释剩余差距。

本次只做源码分析、独立计数、CPU 采样和清空后引用观察，没有修改框架或 benchmark 算法，也没有把探针下的耗时混入正式结果。对 Glory 的建议是待验证的实验，不是已取得的提速。

后续新增的[持仓看板持续更新场景](portfolio-scenarios-benchmark-2026-10-25.md)另外对比 Dioxus 父列表与行组件两种组织：局部更新的差距缩小，但行组件版仍更快。该报告包含新的独立构建和 1,152 个样本，不与本文的原批量数据混合。

## 1. 测量边界与现有差距

正式结果沿用[最新稳定版独立构建报告](framework-benchmark-latest-stable-2026-10-25.md)：三个全新 target、每格 24 样本，版本 Glory 0.3.1 / Leptos 0.8.22 / Dioxus 0.7.10。下表单位为 ScriptDuration 中位数 ms。

| 操作 | Glory | Dioxus | Glory / Dioxus |
|---|---:|---:|---:|
| 创建千行 | 19.391 | 4.388 | 4.42 |
| 替换千行 | 29.980 | 7.548 | 3.97 |
| 追加千行 | 21.035 | 5.358 | 3.93 |
| 创建万行 | 230.614 | 44.815 | 5.15 |
| 清空千行 | 14.975 | 2.688 | 5.57 |
| 更新每十行 | 0.501 | 0.871 | 0.58 |
| 交换行 | 0.590 | 0.910 | 0.65 |
| 删除行 | 0.600 | 0.791 | 0.76 |

万行 click→两个 RAF 的 wall 中位数是 509.100 / 289.650 ms，比值约 1.76。脚本五倍差距不能称为画面呈现五倍差距，也不能转换为 FPS。

两者最终都创建 80,008 个 Element、30,006 个 Text；全部数据、存续 Element/Text 对象身份、Glory 完整语义身份、10,000 个行矩形和六处截图一致。最终注释数 Glory 4 / Dioxus 0，属性数 320,037 / 140,015；不同的框架元数据并未被删除来凑性能。

本次诊断重用上述独立构建：当前 HEAD `88e0aaf` 的 **511 个输入文件与已测构建清单逐一哈希一致**。CPU 采样为各四项、各三次、100 µs 采样间隔；计数探针另开进程包装导入/DOM/解码；引用观察另开进程并显式 GC。三类诊断均不并入正式计时。

实际源码以 D 盘 Cargo registry 的 **0.7.10 发布包**为准。其 VCS 元数据指向 `57d6794ad60b949e5bd8aa282f6f8c3dc97a365e`，部分包标记 `dirty=true`；下文 GitHub 链接用于源码导航，不把提交标签当成发布包逐字节一致的证明。

## 2. 静态树只描述一次，实例主要记录动态槽

Dioxus 的 `Template` 保存静态 roots、动态节点路径和动态属性路径；`VNodeInner` 保存 key、Template 引用、动态节点及动态属性。相同模板的 diff 不重新比较整棵静态后代树，而是比较动态属性和动态节点。Template 比较有满足静态合并条件时的指针快路，也有按值比较回退，并非无条件只比较地址。

`VNodeMount` 长期保存 root_ids、mounted_attributes、mounted_dynamic_nodes。本 benchmark 一行有 8 个 Element 和 3 个 Text，但 JS 节点表新增约 **5 个目标/行**：行根、两个事件目标、两个动态文本。其余静态后代仍然是真实 DOM，只是不需要分别保存为可更新的 Rust 语义实例和跨边界 DOM 句柄。

Glory 的 Plan 已共享不可变描述，实例 Props 独立；Web 也已 `cloneNode(true)` 整行克隆。不过它还要：

- 在 UI staged Memory 和 HTML Mapper Memory 中分别建立 8 个节点的可变实例、身份和树关系，保留独立验证。
- 为 8 个语义节点注册完整 Runtime/slot/generation 身份、UI Owner 归属、HTML key 和 DOM Entry。
- 为这行的 11 个物理部件取得 Rust 可访问的 Node 句柄。

因此当前差别主要是**克隆之后要建立多少实例状态**，不只是克隆方式。不能直接删除 Glory 的静态节点身份：其独立查询、事件、移动、挂起、退休和适配器契约都可能使用这些身份。

源码：[Template 文档](https://docs.rs/dioxus-core/0.7.10/dioxus_core/struct.Template.html)、[nodes.rs](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/core/src/nodes.rs)、[diff/node.rs](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/core/src/diff/node.rs)。Glory 对照：[Memory::CreatePlan](../crates/render/src/memory.rs)、[Mapper](../crates/html/src/lib.rs)、[DOM Plan](../crates/web/src/browser/plans.rs)。

## 3. 最明显的实际差异：DOM 工作留在 JS，一次批量过桥

Dioxus Web 的执行路径是：

```text
组件生成动态值 → VDOM diff → WriteMutations 直接写紧凑缓冲
→ Interpreter.flush → RawInterpreter.run
→ JS 从缓冲读取操作，在本地 nodes[] / templates[] 上执行 DOM 工作
```

`WriteMutations` 是 Rust trait；Web 实现通过泛型接收差异，直接写 Sledgehammer 编码器，避免先构造通用 Mutation Vec 再逐条动态分发。指令、数字和字符串分区传递；属性名、事件名、namespace 有缓存，不是每节点 JSON 序列化。生成的 JS 用 DataView 读取数字，批量读取字符串，ASCII 还有专门路径。

两个很短的源码要点：`load_template` 在 JS 内执行 `cloneNode(true)` 并把结果存入 `this.nodes[id]`；`replace_with` 从 JS 栈取一批根，调用 `root.replaceWith(...els)`。这些节点不必创建一个就回传一个 Rust 句柄。

### 同一千行创建的独立计数

热身创建/清空后计数，不使用包装后的时间。

| 工作 | Glory | Dioxus |
|---|---:|---:|
| WASM→JS 导入函数调用总数 | **19,033** | **10** |
| 其中 Array.get | 11,000 | 0 |
| 其中 RawInterpreter.run | 0 | 1 |
| TextDecoder.decode 次数 | **5,011** | **2** |
| 这些 decode 的输入字节 | 106,035 | 30,949 |
| cloneNode 深克隆 | 1,000 | 1,000 |
| setAttribute | **9,000** | **3,000** |
| 其中框架身份属性 | data-gly-node 8,000 | data-dioxus-id 2,000 |
| class 属性写入 | 1,000 | 1,000 |
| 动态文本初始化 | nodeValue 2,000 | createTextNode 2,000 |
| 最终批量接入行根 | insertBefore 1,000 | 一次占位节点 replaceWith，1,000 参数 |

Dioxus 还发生 2,000 次动态文本占位替换；并不是所有 DOM 方法都合并成一次调用。总 `CharacterData.replaceWith` 为 2,001 次，其中仅最后一次挂入千行。它内部 `appendChildren` 的一般路径仍可能逐项 appendChild，不能把本案例的挂接方式泛化为所有列表操作。

万行创建，Glory 导入调用为 190,033 次，Dioxus 12 次；深克隆均为 10,000 次。Dioxus 的 JS 循环、字符串处理和原生 DOM 调用仍然是 O(n)，减少的是桥接次数、传输体积和实例维护，不是把创建复杂度变成 O(1)。TextDecoder 次数也不包含 ASCII 专用字符串路径，不能解读为只做两次字符串处理。

**19,033 / 10 只是调用数比，不能当作提速比或归因百分比。** 调用负载完全不同；Dioxus 的一个 run 内有大量工作。Glory 上一轮把 Array 返回改成 Vec<JsValue> 的实验反而变慢，因为它增加了反方向的 externref 分配和 ABI 复制：千行 +6.7%、万行 +8.7%。减少某一方向调用并不足以证明优化成功，见[已撤回实验](html-dom-bridge-optimization-2026-10-25.md#撤回实验)。

源码：[Web WriteMutations](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/web/src/mutations.rs)、[WebsysDom](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/web/src/dom.rs)、[Interpreter 绑定](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/interpreter/src/unified_bindings.rs)。实际生成的 JS 随独立构建保存在 `site/dioxus/snippets/`。

## 4. 响应式成本：当前应用用了不同的粒度

本 benchmark 的 Dioxus 应用只有一个 App 组件，rows / selected 两个 Signal；每行不是一个独立 Row 组件，也没有为 label 和 index 分别建立 Signal。组件读取 Signal，当前 ReactiveContext 订阅；写入使组件变脏，再生成动态 VNode 并做 diff。这不表示 Dioxus 没有 Owner、挂载记录或闭包：每行仍有 VNodeMount、key、动态数组和事件回调。

Glory 的当前实现则为每行建立：

| 每行对象 | 数量 | 来源 |
|---|---:|---|
| Source | 3 | Each 的 item、Each 的 index、业务 label |
| Derived | 1 | selected.map 计算 class |
| Binding | 2 | class 和 label |
| Owner | 2 | Each data Scope 和 widget Scope |
| 语义节点 | 8 | 显式 RowPlan |

万行实测 **graph=60,003、owners=20,013、nodes=80,010**。图的组成是 30,002 个 Source + 10,000 个 Derived + 20,000 个 Binding + 1 个 Structure。旧报告中的 90,013 个 Owner 已过期；不能据此评价当前实现。

这里有两个可以具体讨论的多余构造：本应用的 Each 工厂忽略 index，但通用 Rows::mount 仍无条件 `data.cage(index)`；seed 只用 get_untracked 读一次初始值，却仍建立可更新 item Cage。它们是 API 能力的成本，不是当前业务正在使用的能力。

反过来，Dioxus 当前 App 每次相关更新会重新迭代行集合，`.iter().cloned()` 还复制 Row/String，随后才由 diff 避免无变化的 DOM 写入。Glory 的 label 更新直接命中 Binding，因此每十行更新 0.501 ms，快于 Dioxus 0.871 ms。交换和删除同样没有表现为 Dioxus 全面领先。Dioxus 的组件级粒度减少了批量建图/拆图成本，但在这个写法下会增加局部更新的重算。

所以这里同时比较了框架和应用的惯用建模方式。不能把 60,003 对两个 Signal 当作框架所有分配量的比较，也不能把差距全部归给某个 Signal 内部锁或 HashSet。Dioxus 换成细粒度行组件/Signal 后，构造成本和更新成本都会改变，需要另建对照。

源码：[Dioxus benchmark](../benchmarks/dioxus/src/main.rs)、[Glory benchmark](../benchmarks/glory/src/main.rs)、[Rows::mount](../crates/ui/src/control.rs)、[Signal](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/signals/src/signal.rs)。活动 Core Scope 在 [runtime.rs](../crates/core/src/runtime.rs)，旧 core/src/scope.rs 已于 2026-10-26 随旧运行时删除。

## 5. 替换走专门的 keyed 快路，LIS 不是这项优势的主要原因

Dioxus keyed diff 先处理相同前后缀；完全没有共同 key 时，创建整批新子树，再删除旧子树并批量替换。当前 generator 每次建表产生新 id，千行替换正好命中这一路径。

实际计数：Dioxus 一次千行替换深克隆 1,000 次、替换动态文本 2,000 次、Element.remove 999 次、Element.replaceWith 1 次，最后一次携带 1,000 个新行根。Glory 则 removeChild 1,000 次、深克隆 1,000 次、insertBefore 1,000 次，另有完整语义实例及响应图的退休/创建。

存在共同 key 的重排才会涉及 LIS 等减少移动的算法。把本次完全替换的优势归因于“LIS 更先进”，与实际命中路径不符。同样，两者都做事件委托，不能把事件委托本身列为 Glory 缺失的能力。

源码：[keyed iterator diff](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/core/src/diff/iterator.rs)、[解释器核心 DOM 操作](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/interpreter/src/ts/core.ts)。

## 6. 清空计时还包含一个重要的回收策略差异

Dioxus 的 Rust diff 会回收 VNode 挂载、ElementId 和相关动态状态；但本版 JS remove 主要做监听清理和 `node.remove()`，不把 `this.nodes[id]` 清成空值。Rust ID 可以复用，JS 数组的旧条目会在相应位置再次使用时被覆盖。

本次另做“创建万行→清空→显式 CDP GC→四轮小千行创建/清空”观察，检查真实 RawInterpreter.nodes 和 Glory WASM externref 表。探针不保留逐节点数组；Dioxus 的解释器同时仍在 WASM externref 表内，确认不是探针单独延长一个已废弃解释器的生命。

| GC 后的观察 | Glory | Dioxus |
|---|---:|---:|
| 文档中行数 | 0 | 0 |
| 注册表仍引用的离树行根 | **0** | **10,000** |
| 后续四轮小列表清空后的离树行根 | 0 | 仍为 10,000 |
| 表的容量/长度 | externref 容量 132,100 | JS nodes 长度 50,009 |

Glory 的容量也没有缩小，但行对象引用已释放；Dioxus 这里保留的是实际离树 DOM 对象引用，不只是数字容量。行根还连着该行后代，所以不能用“两者文档已空”证明双方都完成了相同范围的资源回收。

这项证据只说明**存活 App 在该序列中保留峰值节点引用**：不能推导无限增长、精确 RSS，或 App 卸载后的保留情况。它也不表示 Dioxus 不销毁 Rust 状态。当前小循环中表没有继续增长。

因此 2.688 ms 的清空成绩是当前框架清空路径的真实成绩，但不代表完整主机对象回收快 5.57 倍。Glory 应保持退休清除实际引用的约束，不借用这个策略改善数字。

## 7. CPU 采样验证了哪些方向

独立 unwrapped CPU profile，各项取三次中位数；下表为 inclusive ms，**调用关系有包含和重叠，不可相加，也不能从正式时间减去这些值计算占比**。

| 路径 | 创建千行 | 替换千行 | 清空千行 | 创建万行 |
|---|---:|---:|---:|---:|
| Glory Mapper.apply | 13.542 | 20.115 | 5.919 | 159.189 |
| Glory gloryClonePlan | 5.803 | 6.387 | — | 62.251 |
| Glory Memory.apply | 3.033 | 4.642 | 1.618 | 45.866 |
| Glory Owner.dispose | — | 4.859 | 4.286 | — |
| Dioxus JS run | 5.403 | 7.518 | 2.676 | 57.388 |

万行 cloneNode 的 sampled self 时间 Glory / Dioxus 为 **21.172 / 19.867 ms**，已经很接近；setAttribute self 为 **32.046 / 11.270 ms**，与 90,000 / 30,000 次写入方向一致。其余数据支持继续关注实例登记、属性和桥接，不支持再将全部差距归给深克隆。

Dioxus 的 sampled run 57.388 ms 已高于正式万行 Script 44.815 ms，说明采样有扰动；不能拿它作为新一轮成绩。Glory ensure 的调用栈还包含业务建树/提交回调，不能把其 inclusive 时间全部命名为“响应图算法成本”。

## 8. 对 Glory 的改进优先级

以下均保留纯手写 Rust、Cage/Bond/Lotus、双 Memory、完整节点身份及输入/查询/成功前缀契约。Dioxus 的 Sledgehammer 本身属于通用指令解释机制，不建议直接引入；静态/动态分离和批量传输可以独立借鉴。

| 优先级 | 具体实验 | 可减少的计算 | 约束与验收 |
|---|---|---|---|
| P0 | 为显式 Plan 预计算不可变摘要 | 每实例重复扫描 layer/focus 声明、focus 子树大小，以及纯声明局部条件 | 不缓存身份/父上下文/业务值；每份 Memory 独立做世代、存活、名称冲突、容量和放置验证；不能声称已知提速比例 |
| P1 | 显式选择不需要 item/index Cage 的行工厂 | 当前无用的 index Source；只消费初值时的 item Source 和关联 Owner 工作 | 现有 Each 保持活更新语义，另建入口/迁移对照；仅去 index 的图槽理论为 60,003→50,003，二者都省才为 40,003，均非实测提速 |
| P1 | 固定用途的多实例创建/挂接桥接，研究 JS 内保留物理句柄 | 每行 11 次 Array.get、逐行插入及部分字符串往返 | 不引入通用 opcode VM；保留全部语义 ID/世代校验及退休清引用；节点查询/局部移动/挂起/输入仍可寻址；同时量两方向 ABI 调用和分配 |
| P2 | Plan 实例的初始动态字段一次初始化 | 先建空 id/label，再发首次 text/label/class patch 的重复路径 | 业务值属于实例；两份 Memory 独立验证初值；不能跨输入/查询/动作屏障重排；错误时成功前缀和 receipt 必须明确 |
| P2 | 使用现有 class_fn 或折叠部分 Owner 的独立对照 | 每行 Derived 或不需要的构造层 | class_fn 先前已测，减少图槽但万行未稳定改善；Owner 合并必须保持 cleanup 顺序、Instance 上下文和 liveness，不能只减计数 |

P0 是风险最小、范围最清楚的先行项；P1 的主机批处理最接近当前实际大差异，但比新增一个 bridge 函数更复杂，必须单独原型验证。Glory 当前 RowPlan 根未声明 `bench-row`，首次 class patch 是必要写入；不能以“初值等值”直接删掉它。把固定 class 移入声明属于应用迁移，须单独保持前后同输入。

每个实验独立构建、同源码基线 A/B、正反顺序，不使用带计数探针的时间。至少比较创建/替换/清空及局部更新；通过数据、所有存续 Element/Text 和完整语义身份、全部万行几何、相同截图，以及清空后图/Owner/查询/主机引用回到约定范围，再运行完整回归。不要为缩小批量差距牺牲 Glory 已有的小更新优势。

## 9. 原始证据与复现

全部诊断输出位于 `D:/BuildCache/glory-dioxus-analysis-20261025`，未修改 Cargo 托管源码、未 vendor、未新增依赖或编译目录。

- [输入/版本出处](D:/BuildCache/glory-dioxus-analysis-20261025/provenance.json)、[分析源码指纹](D:/BuildCache/glory-dioxus-analysis-20261025/source-audit.json)。
- [完整导入/DOM/解码计数](D:/BuildCache/glory-dioxus-analysis-20261025/counts.json)、[精简计数](D:/BuildCache/glory-dioxus-analysis-20261025/counts-summary.json)、[计数驱动](D:/BuildCache/glory-dioxus-analysis-20261025/driver/count.cjs)。
- [显式 GC 后的引用观察](D:/BuildCache/glory-dioxus-analysis-20261025/retention-gc.json)、[驱动](D:/BuildCache/glory-dioxus-analysis-20261025/driver/retention.cjs)。
- [CPU 采样清单](D:/BuildCache/glory-dioxus-analysis-20261025/profiles/evidence.json)、[函数汇总](D:/BuildCache/glory-dioxus-analysis-20261025/profile-summary.json)、[驱动](D:/BuildCache/glory-dioxus-analysis-20261025/driver/profile.cjs)。
- [正式计时与正确性证据](framework-benchmark-latest-stable-2026-10-25.md)，来自此前独立构建，不是本次诊断重计。

复现计数/采样须先在 29035 启动只读服务，根目录为 `D:/BuildCache/glory-framework-latest-20261025`；三个驱动读取其 site/。使用 D 盘 TEMP/TMP、浏览器 profile/下载和输出目录；CPU 采样驱动通过 pin-browser.ps1 固定核策略。分析结束会停止本任务自有服务，不能假定该端口长期存在。
