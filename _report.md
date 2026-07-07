# Glory 当前完成度、缺陷与 Gaps 审计报告

日期: 2026-07-07

审计范围: `D:\Works\glory-rs\glory`, branch `main`, HEAD `293de1c`。

本报告替代旧版 `_report.md` 的单一 Dioxus 对比视角。当前报告聚焦 Glory 自身:
完成度、缺陷、gaps、验证结果、可继续审核的维度和优先级。

## 0. 审计方法

本次审计基于:

- 仓库结构、workspace manifest、crate feature matrix。
- `README.md`、`docs/*`、`_todos.md`、`_current_report.md`、release checklist。
- 关键源码路径: core reactivity、widgets/SSR、command renderer、serverfn、routing、CLI、desktop、native、liveview、mobile templates。
- 本机验证命令。见本报告末尾“实际执行的验证”。

本报告中的百分比是工程成熟度估算,不是 `_todos.md` 的任务计数。成熟度综合考虑:
可编译性、测试覆盖、端到端验证、文档准确性、平台运行态闭环、发布门禁和用户可用性。

## 1. 可审核维度列表

当前项目可以从以下维度审核:

| 维度 | 核心问题 |
|---|---|
| 产品定位与范围 | 当前是研究框架、可用框架、还是可发布产品? |
| Feature matrix | CSR/SSR/backend/mobile/native/liveview 的 feature 组合是否清晰且被测试? |
| Core reactivity | `Cage`/`Bond`/`Lotus`、Owner、effect/resource、scope 生命周期是否稳定? |
| Widget/SSR/hydration | builder widget 生命周期、SSR HTML、streaming/hydration 是否可靠? |
| Renderer abstraction | command stream 是否足以支撑 desktop/native/liveview/TUI? wire protocol 是否稳定? |
| Routing | typed routes、derive、query、redirect、history、SSR routing 是否完整? |
| Fullstack/serverfn | `#[server]`、encoding、middleware、stream、WS、cache、adapters 是否可用? |
| CLI/build/bundle/assets | serve/build/bundle/SSG/assets/css/images/installers 是否产品化? |
| Platform targets | web、desktop、native、liveview、mobile 的运行态是否闭环? |
| Testing/CI/release | 本地必跑、CI、feature guards、semver/MSRV、package/publish 是否可靠? |
| Docs/examples/DX | README/docs/examples 是否匹配当前实现,新用户是否能顺利上手? |
| Security/reliability | XSS/路径穿越/CSP/协议兼容/资源释放/错误隔离是否有测试? |
| Performance | benchmarks、wasm size、render/patch 性能是否有基线和回归门禁? |

## 2. 结论摘要

Glory 现在已经不是“只有核心雏形”的项目。core reactivity、SSR、command-stream
renderer、serverfn、routing、CLI、desktop/native/liveview/mobile 都有实代码和测试。

更准确的定位是:

- Core + SSR + command renderer 已经较扎实。
- Server functions、routing、CLI/bundle 已达到“可试用/可继续产品化”的阶段。
- Desktop webview runtime 是可用雏形,但仍缺真实窗口/安装包的常规人工或 CI 验证。
- Native Blitz、LiveView、Mobile 已经有技术路径和部分测试,但整体仍偏 experimental。
- 本轮已消除默认 `cargo test --workspace` 失败;剩余最大缺口集中在运行态闭环:
  部分平台只做了 compile/unit 级验证、移动端和打包发布缺真实环境结果。

## 3. 当前成熟度估算

| 模块 | 估算完成度 | 判断 |
|---|---:|---|
| Core reactivity / scope / owner | 85-90% | `Cage` Copy handle、Owner invalidation、effect/resource/store lens 已有测试。 |
| Widget / SSR / snapshots | 80-88% | SSR snapshot 覆盖 Each/Switch/Suspense/ErrorBoundary/forms/SVG/MathML/head/style。 |
| Command renderer / protocol | 80-85% | `Command`/`EventData`/`QueryResponse` 跨 desktop/native/liveview 复用。 |
| Routing | 70-78% | typed derive/query/redirect/history/server aviator 已有; async/lazy route 仍延后。 |
| Server functions / fullstack | 72-80% | method/encoding/middleware/stream/bytes/WS/cache/adapters 可用; 真首字节 streaming 未闭环。 |
| CLI / build / bundle / assets | 65-75% | serve flags、SSG、bundle、installers、image optimization 已有; 外部工具/签名/打包验证不足。 |
| Desktop webview | 70-80% | wry runtime、multi-window、tray/hotkey/eval/child webview 有测试; 真窗口 smoke 未常规化。 |
| Hot reload | 60-70% | reload state/relink/overlay 有测试; 自动跨重载枚举和 subsecond 二进制热补丁未做。 |
| LiveView | 55-65% | protocol/session/三适配器/e2e spec 有; registry/resume/backpressure 未完整接入协议/adapter。 |
| Native Blitz | 50-60% | command consumer、events、IME、layout、image、a11y、screenshot test 有; crate 自称 spike。 |
| Mobile | 35-45% | Android/iOS templates、reload、safe-area、smoke script 有; 真机/模拟器绿色结果仍缺。 |
| CI / release governance | 65-75% | CI 很丰富,默认 workspace test 已恢复通过;release/publish/package 未本地验证。 |

`_todos.md` 当前任务计数为 47 个 `[x]`、12 个 `[~]`、0 个 `[ ]`。这个任务板已经覆盖了大量
成熟度工作,但它更像“阶段任务已处理”而不是“项目整体可发布”。其中 12 个 `[~]` 基本都落在
外部硬件、运行态验证、架构延后或产品化深度上。

## 4. 已修复缺陷与保留 Gaps

### P0 已修复: 默认 `cargo test --workspace` 失败

本机执行:

```powershell
cargo test --workspace
```

初版结果: 失败。整改后结果: 通过。

失败原因: `crates/glory/tests/routable_derive.rs` 默认编译,但该测试直接依赖 `glory::routing`
和 `glory::Routable`;这些导出被 `routing` feature gate 保护。`crates/glory/Cargo.toml`
默认 feature 为空,所以 workspace 默认测试无法编译该集成测试。

证据:

- `crates/glory/tests/routable_derive.rs:1` 使用 `glory::routing::Routable`。
- `crates/glory/tests/routable_derive.rs:3` 使用 `#[derive(..., glory::Routable)]`。
- `crates/glory/Cargo.toml:28` `default = []`。
- `crates/glory/Cargo.toml:35` `routing = ["dep:glory-routing"]`。
- `docs/release-readiness.md` 需要把 feature-gated integration test 的 targeted command 单独列出。

修复前影响:

- 常规 workspace 测试门禁不绿。
- contributor 和 CI/release checklist 会互相矛盾。
- 这是发布治理层面的硬缺陷,即使 routing derive 本身在显式 feature 下可用。

整改:

- 已给 `crates/glory/Cargo.toml` 增加显式 test target:

```toml
[[test]]
name = "routable_derive"
path = "tests/routable_derive.rs"
required-features = ["routing"]
```

- 已把 `cargo test -p glory --features routing` 加入 `docs/release-readiness.md`。
- 已重新跑 `cargo test --workspace` 和 `cargo test -p glory --features routing`,均通过。

### P1: SSR streaming 仍不是真正的 over-the-wire 首字节流式

现状:

- `ServerHolder::new_streaming`、`HtmlChunk`、`PlaceholderPatch`、resource hydration payload 已存在。
- `render_stream()` 返回 `futures::stream::iter(self.rendered_chunks())`。
- adapter streaming response 已能把 holder stream 转为响应体。

Gap:

- 当前核心路径仍先构造 `Vec<HtmlChunk>` 再流出,不是 async resource 边完成边把 chunk 推给 HTTP body。
- `_todos.md` 中 FS4 也明确标记为 `[~]`,原因是 `!Send` 响应式 holder 需要专用 executor/channel
  才能真正边渲染边 flush。

影响:

- 可以表达 streaming SSR 结构,但还不能把它当作与 Leptos/Dioxus 成熟 streaming 同级的 TTFB 能力。

### P1: LiveView session 生命周期类型已有,但协议/adapter 没有完全闭环

现状:

- `LiveViewConfig`、`SessionRegistry<T>`、`ResumeToken`、`OutboundBuffer` 已在
  `crates/liveview/src/lib.rs` 中实现并有单测。
- `LiveViewMessage::Hello` 有协议版本协商。
- Salvo/Axum/Actix adapter 存在,Playwright spec 也已接入 CI。

Gap:

- `SessionRegistry` 目前主要是库类型和单测,adapter handle_socket 路径没有完整使用它来提供
  协议级 resume/TTL/reap。
- `docs/liveview.md` 的“Session Lifetime”仍说明没有协议级 resume token/TTL。这与底层类型存在并不矛盾,
  但说明用户可见能力还没有闭环。

影响:

- LiveView 能跑 smoke 和基础交互,但还不应被描述为生产级 session/resume 系统。

### P1: Mobile 缺真实设备/模拟器绿色结果

现状:

- Android/iOS template、mobile-counter payload、safe-area/keyboard/bootstrap、reload websocket、
  smoke script、GitHub Actions workflow 都已存在。

Gap:

- `docs/mobile-validation.md` 仍记录当前 Windows host 无在线 Android device/emulator,
  iOS simulator 需要 macOS。
- `_todos.md` MB1/MB4 仍是 `[~]`。

影响:

- Mobile 是“模板和构建路径已铺好”,不是“用户可稳定发布到移动端”。

### P1: Native Blitz 仍是 spike,虽然验证比旧报告强很多

现状:

- `glory-native` 明确标注 `Status: spike`。
- `BlitzConsumer`、shell wrapper、event mapping、layout cache、image provider、a11y feature、
  screenshot regression 都已有。
- 本机 `cargo test -p glory-native --features shell` 通过;GPU screenshot 测试默认 ignored。

Gap:

- 真实窗口交互、跨平台 GPU/driver 差异、文本选择/caret round-trip、network image provider 仍不完整。
- 需要 CI lavapipe 或本机 `--include-ignored screenshot` 覆盖真正像素路径。

影响:

- Native 路径已经证明 command stream 可行,但仍是 experimental target。

### P1: all-features / feature matrix 覆盖还不完整

现状:

- CI 有 `cargo check-all-features -p glory-core -p glory -p glory-routing`。
- feature guards 覆盖 `web-csr + web-ssr` 互斥、`backend-command + single-app` 互斥、
  `backend-command + web-ssr` 可共存。

Gap:

- `serverfn`、`liveview`、`desktop`、`native` 等重 feature crate 尚未全部纳入 all-features 组合。
- `glory` crate 的 cargo-all-features metadata 在 manifest 中仍是注释状态。

影响:

- feature 组合风险仍集中在边缘 crate 和重依赖平台 crate。

## 5. 逐维审核

### 5.1 产品定位与范围

判断: Glory 是实验性 Rust web framework,但已经进入“多平台能力原型 + 核心可用”的阶段。

完成情况:

- README 已明确 builder API、无 VDOM、CSR/SSR/desktop/mobile/native experimental path。
- Workspace 有 15 个 crate: `glory-core`, `glory`, `routing`, `cli`, `serverfn`, `desktop`,
  `native`, `liveview`, `hot-reload`, adapters, macros, tui, cli harnesses。
- examples 覆盖 counter、routing、SSR、fullstack、desktop、mobile、liveview、file upload、realtime notify 等。

Gap:

- README 的主标题和开头仍偏早期项目表述,没有把当前能力和限制分层讲清楚。
- “experimental” 与“多平台完整能力”的边界需要更明确: web/SSR 可试用,desktop/native/mobile/liveview 不能同等承诺。

### 5.2 Feature matrix

判断: 关键 feature 约束已经清晰,但默认测试 gate 有缺陷。

完成情况:

- `glory-core` 明确 `web-csr` implies `single-app`;`web-ssr` 可与 `backend-command` 共存。
- compile guards 覆盖:
  - `web-csr + web-ssr` 互斥。
  - wasm32 下 `backend-command + web-csr` 互斥。
  - `backend-command + single-app` 互斥。
- CI 有 feature-guards job。

整改状态:

- 已修复: `crates/glory/tests/routable_derive.rs` 通过 manifest `required-features = ["routing"]`
  从默认 workspace test 中跳过,并由显式 `cargo test -p glory --features routing` 覆盖。

### 5.3 Core reactivity

判断: 当前最成熟的部分之一。

完成情况:

- `Cage<T>` 是 Copy handle,有 owner invalidation、stale handle、slot recycle、active borrow 错误测试。
- `Bond<T>` 有 dependency version tracking 和 `.with_partial_eq()`。
- `effect/resource/use_future/use_coroutine` 已有。
- Store lens 和 `#[derive(Store)]` 提供 typed field projection。
- Devtools snapshot 可观察 cage/bond/subscribers。

Gaps:

- `Bond<T>` 仍是 Clone 非 Copy,这是明确设计延后项。
- `CageLens` 的字段订阅仍穿过根 Cage 版本,不是完全字段级 invalidation。
- 默认仍是 `Rc`/`RefCell` 单线程模型;SSR/LiveView 通过 holder/worker 边界规避,不是通用 Send/Sync reactivity。

### 5.4 Widget / SSR / hydration

判断: SSR 和 widget 回归覆盖扎实。

完成情况:

- `Each` 有 LIS、reorder、large reverse、random shuffle、property random tests。
- `Switch`, `Suspense`, `ErrorBoundary`, `Loader/resource`, forms, SVG/MathML, scoped style, head mixin 都有 SSR snapshot。
- Hydratable resource payload 注入存在。
- ErrorBoundary 能捕获 build/patch panic 并渲染 fallback。

Gaps:

- 真正 incremental streaming flush 未完成。
- wasm CSR 行为测试存在,但本机未跑浏览器 wasm-bindgen runner;依赖 CI/Playwright 或 wasm test runner。

### 5.5 Renderer abstraction / command stream

判断: 这是当前架构的核心优势。

完成情况:

- `Command`, `CommandQueue`, `CommandNode`, `CommandRenderer`, `EventData`, `NodeQuery`,
  `QueryResponse`, `AttributeValue` 构成统一协议。
- SSR 可通过 command replay 与 legacy SSR HTML 对齐。
- desktop JS interpreter、native Blitz consumer、LiveView client 都围绕同一协议。
- 命令 coalescing、query round-trip、event dispatch/reentrancy、handler release 有测试。

Gaps:

- CSR 仍是浏览器直接路径,非浏览器才走 command backend;这是设计权衡,但协议一致性测试要持续加强。
- Wire protocol 变更会影响 `command.rs`、`command_dom.rs`、`wry_interpreter.js` 和 liveview/native consumers,
  需要更硬的 conformance suite。

### 5.6 Routing

判断: 已从早期 wasm-only 路由提升为 typed routing 可用状态。

完成情况:

- `Routable` trait、`#[derive(Routable)]`、typed params、catch-all、query、redirect、not_found 已有。
- `MemoryAviator`, `ServerAviator`, browser aviator, scroll restoration core 已有。
- `Router` 有 route specificity ranking,静态优先于动态/catch-all。
- `Outlet`/layout tests 存在。

Gaps:

- async/lazy route 依赖 wasm-split,已延后。
- `glory` umbrella crate 的 routing derive 集成测试 feature gate 已修复;仍需持续用
  `cargo test -p glory --features routing` 覆盖。
- 浏览器 history/scroll 行为仍需要 Playwright/e2e 层持续覆盖。

### 5.7 Fullstack / server functions

判断: 功能面已经比较宽,但成熟框架级 batteries 仍不足。

完成情况:

- `#[server]` 宏、inventory registry、GET/POST、JSON/CBOR/Postcard negotiation。
- HTTP typed errors、redirect、headers、cookies、request context。
- per-function middleware。
- form/multipart。
- `StreamResponse`, `StreamingBytes`, typed `WebSocketEndpoint`。
- process-local state/cache、preloaded state、ISR/FileSystemCache。
- Salvo/Axum/Actix adapters 有测试。

Gaps:

- 真正首字节 SSR streaming 未闭环。
- `#[server]` 宏展开要求 app crate 同时直接依赖 `glory-serverfn`;这是文档化了的 DX 缺口。
- reqwest non-wasm client 是 feature gated;端到端 remote client matrix 还可加强。

### 5.8 CLI / build / bundle / assets

判断: CLI 已经不是空壳,但 release artifact 仍缺真实平台验证。

完成情况:

- `serve` 支持 address/port/open/no-open/https/tls/proxy 配置透传。
- `build --ssg` 可显式 prerender routes。
- bundle manifest、hashed assets、precompression、image WebP/responsive variants、bundle size report。
- desktop bundle 支持 macOS `.app`/DMG、codesign/notary args、Linux AppImage staging、Windows WiX auto-download/cache。
- HMR build error overlay 和 build progress model 有单测。

Gaps:

- TLS/proxy 实际监听/forwarding 仍由应用 server 实现,CLI 主要做配置透传。
- macOS signing/notarization、AppImage、WiX MSI、Android/iOS install 都依赖平台工具,未在本机验证。
- CL8 仍部分完成:字体子集化、CSS 资产指纹等未闭环。

### 5.9 Platform targets

#### Web / CSR

完成情况:

- Examples 和 Playwright CI 覆盖 counter/routing/fullstack/hot-reload 等。
- wasm behavior tests 存在。

Gaps:

- 本机未运行 browser e2e 或 wasm-bindgen browser runner。
- `web-csr + web-ssr` 必须分 target 构建,用户文档需要持续强调。

#### Desktop

完成情况:

- wry/tao runtime、multi-window、menu、tray、global hotkey、custom protocol、file drop、print、
  eval、close behavior、child webview、asset protocol 都有实现或测试。

Gaps:

- 真窗口人工 smoke、WebView runtime 差异、installer artifact 验证仍缺常规记录。

#### Native Blitz

完成情况:

- Command stream to Blitz DOM、event mapping、query、layout cache、image provider、a11y feature、screenshot regression。

Gaps:

- 仍标注 spike;真实窗口交互和 GPU screenshot 需要设备/CI。
- Network images 和文本 selection/caret round-trip 仍受 Blitz alpha 限制。

#### LiveView

完成情况:

- Protocol/session、client JS、Salvo/Axum/Actix adapters、Playwright liveview spec、version negotiation。

Gaps:

- Resume/TTL registry 未完整成为协议/adapter 用户能力。
- HTML shell 由用户负责,还缺更完整 batteries。

#### Mobile

完成情况:

- Android/iOS host templates、mobile-counter、reload、safe-area/keyboard/lifecycle webview events、smoke script。

Gaps:

- Android/iOS 真机/模拟器绿色结果缺失。
- Native permission/share/camera/gallery APIs 仍保持 recipes,不是 Glory API。

### 5.10 Testing / CI / release

判断: CI 面很宽,默认 workspace gate 已恢复通过;release/publish 仍需要补齐。

完成情况:

- CI jobs 覆盖 fmt、core/serverfn/CLI、feature guards、browser e2e、cross-platform、bench compile、
  cargo-all-features 部分 crate、MSRV、semver、native screenshot。
- 本机跑过多条 release readiness 关键命令,大多通过。

整改状态:

- 已修复: `cargo test --workspace` 恢复通过。
- 已修复: release checklist 增加 feature-gated routing derive 的 targeted command。

Gaps:

- `cargo package` / `cargo publish --dry-run` 未本机验证。
- Playwright 和 mobile smoke 未本机跑。
- cargo-all-features 尚未覆盖全部重 feature crate。

### 5.11 Docs / examples / DX

判断: 文档数量充足,但存在阶段性漂移和成熟度表述过满的风险。

完成情况:

- docs 有 API guide、fullstack、desktop、platform APIs、mobile validation、liveview、renderer author guide、
  performance、release readiness、多平台计划等。
- examples 覆盖面不错。

Gaps:

- `_todos.md` 很有价值,但有些 `[x]` 是“代码/设计/compile 级完成”,不是运行态/产品级完成。
- README 对当前能力和限制的分层仍可更清晰。
- LiveView docs 已更新为“底层 building blocks 已有,first-party adapters 尚未暴露协议级 resume/auth/reap”。

### 5.12 Security / reliability

判断: 关键安全点有意识地覆盖,但还不是完整 threat model。

完成情况:

- Workspace `unsafe_code = "deny"`。
- SSR HTML escaping、invalid names、resource hydration escaping、preloaded state script escaping 有测试。
- Desktop/native asset path traversal 防护有测试。
- CLI overlay 用 `textContent` 并测试 `</script>`/控制字符转义。
- ErrorBoundary 覆盖 build/patch panic。

Gaps:

- CSP nonce、serverfn auth/middleware、LiveView auth/session、desktop custom protocol 权限边界需要更明确 threat model。
- WebSocket/LiveView 还缺协议级 auth/resume/token 策略。

### 5.13 Performance

判断: 有基准和性能意识,但数值回归门禁还弱。

完成情况:

- Core benches: each reorder、command wire、event handlers、scheduler、ssr stream。
- CI 编译 benches。
- docs/performance 和 benchmark scripts 存在。

Gaps:

- CI 主要是 bench compile,不是性能数值回归。
- wasm size、create10k 等 benchmark 需要固定机器/阈值策略才能成为门禁。

## 6. 建议优先级

### P0

已完成:

1. `cargo test --workspace` 已恢复通过。
2. `crates/glory/tests/routable_derive.rs` 已配置 `required-features = ["routing"]`。
3. `docs/release-readiness.md` 已加入 `cargo test -p glory --features routing`。

### P1

1. 完成 FS4: 真正 over-the-wire streaming SSR flush。
2. LiveView: 将 `SessionRegistry`/TTL/resume/backpressure 接入 adapter 或明确标为 internal utility。
3. Mobile: 获取 Android emulator 和 iOS simulator 首次绿色 smoke 结果,并把结果写入 docs。
4. Native: 在 CI 或本机固定跑 ignored GPU screenshot test,记录 adapter/driver 条件。
5. 扩展 cargo-all-features 到 `serverfn`、`liveview`、`desktop`、`native` 的可行组合。

### P2

1. CLI bundle: 对 DMG/AppImage/MSI/Android/iOS artifact 做平台 smoke 或 artifact inspection。
2. Assets: 完成字体子集化、CSS 指纹/映射策略。
3. Docs: README 改成能力矩阵 + maturity badge 风格,避免把 experimental targets 写成同等成熟。
4. Security: 写一页 threat model,覆盖 SSR escaping、serverfn auth、LiveView WS、desktop custom protocol、asset roots。
5. Performance: 把关键 benchmark 变成 nightly trend,不要直接挡 PR。

## 7. 实际执行的验证

本机通过:

```powershell
cargo fmt --all --check
cargo test --workspace
cargo test -p glory --features routing
cargo test -p glory-core --lib
cargo test -p glory-core --lib --features web-ssr
cargo test -p glory-core --features backend-command --test command_backend
cargo test -p glory-serverfn
cargo test -p glory-serverfn --features "salvo axum actix"
cargo test -p glory-cli --lib --no-default-features
cargo test -p glory-routing --features web-ssr
cargo test -p glory-liveview --features "salvo axum actix"
cargo test -p glory-hot-reload
cargo test -p glory-desktop --features runtime
cargo test -p glory-native --features shell
cargo test -p glory-native --features "shell accessibility"
cargo clippy -p glory-core --lib --features web-ssr -- -D warnings
cargo clippy -p glory-serverfn --all-targets -- -D warnings
cargo clippy -p glory-cli --lib --no-default-features -- -D warnings
```

本机未验证:

- Playwright browser e2e 实跑。
- wasm-bindgen browser runner 实跑。
- Native ignored GPU screenshot test: `cargo test -p glory-native --features shell -- --include-ignored screenshot`。
- Android/iOS device or simulator smoke。
- Desktop 真窗口人工启动。
- DMG/AppImage/MSI/Android/iOS bundle artifact 实际安装运行。
- `cargo package` / `cargo publish --dry-run`。

## 8. 最终判断

Glory 当前的核心技术路线是成立的: builder widget + fine-grained reactivity +
command-stream backend 已经能支撑 SSR、desktop、native、liveview 的同构抽象。

但项目整体还不能按“成熟全平台框架”发布叙事来衡量。最需要优先处理的是:

1. 保持默认 workspace test 和 feature-gated targeted tests 同时绿色。
2. 把 streaming/liveview/mobile/native 这类 experimental 能力的“底层已实现”和“用户可依赖”边界写清楚。
3. 补齐真实平台运行态验证,尤其 mobile、desktop installers、native GPU screenshot、browser/wasm runner。

短期目标应该是: core/web/SSR/serverfn/routing/CLI 达到稳定 beta 叙事;desktop/native/liveview/mobile
继续标为 experimental,并用明确 smoke checks 推进成熟度。
