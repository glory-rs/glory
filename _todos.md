# Glory 现状评审任务板（2026-06-12，对照本地 Dioxus checkout）

> 评审范围:`D:\Works\glory-rs\glory` 对照 `E:\Repos\dioxus`。
> 本板替代旧的历史实现日志(旧内容见 git 历史 `git show 080404b:_todos.md`),
> 并接续 `_improve_todos.md`(2026-06-11 基线)中仍未完成的 E9/E10/E11/D2。
>
> 状态:`[ ]` 未开始 / `[~]` 部分完成 / `[x]` 完成。
> 优先级:P0 = 阻塞成熟度叙事的关键差距 / P1 = 高价值近期 / P2 = 中期 / P3 = 长期可选。
> 所有任务按 Lane 分组,**同一 Lane 内串行依赖已标注,不同 Lane 之间可完全并行**。

---

## 评审结论摘要

经四路代码级核查(未完成标记扫描、核心渲染/热重载、平台后端、CLI/全栈),与 Dioxus 的
剩余差距集中在七个方面,前一轮 `_improve_todos.md` 关闭的条目基本属实:

| 领域 | 现状 | 差距等级 |
|---|---|---|
| 路由 | 运行时字符串匹配,无类型化路由/嵌套布局/Outlet/重定向/查询参数解析 | **关键** |
| 服务器函数 | 仅 POST + JSON;无 HTTP 动词选择、多编码、逐函数中间件、响应式 WebSocket hook | **高** |
| 异步/错误原语 | 无 Suspense 式自动边界、无 ErrorBoundary;`resource_in` 竞态已修 | **高** |
| CLI/构建 | wasm-split 暂缓;Windows/Linux 原生安装器已有最小路径,macOS/AppImage/签名仍缺 | **中-高** |
| 资产 | 无类型化清单、图片优化、CSS Modules、folder 资产宏(对照 manganis) | 中 |
| 桌面 | 协议扎实,缺托盘/全局热键/窗口状态 API/文件对话框/异步自定义协议 | 中 |
| Native(Blitz)/LiveView/移动端 | 分别处于 spike(~20%)/可用但单适配器(~30%)/模板可编译但无真机验证(~30%) | 中-高 |

性能:官方 js-framework-benchmark 9 项聚合 Glory 442ms vs Dioxus 456ms vs Leptos 589ms,
`create10k` 脚本时间仍落后 Dioxus(78ms vs 64ms)。架构层面已无硬性差距,余下是产品化。

---

## Lane R — 响应式与核心原语(crates/core/src/reflow, widgets)

- [ ] **R1 P0** Suspense 式自动异步边界。现状 `Loader` 要求手工分支
  `LoadState::{Idle,Loading,Loaded}`(`crates/core/src/widgets/loader.rs`),Dioxus 的
  `Suspense`(`packages/core/src/suspense/component.rs`)自动捕获 `use_resource` 挂起。
  目标:提供一个 widget 级 Suspense 边界,子树内任意未就绪 resource 自动显示 fallback,
  SSR 流式输出与 hydration 兼容(与既有 `render_stream()` DOM 边界分块衔接)。
- [ ] **R2 P0** ErrorBoundary。当前 panic/错误直接传播,无捕获渲染机制(Dioxus:
  `packages/core/src/error_boundary.rs`,含 SSR 预载错误状态)。目标:子树错误捕获 +
  自定义错误视图 + hydration 状态序列化。
- [x] **R3 P1** 修复 `resource_in()` 过期写竞态(`crates/core/src/reflow/effect.rs:136`
  附近):依赖快速连续变化时旧 future 不取消,旧结果可能覆盖新结果。引入代际计数或
  abort handle,仅最新一代可写回。补回归测试。
- [x] **R4 P1** 响应式全局/上下文状态。`Truck` 是非响应式 Any 容器
  (`crates/core/src/truck.rs`),Dioxus 有 `GlobalSignal`/`GlobalMemo`
  (`packages/signals/src/global/`)。本轮选择文档化 "Truck 中存 Cage" 模式边界:
  `docs/api-guide.md#app-wide-reactive-context` 说明 Truck 本身不订阅/不触发 patch,
  需要响应式时存放 `Cage` 句柄;暂不引入模块级静态 API。
- [x] **R5 P2** 评估 Copy 句柄。`Cage` 已是 Copy + generation/free-list slot;
  `Bond` 保持 Clone,暂不迁移 generational slot。设计结论见
  `docs/reactivity-copy-handles.md`;只有 E10/专项 bench 证明 derived handle clone
  是热路径成本时才重开。
- [x] **R6 P2** 事件委派两处 TODO:Shadow DOM 重定向与 `currentTarget` 模拟
  (`crates/core/src/web/helpers/event.rs:62-64`)。委派监听器下读取
  `event.currentTarget` 的处理器会拿到错误节点。

并行性:R1/R2 可同时开(共享 hydration 序列化决策时对齐一次);R3/R4/R6 完全独立;
R5 仅评估,不阻塞任何人。

## Lane T — 路由类型化(crates/routing)— 对照差距最大的单项

- [x] **T1 P0** 类型化路由定义。现状是运行时字符串 `Router::push/filter`
  (`crates/routing/src/router.rs`),Dioxus 有 `#[derive(Routable)]` 枚举路由
  (`packages/router-macro/src/lib.rs`):静态类型、动态段类型推导、编译期检查。
  Glory 哲学是"无 view DSL",但 Routable 派生不是 view 宏 — 需先做一次设计决策:
  derive 宏 vs 类型化 builder(如 `route::<UserRoute>("/user/:id")`)。产出设计文档
  后实现核心:路由枚举 ↔ URL 双向转换 + 类型安全 `goto(Route::X{..})`。
  2026-06-19 第一阶段:已新增手写 `Routable` trait、`AviatorExt::goto_route`、
  `Locator::route::<R>()`、`LocatorModifier::from_route` 和 path 参数 encode/parse
  helpers;剩余是 derive/builder 决策与自动生成。
  2026-06-19 第二阶段:已选择 derive 路线并新增 `#[derive(glory::Routable)]`,
  支持 `#[route]`、`#[redirect]`、`#[not_found]`、typed path 参数和 catch-all;
  复杂 query 结构继续用现有 helper 手写。
- [x] **T2 P0** 嵌套布局 / Outlet 等价物。Dioxus 有 `#[nest]`/`#[layout]`/`Outlet`
  (`packages/router/src/components/outlet.rs`)。Glory `Router` 已有层级 children,
  缺的是"父布局渲染插槽"语义。依赖 T1 的设计结论。
  2026-06-19 已完成:新增 `Outlet` 语义组件与 `Router::layout` /
  `layout_keyed` / `outlet` helpers,父 layout 以 widget type key 去重,叶子路由
  渲染进命名 outlet。
- [x] **T3 P1** 查询参数与 catch-all 段解析:对照 `FromQuery`/`FromQueryArgument`/
  `FromSegments`(`packages/router/src/routable.rs`),提供 trait 化解析 + 默认值回退。
  可与 T1 并行(trait 层独立于路由定义形态)。
  2026-06-19 已完成:新增 `RouteQuery`/`FromRouteQuery`、required/optional/
  repeated/default query helpers、query 编码 helper,以及 catch-all encode/split/parse
  helpers;未来 derive/builder 可直接复用。
- [x] **T4 P1** 声明式重定向(Dioxus `#[redirect(...)]`)与 404 默认处理。依赖 T1。
  2026-06-19 已完成: `Routable` 新增 `redirect()`/`not_found()`/`resolve_url()`,
  并提供 `match_route_pattern()`/`redirect_url()` helpers;`Locator::route::<R>()`
  现在会解析 redirect 与 typed 404 fallback。
- [x] **T5 P2** 导航增强:滚动恢复、`GoBack/GoForward` 等价 API。另核查
  `crates/routing/src/aviators/browser.rs:105` 的 TODO(history state 以 prop 而非
  attribute 存储,部分导航模式可能丢 state)。
  2026-06-19 已完成: `Aviator` 统一暴露 `back()`/`forward()`,Memory/Browser
  后端实现对应历史移动;浏览器端保留 hash/top scroll 和 `noscroll` 控制,并修正
  anchor `state`/`replace` 同时读取属性与 property。

并行性:T1 是本 Lane 关键路径;T3/T5 可与 T1 并行;T2/T4 依赖 T1 设计落定。

## Lane S — 服务器函数 / 全栈(crates/serverfn, macros, 适配器)

- [x] **S1 P0** HTTP 动词支持。`#[server]` 目前只生成 POST(`crates/macros/src/lib.rs`),
  Dioxus 支持 `#[get]`/`#[post]` 等。目标:`#[server(method = "GET")]`(GET 走查询串
  编码,可缓存),三适配器(Salvo/Axum/Actix)同步,wire 协议测试覆盖。
  2026-06-19 已完成:宏支持 `method = "GET"`,客户端 GET 通过
  `__glory_args` 查询参数传 JSON tuple,Salvo/Axum/Actix adapter 同时挂 GET/POST,
  runtime 按 method dispatch 并对方法不匹配返回 405。
- [x] **S2 P1** 多编码协商。当前仅 JSON;Dioxus 有 `Encoding` trait + JSON/CBOR/
  MessagePack/Postcard(`packages/fullstack/src/encoding.rs`)。目标:可插拔编码 trait,
  feature-gate CBOR/Postcard,默认 JSON 不变。
  2026-06-19 已完成:新增 `ServerFnEncoding` 与 `encode/decode_*_with` helpers,
  三适配器按 `Content-Type` 解码 POST、按 `Accept` 编码响应;新增
  `glory-serverfn/cbor` 与 `glory-serverfn/postcard` features,宏支持
  `#[server(encoding = "cbor" | "postcard")]`,默认 JSON 和 GET 查询参数不变。
- [x] **S3 P1** 逐函数中间件。Dioxus 支持 `#[middleware(tower_layer)]`。Glory 三适配器
  不共享 tower,需设计适配器中立的 hook 点(如 `#[server(hoops = ...)]` 映射到各框架
  原生中间件)。先设计后动宏。
  2026-06-19 已完成:新增 adapter-neutral `ServerFnMiddlewareContext` 与
  `ServerFnMiddleware` hook,dispatch 在函数体前顺序执行并可用 `ServerFnError`
  短路;宏支持 `#[server(middleware = path)]` 与 sibling `#[middleware(path)]`。
- [x] **S4 P1** 客户端响应式 WebSocket。已有 `TransportMessage<T>`/`WebSocketFrame`
  帧助手(`crates/serverfn/src/lib.rs`),缺 Dioxus `use_websocket()` 式 hook
  (`packages/fullstack/src/payloads/websocket.rs`):自动重连 + 类型化双向通道 +
  连接状态 Cage。
  2026-06-19 已完成:新增 `use_websocket::<T>()` /
  `use_websocket_with_options()` browser handle,暴露
  `Cage<WebSocketConnectionState>`、latest `TransportMessage<T>`、error Cage,
  支持 typed `send()`、手动 `reconnect()`、默认自动重连;非 wasm 返回 Failed 状态。
- [x] **S5 P2** 服务端原生 extractor 直通(Dioxus 允许 axum `FromRequest` 直接做参数)。
  Glory 的 `RequestContext` 是统一抽象;评估按适配器 feature-gate 的原生 extractor
  直通是否值得破坏适配器中立性,先写决策记录。
  2026-06-19 已完成:新增 `docs/serverfn-extractor-decision.md`,决策为核心
  `#[server]` 暂保持 adapter-neutral,原生 Salvo/Axum/Actix extractor 放在自定义
  框架 route 或未来显式 feature-gated adapter 扩展。
- [x] **S6 P1** 提交此前未提交改动:`salvo_mount::streaming_response()` 返回式助手 +
  `write_streaming_response` 显式 `StatusCode::OK` + salvo-adapter `into_response()` +
  recipes 文档。补一条 Salvo 流式响应状态码测试后提交。
  2026-06-19 已确认实现存在,并新增 Salvo streaming response 200/content-type
  回归测试。

并行性:S6 立即可做;S1/S2/S4 互相独立;S3/S5 需各自先出小设计决策。

## Lane C — CLI / 构建 / 开发体验(crates/cli)

- [x] **C1 P0** serve 开发体验追平:浏览器 auto-open(默认开)、`--port/--address`
  显式参数、HTTPS 选项、代理转发配置。对照 `packages/cli/src/cli/serve.rs` 的
  `ServeArgs`(open/hot_reload/cross_origin/wsl_file_poll_interval 等 11 个字段),
  Glory `crates/cli/src/command/serve.rs` 此前是 14 行薄封装。
  2026-06-19 第一阶段:已新增 `glory serve --address/--port` 对 `site_addr`
  的运行时覆盖,默认打开浏览器并支持 `--no-open`。
  2026-06-19 第二阶段:已新增 `--https`、`--tls-cert/--tls-key`、重复
  `--proxy PATH=URL`,统一通过 `GLORY_SITE_SCHEME`/`GLORY_SITE_URL`、
  `GLORY_TLS_CERT`/`GLORY_TLS_KEY`、`GLORY_PROXY_CONFIG` 传给应用服务器。
- [x] **C2 P1** serve 交互能力:运行中按键触发重建(r)、verbose 切换(v)、帮助(/)。
  不必复刻 Dioxus 全屏 TUI,行式交互即可。依赖 C1 的参数结构。
  2026-06-19 已完成:watch 模式 stdin 支持 `r` + Enter 强制全量 rebuild,
  `v` + Enter 在 info/debug/trace 间切换日志级别,`/`/`?` + Enter 打印帮助。
- [x] **C3 P2** wasm-split 评估与原型。Dioxus 有成熟 `#[wasm_split]` 宏 + CLI 切分 +
  Playwright harness(`packages/wasm-split/`),Glory 完全没有。评估见
  `docs/wasm-split-evaluation.md`:当前体积不先做 split,原型等 gzip/路由懒加载阈值
  触发后再做 CLI-owned 次入口方案。
- [x] **C4 P1** 原生安装器产物:`glory bundle --target desktop` 目前输出裸 exe +
  assets,Dioxus 经 tauri-bundler 出 MSI/DMG/DEB/AppImage。评估直接复用
  tauri-bundler crate,先支持 Windows MSI + Linux deb。
  2026-06-19 已完成:未直接引入完整 tauri-bundler/Dioxus bundler 依赖,而是在现有
  `bundle` 流程中补最小原生安装器路径:Windows 写 WiX `product.wxs`、payload staging
  和 `build-msi.ps1`,检测到 WiX v3 工具时直接产出 `.msi`;Linux 纯 Rust 组装 `.deb`
  (ar + control/data tar.gz),并安装到 `/usr/lib/<package>` + `/usr/bin` symlink +
  `.desktop` 文件。`examples/desktop-counter` 已补 metadata 作为 bundle 验证样例。
- [x] **C5 P2** CLI 小命令补齐:`run`(无热重载直跑)、shell `completions`、
  `self-update` 提示。`translate`/`components` 属 RSX 生态,明确不做。
  2026-06-19 已完成:`glory run` 复用无 watch/live reload 的 serve 路径,
  `glory completions <shell>` 输出 clap completion script,`glory self-update`
  输出安装/升级提示。
- [x] **C6 P1** 修复 CLI 现存 TODO:
  - `crates/cli/src/compile/style.rs:107` Style 产物返回空串占位,样式指纹/缓存失效
    可能不工作;
  - `crates/cli/src/ext/exe.rs:703,729` 工具版本匹配无 semver 区间支持;
  - `crates/cli/src/tests.rs:27` cwd 全局可变导致测试无法并行。
  三项互相独立,可拆三个小 PR。

并行性:C1→C2 串行;C3/C4/C5/C6 与 C1 完全并行,C6 内部三项也可并行。

## Lane A — 资产管线(crates/core/src/assets.rs, cli)

- [ ] **A1 P1** 类型化资产清单:`asset!` 目前只做路径解析,无编译期存在性校验与
  link-time 哈希文件名(manganis 用 link section + 占位哈希,
  `packages/manganis/manganis-core/src/asset.rs`)。最小目标:`asset!` 编译期校验
  文件存在 + bundle 时内容哈希进文件名 + 运行时映射。
- [ ] **A2 P2** 图片优化管线(PNG/JPEG→WebP,按平台格式),对照 manganis
  `images.rs`。保持默认关闭,作为 bundle 显式选项。依赖 A1 的清单结构。
- [ ] **A3 P2** folder 资产宏(递归枚举 + 清单),对照 manganis `folder.rs`。
- [ ] **A4 P3** 类型化 CSS Modules(`.module.css` → 类名结构体)。现有运行时
  `ScopedStyle` 已覆盖主场景,此项仅在用户需求出现后做。

并行性:A1 先行,A2/A3 依赖其清单格式;A4 独立但缓做。

## Lane D — 桌面(crates/desktop)

现状 80%:协议/多窗口/菜单/自定义协议/热重载扎实;对照
`packages/desktop/src/desktop_context.rs`(30+ 方法)缺以下 API。

- [ ] **D1 P1** 窗口控制 API 包:`drag_window`、`set_fullscreen`/查询、
  `is_maximized`/`toggle_maximized`、`focus`、`set_zoom_level`、按 id 关闭任意窗口、
  运行时从窗口内新开窗口。多为 wry/tao 一行调用,作为一个 PR 打包。
- [ ] **D2 P2** 托盘图标支持(tray-icon crate;`DesktopConfig` 字段 + 事件回调)。
- [ ] **D3 P2** 全局热键(global-hotkey crate;注册/注销 API)。
- [ ] **D4 P1** 异步自定义协议处理器:当前 `glory://` 仅同步;对照 Dioxus
  `AsyncWryProtocol` + `RequestAsyncResponder`,支持长耗时资源/RPC 端点。
- [x] **D5 P2** 原生文件对话框集成指南或 rfd 助手(当前文档定位"应用自理",至少给
  recipes)。`docs/platform-apis.md#file-dialogs` 记录了 app-owned `rfd` 集成模式,
  `docs/desktop.md` 已补入口。
- [ ] **D6 P3** 拖放文件事件桥接、打印对话框。

并行性:D1-D6 全部互相独立,均不动指令流协议(注意 wire 协议三处同步规则)。

## Lane N — Native / Blitz(crates/native)— spike → 可用

- [~] **N1 P1** 完整事件桥接:当前已映射 MouseMove/Down/Up/Click/Input +
  Keyboard/IME(`crates/native/src/blitz_consumer.rs`),仍缺滚轮/scroll、
  focus/blur、touch。Blitz 0.2 当前 `DomEventData` 尚未暴露这些数据,需等上游事件
  数据或扩展 shell 后继续映射。
- [ ] **N2 P1** 布局查询接通:`blitz_consumer.rs:210` 起 Query(BoundingRect/
  ScrollOffset)是 no-op,需接 Blitz layout pass 后应答 `NodeQuery`。
- [ ] **N3 P2** 窗口生命周期:`launch_blitz_window` 单窗口 shell(13 行),需
  WindowId 跟踪、resume/pause、重绘调度,对照 `packages/native/src/dioxus_application.rs`。
- [x] **N4 P2** 属性/property 区分:`blitz_consumer.rs:129` spike 把 property 一律当
  attribute 写,表单控件(value/checked)行为会偏。
- [ ] **N5 P3** accesskit 无障碍接入。

并行性:N1/N2/N4 互相独立;N3 与其余并行;建议 N1+N4 先行(直接影响表单可用性)。

## Lane L — LiveView(crates/liveview)

- [ ] **L1 P1** 异步化重构:当前每会话一个 `std::thread` + mpsc
  (`crates/liveview/src/lib.rs`),百级并发即吃紧;Dioxus 用
  `LocalPoolHandle` task-per-session(`packages/liveview/src/pool.rs`)。改为
  spawn_local 固定任务 + 有界队列(背压),保住 non-Send 树约束。
- [ ] **L2 P1** Axum/Actix 适配器:抽 `LiveviewRouter` 式 trait(对照
  `packages/liveview/src/adapters/mod.rs` 约 30 行),消除 Salvo 硬编码。可与 L1 并行
  (传输 trait 先定)。
- [ ] **L3 P2** Query 应答接通:协议已有 Query 消息,服务端无应答路径;复用桌面解释器
  的 query 答复逻辑。
- [x] **L4 P2** HTML 模板配置(head/root div 自定义)、会话空闲超时/TTL、重连退避
  语义文档化。
  2026-06-19 已完成:`docs/liveview.md` 明确 LiveView 只拥有协议/session/
  route/client JS,HTML shell/head/root 由宿主应用负责;记录当前每 WebSocket
  连接一会话、无 resume/idle TTL 配置,以及客户端 `reconnectMs` /
  `maxReconnectMs` 指数退避语义。
- [ ] **L5 P2** LiveView 示例项目(examples/ 下,Salvo 起步)。依赖 L2 的 trait 定型
  可顺带出 Axum 版。

## Lane M — 移动端(cli templates, examples/mobile-counter)

- [ ] **M1 P0(外部受阻)** 真机/模拟器验证:`scripts/mobile-device-smoke.ps1` 主机
  检查已过,需 Android 模拟器或真机在线 + macOS 跑 iOS。接续 `_improve_todos.md` D2。
  CI 侧:GitHub Actions Android emulator job(API 34, x86_64)夜间跑安装+启动冒烟。
- [ ] **M2 P1** 设备上热重载:当前 reload client 仅桌面 webview 可达,Android 需
  `adb reverse` 或局域网地址注入模板。依赖 M1 有验证环境。
- [x] **M3 P2** 权限请求助手与原生桥模式文档(相机/相册/分享等先给 recipes,不急于
  封装 API)。见 `docs/mobile-native-recipes.md`,覆盖 app-owned bridge、权限结果、
  相机/相册 URI、分享 sheet 的平台中立边界。
- [x] **M4 P2** 评估"单代码库 + cfg 分平台"路线:Dioxus 桌面/移动同 crate
  feature-gate(`packages/desktop/src/mobile.rs` 约 50 行),Glory 是独立模板树。
  决策见 `docs/mobile-architecture-decision.md`:设备 CI 绿之前保持透明 host 模板,
  后续优先共享 helper crate,不做单个 cfg-heavy 大 crate。

## Lane E — 性能与基准(接续 _improve_todos.md E9/E10/E11)

- [~] **E9 P0** 普通 builder 元素子树的自动分配压缩。`DomSubtree` 已验证收益
  (benchmark 行从完整 view 树坍缩为单 view + 原生子节点),余下是让普通
  `div().child(...)` 静态子树内部自动走紧凑模式,不破坏公开 API。
- [x] **E10 P1** 稳定多样本基准对比:扩展
  `benchmarks/official-js-framework-benchmark.ps1`,保留命名基线、Glory-only A/B、
  `Count>=5` 中位数/极差表。**应先于 E9/E11 的验收**(否则小优化无法接受/拒绝)。
- [ ] **E11 P1** 事件处理器安装成本隔离:同 DOM 无 handler vs 委派行 id 查找的合成
  基准,量化 `create10k` 中 closure 安装占比;若显著,出委派行 API 提案。
- [ ] **E12 P2** `create10k` 脚本时间追平 Dioxus(78ms vs 64ms):在 E10 落地后,
  对 wasm→DOM 调用次数与字符串分配做 profile 驱动优化。

并行性:E10 先行;E9/E11/E12 随后并行,均以 E10 的报告为验收标准。

## Lane F — 修剩余小项与流程

- [x] **F1 P2** README 与文档引用更新:README 仍把 `_todos.md` 描述为"历史实现日志",
  本次重写后需同步措辞;`_improve_todos.md` 标注"由本板接续"。
- [x] **F2 P2** Playwright 全量浏览器 CI 化:F3 已建项目但 CI 只跑 install/list/skip
  冒烟,补真实浏览器执行(至少 CSR counter + SSR hydration 两项)。
  2026-06-19 已完成:新增 CI `browser-e2e` job,安装 Chromium,通过 `glory serve`
  启动 CSR counter 与 SSR hydration 示例,设置 `GLORY_COUNTER_URL`/`GLORY_SSR_URL`
  后实际运行对应 Playwright 项目;同时让 counter 示例具备 SSR 静态服务 bin,并修复
  CLI `bin_target` 与 package name 不一致时的可执行文件路径。
- [x] **F3 P3** 桌面 IPC `panic!("unexpected message")`(`crates/desktop/src/lib.rs:157`)
  改为带日志的容错降级。生产路径 `runtime.rs` 已对无法解码的 IPC 消息
  `tracing::warn!` 后丢弃;`lib.rs` 里剩余 panic 是测试断言,非宿主运行时路径。
- [x] **F4 P0** 修复 release checklist 中的 CLI clippy 失败:
  `cargo clippy -p glory-cli --lib --no-default-features -- -D warnings`
  之前因 `crates/cli/src/lib.rs` 的 `clippy::let_and_return` 失败,已直接返回
  `match` 表达式并通过验证。
- [x] **F5 P0** 同步 feature/test 文档漂移:`AGENTS.md` 已更新
  `backend-command + web-ssr` 可组合、`backend-command + single-app/web-csr`
  仍互斥的当前规则;`docs/release-readiness.md` 已移除 workspace 测试历史失败说法。
- [x] **F6 P0** Rust 主 CI 化:新增 `.github/workflows/ci.yml`,覆盖 fmt、
  core default/web-ssr/backend-command tests、serverfn tests、CLI tests、
  public feature-set check、feature guard 负向检查和 release clippy gates。

---

## 建议并行执行批次

```text
批次 1(立即,全部互相独立):
  S6 提交未提交改动 | E10 基准报告 | C6 三个 CLI TODO | R3 resource 竞态
  | D1 窗口 API 包 | L2 LiveView trait 抽象 | T1 路由设计文档

批次 2(批次 1 设计落定后):
  R1+R2 Suspense/ErrorBoundary | T2/T3/T4 路由实现 | S1 HTTP 动词
  | C1 serve 体验 | L1 LiveView 异步化 | N1+N4 Blitz 事件/property
  | A1 类型化资产清单 | E9/E11(以 E10 验收)

批次 3(中期):
  S2/S3/S4 | C2/C4 | A2/A3 | D2/D3/D4/D5 | N2/N3 | L3/L4/L5 | M2/M3 | E12

长期/按需:A4 | N5
```

## 与 Dioxus 的差异化定位提醒(评审附带结论)

以下 Dioxus 能力**明确不抄**:RSX/`rsx!` 与 view DSL(项目原则)、`translate`/
`components` 命令(RSX 生态专属)、VDOM 路线、TUI 产品化(D6 已决策为只读调试)。
Glory 的相对优势(builder API、指令流统一后端、`glory config --schema`、SSR 命令重放)
应在 docs 与 README 持续强化,而不是在所有维度对齐 Dioxus。
