# Glory 核心响应式与渲染优化评估

日期：2026-10-24（Asia/Shanghai）

状态：已按当前实现与声明共享专项结果更新，并完成文档/源码复核；本次未实施下列候选或运行新基准。

## 结论与证据范围

保留当前架构：Cage/Bond/Lotus、属性级 Binding、显式 Owner、代际身份、有限语义树和类型化渲染协议。下一轮重点是降低这些机制的分配、复制、登记和提交成本。

本文区分已完成实现、源码可见候选和已有冻结实验。工作区存在同期功能修改，既有冻结基准不能视为覆盖全部当前源码的测量。所有候选收益均待独立 A/B 验证，不能把容器、clone 或宿主调用数量直接换算成时间收益。

Glory 的日常更新粒度更接近 Leptos：组件构造不自动成为订阅者，动态属性和控制流各自建立观察者。Dioxus 的组件本身是 reactive context，常见更新路径是重跑订阅组件，再通过模板辅助的 VDOM diff 生成 mutation。Dioxus 的静态结构复用和宿主提交方式值得借鉴，但无需为此改变 Glory 的更新模型。

## 1. 当前实现与已有结果

当前主路径：

```text
Cage 写入并推进版本
  → 直接依赖图传播失效
  → Runtime flush：Structure → Binding → Effect
  → 按需验证或重算 Bond
  → 属性 Binding 比较最后成功注入值
  → preflight / staged Memory 校验与排队
  → Render::apply
  → HTML mapper / Web / SSR / Blitz
```

关键源码：

- [响应图与调度](../crates/core/src/runtime.rs)：直接依赖、版本验证、传播前沿、Owner 和本地执行上下文。
- [属性构造](../crates/ui/src/builders.rs)、[Plan 绑定](../crates/ui/src/plan.rs)：属性比较及类型化 Patch。
- [App](../crates/ui/src/app.rs)：staged Memory、提交、查询和生命周期。
- [Memory](../crates/render/src/memory.rs)：字段级 prepared patch、身份和结构规则。
- [HTML mapper](../crates/html/src/lib.rs)、[字段 lowering](../crates/html/src/patch.rs)：独立验证、成功前缀与增量 Edit。
- [Web Plan](../crates/web/src/browser/plans.rs)、[DOM](../crates/web/src/browser/dom.rs)：静态原型、宿主登记与释放。

以下优化已存在，不再列作待办：

1. 显式静态 Plan 和 DOM 子树深克隆。
2. 分页 NodeMap，应用于两份 Memory 和 DOM Entry。
3. DOM Part 的固定槽位存储。
4. parent 信息并入 Entry，移除重复 token 字符串索引。
5. 部分父子 Dispose 合并、Owner 记录归并及有界调度队列压缩。
6. 字段级 prepared patch 和增量 lowering，避免普通属性更新构造两份完整 Shape。
7. 显式 Plan 的静态声明共享：LiveDescriptor / DescriptorView、每份 Memory 独立 Props、按字段分离覆盖值及回滚后恢复共享状态。普通 Create 仍独占。
8. Web Plan 在离线物理创建完成后，以一次 Entry 表可变借用登记整个实例；实例内仍逐节点创建 Entry、保存完整身份，不能将这一步再次整体列为待实现。

当前 builders 源码还包含响应式初值与订阅安装分离的改动；必须把它与后续候选分开测量，不能重复计算收益。

最新可用实测来自[双份 Memory 声明共享专项](../docs/shared-declarations-2026-10-24.md)。下表是其最终同批结果，单位为 ScriptDuration 中位数 ms；Glory after 指该报告保留的 inline 版本。

| 操作 | Glory before | Glory after | Leptos 0.8.19 | Dioxus 0.7.9 |
|---|---:|---:|---:|---:|
| 创建千行 | 23.35 | 19.59 | 15.30 | 4.52 |
| 替换千行 | 36.70 | 30.43 | 22.86 | 7.85 |
| 创建万行 | 270.61 | 234.37 | 187.55 | 60.93 |
| 清空千行 | 17.53 | 15.45 | 17.59 | 8.96 |

该批包含六个独立构建、两轮正反顺序、1,296 个主样本，每版本每操作 24 个样本。Glory before→after 四项 Script 分别下降 16.1% / 17.1% / 13.4% / 11.9%。复制声明消融版的替换慢样本、绑核辅助脚本中断/恢复、选行历史效应均保留；初次五版计时批的 1,080 样本不与最终批合并。千行追加和局部更新的总耗时未全面改善，最终 WASM 文件约增大 3%，不能只记录脚本收益。

被测 before 冻结 287 个输入，候选为 289 个，15 个 benchmark 输入一致。该性能候选已有 Windows/Linux 各 522 项工作区测试、27 组浏览器套件和 64/1024 真实行高验收；同期功能另行冻结的最终合并快照通过 566 项工作区测试及 native/WASM 严格 Clippy。后者是合并验收，不是性能测量版本，也不是本次文档更新重新执行的测试。

历史的[节点存储专项冻结基准](../docs/node-storage-dom-metadata-2026-10-24.md)仍保留，供理解前一轮变化，单位同为 ScriptDuration 中位数 ms：

| 操作 | Glory after | Leptos 0.8.19 | Dioxus 0.7.9 |
|---|---:|---:|---:|
| 创建千行 | 23.90 | 14.79 | 4.48 |
| 替换千行 | 36.87 | 21.63 | 7.88 |
| 创建万行 | 256.21 | 183.78 | 49.81 |
| 清空千行 | 16.86 | 16.71 | 2.70 |

历史报告含四轮、2,160 个主样本，每版本每操作 48 个样本。不能从它的 Glory after 直接减去新表的 after 来计算声明共享收益，三框架比值也只在各自同批内比较。已有结果支持继续研究批量构造、提交和退休，不支持把全部差距归因于响应式算法或两份 Memory。Script CPU 不是显示完成时间或 FPS；不同 inclusive 采样不能相加。Leptos 混合流程选行异常也不能泛化为通常选行成本。

## 2. 任务状态与优化候选

P0 表示实施前的基线与观测准备；P1 表示优先独立实验；P2 表示范围扩大前需要窄实验支持。优先级不是已证明的收益排序。

| 任务 | 状态 / 优先级 | 下一步边界 |
|---|---|---|
| 2.1 静态声明分离 | 已完成 | 仅对剩余 Props 初始化、Plan 引用和普通 Create 成本提出新假设 |
| 2.5-A 提交/输入缓冲复用 | 候选 / P1，先行 | 保持协议和排队顺序，量化容量复用与保留成本 |
| 2.5-B queued 成员资格 | 候选 / P1，单独实验 | 替换成员查询，不改变 propagated、FIFO 和取消/压缩规则 |
| 2.2 依赖快照与追踪 | 候选 / P1，观测后实施 | 快照优化与容器替换分开；少依赖路径和大 fan-out 同测 |
| 2.3 直接属性计算 | 候选 / P1，针对批量构造 | 显式 API，单独记录应用迁移对照 |
| 2.6 退休遍历 | 候选 / P1，profile 决定具体项 | 只处理仍存在的收集、查找或扫描热点 |
| 2.4-A 窄范围宿主批处理 | 候选 / P2 | 剩余逐节点桥接、多个离线根插入或批量释放，分别试验 |
| 2.4-B 通用编码提交器 | 待窄实验结果 / P2 | 尚不作为下一轮首项建设任务 |

### 2.1 静态声明载荷与实例状态分离：已完成

实现见 [LiveDescriptor](../crates/render/src/props/live.rs)、[DescriptorView](../crates/render/src/props/view.rs)、[Memory](../crates/render/src/memory.rs) 与[声明共享测试](../crates/render/tests/declarations.rs)，不再作为下一轮待办。

Plan 实例现在保存独立 Props、Plan 引用、声明索引和可选覆盖字段。Style、classes、options 等元数据借用不可变声明，修改时只分离被触及的字段；失败、guard drop 与 unwind 恢复原值及覆盖/共享状态。两份 Memory 不再各初始化完整的静态 Descriptor。普通 Create 保存独占 Box<Descriptor>。

每个语义节点仍有完整 NodeId、可变属性、查询目标和事件身份；没有将整份 Descriptor 或 Props 改成共享写时复制。该范围符合静态声明/动态路径分离的思路，不要求照搬 Dioxus 的宿主句柄分配方式。

两份 Memory 继续分别表达 staged 状态和 committed/accepted 状态。不能让适配器提前看见尚未提交的未来状态，也不能省略 raw apply 的独立验证。

后续若发现 Props 初值复制、Plan 引用维护、普通 Create 间接访问或映射结果重复，应另立假设与消融，不把本次已完成收益重复计算。专项 size_of 的 Node 760→176 字节仅适用于被测原生源码的内联记录，不代表当前全部类型布局、wasm32 大小或 RSS。

### 2.2 依赖存储与追踪缓冲

**优先级：P1；源码可见候选，先分项观测，再做少依赖路径实验。**

当前 Node 使用 BTreeMap 保存依赖与观察版本，使用 BTreeSet 保存订阅者。ensure 对 Clean 节点直接返回；只在需要验证或计算时克隆旧依赖表，重算时收集新表，之后比较新旧边。不能将它描述为每次读取都复制依赖。

node.compute.clone() 和 node.value.clone() 中的引用计数句柄克隆，应与依赖 BTreeMap 克隆及业务 T::clone 区分计量。空 BTreeMap::new 本身也不等于发生堆分配。publish/invalidate 虽有临时 Vec，但 Vec 消费/collect 可能复用存储，实际分配次数应由实验确认，不能仅按源码中的 Vec 变量数相加。

先按 Source / Derived / Binding / Structure / Effect 统计活跃节点数量、依赖数与 fan-out 分布、非 Clean 验证和实际重算次数、分配次数/字节及阶段耗时。使用按区间聚合的有界直方图，不保留访问过的行键或每次求值历史。

拆成两个实验：A 减少验证快照/重复登记；B 少量依赖的紧凑存储、追踪缓冲复用与大集合回退。先完成 A 的语义验证，再独立评价 B，避免把快照、容器、传播算法同时修改。必须同时测量小节点和高扇出图；给所有节点增加内联容量可能使无依赖 Source 和大图承担额外 Node/缓存压力。

Leptos v0.8.19 的实际边集代码使用 IndexSet 和 FxHasher，并要求删除时保持订阅顺序。源码顶部关于 Vec 的注释不应替代实际实现。Glory 当前 Key 按 slot/generation 排序，订阅集合按 Key 遍历，invalidate 用栈继续传播，调度保持各阶段 live FIFO；保留“插入顺序”不等于保留这个执行顺序。容器替换不能机械套用另一框架。

保留动态退订、版本验证、传播前沿重置、嵌套读取、Owner 退休、panic 恢复及用户回调外借用边界。不能在持有图可变借用时执行计算以省掉快照。

若尝试移走而非克隆旧依赖，必须证明求值期间的退休/重入仍能完整退订；不能简单 mem::take 让退休路径看见空依赖，也不能跳过已有边对应的传播前沿重置。

新增验收：0/1/2/4/多依赖、重复读取同一源、分支切换、菱形共享、已失效但输出相等、晚订阅、求值期间退休、panic 后重试，以及高 fan-out 的创建/写入/清空；对照回调与阶段顺序。单独报告 Source 和其他节点的记录大小、分配与保留容量，不以小图时间改善掩盖大图退化。

### 2.3 廉价属性转换直接进入 Binding

**优先级：P1；针对批量创建/替换，可先于通用宿主编码提交评估。**

研究显式属性计算入口，将只被一个属性使用的简单格式化从 Cage → Bond → Binding 缩短为 Cage → Binding 内计算。

当前 [benchmark 每行选中样式](../benchmarks/glory/src/main.rs) 经 selected.map(...) 创建 Bond，再由 instance.class 安装 Binding，具有具体的单消费者转换作用点。直接属性计算有机会减少每行一个 Bond 及其值、边和退休工作；减少节点数不等于相同比例的时间收益，标签 Cage 和属性 Binding 仍保留。

借鉴 Leptos 对普通派生闭包与 Memo 的区分：昂贵计算、多消费者共享结果和需要派生层相等门控时保留 Bond；廉价单消费者转换可以避免独立缓存节点。直接属性计算仍保留 Binding 对最后成功注入值的比较，不能把“属性值需要相等比较”本身当成必须额外创建 Bond 的理由。

这是显式 API 候选，不是悄悄改变 Cage::map 或 Lotus 的语义。不得自动合并无关属性的订阅，不得引入隐式回调记忆化。计算仍应纯净，生命周期仍归 Binding 的 Owner。

初值与后续运行都要受纯计算限制，不能只把初值包在 untracked 中就认为已经禁止写入。普通 builder 保留完整初值校验/预检先于身份分配的边界；PlanInstance 已有身份，其动态安装应遵守现有 patch 预检与失败前不改变模型/队列的边界，不能误称实例尚未分配身份。追踪只属于最终属性 Binding。还要验证首次安装、源在安装前变化、相等输出、条件依赖、Owner 退休与值版本耗尽。

该实验若将 benchmark 改用显式新 API，应标为“相同工作负载的应用迁移对照”，保留迁移前后源码和业务/布局/身份 oracle，不能宣称 benchmark 源码未变。核心内部优化仍使用完全相同应用输入的独立构建对照，两个系列分开记录。

另可检查属性更新时 try_get、previous 和 Patch 之间的值复制，但不能为了减少 clone 而把用户 PartialEq、Clone 或 patch 闭包放入不兼容的借用范围。

### 2.4 有界宿主批量提交

**优先级：P2；拆分为窄范围实验和待定的通用提交器。**

当前 Web Plan 已有深克隆，以及离线创建后一次 Entry 表借用的实例登记。它仍在 Rust 中逐节点创建 Entry、登记完整身份；“已有批量登记”与“剩余逐节点成本”应分别表述。

2.4-A 只挑一种剩余成本：固定字段的逐节点跨语言桥接、多个离线根的批量插入，或批量物理释放。分别记录桥接次数、编码/解码分配与耗时、实际 DOM 工作和最终状态，不能只凭调用次数减少保留改动。

区分批量传输与合并语义修改。批量传输可以保留每个原操作；合并则可能改变验证、拒绝位置和事件可观察顺序。

必须把完成位置映射回原 Mutation，保留成功前缀、Receipt 和查询水位。不得跨查询、动作、受控输入等屏障合并；失败操作自身仍可能留下现有契约允许的宿主部分修改，不能宣称新增事务回滚。

2.4-B 再借鉴 Dioxus interpreter 的固定词汇、数字索引与编码传输；只有窄实验支持，才决定是否建设通用编码提交器。它不应扩展成模板 VM/UI DSL。既有属性字符串打包实验曾出现不利结果，减少桥接次数本身不保证更快。

新增验收包括原 Mutation 与宿主操作的对应表、批次中每个失败位置、CreatePlan 内部失败、查询/动作/受控值屏障、布局读取时机、Move/Suspend/Resume 与焦点/IME 状态。一次语义操作可以产生多个宿主步骤，不能把宿主步骤数误当作 Receipt 的提交数。

### 2.5 提交与输入缓冲复用

**优先级：P1；A/B 两项分别构建和测量，不同时修改。**

2.5-A：当前 commit 通过 mem::take 取走 commands/actions/listeners，处理后没有归还原 Vec 容量；pump 创建新的输入 Vec。研究双缓冲或 RAII 归还空缓冲，减少长期帧循环的反复分配。

归还必须兼容处理期间的新排队内容、错误和重入；不能覆盖新命令，不能在借用内部销毁用户捕获。复用存储只保留容量，不保留访问过的行或用户值历史。

空缓冲归还不应恢复已接受命令或覆盖新队列；unwind 时归还逻辑本身不能因借用冲突再次 panic。报告一次大创建/清空后保留的各缓冲容量，明确超大批次是否退还存储，不能把保留容量忽略为零成本，也不将容量直接解释成 RSS。

验收持续小帧、突发大批次后回到小帧、空帧、适配器错误、处理期间新排队内容和 panic；pump 仍须处理完整 drained tick、完成后续查询并执行必需 flush，不能在首个路由错误处提前退出。

2.5-B：当前 State.queued 为 BTreeSet。独立试验节点 queued 标志以减少成员资格集合操作，但不能复用 propagated：是否已入队与当前失效是否已经传到订阅前沿是不同事实。

保留即时取消、代际校验、Structure → Binding → Effect 及各阶段 live FIFO；压缩仍在取消或有效出队后、回调/重入前进行。物理队列仍满足 entries ≤ 2 × live queued + 64，live queued 为零时清空长度；重复入队、退休槽位复用、计算期间再次失效、部分读取与晚订阅均需覆盖。与边集容器替换分开，保持传播逻辑不变。

### 2.6 退休遍历与临时集合

**优先级：P1；先确认当前销毁热点，必要时可提前某个已定位的小项。**

Core、UI、Memory、Mapper 和 DOM 各自承担不同退休责任。先减少同一层内重复收集子树、临时 Vec/HashSet 和已知成员的重复查找，不直接跨层复用未经独立校验的可变树。

既有父节点表全表 retain、逐兄弟查找/搬移、部分父子 Dispose 合并和 Owner 记录归并不再整体列为待做。当前 DOM dispose 仍构造 removed Vec 和 subtree HashSet，并过滤 layout/inputs；这些是复核入口，但对应集合为空时不能假定它们是 bulk benchmark 的主热点。

最新专项的独立采样中，千行替换 Owner dispose 约 4.72 ms inclusive、removeChild 约 3.17 ms self；清空为 4.42 / 4.51 ms。它支持分别研究生命周期与物理删除，不能将两类采样相加或据此指定任一临时集合的精确收益。

保留 children-before-parents 与反向注册 cleanup 顺序、借用外析构、单独 Suspend 后代的回收、跨 Scope 监听和 Owner-owned query/task。物理删除尽量作用于根范围，运行时后代记录仍需完整退休。

新增验收覆盖深/宽树、叶 Owner、重复 dispose、独立 parked 后代、跨 Scope 监听、退休查询与任务、析构/cleanup 重入及错误前缀。同时观察空队列与有待处理布局/事件的场景；后者的改善不能被误报为无观察业务场景的同等收益。

## 3. 应继续保留的设计

| 设计 | 原因 |
|---|---|
| Cage/Bond/Lotus 和属性级 Binding | 保持公共模型与局部更新粒度 |
| 直接依赖、推送失效与按需验证 | 避免整组件重跑，并支持动态依赖 |
| Bond 显式相等门控 | 不强迫任意业务数据实现 PartialEq |
| Structure → Binding → Effect | 结构和行状态先于属性注入 |
| 显式 Owner 与 Runtime/slot/generation | 生命周期、取消和过期身份可靠 |
| staged / committed Memory | 独立表达待提交状态和已接受前缀 |
| 字段 prepared patch 与纯 lowering | 保持局部修改、回滚与校验边界 |
| 宿主监听只排队 | 避免业务同步重入 |
| 窗口列表的活跃范围预算 | 不把缓存变成访问历史 |
| 手写 Rust builder 和有限语义协议 | 多后端共享契约，不引入 RSX/DSL/模板 VM |

## 4. thread_local!：三者都使用

### Glory

活动 core 的 runtime.rs 中有四项线程局部存储：

| 名称 | 用途 |
|---|---|
| RUNTIMES | RuntimeId → Weak<State> 的本线程句柄查找表 |
| TRACKING | 当前求值的观察者栈及本次收集的直接依赖 |
| EVALUATING | 当前 Pure/Effect 执行模式栈 |
| CONSTRUCTING | 当前构造所属 Runtime/Owner 栈 |

TRACKING 不只是一个指针：它也暂存当前求值的依赖收集结果。但实际持久图、用户值、Owner、任务、队列和调度标志都在各 Runtime 的 State 中；RUNTIMES 只保存弱引用。Runtime drop 会移除对应注册。

这些 TLS 的目的是支持无须显式传入上下文的读取追踪、Copy handle 查找、纯计算约束与所有权归属。Glory 的响应式句柄被明确设计为 !Send/!Sync。

### Dioxus 0.7.9

官方发布源码明确使用：

```rust
// packages/core/src/runtime.rs
thread_local! {
    static RUNTIMES: RefCell<Vec<Rc<Runtime>>> = const { RefCell::new(vec![]) };
}

// packages/core/src/reactive_context.rs
thread_local! {
    static CURRENT: RefCell<Vec<ReactiveContext>> = const { RefCell::new(vec![]) };
}
```

前者是当前激活 Runtime 的栈，后者是当前响应式订阅上下文的栈。与 Glory 相比，Glory 的 RUNTIMES 是按 RuntimeId 查找的弱注册表，Dioxus 此处是当前进入 Runtime 的强引用栈，二者不能只凭同名视为完全相同。

### Leptos 0.8.19

该标签下 reactive_graph 官方源码明确使用：

```rust
// reactive_graph/src/owner.rs
thread_local! {
    static OWNER: RefCell<Option<WeakOwner>> = Default::default();
}

// reactive_graph/src/graph/subscriber.rs
thread_local! {
    static OBSERVER: RefCell<Option<ObserverState>> = const { RefCell::new(None) };
}
```

OWNER 提供当前所有权上下文，OBSERVER 提供当前依赖追踪目标和 untracked 状态。虽然这里表现为 Option，进入/退出的 guard 会保存和恢复前一个值，支持动态嵌套。

Leptos 的 Owner 内部使用 Arc/RwLock，响应式系统也提供线程安全和 local 存储选项。使用 TLS 获取“当前上下文”不等于全部数据只能位于一个线程，也不等于所有框架都有同样的线程模型。

### 对 Glory 的建议

保留 TLS 上下文机制。不能仅因出现 thread_local! 就判断存在进程级单例状态、无法隔离多个 App 或性能瓶颈。

应测量的是 TLS 访问之后的操作，例如 Glory 每次 handle lookup 的 RuntimeId 哈希查找、Weak 升级，以及依赖记录的容器维护。若这些确实成为热点，再评估有明确作用域的访问快路径；不能假设一次 TLS 读取比额外分配、哈希、克隆或宿主调用更贵。

同时保留这些边界：TLS 不会自动跨 await 或线程传播；进入上下文与恢复必须兼容嵌套和 unwind；跨 runtime 读取与退休 Owner 必须检查。Glory 已有 Runtime::enter，但它是同步、非拥有的入口，不是异步上下文载体。

## 5. 实施与验收顺序

1. **P0：建立新基线与观测。** 冻结当轮源码、锁、构建工具与应用输入，统计节点类型、依赖数/fan-out 分布、分配、队列操作和阶段耗时。使用当前代码新建 before，不将本报告历史表当作将来任一实验的 before。
2. **P1：2.5-A 缓冲复用。** 比较稳态小帧、批量创建/替换/清空和大批次后的容量保留；只修改缓冲生命周期。
3. **P1：2.5-B queued 成员资格。** 保持原边集与传播算法，单独比较入队/出队/取消开销和队列上界。
4. **P1：2.2 依赖快照，再到紧凑存储。** 依据 P0 分布选择少依赖路径，快照/登记与容器替换分别 A/B；大 fan-out、动态依赖和退休语义不退化。
5. **P1：2.3 显式直接属性计算。** 优先验证当前行样式的单消费者用例；报告图节点/边/值存储变化，作为单独的 API 迁移系列，不混入应用源码不变的引擎对照。
6. **P1：2.6 退休热点。** profile 若已指出某个低风险重复遍历，可提前该小项；每次只优化一层或一个循环，不取消各层职责。
7. **P2：2.4-A 窄范围宿主实验。** 与 Rust 图/生命周期优化分开；仅在结果支持时进入 2.4-B 通用编码提交。2.1 已完成，不再重复排期。

每项使用以下记录模板，不能仅以“更少 clone/调用”判定保留：

| 记录项 | 必须写清的内容 |
|---|---|
| 假设与边界 | 修改哪个成本、保留哪些语义、哪些已完成路径不重复计功 |
| before / after | 冻结输入哈希、独立 target、编译参数、工具版本、应用是否迁移 API |
| 成本证据 | 相关分配次数/字节、节点/边/队列操作、缓冲容量与 WASM 文件变化；容量/size_of 不当作 RSS |
| 主结果 | 同工作负载原始样本、轮次与顺序、Script/总耗时、未改善项；采样 profile 单列 |
| 语义验收 | 该项专属边界，加下述共享回归矩阵；失败/恢复和慢样本保留 |
| 保留决策 | 实际收益、退化及空间代价；改善不稳定时说明待定位，不以补跑挑选较快结果 |

不预设未经测量的提速百分比。若要改变传播顺序或其他公共可观察语义，应另开架构决策，不能作为容器替换顺带实施。

验证覆盖冷创建、热替换、局部更新、大 fan-out、动态依赖、菱形图、嵌套 batch、panic/重入、退休与晚到事件/查询。局部清空应恢复仍挂载应用的基线；完整卸载后该应用持有的节点/监听/任务/查询应释放，不影响其他 Runtime 或应用的活跃状态。

渲染变更必须继续验证全部节点身份、输入 revision/选区/composition、Suspend/Resume、直接 apply 拒绝与成功前缀、SSR/stream/hydration 以及适用后端。CPU 数据与真实布局、画面和宿主验收分开记录；不使用减功能或放宽容差制造提速。

继续保留相同业务数据、语义/物理节点 census、存续 Element/Text 对象与完整 NodeId、所有行真实几何和最终截图对照。变高列表使用 64/1024 测量缓存、真实行高与原容差，检查小数锚点、滚动位置和身份；不能用 bulk benchmark 的通过替代虚拟列表或输入协议回归。

新构建与缓存全部使用独立 D:/BuildCache 路径，并用 cargo metadata 验证 target_directory。新候选按影响范围运行 core/UI/各适配器单测、native/WASM 严格检查与浏览器/SSR/stream/hydration 验收；涉及 native 路径时保留适用平台的布局、窗口和辅助技术验收。减少 workspace 或只测无 DOM 路径不能充当完整回归。本文更新没有执行编译或新基准。

## 6. 本次更新与复核记录

本次修改范围仅为本报告；以下“通过”指文档事实、链接和任务边界的复核，不表示候选已经实现或运行时测试重新通过。

| 复核项 | 依据与结论 |
|---|---|
| 已完成状态 | render/props/live.rs 的 Planned(Instance)、DescriptorView、字段覆盖/回滚，以及 declarations.rs 测试与专项报告一致；2.1 已移出待办 |
| 性能数值与批次 | 最新表取专项最终 inline / before / Leptos / Dioxus，同批 1,296 样本；旧表 2,160 样本标为历史，两批不相减或合并 |
| 依赖成本 | runtime.rs 的 ensure 先判断 Clean；旧依赖克隆、Frame 追踪、Rc 句柄与业务克隆已区分，顺序描述对应 Key/BTreeSet/传播栈 |
| 属性入口 | builders.rs、ui/plan.rs 与 benchmark 的 selected.map → class Binding 对应；新 API 仍为候选，普通创建与已有实例的预检边界分开 |
| 宿主批处理 | web/browser/plans.rs 已有 clone_plan 与一次表借用登记；新增范围只列剩余逐节点桥接/多根/释放，通用编码后置 |
| 缓冲与队列 | ui/app.rs 的 commit/pump 和 core/runtime.rs 的 queued 与 propagated 分离对应；拆成 2.5-A/B，保留 FIFO、即时取消与压缩上界 |
| 退休范围 | core/UI/Memory/Mapper/DOM 各层责任保留；已完成全表 retain 等优化不重复排期，现有临时集合不直接宣称为热点 |
| 官方实现 | 固定标签下核对 Leptos 的 Owner/Observer TLS、Arc/RwLock 与 IndexSet 保序删除，以及 Dioxus 的 ReactiveContext TLS；保留文末官方源码引用，不据此推导 Glory 的收益 |
| 验收与来源 | 21 个本地链接可解析，最新表 16 个数值与原始汇总一致、历史表 12 个数值与旧报告一致；数值沿用既有冻结实验，当前新增候选收益未知，文档检查不冒充运行时回归 |

## 7. 官方参考与既有报告

- [Dioxus v0.7.9 Runtime / TLS](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/core/src/runtime.rs)
- [Dioxus v0.7.9 ReactiveContext / TLS](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/core/src/reactive_context.rs)
- [Dioxus v0.7.9 Template](https://docs.rs/dioxus-core/0.7.9/dioxus_core/struct.Template.html)
- [Dioxus v0.7.9 interpreter](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/interpreter/src/unified_bindings.rs)
- [Leptos v0.8.19 Owner / TLS](https://github.com/leptos-rs/leptos/blob/v0.8.19/reactive_graph/src/owner.rs)
- [Leptos v0.8.19 Observer / TLS](https://github.com/leptos-rs/leptos/blob/v0.8.19/reactive_graph/src/graph/subscriber.rs)
- [Leptos v0.8.19 边集](https://github.com/leptos-rs/leptos/blob/v0.8.19/reactive_graph/src/graph/sets.rs)
- [Leptos Memo 与普通派生闭包](https://book.leptos.dev/reactivity/14_create_effect.html)
- [静态构造计划及独立对照](../docs/static-construction-plan-2026-10-24.md)
- [静态计划后的三框架比较](../docs/framework-benchmark-static-plan-2026-10-24.md)
- [分页节点存储与 DOM 元数据专项](../docs/node-storage-dom-metadata-2026-10-24.md)
- [双份 Memory 声明共享、最终六版对照与完整验收边界](../docs/shared-declarations-2026-10-24.md)
- [较早的创建桥接实验与未保留候选](../docs/bulk-create-dioxus-2026-10-24.md)
