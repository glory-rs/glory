# Glory v2 DSL 与浏览器批量提交调查

日期：2026 年 10 月 28 日。调查起点：`f046098d795027e2d15e8c6b587188327ea1b7fc`，分支 `v2`。
对照对象为仓库 benchmark 锁定的 Dioxus **0.7.10**，不把其他版本的行为混入结论。

后续用户进一步明确：HTML/CSS 是标准显示契约，原生 HTML 元素与 Glory 高层 widgets 分层，
静态显示元素不应默认进入完整语义实例模型。以新的
[HTML 模板运行时架构决策](v2-html-template-runtime-architecture-2026-10-28.md)为 v2 设计基准。
下文优先将 DSL 展开为现有 Plan 的建议保留为前期调查，不再作为新显示路径的目标。

**建议 v2 将编译前端、实例表示、宿主提交分别设计和验证。** XML 风格 DSL 可以把结构提取为共享声明，把响应式表达式编译为字段绑定；这些绑定继续使用 Cage/Bond/Vary。浏览器可以将一段连续提交编码后交给 JS 顺序执行。两者可以独立实施，也可以共同使用。

最大的潜在结构收益来自减少重复的构造和实例记录。XML 语法本身不会消除语义节点、响应图或 DOM 成本；一次 JS 调用也不会消除 JS 循环、原生 DOM 操作和布局。保留现有每节点契约的方案与压缩静态节点身份的方案，需要分别评价。

本次交付为可行性和设计调查，未实现 DSL、改变运行时契约或重跑性能基准。用户此次要求允许讨论原有“纯手写 builders、无 UI DSL”约束之外的 v2 方向；生产实现仍使用现有架构。

## 1 当前实现已经具备的基础

| 能力 | 当前实现 | 对 v2 的含义 |
|---|---|---|
| 静态声明复用 | `ui::plan` 将静态 builders 收集为 `render::Plan`；声明共享不可变元数据 | DSL 可以生成现有 Plan，先验证前端价值 |
| 每实例字段绑定 | `PlanInstance::text/bind/bind_fn/on` | 动态值与事件捕获有明确安装位置 |
| 静态值与响应值区分 | `Vary::Value/Cage/Bond`；builders 对 Value 不建 Binding | “常量不订阅”已经存在 |
| 动态表达式直接绑定 | `bind_fn` 支持纯闭包，不要求先建一个 Bond | 编译器可以生成细粒度绑定，避免无共享用途的中间派生 |
| 元数据共享 | `LiveDescriptor` 的 Planned 表示共享 classes/style 等，仅被改字段分离 | 不能把“共享所有静态描述”全部算作 v2 的新增收益 |
| 完整实例模型 | 每个 Plan 节点仍有独立 Props、NodeId、树关系；UI 与 Mapper 各有 Memory | 静态节点仍有实例成本 |
| 浏览器构造 | Plan 已支持离线原型和 `cloneNode(true)`；不满足条件时走通用路径 | 已有整树克隆，后续重点包括初始化、挂接、登记与退休 |
| 应用收集修改 | `Session::commit` 已取出 commands，一次调用 `Renderer::apply(&commands)` | 缺少的是 Web 后段完整的批量执行协议 |
| 浏览器提交 | Mapper 顺序映射，每个 Op 调用 Sink；Plan 有有限字段/放置/移除专用 JS 目标 | 当前一次 Renderer 调用会产生多次 WASM→JS 调用 |

源码入口：[UI Plan](../crates/ui/src/plan.rs)、[builders](../crates/ui/src/builders.rs)、[实例共享](../crates/render/src/props/live.rs)、[应用提交](../crates/ui/src/app.rs)、[HTML Mapper](../crates/html/src/lib.rs)、[Web Renderer](../crates/web/src/browser.rs)、[DOM 执行](../crates/web/src/browser/dom.rs)、[浏览器 Plan](../crates/web/src/browser/plans.rs)。

当前 `Plan` 是**运行时显式构造一次并复用**的声明，不是宏已生成的编译期常量。它含 Rc 和拥有的描述符；将其直接变为静态 Rust 数据，需要新的只读声明表示或一次性的转换过程。当前每份 Plan 最多 256 个语义节点，较大界面应显式分段，不能静默突破上限。

## 2 XML 风格 DSL 可以自动完成什么

### 2.1 自动解析应建立在明确的组件契约上

可以将标签解析为语义构造器，将属性解析为有类型的字段，将文本解析为 Text 声明，将表达式解析为一次性值、响应式绑定或结构工厂。组件只在挂载时执行构造过程；静态标签不必各自对应一个长期存活的 Rust Widget 对象。

“根据内部代码结构自动解析”有三种不同范围：

| 范围 | 可行性 | 合理做法 |
|---|---|---|
| 内建标签对应已知字段与事件 | 高 | 生成有类型的 builder/Plan 调用，复用既有验证 |
| 自定义组件对应 Rust Props | 高，但需要显式协议 | derive 或显式注册生成 Props builder；宏输出字段调用，由 Rust 检查类型 |
| 扫描任意函数、impl、模块，推断 UI 语义和生命周期 | 不适合作为基础 | 参数类型不能充分表达内容约束、焦点、Owner、双向编辑和资源权限 |

Rust 的稳定过程宏接口消费、产生 TokenStream；它没有直接向宏提供任意表达式经类型检查后的完整语义信息。因此宏识别语法结构，生成普通 Rust，让编译器完成名称解析、泛型和字段类型检查。自定义组件可约定 Props 协议，无需扫描全项目。[Rust Reference](https://doc.rust-lang.org/reference/procedural-macros.html)

字段说明最好从 Rust 的类型化契约生成，避免另写一份 XML 属性表后长期与 builders 漂移。跨字段、祖先上下文、当前名字占用和 adapter profile 等验证仍要经过现有共享规则；仅在编译期能确定的部分提前报错。

### 2.2 建议先用 Rust 内嵌的类 XML 语法

以下是候选语法，**不是现有 API**：

```text
ui! {
    <view class="counter">
        <text content="点击次数" />
        <text bind:content={move || count.get().to_string()} />
        <button label="增加" on:activate={move |_| count.revise(|v| *v += 1)} />
    </view>
}
```

预期展开为四部分：共享静态声明；本次实例的 NodeId/parts；一个读取 count 的 Text Binding；本次 Owner 下的 Activate 处理器。count 的改变只触发该文本绑定，保持 Glory 当前的直接依赖模型。

第一版显式区分 `field={expr}` 的一次性输入、`bind:field={closure}` 的持续绑定和 `on:event={handler}` 的事件。一次性表达式求值一次；绑定的读取发生在自己的观察者中，不能先在构造阶段读取成普通 String，再期待自动订阅。后续可用类型化 `Into<Vary<T>>` 减少标记，但宏不能仅凭 `{expr}` 推断任意业务函数是否应反复运行。

同一声明可复用的必须是字面量或显式声明的静态输入。一个 `String` 或 `Vary::Value` 虽无订阅，也可能每行都不同；它属于实例数据，不能按值自动进入全局模板缓存。事件和绑定闭包也不能被共享声明捕获。

语法应明确组件路径、children、key、引用和错误位置。`when/each/list` 继续调用现有结构控制器，按子模板划分声明；自定义组件的内部结构默认是编译边界。嵌入 Rust 表达式的语法应称“类 XML”，不承诺符合 XML 标准。

内嵌宏的 TokenStream 不把源码空白作为独立 token 保留。第一版文字宜使用字符串属性或明确的字符串子项；不能把任意裸文本重新拼接后声称保留了原始空格、换行和标点。若要求真正 XML 的文本保真，需要解析字符串字面量或独立文件，并承担对应诊断成本。[Rust token stream](https://doc.rust-lang.org/reference/procedural-macros.html#the-proc_macro-crate)

独立 `.xml` 文件可在需要编辑器集成时增加，但必须补上 Rust 表达式关联、文件变化重编译、跨文件诊断、格式化、命名空间和空白规则。运行时解析还会增加解析预算、版本和动态加载边界。当前目标适合先做编译前端。

### 2.3 静态内容如何表示

| 内容 | 共享声明 | 每实例运行时 |
|---|---|---|
| 字面文本、固定结构和样式 | 常量内容、父子索引、样式声明 | 现有语义实例与实际宿主节点 |
| 一次性业务值 | 类型化输入槽定义 | 只保留实例需要的值，不建订阅 |
| 响应式文本/属性 | 类型化槽定义 | 当前值、Binding、依赖关系 |
| 条件和集合 | 子声明与结构槽 | Owner、key、存活范围与现有协调器 |
| 事件 | 事件槽定义 | 当前实例的处理器与注册 Owner |
| 输入控件 | 控件结构和有限配置 | 值、revision、selection、composition 等活状态 |

普通文本按文本转义。原始 HTML 应继续是单独的能力边界，不能把字符串自动当作 markup；相邻文本合并也必须保留 WhiteSpace、独立身份和内容模型的可观察行为。

## 3 把元素看成数据的显示装饰

这个视角适合描述**数据到呈现字段的投影**：业务模型保存 count，Binding 读取 count 并格式化为 Text.content，节点提供字体、布局和语义。相同数据可以同时投影到文本、颜色、可用状态等多个字段。

但 UI 还包含业务值之外的状态：布局容器、焦点范围、模态关系、输入法、文本选区、资源租约与无障碍树。Button 还表达激活意图，Input 产生带版本的编辑提案。因此建议模型为：

```text
共享结构与语义 + 实例数据 + 字段投影 + 交互命令 + 宿主状态
```

数据到 UI 的路径可以自动生成；UI 到数据的写入继续由明确事件/命令接受或拒绝。`value` 的显示绑定不应自动推断成双向赋值，尤其涉及解析失败、校验、IME、密码和选择控件时。

### 3.1 两种实例策略需要分开

**策略 A：保留每个语义节点身份。** DSL 自动产生 Plan 和绑定。它能省手工 mark/part 胶水，并更稳定地复用静态声明；现有每节点 Props、两份 Memory 和适配器记录仍在。适合先实现，运行时收益要与手写同等 Plan 比较。

**策略 B：增加受限的静态子树边界。** 内部不可独立 patch、查询、移动或挂起的成员，可研究仅保存声明与少量动态槽身份。这样更接近 Dioxus 的模板记录，但改变 Glory 当前每节点的能力契约。仅仅“内容没有绑定”不足以采用策略 B。

策略 B 必须先规定焦点、命中、事件冒泡、祖先内容检查、AX 导出、名字关系、样式状态、SSR/hydration/stream 和整个子树退休如何工作。可访问性需要的静态元素仍要出现在宿主语义树中。它属于单独的高风险研究项，不作为第一版 DSL 的隐含优化，也不自动升级静态成员以掩盖身份语义变化。

## 4 与 Dioxus 的具体差异

本轮阅读 D 盘 Cargo registry 的 `dioxus-rsx/core/web/interpreter-js 0.7.10` 发布源码，以及 `sledgehammer_bindgen_macro 0.6.5`。发布包 VCS 信息指向 `57d6794ad60b949e5bd8aa282f6f8c3dc97a365e`，至少 core 标记 dirty；GitHub 固定提交链接用于导航，发布包才是本轮实现对照的依据。

| 维度 | Dioxus 0.7.10 | 当前 Glory | 建议的 v2 |
|---|---|---|---|
| 前端 | RSX 宏识别静态树、动态节点和属性 | 手写 builders，显式 Plan | 可选类 XML 宏，自动生成声明和槽 |
| 静态结构 | 宏生成静态 Template roots 与动态路径 | 运行时一次 Plan 构造 | 先输出 Plan，再评估编译期只读声明 |
| 更新计算 | 相关组件重新运行，生成动态 VNode 数据，再 diff | 直接依赖驱动字段 Binding；结构单独协调 | 保留直接字段绑定 |
| 每实例表示 | VNode、动态数组和 mount 信息；静态后代不逐个建立同等记录 | 每个语义节点拥有活实例和完整身份 | 策略 A 保留；策略 B 另行论证 |
| 输出接口 | `WriteMutations`，Web 实现直接写编码器 | `Mutation` 批 → Mapper → Op/Edit → Sink | 语义层保持，Web 提交端直接写有限编码器 |
| JS 端 | 持有 nodes/templates/stack 并批量执行 | 已有 Plan 部件表和专用目标，仍逐目标过桥 | JS 持有物理表，按批执行、一次返回结果 |
| 初次模板注册 | 可在 Rust 创建原型并额外调用 JS 注册 | 文档内原型构造与有限预算 | 冷启动单独计量，不能声称始终仅一次调用 |
| 输出语义 | HTML 元素/属性等渲染操作 | 有限跨宿主语义、事件、查询与能力约束 | 保持共享语义，浏览器字节协议归 Web |
| 故障确认接口 | 所读 WriteMutations 方法返回 `()`，flush 无 Glory 式前缀结果 | Result、成功前缀、receipt、Faulted | 需独立设计前缀确认，不能直接照搬 flush |
| 生命周期 | 有组件 Scope、signal、动态 VNode 和回调 | Owner/Scope/Instance、任务、查询等 | 共享声明不共享实例捕获 |

Dioxus 的发布源码中，`TemplateBody::to_tokens` 生成静态 roots、node_paths/attr_paths 和动态数组；`VirtualDom::render_immediate` 处理脏 Scope 并写出 mutations。不能把“有静态模板”理解为“完全没有 Rust 实例”。模板的公开结构见 [Template](https://docs.rs/dioxus-core/0.7.10/dioxus_core/struct.Template.html)。

Web 的 `WriteMutations` 实现将常规修改写入 Interpreter；事件循环执行 `render_immediate` 后调用 `flush_edits`，最终由生成绑定的 flush 调用 JS `run()`。这一轮常规修改可以共用一次 run，首次模板注册和宿主辅助工作仍有额外调用。源码导航：[Web mutations](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/web/src/mutations.rs)、[Web loop](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/web/src/lib.rs)、[JS 绑定](https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/interpreter/src/unified_bindings.rs)、[WriteMutations](https://docs.rs/dioxus-core/0.7.10/dioxus_core/trait.WriteMutations.html)。

本版 Web loop 的 RAF 等待代码被注释，不能描述成“Dioxus 每个屏幕刷新必定只执行一次”。应比较一次渲染提交的工作，另外计量浏览器帧和显示延迟。

Glory 可以采用模板和批量宿主执行，同时保持不同于 Dioxus 的直接字段依赖。原有对比中的父组件全表重算是某个应用组织的成本，行级组件可以缩小它；不能把它写成 Dioxus 每次更新的必然行为。参见[持仓场景对比](portfolio-scenarios-benchmark-2026-10-25.md)。

## 5 一次刷新收集所有修改交给 JS 是否可行

**可以。建议目标是一次无须返回 Rust 获取中间结果的提交段，共用一次 JS 执行入口。** 常规无查询、未超预算的刷新可以只有一包。存在查询、宿主结果依赖或大批量预算时，允许有序分包。

需要区分四件事：

1. 多次数据写入合并调度：Glory 已有 runtime batch/flush。
2. 多个语义 mutation 一次交给 Renderer：当前 commit 已做到。
3. 多个物理修改一次过 WASM→JS：当前仅有局部专用批，适合继续研究。
4. 减少真实 DOM 工作：批量挂接、字段初始化和节点表示，需要另外设计。

把一个包含千项修改的包送入 JS，里面通常仍有千项以上 DOM 操作。按顺序执行的批次也不承诺整批原子回滚，或一次浏览器布局；某些宿主读取仍可能触发布局。

### 5.1 建议的流水线

```mermaid
flowchart LR
    A[Cage Bond 更新] --> B[结构协调与字段 Binding]
    B --> C[语义 Mutation 批]
    C --> D[Mapper 顺序验证与纯映射]
    D --> E[有限浏览器操作编码器]
    E --> F[JS 执行一包]
    F --> G[返回完成边界与错误]
    G --> H[Rust 确认前缀和宿主状态]
```

使用已完成语义映射的有限物理操作：创建/克隆声明、写字段、受控 property、放置或移动、挂起/恢复和退休。JS 不解释 Cage、业务表达式或组件树，语义到 HTML 的规则继续只有 Mapper 一份。

这确实需要一个**有限宿主指令解释器**，应该明确承认这项复杂度。其范围不含业务求值、任意函数、分支语言或通用模板 VM。协议与生成 JS 按同一次构建配对，优先保持为 Web 的私有实现。

只给当前 Sink 加一个“先保存 Op、最后执行”的包装还不够：当前 Mapper 在 sink 成功后推进 receipt，DOM 执行还更新 Entry、父关系和输入状态。延迟执行要同时解决这些状态的确认时机。

### 5.2 包结构与 JS 物理表

建议包中保留操作码、声明/实例/部件地址、参数区、字符串区及**语义 mutation 结束标记**。地址引用当前物理实例；完整 NodeId 的 runtime/generation 验证仍归 Rust，退休后不能让迟到命令操作复用后的槽。

从编码器直接输出紧凑记录，复用当前提交缓冲，避免先堆叠多份 Mutation/Op/JSON/JS 对象。第一版先固定原始顺序，不做跨操作去重、排序或最终值折叠。有限且稳定的属性/事件名可以编码为枚举；动态用户值不能进入长期 intern 表。

JS 继续拥有原始 Node 引用和声明的部件索引。Rust 平常发数字地址，真正查询或必要宿主逻辑才取回对象。部分退休及时清空对应槽；全实例退休释放整表。不能为了复用 ID 保留离树控件、输入值或访问过的行。

原型、组范围、普通创建和 hydration 节点都需要覆盖。仅优化已有 Plan 的 Text/Class，无法把整个刷新缩到一个宿主入口。冷原型注册、fallback、事件兴趣、动作和 Web 能力驱动的额外调用，应单独计数。

### 5.3 顺序屏障不都意味着拆包

| 情形 | 同包可行性 | 必须保留的行为 |
|---|---|---|
| 文本、class、style、普通属性 | 可以 | 顺序、Clear/Keep、转义和校验 |
| value/checked/revision/selection | 可以作为专用操作同包执行 | 受控属性屏障、版本、composition 与选区保护；禁止按普通属性折叠 |
| 创建后设置字段再挂接 | 可以 | 离线初始化、先创建再引用；控件初始化语义保持 |
| 已连接节点移动 | 可以，需完整实现 | 当前 focus/selection/IME 保留和 moveBefore fallback |
| Group 范围、Suspend/Resume、Dispose | 可以，复杂度较高 | 多物理部件、停放、独立挂起后代、部分退休 |
| Query 或后续 Rust 计算依赖宿主结果 | 当前语义下先提交前段 | 返回对应 watermark；不得用批尾状态回答批中查询 |
| Focus、滚动等宿主动作用于后续读取 | 可设计专用有序段 | 保持动作在 mutation 后、query 前；如需结果则返回确认 |
| JS 同步回到 WASM | 必须防重入污染 | 只排入后续输入，当前包保持独占且可继续读取 |
| 大包、巨大单字段或多次稳定化 | 允许分包/标量路径 | 明确字节、记录和对象上限；不缩小业务支持范围 |

受控 Property 是**不可重排的顺序屏障**，并不天然要求另一趟 JS 调用。查询是**观察屏障**，后续业务如果依赖其结果，就无法承诺整次交互始终只有一次调用。一次 Runtime flush、一次 commit、一次 RAF 与一次屏幕显示也没有固定一一对应关系。

### 5.4 主要难点是错误与模型确认

正常流程很容易写成 `run(packet)`。困难在于 JS 完成第 k 个语义 mutation 后失败，以及 Rust 准备后面的 mutation 时发现语义错误。

必须记录最后完整完成的语义边界；单个 mutation 内发生的物理写入可能部分完成，App 按现有规则进入 Faulted，不宣称 DOM 回滚。若后面的语义错误已被发现，也必须先执行前面的合法包；若该包更早发生宿主错误，应保持原有错误先后关系。

当前 Mapper 的 Memory 会在逐操作映射时更新。批末才收到 JS 结果时，不能把尚未物理执行的后缀当作成功提交。至少有两条候选路线：

- 为当前有限批保存触及字段/拓扑/登记的撤销信息，成功后保留前缀，失败时恢复未确认后缀。记录只能跟本次修改规模有关，不能 clone 全 Memory；创建、放置和退休的撤销远比 Text 字段复杂。
- 在本次提交的增量暂存层准备操作，JS 返回前缀后再将确认部分写入 Mapper。暂存层需要支持顺序读取此前变化，也可能带来二次回放成本；最终 UI Memory 不能直接代替中间历史状态。

两条路线都需要原型和测量。仅在成功时推进 receipt、却让 Mapper 保留未来后缀，不能满足当前独立 direct apply 和查询契约。改为故障后丢弃整份模型会改变既有契约，应作为明确的 v2 取舍，不能隐式引入。

JS 执行期间还可能触发 WASM memory.grow。包应在首次可重入宿主调用前形成有界 JS 自有快照，或有经验证的内存视图更新方案；不能跨可能增长内存的回调继续读取旧的 TypedArray/DataView。原生监听器继续只排队，业务回调不得重入借用中的文档。

## 6 为什么旧批量字段实验不足以否定整批协议

[2026 年 10 月 26 日实验](field-submit-plan-2026-10-26.md)只打包连续 Text/Label/Classes，最多 128 个 mutation；创建、放置、退休和受控属性仍在原路径。它已经验证前缀日志和 memory.grow 场景，但不是完整宿主批。

该轮独立计数中，千行创建 WASM→JS 从 5,034 降至 3,065。第二版同路径 A/B 的创建千行和万行约改善 1.5%，替换回退 4.4%、选行回退 23.6%、清空回退 13.9%，因此撤回。清空并未使用字段包却仍变慢，报告没有证明单一因果。

这些结果说明：剩余主要结构工作没有被字段包覆盖，编码、日志和临时分配也可能抵消收益。它们既不能证明完整协议必然失败，也不能支持直接恢复旧实验。Dioxus 的优势同时包含静态实例表示、缓冲直写、宿主对象表、批量结构操作和回收策略。

原 Dioxus 性能分析中的 19,033 次 Glory 调用属于较早快照。后续 Plan 宿主目标已把对应独立计数降至约 5,034；这些不同快照的计数不能拼成当前的新性能比。参见[早期 Dioxus 分析](dioxus-performance-analysis-2026-10-25.md)、[Plan 宿主目标](plan-host-goals-2026-10-26.md)。

## 7 建议的验证顺序

1. **先固定语义与表示。** 明确一次性输入、绑定、事件、结构槽和静态声明边界。用手写 Plan 表达候选 DSL 的展开结果，确认不需要更换响应式模型。语法试验与运行时改造分别留变更。
2. **优先验证 Web 整批宿主提交。** 先以重复行创建/初始化/挂接/退休为完整窄场景，保留原顺序与完整 NodeId；未覆盖操作使用有序 fallback。先定义前缀与暂存模型，再扩大指令范围。协议可以由现有 builders/Plan 使用，不等待 DSL。
3. **增加可选编译前端。** 对固定语义标签、类型化组件 Props、字面量、绑定和事件做小范围实现，生成现有结构。第一版只接受可诊断的明确语法，避免运行时 XML 和全项目扫描。
4. **单独评估静态子树身份压缩。** 只有完整契约、跨宿主语义与收益成立后，再决定是否成为 v2 的新构造单元。不要将这项风险捆绑到语法实现。

### 7.1 如何分离各项收益

| 对照 | 只改变什么 | 能回答的问题 |
|---|---|---|
| A 现有手写 Plan | 基线 | 当前调用、实例与更新成本 |
| B DSL 生成等价 Plan | 前端 | 语法和自动提取是否引入额外成本 |
| C A 使用整批宿主协议 | 提交后段 | 在完整身份与相同图粒度下，批量执行是否值得 |
| D B 使用同一整批协议 | 前端与协议组合 | 两者是否有额外耦合成本 |
| E 受限静态子树候选 | 实例能力与表示 | 记录压缩的收益和能力代价，独立报告 |
| Dioxus 0.7.10 两种组件组织 | 已锁版本，父列表与行级组件 | 区分组件组织、模板和宿主执行的差异 |

创建、替换、追加、清空、稀疏更新、密集更新、选择和重排都要测；冷模板构造/注册必须纳入冷启动数据。单字段低延迟与大批吞吐分别评价。计数要覆盖两个方向的 ABI、字符串字节、DOM 方法、图/Owner/节点数与释放后对象引用；调用次数不替代时间或 RSS。

正确性门槛包括：编译错误定位、未知标签/字段/事件、空白和 Unicode、内容关系、跨实例隔离、Owner 清理、keyed 身份、查询水位、晚语义错误、包内宿主失败、memory.grow、受控输入/selection/IME、Group/部分退休、SSR/hydration/stream 和原生 fallback。纯语法试验也要与等价手写构造进行语义对照。

所有未来构建、Cargo home、临时文件和测量产物使用 `D:/BuildCache/glory-v2-*`，设置 CARGO_TARGET_DIR 后由 cargo metadata 核对实际路径。独立构建、交错顺序配对、原始慢样本和失败记录沿用现有项目标准。

**优先方向：自动提取共享声明与字段投影，同时研究覆盖结构操作的整批浏览器提交。** 这样可以保留 Glory 的细粒度更新，并具体检验 Dioxus 在模板实例和宿主执行上的优势；静态成员身份压缩留作独立的 v2 架构决策。
