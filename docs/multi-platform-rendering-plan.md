# Glory 多平台渲染支持规划

> 目标:让 Glory 从"Web 框架"演进为"多平台框架"(Web / Desktop / LiveView / TUI / Native GPU)。
> 参照对象:Dioxus(`E:\Repos\dioxus`)。
> 本文档为阶段性规划,核心是**阶段 0:统一渲染抽象**。

---

## 0. 背景:与 Dioxus 的核心差异

| | Dioxus | Glory |
|---|---|---|
| 渲染模型 | 有 VDOM(编译期 Template 剔除静态部分) + diff | 无 VDOM,细粒度订阅,命令式直改节点 |
| 核心产出 | 平台无关、可序列化的 `Mutation` 流(整数 `ElementId`) | widget 直接持有/操作具体 `Node`,部分走 trait、部分 cfg 硬编码 |
| 新平台接入点 | 实现 `WriteMutations`(约 18 个方法)+ launch 循环 | 实现 `Renderer`,**但还得先把 widget 层从 web_sys 解耦** |
| 跨 IPC/网络 | 天然(指令流可序列化) | 需另起命令序列化(desktop 已部分做),但与主路径割裂 |

**Dioxus 能多平台的根本原因**:核心只吐一条平台无关、可序列化的 mutation 指令流,引用整数 `ElementId`,"id ↔ 真实节点"的表由各渲染器维护。
**Glory 当前瓶颈**:虽有干净的 `Renderer` trait(`crates/core/src/renderer/mod.rs:102-137`),但生产主线(CSR/SSR)仍用 `cfg` + `web_sys` 硬编码绕过它。

### Dioxus 平台矩阵(参考)
| 平台 crate | 目标 | 落地方式 | 成熟度 |
|---|---|---|---|
| dioxus-web | 浏览器 WASM | wasm-bindgen + web-sys,经 dioxus-interpreter-js(sledgehammer)批量打 DOM | 一等公民 |
| dioxus-desktop | Win/macOS/Linux | WebView(wry + tao),mutation 二进制序列化经 WebSocket 发给 webview JS | 一等公民 |
| dioxus-mobile | iOS/Android | 同 desktop(共用 wry) | 一等公民 |
| dioxus-liveview | 服务器驱动 | mutation 经 WebSocket 推到浏览器,依赖 axum/tokio | 一等公民 |
| dioxus-ssr | HTML 字符串 | 遍历 VNode 输出字符串 | 一等公民 |
| dioxus-native(Blitz) | 原生 GPU | blitz-dom(Taffy 布局 + CSS) + vello/skia(wgpu),无 webview | 实验性 |
| TUI/ratatui | 终端 | 官方已移除,需社区自行实现 | 无 |

---

## 1. 总体路线图与优先级

```
阶段0 统一渲染抽象(必做地基)
   └─► 阶段1 Desktop/Wry (ROI 最高,已 60%+)
          └─► 阶段2 LiveView (增量小,秀细粒度优势)
   └─► 阶段3 TUI (独立新 Renderer,中等)
   └─► 阶段4 Native GPU (最大,建议复用 Blitz)
```

### 阶段 1:Desktop(Wry / WebView)— 约已完成 60-70%
已有:`WryRenderer`、`WryCommand` 序列化、`wry_interpreter.js`。缺:
1. 窗口宿主:封装 `wry::WebViewBuilder` + `tao` 事件循环,实现真实的 `WryCommandSink`(目前只有测试用 RecordingSink),命令流经 IPC/`evaluate_script` 注入 webview。
2. 事件回传:webview JS → IPC → Rust,反序列化成 `WryEventPayload` → 派发到对应 `ViewId` handler。
3. 接入阶段 0 的统一 widget。
4. launch 入口 + feature gate(`desktop`)。

### 阶段 2:LiveView — 复用 Desktop 指令流,换传输层
1. 把命令序列化从"IPC 发本地 webview"改成"WebSocket 发远程浏览器"。
2. 服务器侧持有 Scope 树与响应式状态(细粒度天然适合,只推变化的绑定点,理论比 Dioxus scope 级重渲染更省带宽)。
3. 复用已有 axum/actix/salvo adapter 做 WS 服务器,客户端复用 `wry_interpreter.js` 同款解释器。

### 阶段 3:TUI(ratatui)— 需要全新 Renderer
1. 新建 `TuiRenderer`(当前 `glory-tui` 的纯别名要删掉重写),节点为内存树 `TuiNode`(标签/文本/样式/布局约束)。
2. 布局:HTML 盒模型 → 终端字符网格,接 Taffy 或定义受限子集。
3. 绘制:patch 后把 TuiNode 树转成 ratatui `Widget`/`Buffer`。
4. 事件:crossterm 键鼠 → `EventPayload`。

### 阶段 4:Native GPU(对标 Blitz)— 工作量最大
1. 现实选择:**直接接 Blitz**(blitz-dom + vello/wgpu),让响应式树产出 Blitz 能消费的 DOM,而非自研 GPU 渲染器与布局引擎。
2. 若自研:Taffy 布局 + wgpu/vello 绘制 + 字体/文本整形 + 命中测试,基本是另一个子项目。

---

## 2. 阶段 0:统一渲染抽象(核心)

### A. 耦合点清单(改造靶点)

| # | 位置 | 现状 | 为什么挡住多平台 |
|---|---|---|---|
| 1 | `crates/core/src/node/mod.rs:24-31` | `Node` = cfg 别名:`web_sys::Element`(wasm+csr)/ `ssr::Node`(其余) | 二选一编译期分叉,没有第三个平台的位置;是具体类型,不是 `R::Node` |
| 2 | `crates/core/src/scope.rs:55-58` | `parent_node/render_node/first_child_node/last_child_node: Option<Node>` | Scope 直接持有具体 Node,Scope 又被 `View`/`ROOT_VIEWS`/routing 到处引用 |
| 3 | `crates/core/src/scope.rs:287-299` | `nodes()` 用 `web_sys::Node` + `nodes_between` | CSR 专属 DOM 区间查询,无平台无关替身 |
| 4 | `crates/core/src/widget.rs:74` | `mount_to(self, ctx: Scope, parent_node: &Node)` | trait 签名钉死具体 Node |
| 5 | `crates/core/src/web/widgets/csr.rs:20-44` | `Element<T: AsRef<web_sys::Element> + JsCast>`,字段 `node: T`、`renderer: WebRenderer`、`listeners: Vec<Box<dyn FnOnce(&T)>>` | 整个元素 widget 泛型边界绑死 web_sys;renderer 是具体类型 |
| 6 | `crates/core/src/web/attr.rs:16` + 各 impl | `AttrValue::inject_to(&self, …, node:&mut Node, …)`,cfg 体内直接 `node.set_attribute()`(web_sys/ssr) | **属性/class 主路径完全绕过 `Renderer` trait** |
| 7 | `crates/core/src/web/prop.rs` | `PropValue` 同上,CSR 还用 `JsValue` | 同 6,且引入 wasm 专属类型 |
| 8 | `crates/core/src/web/events/csr.rs:9` | `EventDescriptor::EventType: FromWasmAbi`;`ssr.rs` 另一套 | 事件类型系统绑 wasm ABI;委托逻辑(bubbles→全局委托)是 CSR 私有 |
| 9 | `crates/core/src/web/widgets/csr.rs:113-115` | `listeners: Vec<Box<dyn FnOnce(&T)>>`,build 时 `(listener)(&self.node)` | 事件挂载直接拿 web_sys 节点,不走 `Renderer::attach_event` |

**结论**:`Renderer` trait 已定义得很好,但主路径(Element/AttrValue/PropValue/事件)一条都没走它。阶段 0 = 把 #2 #4 #6 #7 #9 全改成只经过 `Renderer`,并解决 #1 这个"只能二选一"的类型别名。

### B. 架构岔路(开工前必须先选)

**路线 ① 泛型单态化 —— `Scope<R>` / `Widget<R>`,到处用 `R::Node`**
- 优点:零成本、类型安全,保留 CSR 直连 web_sys 的性能;契合已有 `Renderer{type Node}`。
- 缺点:泛型病毒式扩散到 `Scope`/`View`/`Widget`/`ROOT_VIEWS`/routing/`Truck`,改动面巨大;**不白送序列化**,desktop/liveview 仍要各自再写命令编码。

**路线 ② 对象安全渲染器 + 不透明节点句柄(Dioxus 风格)✅ 推荐**
- `Node` 变平台无关轻量句柄(整数 id,如 `Node(u32)`);真实节点表(`id → web_sys::Element`/内存节点)由渲染器自己持有;`Scope` 经 `truck` 拿 `Rc<dyn DynRenderer>`,所有节点操作走它。
- 优点:几乎不动泛型;命令流天然可序列化 → desktop(IPC)/liveview(WS)几乎白送;最短路径达成多平台目标。
- 缺点:热路径多一次 slotmap 查表 + 一次 vtable 派发(web 主平台);`Renderer` trait 要改成对象安全(擦除关联类型)。可用"WebRenderer 内部 `SlotMap<u32, Element>` + 单次派发"把代价控住。

> **推荐路线 ②**:阶段 0 目标就是让 desktop/tui/native/liveview 顺势而下,而②正好让"记录式/转发式渲染器"白送命令流。下方清单按②写;若选①,差异在每步标注。

### C. 分步迁移(每步可独立编译 + 测试绿)

**Step 1 — 把 `Renderer` 改成对象安全的 `DynRenderer`**(`crates/core/src/renderer/mod.rs`)
- 擦除关联类型:`Node` → 统一句柄 `Node(u32)`;`Event` → `Box<dyn EventPayload>`。
- 方法以句柄收发:`create_element(name,is_void)->Node`、`set_attribute_value(&Node,…)`、`insert_child(parent:&Node,child:&Node,pos:InsertPosition<Node>)`、`attach_event(&Node,name,bubbles,Box<dyn FnMut(Box<dyn EventPayload>)>)`。
- 提供 `trait DynRenderer` + blanket 使 `dyn` 可用;复用现有默认方法(`set_attribute_value`/`set_property_value`,见 `renderer/mod.rs:110-124`)。
- 保留 `MockRenderer`/`MockCommand`(`renderer/mod.rs:164+`)作回归基线——已是 id 化命令,正好当样板。
- *(路线①:保持关联类型,只补齐所有方法;不引入句柄。)*

**Step 2 — `Node` 句柄化**(`crates/core/src/node/mod.rs`)
- 删 cfg 别名,改 `#[derive(Clone,Copy,Debug,PartialEq)] pub struct Node(pub u32);`(加生成器)。
- `web_sys::Element` 与 `ssr::Node` 降级为各渲染器内部存储(`SlotMap<u32,_>`),不再出现在核心签名。
- `NodeRef<N>`(`node/mod.rs:5-22`)改持有 `Node` 句柄。

**Step 3 — 渲染器入驻 `Truck`,Scope 经它操作**
- `Truck` 增加 `renderer: Rc<dyn DynRenderer>`(替代 `Element.renderer: WebRenderer` 那个 ZST 字段,`web/widgets/csr.rs:39`)。
- `Scope` 提供 `fn renderer(&self) -> Rc<dyn DynRenderer>`;`scope.rs:55-58` 四个 `Option<Node>` 字段类型不变(已是新句柄),语义不变。
- `scope.rs:287-299` 的 `nodes()`/`nodes_between` 抽成 `DynRenderer::nodes_between(first,last)->Vec<Node>`(CSR 内部仍用 web_sys,其它给等价实现)。

**Step 4 — `AttrValue`/`PropValue` 改走渲染器**(`crates/core/src/web/attr.rs`、`web/prop.rs`)
- 签名:`fn inject_to(&self, renderer:&dyn DynRenderer, node:&Node, name:&str, first_time:bool)`,内部调 `renderer.set_attribute_value(node,name,AttributeValue)`;删掉所有 `#[cfg(...web-csr)]` 直接 `node.set_attribute()` 分叉(`web/attr.rs:53-73` 这类全删)。
- 标量(bool/i64/f64/String)映射到 `AttributeValue` 枚举(`renderer/mod.rs:31-93` 已备 `From`)。
- `Cage`/`Bond` 的 `inject_to`(`web/attr.rs:20-51`)逻辑不变(`is_revising()||first_time` + `bind_view`),只是底层落到 renderer。

**Step 5 — `Element<T>` 去 web_sys 化**(`crates/core/src/web/widgets/csr.rs` + `ssr.rs` 合一)
- `Element<T: AsRef<web_sys::Element>>` → `Element { name, is_void, classes, attrs, props, fillers, listeners, node: Node }`,不再泛型于 web_sys。
- `build`(`csr.rs:90-116`):`renderer.create_element` 拿句柄 → 写 `ctx.render_node/first/last_child_node` → fillers → `value.inject_to(renderer,node,…)` → 事件走 Step 6。
- `flood`(`csr.rs:46-68`)的 `insert_child` 改成 `ctx.renderer().insert_child(...)`;`ViewPlacement` 分支逻辑原样保留。
- **CSR/SSR 两份 widget 文件合并成一份**(平台差异已下沉到 renderer),消灭 `web/widgets/{csr,ssr}.rs` 重复。

**Step 6 — 事件统一**(`crates/core/src/web/events/*`、`csr.rs:113-115`)
- 顶层保留类型化糖 `on(events::click, handler)`;边界 lower 成 `renderer.attach_event(node,"click",bubbles,Box<dyn FnMut(Box<dyn EventPayload>)>)`。
- `EventDescriptor`(`events/csr.rs:7-22`)去掉 `FromWasmAbi` 约束,上移到 WebRenderer 内部:WebRenderer 在回调里把 `web_sys::Event` 包成 `EventPayload`,handler 再 `downcast`。`bubbles()`→全局委托策略移进 WebRenderer 实现。
- `listeners: Vec<Box<dyn FnOnce(&T)>>`(`csr.rs:35`)删除,改为 build 时直接 `renderer.attach_event`。

**Step 7 — 落两个参考渲染器,锁死回归**
- `WebRenderer`:内部 `SlotMap<u32, web_sys::Element>`,实现全部 `DynRenderer`;保留 hydration 钩子(见 D)。
- `SsrRenderer`:句柄 → `ssr::Node` 内存树,`outer_html()` 输出不变。
- 跑通 `examples/counter`、SSR 快照、`MockRenderer` 单测(`renderer/mod.rs:604+`)三条线全绿,阶段 0 收口。

### D. 迁移中必须守住的不变量(回归雷区)

1. **Hydration**(`web/widgets/csr.rs:69-89` `hydrate` + `flood` 里 `gly-hydrating`/`gly-id` 判断):句柄化后 WebRenderer 要支持"用现有 DOM 建句柄表"(`query_selector` 命中即登记 id),而非 `create_element`。`is_hydrating()` 跳过插入的逻辑保留在 WebRenderer 内。
2. **事件委托**:`bubbles()==true → 全局委托,false → 直挂`(`events/csr.rs:19-21`)语义必须由 WebRenderer 完整接管,否则 `undelegated`/`Custom` 行为变样。
3. **Each 重排锚点**:`first_child_node/last_child_node` 作为兄弟定位锚点(`scope.rs:189-257` `attach_child` 依赖、`widget.rs:119-133` `flood` 注释强调不能预设 placement)。句柄替换 Node 后,`nodes_between` 语义要逐字对齐,否则 `Each` 重排错乱。
4. **`shift_remove` 顺序**(`scope.rs:259-285`):与渲染无关,别误改。
5. **`single-app` 双路 cfg**:`batch`/`holder_id` 到处分叉,renderer 注入要在两种 cfg 下都成立。

---

## 3. 关键文件速查

### Glory(`D:\Works\glory-rs\glory`)
| 功能 | 路径 |
|---|---|
| Renderer trait + Mock | `crates/core/src/renderer/mod.rs` |
| Node 别名(待句柄化) | `crates/core/src/node/mod.rs` / `node/ssr.rs` |
| Scope | `crates/core/src/scope.rs` |
| Widget trait | `crates/core/src/widget.rs` |
| Element(CSR/SSR) | `crates/core/src/web/widgets/{csr,ssr}.rs` |
| AttrValue / PropValue | `crates/core/src/web/attr.rs` / `web/prop.rs` |
| 事件 | `crates/core/src/web/events/{mod,csr,ssr}.rs` |
| Desktop(Wry,半成品) | `crates/desktop/src/lib.rs` |
| Native/TUI(纯别名占位) | `crates/{native,tui}/src/lib.rs` |
| 响应式核心 | `crates/core/src/reflow/{cage,bond,effect,scheduler,owner,mod}.rs` |

### Dioxus 参考(`E:\Repos\dioxus`)
| 功能 | 路径 |
|---|---|
| WriteMutations trait | `packages/core/src/mutations.rs` |
| Template/VNode | `packages/core/src/nodes.rs` |
| VirtualDom | `packages/core/src/virtual_dom.rs` |
| Web 渲染器 | `packages/web/src/mutations.rs` |
| Desktop(wry+WebSocket) | `packages/desktop/src/edits.rs` |
| Native(Blitz) | `packages/native/src/dioxus_renderer.rs` |
| LiveView | `packages/liveview/src/lib.rs` |
| SSR | `packages/ssr/src/renderer.rs` |
| 平台选择 launch | `packages/dioxus/src/launch.rs` |

---

## 4. 一句话总结

阶段 0 不是"再写渲染器",而是**把 `Node` 从 cfg 别名变成平台无关句柄、让 Element/Attr/Prop/事件四条主路径全部只经过一个对象安全的 `DynRenderer`**。做完后:
- desktop = 把命令转 IPC 的 DynRenderer
- liveview = 转 WebSocket 的 DynRenderer
- TUI = 转 ratatui 绘制的 DynRenderer
- native = 喂 Blitz 的 DynRenderer

全都只是再实现一遍同一个 trait。

---

## 5. 待决策事项(开工前)

- [ ] **架构路线 ① vs ②**(B 节)——推荐 ②,需最终拍板。
- [ ] 是否需要"路线① vs ②"的硬核取舍对比(性能 / 改动文件数 / 序列化 / 对 hydration·委托的冲击)。
- [ ] 选定后,先草拟 Step 1 的 `DynRenderer` trait 新签名再开工。
