# Glory 正式发布剩余任务报告

日期: 2026-07-15

审计范围: `D:\Works\glory-rs\glory`, branch `main`。本次重新读取
`_report.md`、`_current_report.md`、`docs/release-readiness.md`、workspace
manifest 和 release workflow,并补跑发布打包检查。

本报告只保留未完成事项。已完成的历史整改不再展开。

## 发布距离判断

| 发布面 | 当前距离 | 未完成原因 |
|---|---:|---|
| Core / SSR / routing / serverfn / CLI beta | 约 2-4 个集中 PR | 版本策略、package/publish 顺序、浏览器/wasm e2e 和 release artifact 验证仍未闭环。 |
| Web 正式发布 | 约 1-2 个里程碑 | 需要稳定的发布流水线、docs.rs/package 验证、Playwright/wasm runner 本地或 CI 证据。 |
| 全平台正式发布 | 约 2-3 个里程碑以上 | desktop/native/liveview/mobile 仍有真实运行态、安装包、会话恢复、GPU/设备验证缺口。 |

当前不应按“成熟全平台框架”发布叙事。短期更合理的目标是:
`core + web + SSR + routing + serverfn + CLI` 进入可验证 beta,同时继续把
desktop/native/liveview/mobile 明确标为 experimental。

## 本轮发布阻塞证据

- `cargo package -p glory-core`: 通过。
- `cargo package -p glory-macros`: 通过。
- `cargo package -p glory`: 失败,package tarball 解析不到 registry 中的
  `glory-macros = 0.3.1`。
- `cargo package -p glory-routing`: 失败,dev-dependency `glory-macros = 0.3.1`
  当前无法从 registry 解析。
- `cargo package -p glory-serverfn`: 失败,dev-dependency `glory-macros = 0.3.1`
  当前无法从 registry 解析。
- `cargo package -p glory-cli`: 失败,tarball 验证阶段从 registry 解析到的
  `glory-hot-reload = 0.3.1` 不包含当前源码使用的
  `HotReloadFunctions` / `FunctionReloadBatch` API。

## P0: 发布阻塞

- [ ] 决定并执行下一版版本策略。当前 crates.io 已有 `glory`、
  `glory-cli`、`glory-hot-reload`、`glory-core`、`glory-routing` 的
  `0.3.1`;当前源码不能按同一个 `0.3.1` 重新发布。正式发布前需要 bump
  workspace version,或把源码恢复到 registry 已发布 API 的兼容状态。
- [ ] 按依赖顺序验证并发布核心发布面:
  `glory-hot-reload` -> `glory-macros` -> `glory-core` ->
  `glory-routing` -> `glory-serverfn` -> `glory` -> `glory-cli`。
- [ ] 每发布一个上游 crate 后,等待 crates.io index 可见,再 package/publish
  下游 crate。否则 `cargo package` 会把 workspace path dependency 转成
  registry dependency 后失败。
- [ ] 对核心发布面逐个跑 `cargo package -p <crate>` 和
  `cargo publish -p <crate> --dry-run`,并把结果记录到
  `docs/release-readiness.md`。
- [ ] 处理 `spin v0.10.0` yanked warning: 确认它只来自可接受的间接依赖,
  或升级依赖链让 release dry-run 不再带 yanked lockfile 风险。
- [ ] 为所有需要发布到 docs.rs 的 crate 明确 docs.rs feature set;不能依赖
  `all-features` 在互斥 feature 上碰运气。

## P1: 正式发布质量门禁

- [ ] 跑完 `docs/release-readiness.md` 的 Required Checks、Feature Matrix、
  Example Smoke、Release Artifacts、Documentation、Publishing 全部条目。
- [ ] 把 Playwright browser e2e 和 wasm-bindgen browser runner 的实跑结果纳入
  release 证据,而不是只依赖普通 Rust unit tests。
- [ ] 完成真正 over-the-wire SSR streaming flush。当前 streaming 结构存在,
  但核心路径仍偏向先构造 chunk 再输出,还不是资源完成后边渲染边 flush。
- [ ] LiveView 剩余治理: 为 resume token 增加认证绑定、跨刷新持久化策略和
  无流量时的后台 reaper;当前不应把内存 token 当作安全会话凭据。
- [ ] 扩展 cargo-all-features 或等价 feature matrix 到 `glory-serverfn`、
  `glory-liveview`、`glory-desktop`、`glory-native` 等重 feature crate。
- [ ] 建立 release conformance suite,覆盖 `Command`、`EventData`、
  `QueryResponse`、`renderer/command_dom.rs`、desktop JS interpreter、
  native/liveview consumer 的协议一致性。

## P1: 平台运行态缺口

- [ ] Web: 记录 CSR/SSR/hydration/browser history/scroll 的浏览器级 smoke
  结果,并确保 `web-csr` 与 `web-ssr` 分 target 构建的文档足够明确。
- [ ] Desktop: 固化真窗口 smoke、WebView runtime 差异、asset protocol、
  menu/tray/hotkey 和 installer artifact 验证。
- [ ] Native: 在 CI 或固定本机环境跑 ignored GPU screenshot test,并补真实窗口
  交互、文本 selection/caret round-trip、network image provider 验证。
- [ ] LiveView: 完成协议级 auth/session/token 策略,并补充断线恢复的浏览器级
  e2e 覆盖。
- [ ] Mobile: 获得 Android emulator/device 与 iOS simulator/device 的绿色 smoke
  结果,并更新 `docs/mobile-validation.md`。

## P2: 产品化缺口

- [ ] CLI bundle: 对 DMG/AppImage/MSI/Android/iOS artifact 做平台 smoke 或
  artifact inspection,不要只停留在 bundle manifest 生成。
- [ ] Assets: 完成字体子集化、CSS 资产指纹和 source map/manifest 映射策略。
- [ ] Security: 写 threat model,覆盖 SSR escaping、serverfn auth/middleware、
  LiveView WS/session、desktop custom protocol、asset roots、CSP nonce。
- [ ] Performance: 把关键 benchmark 变成 nightly trend;PR 阶段只做 compile 或
  粗 smoke,避免不稳定性能阈值阻塞普通贡献。
- [ ] Docs/DX: README 和 docs 需要用 maturity matrix 区分 stable beta、
  experimental、spike,避免把 desktop/native/liveview/mobile 写成同等成熟。
- [ ] Examples: release 前至少选定 3-5 个 flagship examples,保证它们从
  `cargo install glory-cli` 到运行/构建的路径可复现。

## 继续审核维度

| 维度 | 下一步只看未完成项 |
|---|---|
| 发布与版本 | version bump、crates.io 顺序、dry-run、release workflow。 |
| Feature matrix | 互斥 feature、重依赖 feature、docs.rs feature set。 |
| Web runtime | CSR/SSR/hydration/streaming/browser e2e。 |
| Fullstack | server function adapters、streaming、auth、middleware、client matrix。 |
| Renderer protocol | command wire compatibility、desktop/native/liveview conformance。 |
| Platform targets | desktop installers、native GPU、mobile devices、liveview sessions。 |
| Security | XSS/path traversal/CSP/session token/custom protocol threat model。 |
| Performance | benchmarks、wasm size、render/patch trend、artifact size。 |
| Docs/DX | maturity language、examples、release notes、migration notes。 |
