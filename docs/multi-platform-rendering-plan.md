# Glory 多平台渲染状态与下一阶段计划

> 更新时间:2026-06-11。
> 参照对象:本地 `E:\Repos\dioxus`。
> 当前结论:早期"先统一渲染抽象"的地基阶段已经完成;现在的主要差距转向产品化、测试矩阵、打包链路和平台后端成熟度。

---

## 0. 当前状态

Glory 现在不再是单纯的 Web/SSR 框架。当前主线已经具备:

| 能力 | 当前状态 | 主要路径 |
|---|---|---|
| 浏览器 CSR | 可用 | `crates/core/src/web/holders/browser.rs`, `web-csr` |
| SSR | 可用;非 wasm 路径已收敛到 command replay | `crates/core/src/web/holders/server.rs`, `renderer/ssr_dom.rs` |
| 指令流后端 | 可用;非浏览器目标共享 `CommandNode` | `crates/core/src/renderer/command.rs` |
| Desktop WebView | 可用;支持 wry/tao、多窗口、菜单、custom protocol、hot reload | `crates/desktop` |
| Server functions | 可用;Salvo/Axum/Actix adapter + request context | `crates/serverfn`, `crates/macros` |
| Mobile | 模板、交叉编译路径、CI smoke、设备验证脚本已落地;真机/模拟器实际运行需外部设备 | `crates/cli/src/compile/mobile.rs`, `crates/cli/templates/mobile`, `scripts/mobile-device-smoke.ps1` |
| Native Blitz | `blitz-dom` consumer、`blitz-shell`/vello 窗口入口、click/input 回程桥已落地 | `crates/native` |
| TUI | 只读 command DOM 展示定位 | `crates/tui` |
| LiveView | 协议/session crate、浏览器 reconnect client、Salvo WebSocket route 已落地 | `crates/liveview` |

与 Dioxus 的核心差异仍然成立:

| | Dioxus | Glory |
|---|---|---|
| UI 构建表面 | `rsx!` + VDOM | builder-pattern widgets + fine-grained subscriptions |
| 核心更新产物 | VDOM diff 后的 mutation stream | 视图订阅触发直接 patch,非浏览器路径输出 command stream |
| 多平台接入 | 每个平台实现 mutation consumer | 非浏览器平台消费 `Command`/`EventData`;CSR 仍走浏览器直连路径 |
| 当前成熟度差距 | CLI、bundle、docs、测试、平台 API 很完整 | 核心路已通,产品化外壳仍薄 |

---

## 1. 已完成的关键架构转折

早期规划里的阶段 0/阶段 1 已经不是待办:

- `Node` 的非浏览器路径已统一为 `CommandNode`。
- SSR 已从独立内存节点主路径收敛为 command replay。
- `Command`, `EventData`, `NodeQuery`, `QueryResponse` 已成为跨 desktop/native/liveview 的协议核心。
- Desktop runtime 已经是真 wry/tao 宿主,不是测试 sink。
- Desktop IPC 事件回程、query 回程、hot reload、自定义资源协议、多窗口和菜单都已接通。
- `asset!` 已存在,desktop 通过 `glory://` custom protocol 读取 assets。
- `#[glory::server]` server functions 已接通三种 adapter,并支持 request context。
- Mobile 已有 Android/iOS 模板和交叉编译路径。
- Blitz DOM consumer 已验证 command stream 可以驱动真实 native DOM;`glory-native/shell`
  进一步提供 `blitz-shell`/vello window entrypoint 和 click/input 回程桥。
- LiveView 已有 `glory-liveview` crate,复用 command stream over WebSocket。

这意味着后续不应再围绕"是否需要 command stream"争论;真正要做的是把这个基础变成可发布、可测试、可维护的产品面。

---

## 2. 下一阶段优先级

### P0:发布可信度

1. **CI 主工作流**
   - 增加 `cargo fmt --all --check`。
   - 增加 targeted tests:`glory-core` default/web-ssr/backend-command, `glory-serverfn`, `glory-cli`。
   - 增加 feature-combination compile checks,覆盖 `web-csr`/`web-ssr`/`backend-command`/`single-app` 规则。

2. **当前任务板与文档收敛**
   - `_todos.md` 是当前成熟度任务板。
   - `_improve_todos.md` 保留为 2026-06-11 基线档案,由 `_todos.md` 接续。
   - 本文档只记录当前架构状态和下一阶段计划。

3. **官方性能基线**
   - 现有 js-framework-benchmark 本地 harness 结果健康。
   - `benchmarks/official-js-framework-benchmark.ps1` 已能生成官方
     `frameworks/keyed/glory-rs` / `dioxus-rs` adapters,完成 release build,
     并跑通官方 Chrome/Puppeteer tracing CPU 01-09。
   - 后续任何 command/scheduler/hydration 改动都要和基线对比。

### P1:产品化外壳

1. **`glory bundle` 平台矩阵**
   - 当前 `bundle` 只支持 Web。
   - 下一步拆成 `bundle_web`, `bundle_desktop`, `bundle_android`, `bundle_ios`。
   - Desktop bundle 至少应包含 exe、assets root、配置元数据和图标。

2. **`glory doctor`**
   - 检查 Rust targets、wasm-bindgen、wasm-opt、cargo-ndk、Android NDK、Xcode/XcodeGen、WebView runtime。
   - 这是 mobile/desktop 用户体验的最短补强。

3. **一等模板**
   - Web CSR、SSR、fullstack、desktop、mobile 模板应能从本仓库 CLI 直接生成。
   - 远程 `cargo-generate` 可以保留,但不应是唯一入口。

4. **Playwright/e2e**
   - `glory end2end` 已有 hook,但仓库缺少一等 Playwright 项目。
   - 优先覆盖 CSR counter、SSR hydrate、router、serverfn、hot reload。

### P1/P2:平台后端成熟

1. **Mobile**
   - Android/iOS:模板、Rust host check、safe-area、键盘 resize、前后台 lifecycle events、CI smoke、设备 smoke 脚本已完成。
   - 剩余缺口是真机/模拟器运行记录和 APK/simulator 产物验证;当前 Windows 机器能找到 `adb`,但没有在线 Android 设备/模拟器,且不能运行 iOS simulator。

2. **Native Blitz**
   - 已有 `glory-native/shell` 真窗口入口和 click/input 回程桥。
   - 下一步是人工/截图验证 counter 真实绘制、布局边界、输入法、a11y 和文本布局。

3. **LiveView**
   - `glory-liveview` 已复用 desktop JS interpreter 思路,传输层为 WebSocket。
   - 服务器持有 `CommandHolder`,浏览器消费 commands 并回传 `EventData`/`QueryResponse`。
   - 下一步是更多 adapter 和端到端浏览器场景。

---

## 3. 与 Dioxus 的成熟度差距

Dioxus 当前领先点主要不是"能不能多平台",而是:

- CLI 平台选择、bundle、doctor、config schema 更完整。
- Playwright 测试和 CLI harnesses 更成体系。
- Fullstack 有更多 batteries:streaming、SSE、uploads、forms、middleware、server state。
- 文档站、教程、examples 和 API reference 更完整。
- Devtools 与 telemetry/diagnostics 更成产品。
- Assets 生态更成熟,包括 resolver、manganis、CSS modules、优化流程。

Glory 的现实策略:

- 保持 builder API 和 fine-grained 更新模型作为差异点。
- 不追 RSX/DSL,也不引入 VDOM。
- 用 command stream 复用非浏览器平台。
- 优先补 CLI、CI、docs、bundle、fullstack examples,让已经打通的架构变成用户能稳定使用的东西。

---

## 4. 当前任务入口

以 [`../_todos.md`](../_todos.md) 为当前任务板;[`../_improve_todos.md`](../_improve_todos.md)
保留为 2026-06-11 基线档案:

- Lane A:release governance + docs。
- Lane B:CLI, bundle, templates, platform serve。
- Lane C:fullstack batteries。
- Lane D:platform backends。
- Lane E:performance and size。
- Lane F:testing and conformance。
- Lane G:public API and ecosystem ergonomics。

推荐执行顺序:

1. A2/A3/F1/F2:文档和 CI 可信度。
2. B1/B2/D1/F4:desktop bundle 与 harness。
3. C1/C2/C3/F3:fullstack 用户体验。
4. D2/D3/F7:mobile 真机/CI。
5. E1/E2/E3:正式性能和体积基线。
6. D4/D5:Blitz 真窗口与 LiveView 并行探索。

---

## 5. 关键路径速查

| 功能 | 路径 |
|---|---|
| Command protocol | `crates/core/src/renderer/command.rs` |
| Command DOM reference interpreter | `crates/core/src/renderer/command_dom.rs` |
| SSR replay DOM | `crates/core/src/renderer/ssr_dom.rs` |
| Server holder | `crates/core/src/web/holders/server.rs` |
| Command holder | `crates/core/src/web/holders/command.rs` |
| Desktop runtime | `crates/desktop/src/runtime.rs` |
| Desktop JS interpreter | `crates/desktop/src/wry_interpreter.js` |
| Server functions | `crates/serverfn/src/lib.rs`, `crates/macros/src/lib.rs` |
| Mobile compile path | `crates/cli/src/compile/mobile.rs` |
| Mobile templates | `crates/cli/templates/mobile` |
| Blitz consumer | `crates/native/src/blitz_consumer.rs` |
| Current maturity board | `_todos.md` |
