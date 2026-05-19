# Glory-rs vs Dioxus 架构对比与改进建议

> 对比对象:`D:\Works\glory-rs\glory`(本仓库,branch `chore/upgrade-dependencies`)与 `E:\Repos\dioxus`(主干)。
> 重点:**响应式数据模型**、**多平台发布**、**渲染引擎可替换性**。本报告刻意跳过 DSL/RSX 相关设计。

---

## 0. TL;DR

- Glory 的响应式系统(`Cage` / `Bond` / `Lotus`)是一套**显式追踪 + 版本号 + 视图绑定**的细粒度信号模型,**思想上更接近 SolidJS / Leptos**,实际实现用 `Rc<RefCell<…>>` 包了一层,API 稳定但每个状态都伴随 3–4 个引用计数对象。
- Dioxus 的状态层是**信号 + 代际盒(generational-box)**,所有 `Signal<T>` 是 `Copy` 的;真正的数据放在一块共享 arena 里,通过"代际号"做悬空检查。这让信号既能跨闭包共享又免去 `Rc::clone()`。
- Glory 的渲染层是**直接绑定到 `web_sys::Element`**,通过 `cfg` 在 CSR / SSR 间切换 `Node` 的类型。**没有"渲染器 trait"**,因此移植到新平台等于在 `web/` 旁边再写一份完整组件库。
- Dioxus 的渲染层把组件树 diff 成一串 `WriteMutations` 指令,每个平台只要实现这个 trait(Web / Desktop / Native / SSR / LiveView 全部共用一份 `VirtualDom` 内核)。
- 想做"多平台发布"和"未来支持别的渲染引擎",**glory 当前最大的结构性短板**是缺一个**渲染目标抽象**(类似 `WriteMutations`)和一个**统一的应用入口/启动器**(类似 dioxus 的 `launch`)。

---

## 1. 响应式数据模型(重点)

### 1.1 Glory:`Cage` / `Bond` / `Lotus` + 视图反向引用

核心位于 [crates/core/src/reflow/](crates/core/src/reflow/mod.rs)。

- **`Cage<T>`**:可写状态容器。内部 `Rc<RefCell<T>>` + `Rc<Cell<usize>>`(版本号)+ `Rc<RefCell<IndexSet<ViewId>>>`(绑定的视图)。
- **`Bond<T>`**:派生/计算值。记录构造时通过 `gather()` 收集到的依赖快照,读取时把依赖版本号求和与上次比,变了就重跑 mapper。
- **`Lotus<T>`**:把 `Bare` / `Cage` / `Bond` 统一为同一个枚举,便于到处接受"任意只读响应值"。
- **`TRACKING_STACK`**:thread-local 的 `Vec<IndexMap<RevisableId, Box<dyn Revisable>>>`。每进入一次 `gather(|| …)` 推一层,内部任何 `.get()` 都把自己 `track(item.clone_boxed())` 进当前层。
- **调度器**([reflow/scheduler.rs](crates/core/src/reflow/scheduler.rs)):`Cage::revise` → `signal()` → `REVISING_ITEMS` 入队 → `schedule()` → 遍历每个 revising item 的 `view_ids`,对每个视图调 `widget.patch(&mut scope)`。`batch` / `RUNNING` / `UNTRACKING` 都是 thread-local 标志。

关键代码片段(节选自 [bond.rs:44-67](crates/core/src/reflow/bond.rs)):

```rust
pub fn get(&self) -> Ref<'_, T> {
    let new_version = self.gathers.borrow().values().map(|g| g.version()).sum();
    if self.version() != new_version {
        *self.gathers.borrow_mut() =
            crate::reflow::gather(|| self.value.replace((self.mapper)())).0;
        self.version.set(new_version);
        for view_id in self.view_ids.borrow().keys() {
            for (_, gather) in self.gathers.borrow().deref() {
                gather.bind_view(view_id);
            }
        }
    } else { /* 把依赖透传到外层 TRACKING_STACK */ }
    self.value.borrow()
}
```

**特点 / 优点**
- **细粒度**:patch 只走那些真正被订阅的 view,不重渲整个组件子树。
- **思路清晰**:`Revisable::view_ids()` 把"哪个状态影响哪些 DOM 视图"显式存下来,debug 时极方便。
- **支持多实例**(`single-app` off):通过 `HolderId` 隔离调度,可在同一进程跑多个独立应用。

**问题**
1. **不是 `Copy`**。每次跨闭包都得手动 `.clone()`(counter 示例里就有 `let value = self.value.clone();`)。多写一层样板代码,且每个状态本身已经持有 4 个 `Rc`。
2. **`!Send + !Sync`**。`Rc` + `RefCell` 决定了状态不能跨线程,这在 wasm 单线程下没问题,但**走 SSR / fullstack / 桌面端 tokio runtime 都会扎手**。
3. **没有真正的 effect 原语**。`Bond` 是惰性 memo;想做"副作用、外部资源、`use_resource`"得自己写 widget。
4. **`Bond` 版本号用 sum** —— 多个依赖版本号叠加成同一个 `usize`,理论上可能巧合相等(虽然实际很难触发,但语义上脆弱)。
5. **重入限制**:`gather()` 内部 `RefCell` 借用,如果 mapper 中又写了同一个 `Cage`,在 dev 会 panic。
6. **生命周期靠引用计数**。状态被任意 `View` 持续 `bind_view` 后,只有显式 `unbind_view` 才能释放;没有"作用域析构 → 一次性回收"。

### 1.2 Dioxus:`Signal<T, S>` + `CopyValue` + `generational-box`

参考 [packages/signals/src/signal.rs](../../E:/Repos/dioxus/packages/signals/src/signal.rs) 和 [packages/generational-box/src/lib.rs](../../E:/Repos/dioxus/packages/generational-box/src/lib.rs)。

```rust
pub struct Signal<T, S: 'static = UnsyncStorage> {
    pub(crate) inner: CopyValue<SignalData<T>, S>,  // 只是一个 (raw_ptr, generation)
}
pub struct SignalData<T> {
    pub(crate) subscribers: Arc<Mutex<HashSet<ReactiveContext>>>,
    pub(crate) value: T,
}
```

要点:

- **`Copy` 信号**:`Signal<T>` 本体只是一个"句柄"(指针 + 代际号),所以可以 `Copy`、可以直接放进闭包,**用法接近 React hooks**。
- **`generational-box` arena**:真正的 `T` 存在 arena 里;`Owner`(通常是 `ScopeId`)负责销毁。销毁后代际号失配,之后访问就返回 borrow error,不会 UAF,也不会拖延释放。
- **`UnsyncStorage` / `SyncStorage`**:同一份 API,选不同的存储后端。`Sync` 用 `parking_lot::Mutex` + 原子代际,适合服务端 / fullstack / 多线程渲染。
- **`ReactiveContext`**:当前订阅者(通常是某个 scope 或某个 effect)。`Signal::read` 时把自己加入 `subscribers`,`Signal::write` 时遍历 `subscribers` 通知它们标脏。
- **`Memo` / `Effect` / `use_resource`**:都通过 `ReactiveContext` + `Runtime` 调度;副作用是一等公民。

### 1.3 头对头对比

| 维度 | Glory(Cage/Bond/Lotus) | Dioxus(Signal/Memo) |
|---|---|---|
| 句柄 `Copy` | 否(必须 `.clone()`) | **是** |
| 后端存储 | `Rc<RefCell<T>>` per 状态 | 全局 arena + 代际号 |
| 线程支持 | 单线程(`Rc`) | `UnsyncStorage` / `SyncStorage` 二选一 |
| 依赖追踪 | thread-local `TRACKING_STACK` + 版本号求和 | thread-local `ReactiveContext` + 订阅集合 |
| 副作用原语 | 无 `Effect`、无 `Resource` | `use_effect` / `use_resource` / `use_future` 一等公民 |
| 派生值更新策略 | 拉(读时 `version != new_version` 才重算) | 推(订阅链路通知,标脏) |
| 内存释放 | 显式 `unbind_view` | scope drop 时 owner drop → 一次性失效 |
| 跨闭包写法 | 每个闭包前先 `clone` | 直接 `move`,语法干净 |
| 与 async 集成 | 需要手写 `spawn` | `tokio::spawn` + signal 自然结合 |

### 1.4 给 Glory 的具体建议(数据模型)

按收益从高到低:

1. **引入"作用域 + 代际盒"语义**。即便不引入 dioxus 的 generational-box crate 本身,也可以用一个 `slab::Slab<Box<dyn Any>>` + `Generation: u64` 自己实现。这一步是后续所有改进的地基。
   - 让 `Cage` 内部的 `Rc<RefCell<T>>` 替换为 `Handle<T> = (slab_key, generation)`,使 `Cage<T>: Copy`。
   - 副产品:同一份 API 可以加 `SyncCage`(用 `RwLock` + atomic),为 SSR 多线程做铺垫。

2. **添加 `Effect` / `Resource` 原语**。当前所有副作用都得塞进 `Widget::build`,既无法和 async 任务配合,也不能在视图外驱动状态(比如 WebSocket / interval)。建议在 `reflow/` 下加:
   - `effect(|| { … })`:订阅依赖、自动重跑,scope drop 时停止。
   - `resource<T>(|| async { … }) -> Lotus<Option<T>>`:支持 SSR 中先在服务端跑、再随 HTML 一起序列化下来。

3. **`Bond` 的版本号策略改用 `(child_id, child_version)` 的散列**,避免 sum 碰撞。或者直接用"每个 Cage 全局递增的 epoch"。

4. **batch 自动化**。目前需要用户在多点写入前手写 `reflow::batch(|| …)`;可以让 `View::on_event` 这一类入口自动包一层 batch,降低出错率(Dioxus 的事件回调也是隐式 batch)。

5. **`untrack` API 暴露给用户层**。现在主要供框架内部使用,但写复杂派生逻辑时很常用,可以加上文档级 API。

---

## 2. 视图/组件层:`Widget`+`Scope` vs `VirtualDom`+`WriteMutations`

### 2.1 Glory 现状

- **`Widget` trait**([widget.rs](crates/core/src/widget.rs))提供 `build / attach / patch / detach / flood`。一个 `Widget` 通过 `show_in(&mut parent_scope)` 落到树里。
- **`Scope`** 是组件本地状态 + 子视图集合,持有 `parent_node / graff_node`(挂载点)和 `Truck`(共享上下文)。
- **`Node`** 是渲染层抽象:
  - `cfg(target_arch = "wasm32", feature = "web-csr")` 下 `Node = web_sys::Element`(直接绑死)。
  - 其余情况(SSR)下 `Node` 是一个自定义结构(name + attrs + classes + props + children + Rc<RefCell<…>>)。
- **`Widget` 与渲染目标的耦合**:`web/widgets/{div,button,…}` 这一层在 CSR 下直接调 `web_sys`,在 SSR 下生成内存 Node,通过 `cfg` 各写一份分支。

→ **结果**:同一段业务组件代码确实可以同时跑 CSR 和 SSR,因为业务代码只看到 `Div::new().class(...).show_in(ctx)` 这样的抽象;但**渲染目标只能是"DOM 或 HTML 字符串"**这两种形态。要再加一种(比如 Tauri webview、终端 TUI、Skia 渲染),必须再 `cfg` 一份。

### 2.2 Dioxus 的关键抽象

`VirtualDom::render_immediate(&mut impl WriteMutations)`,这一行就是 dioxus 渲染层架构的灵魂。`WriteMutations` trait 在 [packages/core/src/mutations.rs](../../E:/Repos/dioxus/packages/core/src/mutations.rs) 定义为一组指令(节选):

```rust
pub trait WriteMutations {
    fn append_children(&mut self, id: ElementId, m: usize);
    fn create_text_node(&mut self, value: &str, id: ElementId);
    fn load_template(&mut self, template: Template, index: usize, id: ElementId);
    fn replace_node_with(&mut self, id: ElementId, m: usize);
    fn set_attribute(&mut self, name: &'static str, ns: Option<&'static str>,
                     value: &AttributeValue, id: ElementId);
    fn create_event_listener(&mut self, name: &'static str, id: ElementId);
    fn remove_event_listener(&mut self, name: &'static str, id: ElementId);
    fn remove_node(&mut self, id: ElementId);
    // …
}
```

每个平台只要给出自己的 `WriteMutations` 实现:

- `packages/web` → `WebsysDom`,落到浏览器 DOM(通过 `interpreter` 的 sledgehammer 二进制协议)。
- `packages/desktop` → `WryQueue`,把同一份指令通过 WebSocket 发到 webview。
- `packages/liveview` → 把指令直接编码发到客户端浏览器。
- `packages/ssr` → 拼字符串。
- `packages/native` → 调 Blitz 排版引擎,走 GPU。

**核心收益**:增加一个平台等于"实现一个 trait + 写一个事件桥",**不需要碰 core / signals / 组件层**。

### 2.3 改进路径(给 Glory)

不动 DSL 的前提下,Glory 同样可以把渲染层换成"指令流 + 后端"。建议分两步走:

**Step A:抽象一个 `Renderer` trait,不必上 VDOM**

Glory 不走 VirtualDom 路线也行——它的细粒度信号本来就不需要 diff。但可以把"对 `Node` 的操作"反向变成 trait 方法:

```rust
pub trait Renderer {
    type Node: Clone;
    fn create_element(&mut self, tag: &str) -> Self::Node;
    fn create_text(&mut self, text: &str) -> Self::Node;
    fn set_attribute(&mut self, node: &Self::Node, name: &str, value: &str);
    fn set_class(&mut self, node: &Self::Node, classes: &BTreeSet<Cow<'static, str>>);
    fn append_child(&mut self, parent: &Self::Node, child: &Self::Node);
    fn remove_child(&mut self, parent: &Self::Node, child: &Self::Node);
    fn replace_child(&mut self, parent: &Self::Node, new: &Self::Node, old: &Self::Node);
    fn attach_event(&mut self, node: &Self::Node, event: &str,
                    handler: Box<dyn Fn(EventPayload)>);
}
```

然后:

- `WebRenderer`(CSR):`Node = web_sys::Element`,方法直接转发到现有 `web/csr.rs` 的实现。
- `SsrRenderer`:`Node = node::ssr::Node`,方法操作内存树,最后 `to_html_string()`。
- `NativeRenderer`(将来):`Node = SomeWidgetHandle`,接 Blitz / Skia / wgpu。
- `TauriRenderer`(将来):`Node = u32 id`,把指令 JSON/二进制发到 webview。

**关键**:这一步**会让现有 `web/widgets/*.rs` 大量代码不再需要 `cfg`**,因为差异都在 `Renderer` 实现里。

**Step B:统一 `launch` 入口,屏蔽平台差异**

参考 dioxus 的 `dioxus::launch(App)`,在 `crates/glory` 顶层导出:

```rust
pub fn launch<W: Widget + 'static>(root: impl Fn() -> W + 'static);
```

不同的 feature 选不同实现:
- `web-csr`:从 `<body>` 挂载。
- `web-ssr`:返回 `String` 或 `Stream<HtmlChunk>` 给 salvo 处理。
- `desktop`(将来):起 wry 窗口、注入 JS 桥。
- `native`(将来):创建窗口、走 wgpu render loop。

---

## 3. 多平台发布策略

### 3.1 Dioxus 怎么做到一份代码多平台

| 层 | 内容 | 是否平台无关 |
|---|---|---|
| 组件 + RSX | `fn App() -> Element` | **是** |
| 信号 / hooks | `use_signal` / `use_memo` / `use_effect` | **是** |
| `core` / `VirtualDom` | diff + 产生 mutations | **是** |
| `WriteMutations` 实现 | 每平台一份 | 否(就该不同) |
| 事件桥 | 每平台一份(浏览器原生 / IPC / Blitz) | 否 |
| CLI(`dx`) | 一个工具打全部包 | 是(工具自适配) |

**关键点**:**RSX 不是平台分裂的根源**,真正分裂的是"指令执行端"。Glory 不用 DSL,但**指令抽象同样是必要的**。

### 3.2 Glory 当前的状态

- 只有 CSR + SSR,两者共享 `Widget` 这一层 builder API,但 builder 内部就直接调 web_sys / 内存 Node 了。
- 没有专门的"渲染器 trait",也没有 mutation buffer / 二进制协议。
- 没有 desktop / native / TUI 后端。
- 没有"统一 launch"——CSR 入口和 SSR 入口风格不一样(SSR 走 salvo handler、CSR 走 `mount_to(body)`)。

### 3.3 一份切实可行的多平台路线图

按里程碑:

**M1 · 渲染层抽象化(本地重构,零新平台)**
- 引入 `Renderer` trait(见 §2.3)。
- 把 `web/widgets/*.rs` 改造成 `RenderedWidget<R: Renderer>` 形态,移除大量 `cfg`。
- 提供 `WebRenderer`、`SsrRenderer` 两套实现,跑通现有 examples。
- 副产品:可以在 `tests/` 里跑一个 `MockRenderer`,做组件单测/快照测试(目前几乎没有)。

**M2 · 启动器(`glory::launch`)**
- 用 feature flag(`csr` / `ssr` / `hydrate` / 将来 `desktop`)区分实现。
- SSR 端给 salvo / axum / actix 都准备一个 adapter crate(参考 dioxus 的 `dioxus-fullstack-*`)。

**M3 · 桌面端(WebView)**
- 仿 dioxus-desktop:wry 起窗口,把 `Renderer` 实现成一个 `WryRenderer`,通过 IPC 把指令二进制序列化发到内嵌 JS interpreter。
- 这里**不需要重写一行业务组件**,组件还是 `Div::new().class().on(click, …)`。
- Glory 没有 `interpreter` 这种现成的 JS 桥,需要写一个最小化版本(几百行)。可以直接抄 dioxus 的二进制协议格式。

**M4 · 原生 / GPU 端(可选)**
- 接 Blitz(dioxus 用它做 `native`)或自研。
- 主要工作量在 layout / 字体 / 事件循环,不在 glory 自身。

**M5 · 移动端 / Tauri / SSR 流式**
- 移动:Tauri 现有方案;或 wry on Android/iOS。
- 流式 SSR:Tokio stream + `<template>` 占位 + 客户端 hydrate(参考 leptos 的 streaming);需要 `Resource` 原语先就位(见 §1.4)。

### 3.4 路由 / 资源 / Hot-reload

- **路由**:`crates/routing` 目前只在 wasm32 工作(`web-sys::History`)。如果 desktop / native 要支持路由,得抽出一个 `History` trait,把"浏览器 history / 内存 history"分别实现。Dioxus 在 [packages/history](../../E:/Repos/dioxus/packages/history) 已经这么做。
- **静态资源**:目前 Glory 没有等价于 dioxus `manganis!()` 的资源声明系统;桌面端打包后资源路径会和 web 不同。引入一个 `asset!()` 宏(或者直接借用 manganis 思路)可以避免后续每平台都改一次路径。
- **Hot reload**:`crates/hot-reload` 现在结构仅占位。dioxus 把 hot reload 拆成 RSX patch(`rsx-hotreload`)和 Rust 函数 patch(`subsecond`)两条。Glory 不做 DSL,所以 RSX 那条用不到;但**`subsecond` 思路(单元函数运行时重链)** 对 builder 风格框架也有价值,优先级中。

---

## 4. 未来支持其他渲染引擎的设计原则

直接落到代码上需要遵守的几条:

1. **`Node` 类型必须由 Renderer 决定,不能写死成 `web_sys::Element`**。即便短期只跑 web,也建议泛型化(`Scope<R: Renderer>` / `Widget<R: Renderer>`)。
2. **业务组件代码不应该出现 `web_sys::*` / `wasm_bindgen::*`**。这些只能出现在 `WebRenderer` 内部 + `events/` 桥接里。今天 `web/widgets/*.rs` 大量违反这一点。
3. **事件类型抽象化**。`events/` 现在直接是 `web_sys::Event`,可以变成 `pub trait EventPayload { fn as_any(&self) -> &dyn Any; }`,每个 Renderer 把自己平台的事件包装一下。
4. **DOM 字符串/属性 vs 富类型属性**。dioxus 用 `AttributeValue { Text, Float, Int, Bool, Listener, Any, None }`,这让 native 端能直接拿到 `f32`,不需要在 web/native 之间反复 parse。Glory 现在 attr 全是 `Cow<'static, str>`,迁移到富类型会方便很多。
5. **避免 `Rc<RefCell<Truck>>` 在 Renderer 边界泄漏**。`Truck` 的概念应该缩窄成"应用级上下文",而不是顺带塞渲染状态。
6. **保留单线程 fast path**。所有抽象都要让 wasm 单线程下零成本(`UnsyncRenderer = WebRenderer`),不要为了将来上 native 就强加 `Send + Sync`。

---

## 5. 其他可借鉴的零散点

- **`tracing` 字段化日志**:Glory 已用 tracing,但调用面较窄。可以学 dioxus 在每个 mutation / patch 打 trace span,便于 devtools 接入。
- **`AGENTS.md` 风格的"贡献者向 AI 提示"**:dioxus 有 `AGENTS.md`(项目根),减少 AI 协作时的来回。Glory 可以放一份,把"signals 不要替换成第三方"、"feature flag 规则"等惯例写下。
- **example 组织**:dioxus 在 `examples/` 下平铺;Glory 现在 examples 也铺得不错,但缺一个 `examples/_README.md` 把每个示例对应特性标出来,便于新用户挑入口。
- **`single-app` 命名**:双下划线在 Rust 社区一般表示"内部 unstable"。如果它是稳定的优化开关,建议改成 `single-app`;如果不是,加文档注释把它的语义讲清楚。

---

## 6. 改进 backlog(按优先级)

| 优先级 | 工作 | 收益 |
|---|---|---|
| P0 | 引入 `Renderer` trait + 把 `Node` 泛型化,清理 `cfg(web-csr)` | 后续一切多平台工作的前提 |
| P0 | 引入 `Cage: Copy`(arena + 代际号) | 用户写法立刻清爽,顺便为 `Sync` 版本铺路 |
| P1 | 加 `effect` / `resource` 原语,并和 `spawn` 打通 | 异步、SSR 数据加载、订阅类逻辑一等公民 |
| P1 | `glory::launch` 统一入口 + 各平台 adapter crate 骨架 | 文档可写"一行代码挂载",降低门槛 |
| P1 | `AttributeValue` 富类型化、`EventPayload` 抽象 | native / TUI 接得上 |
| P2 | `History` trait + 路由解耦 | 桌面端能用路由 |
| P2 | `WryRenderer`(桌面 webview) + 二进制指令协议 | 第一个非 web 平台 |
| P2 | 资源声明系统(类似 `manganis`) | 多平台资源路径不再炸 |
| P3 | 流式 SSR + hydrate | 现代 SSR 体验,需 `resource` 先就位 |
| P3 | `MockRenderer` + 组件快照测试基建 | 框架自身回归保护 |
| P3 | Blitz / wgpu 后端 | 真原生渲染 |
| P3 | Hot reload(subsecond 思路) | DX 提升,实现复杂度高,放后面 |

---

## 7. 参考路径速查

**Glory(本仓库)**
- 响应式核心:[crates/core/src/reflow/mod.rs](crates/core/src/reflow/mod.rs)、[cage.rs](crates/core/src/reflow/cage.rs)、[bond.rs](crates/core/src/reflow/bond.rs)、[lotus.rs](crates/core/src/reflow/lotus.rs)、[scheduler.rs](crates/core/src/reflow/scheduler.rs)
- 视图/组件:[crates/core/src/view.rs](crates/core/src/view.rs)、[widget.rs](crates/core/src/widget.rs)、[scope.rs](crates/core/src/scope.rs)、[holder.rs](crates/core/src/holder.rs)
- 节点抽象:[crates/core/src/node/mod.rs](crates/core/src/node/mod.rs)、[node/ssr.rs](crates/core/src/node/ssr.rs)
- Web 渲染:[crates/core/src/web/csr.rs](crates/core/src/web/csr.rs)、[web/widgets/mod.rs](crates/core/src/web/widgets/mod.rs)

**Dioxus(对照)**
- `WriteMutations`:`E:/Repos/dioxus/packages/core/src/mutations.rs`
- `VirtualDom`:`E:/Repos/dioxus/packages/core/src/virtual_dom.rs`
- `Signal` / `CopyValue`:`E:/Repos/dioxus/packages/signals/src/signal.rs`
- `generational-box`:`E:/Repos/dioxus/packages/generational-box/src/lib.rs`
- Web 渲染:`E:/Repos/dioxus/packages/web/src/dom.rs`
- Desktop:`E:/Repos/dioxus/packages/desktop/src/edits.rs`
- Native(Blitz):`E:/Repos/dioxus/packages/native/`
- Interpreter(JS 桥):`E:/Repos/dioxus/packages/interpreter/`
- History 抽象:`E:/Repos/dioxus/packages/history/`
- Hot reload:`E:/Repos/dioxus/packages/devtools/`、`packages/rsx-hotreload/`、`packages/subsecond/`
