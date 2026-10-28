# v2 显示 API、DSL 与性能改进实施计划

日期：2026-10-28；实施验证延续至 2026-10-29。M0–M3、M5 的公共 API 已实现，
M4 的性能验收单独记录。可运行接口以[当前指南](v2-display-keyed-content-guide-2026-10-28.md)
及源码为准；下文保留最初的目标草案与退出条件。

最终实现、撤回实验和两套完整测量见[交付记录](v2-display-completion-results-2026-10-29.md)。
M4 的剖析与测量已执行，性能目标未全部达到，保留所有回退和剩余成本。

## 完成目标

先把当前受限浏览器显示层做成可以正常写应用的 API，再根据实际剖析优化性能。
本轮完成线是：用户只写业务状态、六个操作和 keyed XML 行模板；列表协调、行 Scope、
事件退休与宿主挂接由框架管理。手写 API 具有同等能力，DSL 只负责静态提取和调用该 API。

继续保留 Cage/Bond/Vary、共享静态模板和稀疏动态实例。小写 HTML 不创建高层 Widget，
也不恢复逐 HTML 元素的完整语义 Memory/Entry。高层组件可以拥有自己的状态和 Scope。

“写得短”和“运行得快”分别验收：减少应用胶水不能代替性能测量；降低图节点数也不能代替计时。
浏览器这一轮完成后，再扩展 HTML/事件和平台支持；不把原型测量当作编辑器、SSR 或 native 已完成。

## 已核实的起点

- [当前主应用](../benchmarks/glory/src/main.rs)已经使用新 DSL，但还手写 `Rows: RowHost`、
  `Cache`、`MountedRow`、`WeakApp`、行 Scope、label HashMap、cleanup 和挂接。
- [现有 keyed Cache](../crates/ui/src/list/cache.rs)可以复用协调算法，包括重复键校验、
  顺序保持和 LIS 移动。旧 `ui::Each` 的完整语义 Widget 路径不能直接充当新显示层的实现。
- [`Cage<Vec<T>>::item`](../crates/core/src/entries.rs)跟随下标，交换后指向该位置的新数据；
  map `entry(key)` 跟随完整键。直接把 `item(index)` 包装成稳定行句柄会出错。
- [`Cage::field`](../crates/core/src/fields.rs)已经支持原数据上的字段投影。
  字段写会通知整模型读者和祖先；列表协调若订阅整个数据集合，仍可能因标签更新扫描全表。
- Cage 是 Owner 所有的 Copy 句柄。把标签 Cage 全部创建在应用根下面，然后删除 Vec 元素，
  并不会自动释放这些 Cage。不能靠消除 benchmark 的显式 cleanup 掩盖生命周期问题。
- 当前 `IntoView::mount` 返回一个 `Mounted`，实例和 Scope 还存在一对一约束；
  `{expr}` 目前走文本值，不能直接组合任意子视图。空内容、多根内容、列表及条件需要明确范围协议。

与同仓锁定版本的比较：

| 实现 | 简洁性的来源 | Glory 借鉴的边界 |
|---|---|---|
| [Dioxus fixture](../benchmarks/dioxus/src/main.rs) | 组件中写普通数据操作，RSX keyed `for` 交给运行时协调 | 静态提取、循环身份与动态位置由框架处理 |
| [Leptos fixture](../benchmarks/leptos/src/main.rs) | `<For>` 管结构；行 `ArcRwSignal` 管局部更新及所有权 | 细粒度更新可以同时有简洁应用 API；生命周期应由公共抽象承接 |
| Glory 当前 fixture | 模板与字段已经稀疏化，结构及行数据所有权仍由应用适配 | 先补结构、数据和生命周期契约，再补语法糖 |

借鉴这些分层，不引入另一套 Signal 引擎，也不通过分析任意 Rust 函数体猜测完整 UI 语义。
此前宏源码对照版本与来源见[源码审计](v2-dsl-source-audit-2026-10-28.json)。

## 目标 API 草案

优先提供框架维护的 keyed 有序集合。内部仍以 Cage 及其投影工作；用户不再管理第二份 label 表。
`KeyedList` 是暂定名称。以下片段只展示 API 形状，省略构造、错误处理和部分按钮：

```rust,ignore
let rows = scope.keyed_list(Vec::<Row>::new(), |row| row.id);
let selected = scope.cage(None::<usize>);

rows.replace(generator.rows(1000))?;
rows.extend(generator.rows(1000))?;
for index in (0..rows.len_untracked()).step_by(10) {
    rows.update_at(index, |row| row.label.push_str(" !!!"))?;
}
rows.swap(1, 998)?;
rows.remove_key(id)?;
rows.clear()?;
```

手写 API 先支持同一列表内容，再做 XML：

```rust,ignore
view(&PANEL).child(BODY, each(rows, move |row| {
    let id = row.key();
    let label = row.field("label", |r| &r.label, |r| &mut r.label);
    view(&ROW)
        .bind(ID, id)
        .bind(LABEL, label)
        .watch(CLASS, move || selected.get() == Some(id))
        .on_click(SELECT, move || selected.revise(|v| *v = Some(id)))
        .on_click(REMOVE, move || rows.remove_key(id).unwrap())
}))
```

上述 CLASS 是示意槽，实际 benchmark 保留原有完整 class 字符串和 DOM 契约。
`row.field` 返回现有 Cage 类型的投影，不把字段值复制到另一个长期存活的模型。
`update_at` 明确表示整行编辑，允许通知该行的字段读者；直接写字段 Cage 才要求只通知其字段分支。
两者都不应使列表顺序观察者执行。

XML 使用同一 `each`，采用显式 `<Each>` 内容工厂。与已有 `ui::Each` 保持列表概念一致，
通过 `display`/`ui` 命名空间区分宿主；不增加内置 `For` 别名：

```rust,ignore
view! {
    <div class="bench-body">
        <Each each={rows} children={move |row| {
            let id = row.key();
            let label = row.field("label", |r| &r.label, |r| &mut r.label);
            view! {
                <div role="row">
                    <span>{id}</span>
                    <button on:click={move || selected.revise(|v| *v = Some(id))}>
                        {label}
                    </button>
                </div>
            }
        }}/>
    </div>
}
```

完整 benchmark 继续保留共同的行结构、六个按钮和九项操作，不用这个缩略模板替代共同 workload。
`Each` 是明确的结构构造，不要求用户手写 Props/Scope，也不创建额外包装元素。
后续可增加 `field!(row.label)`，只生成固定字段名和两条访问器；不承担任意表达式的数据流推断。
直接传 Cage、传一次性值与传计算闭包的区别保持现有语义。

## 实施顺序与退出条件

顺序为 **M0 → M1 → M2 → M3 → M4 → M5**。M1/M2 先通过手写最小应用，M3 再开放 DSL。
每阶段保留可单独审查的改动及验证记录；阶段没有通过就不宣称后续能力完成。

### M0：冻结当前 DSL 基线，定位成本

冻结当前实际工作区、依赖锁、构建命令、产物 hash、Chrome 和运行环境，不能只记录远端 `v2` SHA。
现有工作区包含未提交的运行时和 DSL 改动；历史旧语义入口继续作为参考，而非本轮唯一 before。

重新建立四个独立产物：当前 DSL、候选 DSL、候选手写、同版本 Dioxus；Leptos 作为同批辅助对照。
先确保共享 oracle、布局和后代身份一致，再计时。剖析万行创建、清空、替换和 selected 广播：

| 成本段 | 需要区分的工作 |
|---|---|
| Rust 创建与协调 | key 查找/扫描、Scope/Owner、行字段、Binding、槽解析和实例记录 |
| 编码与提交 | 字符串/缓冲区分配、WASM→JS 调用次数、分包、前缀确认与退休索引 |
| JS/DOM | 模板登记/clone、目标定位、写字段、移动和删除、事件元数据 |
| 浏览器 | 样式、布局、GC 和两 RAF wall 等待 |

现有工具不能分离的成本只在诊断产物增加固定聚合计数/采样，正式计时产物关闭诊断。
保留未经插桩的端到端结果，不能从 inclusive 段时间相加得到总时间。

退出条件：能用原始证据说明剩余创建/清空差距主要出现在哪个阶段；记录尚不能归因的部分。
本阶段不预设“过桥次数”或“响应图节点”仍是最大瓶颈。

### M1：keyed 数据与行字段所有权

先实现纯 core 合同及少量有针对性的测试，再接显示层：

1. 数据只有一份权威值。顺序/成员变化和行内容变化使用独立通知通道。
   优先验证“顺序 token + keyed 值存储 + 现有 Cage 投影”，复用现有 map 路径和集合选择性发布。
   它属于集合存储抽象，不改写整个 core 引擎；存储元数据不能成为业务数据的第二份副本。
2. 行身份为业务 key 加不复用的本次成员代次。移动保留身份；同 key 的原位值替换保留身份并更新字段；
   删除后重新插入获得新代次，旧行句柄不能误写新数据。不同 key 的 hash 碰撞必须正确区分。
   key 提取必须纯且在成员存活期间稳定；初版不支持通过普通字段编辑偷偷更换身份。
3. `.field` 投影由实际使用它的 Owner 拥有；删除成员使旧句柄失效，卸载一个视图只释放该视图的投影。
   数据可被多个视图观察，其生存期不取决于某个 DOM 行是否挂载。
4. `update_at`/`update_key` 的临时业务编辑不在根 Owner 下注册永久投影，不能留下所有历史 key 的别名。
   为此如需 core 增补，只增加可复用的 keyed 编辑原语；不得改变现有 `Cage<Vec<_>>::item` 语义。
5. replace/extend 的全部 key 检查先于可见变更；重复键失败不提交半个列表。
   在运行时 batch 中发布关联变化；借用冲突、外来/退休句柄和用户回调失败沿用明确的 core 错误边界。
   不把 batch 宣称为任意业务 panic 的自动回滚事务。

退出条件：交换后写原行仍命中原 key；删除/重插的旧句柄被拒绝；局部行编辑不触发顺序观察者；
单字段写不重算兄弟字段；清空与反复替换后，活节点/投影/Owner 回到相应数据和视图基线。
测试覆盖双视图、卸载后数据仍可编辑、相同 key 值替换、重复 key、真实 hash 碰撞及批量编辑。

### M2：公共列表与子内容挂载 API

提供 `.child(outlet, content)` 和 `each(source, factory)` 的手写入口，框架内部适配 keyed Cache。
应用不再实现 RowHost。源提供稳定 token 和借用/索引访问；普通字段更新不重新扫描或复制全表。
工厂只在成员首次挂载时运行；同 key 值替换通过已有行字段通知，不能遗留首次 mount 的 seed 值。

把空内容、单根、多根的挂载/移动/退休表示为真实范围。范围可以有必要的端点，
不能为每个静态 HTML 节点增加 Scope，也不能为了统一返回值增加可见 div。
在扩展 `IntoView`/`Mounted` 之前先明确范围与父实例/Owner 的关联，覆盖当前“一 Scope 一实例”约束。

列表控制器负责行子 Scope、事件、Binding 和 mounted range 的释放；使用非拥有的 generational 上下文
创建/进入子 Scope，不能把强 Scope/App 捕获进其自身拥有的 observer，形成循环所有权。
如需公开 core 的非拥有子 Scope 工厂，应校验 runtime、generation 和父 Owner 后才能执行用户代码。

退出条件：手写完整 rows 应用没有 RowHost/Cache/WeakApp/手动 place/dispose/labels 表；
保持存活后代 DOM 身份和最少必要移动，焦点/选区与受控提交的现有错误前缀语义继续通过。
新增覆盖空→多根、全部删除、连续重排、两个列表位置、组件行及 mount/提交失败后的关闭清理。
结构 API 的错误不得在 observer 内被静默忽略或只靠应用 `.unwrap()` 管理。

### M3：DSL 结构控制、组合和字段简写

XML 编译到 M2 的公共 API，而不是另做一套列表协调。提供 `<Each>`、`<Show>`、
普通子 View/片段组合和组件 children。先定义条件分支的状态规则：离开时退休，重新进入创建新状态；
保活属于未来的显式能力。兄弟组件的状态不随该分支切换重置。

`{text}`、`{cage}` 和计算文本延续当前行为；视图/可选视图/序列走内容契约。
对文本闭包与视图工厂使用明确的类型或结构入口，避免 Rust trait 重叠和在宏中猜测任意返回类型。
借助显式 `Component::Props` 保留 Rust 类型检查；增加 children 工厂的生命周期及调用次数约定。
`field!` 先只支持固定的具名 struct 字段，与手写 `.field` 生成相同投影。

退出条件：DSL 与手写输出、事件、身份、图和所有权一致；错误标签、Props、children、
字段及内容类型有源码位置明确的下游诊断；重命名依赖仍可用，默认 runtime 依赖图不引入宏编译器。
全部上述新能力通过真实浏览器验收，不能只编译宏展开。

### M4：主 benchmark 精简与有证据的优化

主应用只留下业务状态、六个操作及 XML 视图，普通源码中不再出现前述框架维护责任。
手写前端放在独立文件；两种前端共享一份业务数据/操作，不复制另一套 workload。
统计和故障导出放 acceptance 模块，正式计时不编入探针。独立 html-template 应用继续保持移除状态。

先对 M1–M3 的功能改动单独测一次，区分抽象成本和随后优化收益。
再依据 M0/新剖析逐项改变实例/Scope、槽解析、分配、协调、编码或宿主执行，
每次保留失败实验及数据；不把多个未验证优化混在一个性能结论里。

测量使用独立 release 产物、同一布局/数据/事件 oracle、fresh browser、正反顺序四轮，
每操作保留 3 次热身和 12 个正式样本；四个主版本共 1728 个正式样本。
若 Leptos 同跑则单独增加其样本，报告所有慢样本、每轮中位数、合并中位数和范围。
报告 Script、wall、WASM/JS、活图/Owner/实例/目标、提交包数及分配/GC 证据，不拼接不同批次排名。

退出条件分两层：

- API 完成：新应用通过全部正确性/生命周期验收，手写与 DSL 同一数据语义；没有用全表重跑替代字段绑定。
- 性能完成：创建/替换/追加/清空有可复现收益，局部更新没有持续回退；重点追赶同批 Dioxus 的创建与清空。
  预设工程目标是创建千行、创建万行、清空的 Script 中位数进入 Dioxus 的 1.10 倍以内，
  且不牺牲当前细粒度更新收益。这是目标，不是目前已经达到或能保证达到的数字。

相对当前 DSL，任一操作 Script 回退超过 5% 且至少三轮方向一致，就触发专项剖析并记录取舍，
不将其视为统计显著性检验。即便低于该阈值，也报告所有结果和每轮波动。
若目标尚未达到，则保留已验收 API，明确列出剩余成本，继续针对该段优化；不能宣布性能工作完成。

### M5：日常组件体验与交付

在 Props/children/所有权合同稳定后，增加可选 `#[component]` 和简单 Props 生成，
减少现有 struct + Props + impl 样板；普通 Rust 函数仍可返回视图，不要求所有组合都创建组件 Scope。
提供简洁根挂载、统一 display prelude、计数器/条件/列表/嵌套组件示例和迁移说明。
手写、XML、组件三种用法都说明值、Cage/Bond/Vary 与计算闭包的区别。

退出条件：示例没有手动生命周期胶水，下游编译诊断及 feature 组合通过；运行相关 Rust 测试、
Native/WASM Clippy 与浏览器 gate，记录现有无关 gate 失败，不把基线失败改称本轮通过。
刷新文档和验证摘要，保留每阶段源码快照与原始结果。

## 代码落点

| 部位 | 主要改动 |
|---|---|
| `crates/core` | keyed 集合合同、选择性编辑、字段/成员生命周期；必要的非拥有上下文原语 |
| `crates/web/src/view.rs` 及新的结构模块 | 内容/范围协议、each/show/children、列表拥有关系 |
| `crates/html/src/view.rs`、`crates/web/src/template*` | 必要范围表示和宿主确认；继续封闭 Source、校验 profile |
| `crates/ui/src/list/cache.rs` | 复用协调；若需抽取到共用层，保持旧语义路径测试及行为 |
| `crates/view-macros`、`crates/glory` | 公共 API 的 XML 降低、字段及组件宏、可选 feature 和 reexport |
| `benchmarks/glory`、驱动和验收 | 一份模型、独立前端、基线/候选/对照测量、正式产物无探针 |
| `examples/display-web`、下游诊断脚本与 docs | 用户级示例、组合/生命周期验收、能力边界和迁移 |

## 随后的平台工作

以上六阶段完成的是受限浏览器显示层的日常编程能力和本 workload 的性能改进。
完整 HTML 标签/属性/property、类型化事件、ref、受控输入/IME/选区、SSR/hydration、
Blitz HTML/CSS 映射和高层 Widget 桥接另立能力矩阵及平台验收；不能以编译成功或本 benchmark 更快代替。
结构范围及数据身份设计要允许这些扩展，但不把未验证平台能力塞入首轮完成声明。

已有证据：[API/DSL 首版](v2-view-api-and-xml-dsl-2026-10-28.md)、
[主 benchmark 迁移与当前性能](benchmark-display-dsl-migration-2026-10-28.md)、
[新显示层架构](v2-html-template-runtime-architecture-2026-10-28.md)。
实施已增加 KeyedList/Each/Show/Content/children、字段与组件宏、布尔 class 绑定、
display prelude 和普通示例，并更新两套 benchmark。实际数据存储采用 generational Arena；
第一次 HashMap 存储试验、直接字段及 Arena 阶段的独立测量均保留。
原计划中的性能目标继续作为目标核验，不能用 API 已完成代替性能已达标。
