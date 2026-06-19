# Glory 当前完成度、缺陷与 Gaps 评审

日期: 2026-06-19

评审范围: 当前工作区 `D:\Works\glory-rs\glory`。本报告初版基于现有文档、源码结构、
feature 组合和本地测试/检查结果；后续条目会随本分支实际完成状态更新。

## 结论摘要

Glory 当前不是只有雏形的状态。核心响应式、SSR、command-stream 渲染、server
functions、desktop webview、hot reload scaffold 都已经能编译并有测试覆盖。

更准确的定位是:

- 核心架构已打通。
- Web/SSR/desktop 基本可用。
- native/mobile/liveview 已有技术路径，但仍偏 spike 或早期产品化阶段。
- 最大差距已经从“架构是否可行”转向“CI、CLI、bundle、路由、fullstack batteries、端到端验证和平台 API 完整度”。

按 `_todos.md` 任务板粗略计数:

| 状态 | 数量 |
|---|---:|
| 已完成 `[x]` | 18 |
| 部分完成 `[~]` | 5 |
| 未完成 `[ ]` | 37 |

按能力权重估算:

| 模块 | 完成度判断 |
|---|---:|
| core 响应式 / Widget / SSR 快照 | 75-85% |
| command-stream 渲染抽象 | 75-85% |
| CSR + SSR 基础使用 | 65-75% |
| server functions | 55-65% |
| CLI / bundle / dev server | 45-60% |
| desktop webview | 60-70% |
| routing | 35-45% |
| native Blitz / mobile / LiveView | 20-35% |
| CI / release governance | 35-45% |

## 初版发现的可复现缺陷与处理状态

### 1. [x] CLI release clippy 门槛失败

命令:

```powershell
cargo clippy -p glory-cli --lib --no-default-features -- -D warnings
```

初版结果: 失败。

位置: `crates/cli/src/lib.rs:77-90`

原因: `clippy::let_and_return`。`run_with_cli` 中先把 `match` 结果绑定到 `result`，随后直接返回
`result`。在 `-D warnings` 下这会变成错误。

影响:

- `docs/release-readiness.md` 中列出的 required check 当前不能全部通过。
- 这是低风险修复，但会阻断严格发布流程。

当前状态:已修复 `clippy::let_and_return`,并通过
`cargo clippy -p glory-cli --lib --no-default-features -- -D warnings`。

### 2. [x] 项目文档与当前源码存在漂移

`AGENTS.md` 仍声明 `backend-command` 与 `web-ssr` 互斥，但当前源码明确允许它们共存。

当前源码证据:

- `crates/core/src/lib.rs:58` 附近注释说明: SSR convergence 后 `backend-command + web-ssr` 可共存。
- 只保留以下 compile guards:
  - `web-csr + web-ssr` 互斥。
  - wasm32 下 `backend-command + web-csr` 互斥。
  - `backend-command + single-app` 互斥。

实际验证:

```powershell
cargo check -p glory-core --features "web-ssr backend-command"
```

结果: 通过。

影响:

- AI/贡献者会按旧约束判断 feature matrix，容易误判。
- release readiness 也仍保留“workspace-wide failures 是历史问题”的表述，但当前 `cargo test --workspace` 已通过。

当前状态:`AGENTS.md` 与 `docs/release-readiness.md` 已同步当前 feature matrix 和
workspace test 状态。

### 3. [~] CI 覆盖不足

`.github/workflows` 初版主要包含:

- `format.yml`
- `release.yml`

当前已新增 `ci.yml` 覆盖 Rust 主路径，但还没有常规 CI 跑以下端到端检查:

- Playwright 浏览器 e2e。
- mobile/device smoke。

影响:

- 本地质量基线不错，但没有被 CI 强制执行。
- 后续改动可能破坏 command protocol、SSR replay 或 CLI 行为而不被 PR 检出。

当前状态:已新增主 Rust CI,覆盖 fmt、targeted tests、feature guard 与 clippy gates。
Playwright 真浏览器、mobile/device smoke 仍未闭环。

## 已完成度较高的部分

### Core / Reactivity

当前 `Cage<T>` 已是 Copy handle，并带 generation/stale-handle 语义。

测试覆盖包括:

- Copy handle 共享状态。
- stale handle。
- owner drop invalidation。
- active borrow 错误。
- slot recycle。
- resource stale completion 防护。
- `Bond::with_partial_eq`。
- devtools snapshot。

主要路径:

- `crates/core/src/reflow/cage.rs`
- `crates/core/src/reflow/bond.rs`
- `crates/core/src/reflow/effect.rs`

### SSR / Widgets

SSR feature 测试覆盖较完整:

- `Each` reorder / random shuffle / large reverse。
- `Switch`。
- `resource` 初始与依赖变化。
- `effect` mount/revision。
- head mixin。
- scoped style。
- form controls。
- SVG/MathML。
- streaming boundary chunks。

主要路径:

- `crates/core/src/widgets/snapshot_tests.rs`
- `crates/core/src/web/holders/server.rs`
- `crates/core/src/renderer/ssr_dom.rs`

### Command Stream

非浏览器路径已经有统一 command protocol:

- `Command`
- `AttributeValue`
- `EventData`
- `NodeQuery`
- `QueryResponse`
- `CommandNode`
- `CommandQueue`

验证覆盖:

- command coalescing。
- command DOM replay。
- SSR replay 与 legacy HTML 对齐。
- event dispatch/reentrancy。
- query round-trip。
- handler release。
- desktop JS interpreter wire shape。

主要路径:

- `crates/core/src/renderer/command.rs`
- `crates/core/src/renderer/command_dom.rs`
- `crates/core/src/renderer/ssr_dom.rs`
- `crates/desktop/src/wry_interpreter.js`

### Server Functions

已有能力:

- `#[server]` 宏注册 endpoint。
- JSON 参数/结果。
- HTTP typed errors / redirect / headers。
- request context。
- cookies。
- form / multipart。
- NDJSON / SSE helpers。
- streaming response helpers。
- process-local server state/cache。
- preload state。
- Salvo/Axum/Actix adapter feature 编译与测试。

主要路径:

- `crates/macros/src/lib.rs`
- `crates/serverfn/src/lib.rs`
- `crates/serverfn/tests/server_fn.rs`

### Desktop

desktop runtime 已经是真 wry/tao 宿主，不只是测试 sink。

已有能力:

- launch / launch_with_config。
- multi-window。
- menu。
- custom `glory://` protocol。
- hot reload message shape。
- IPC event/query 回程。
- asset path traversal 防护。
- command surface interpreter 测试。

主要路径:

- `crates/desktop/src/runtime.rs`
- `crates/desktop/src/lib.rs`
- `crates/desktop/src/wry_interpreter.js`

## 主要 Gaps

### 1. Routing 是当前最大的用户层差距

现状:

- `Router` + runtime string path/filter。
- 有路径解析和检测测试。
- 手写 `Routable` trait、typed `goto_route`、`Locator::route::<R>()` 第一阶段已补。
- 查询字符串可读写，但没有类型化解析体系。

缺口:

- 类型化 route 的 derive/builder 自动生成仍未落定。
- 嵌套布局 / Outlet。
- redirect。
- 404 默认处理。
- query 参数 trait 化解析。
- catch-all 段类型化。
- scroll restoration / go back / go forward。

主要路径:

- `crates/routing/src/router.rs`
- `crates/routing/src/filters/path.rs`
- `crates/routing/src/locator.rs`

### 2. Fullstack batteries 仍少

serverfn 基础可用，但对比成熟框架还缺:

- 多 HTTP method 已补第一阶段:`#[server(method = "GET")]` 可用。
- 多编码协商，例如 CBOR/MessagePack/Postcard。
- 逐函数 middleware。
- 响应式 WebSocket hook。
- 原生 framework extractor 直通设计。

### 3. CLI / Serve / Bundle 产品面偏薄

现状:

- `ServeOpts` 已有 `--no-reload`、`--address`、`--port`、`--open`、`--no-open`。
- `serve` 命令只是 build 后 spawn server。
- `Bundle` 已有 manifest 和压缩资产处理，但平台矩阵和 installer 仍不足。

缺口:

- HTTPS。
- proxy。
- 运行中按键交互，比如 rebuild、verbose、help。
- desktop installer，例如 MSI/DMG/DEB/AppImage。
- shell completions / self-update 提示。

主要路径:

- `crates/cli/src/config/cli.rs`
- `crates/cli/src/command/serve.rs`
- `crates/cli/src/command/bundle.rs`

### 4. Assets 管线还不成熟

现状:

- `asset!` 可声明路径并生成 public path。
- bundle 会生成 `glory-bundle.json`，记录文件 hash/size/compression 等。

缺口:

- 编译期存在性校验。
- link-time 或 build-time typed manifest。
- 内容 hash 文件名 rewrite。
- 图片优化。
- folder asset macro。
- CSS Modules。

### 5. Desktop API 不完整

已有 runtime 很扎实，但平台 API 仍缺:

- drag window。
- fullscreen/maximize/focus/zoom。
- runtime open window / close by id。
- tray icon。
- global hotkey。
- async custom protocol。
- drag-and-drop file events。
- print dialog。

### 6. Native Blitz 仍是 spike

现状:

- `glory-native/shell` 可编译。
- command stream 能写入 Blitz document。
- click/input/keyboard/IME 部分事件已映射。
- property/attribute 分离已有测试。

缺口:

- `Command::Query` 当前 no-op。
- scroll/focus/touch 等事件桥接不足。
- 窗口生命周期很薄。
- 真实渲染截图/人工验证缺失。
- a11y 未接入。

主要路径:

- `crates/native/src/blitz_consumer.rs`

### 7. LiveView 可用但架构仍早期

现状:

- command stream over WebSocket。
- reconnect client。
- Salvo route。
- mount/event/ping/query 基础测试。

缺口:

- 当前 Salvo adapter 每 session 一个 `std::thread` + `mpsc`，并发扩展性有限。
- 缺 Axum/Actix adapter。
- 缺端到端浏览器场景。
- query 应答路径还需要更完整验证。
- 缺 session TTL、重连退避、模板配置。

主要路径:

- `crates/liveview/src/lib.rs`

### 8. Mobile 缺真机/模拟器闭环

现状:

- Android/iOS 模板。
- host check。
- safe-area / keyboard resize / foreground-background JS bootstrap。
- device smoke script。

缺口:

- Android emulator/device 实际安装启动记录。
- iOS simulator/device 验证。
- 设备上 hot reload，Android 需 `adb reverse` 或局域网注入。
- mobile CI 夜间 smoke。

主要路径:

- `crates/cli/templates/mobile`
- `examples/mobile-counter`
- `scripts/mobile-device-smoke.ps1`
- `docs/mobile-validation.md`

### 9. Browser E2E 没有 CI 闭环

Playwright 项目已存在，但测试依赖环境变量，缺失时会 skip。

缺口:

- CI 启动 CSR counter、router、SSR hydrate、fullstack、hot reload 示例。
- CI 设置 `GLORY_*_URL` 并实际跑 Chromium。

主要路径:

- `tests/playwright`

## 实际执行的验证

通过:

```powershell
cargo test --workspace
cargo test -p glory-core --lib
cargo test -p glory-core --lib --features web-ssr
cargo test -p glory-core --features backend-command --test command_backend
cargo check -p glory-core --features "web-ssr backend-command"
cargo test -p glory-routing --lib --features web-ssr
cargo test -p glory-serverfn
cargo test -p glory-serverfn --features "salvo axum actix"
cargo test -p glory-liveview
cargo test -p glory-cli
cargo test -p glory-cli-harnesses
cargo test -p glory-hot-reload
cargo check -p glory-desktop --features runtime
cargo test -p glory-desktop --features runtime
cargo check -p glory-native --features shell
cargo test -p glory-native --features shell
cargo check -p glory --features "web-ssr backend-command routing server-fn"
cargo check -p glory-salvo
cargo check -p glory-axum
cargo check -p glory-actix
cargo fmt --all --check
cargo clippy -p glory-cli --lib --no-default-features -- -D warnings
cargo clippy -p glory-core --lib --features web-ssr -- -D warnings
cargo clippy -p glory-serverfn --all-targets -- -D warnings
cargo test -p glory-cli --lib --no-default-features
```

初版失败、当前已修复:

```powershell
cargo clippy -p glory-cli --lib --no-default-features -- -D warnings
```

初版失败原因: `crates/cli/src/lib.rs:77-90` 的 `clippy::let_and_return`。

未验证:

- wasm32 CSR browser test。
- Playwright 真浏览器 e2e。
- examples 全量 build/run。
- mobile device/emulator。
- desktop runtime 人工窗口启动。
- native Blitz 真实窗口截图/交互。
- cargo package/publish。

## 建议优先级

### P0

- [x] 修复 `glory-cli` clippy failure。
- [x] 更新 `AGENTS.md` / `docs/release-readiness.md` 中与当前 feature matrix 和 workspace test 状态不一致的内容。
- [x] 增加主 CI:
   - fmt。
   - core default/web-ssr/backend-command tests。
   - serverfn tests。
   - CLI tests。
   - feature combination checks。
   - clippy required checks。

### P1

1. [x] 设计并实现 typed routing 第一阶段。
2. [~] 扩展 `glory serve` 参数和基本 DX。
   - 已完成 `--address/--port` 运行时覆盖、默认 auto-open、`--no-open`。
   - 未完成 HTTPS 与 proxy 配置。
3. [x] serverfn 支持 method 选择，至少 `#[server(method = "GET")]`。
4. Playwright CI 跑 CSR counter + SSR hydration 两个最小场景。
5. Desktop 补窗口控制 API 包。

### P2

1. serverfn 多编码和 middleware。
2. asset typed manifest / hash rewrite。
3. LiveView async session worker 和 Axum/Actix adapter。
4. Native query/layout 接通。
5. Mobile Android emulator smoke。

### P3

1. Native a11y。
2. CSS Modules。
3. 图片优化。
4. Tray/global hotkey/drag-drop/print 等 desktop 扩展。
5. 更完整 devtools/telemetry。
