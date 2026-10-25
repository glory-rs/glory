# 2026-10-25 依赖与开放 PR 核对

本次核对 GitHub 的五个开放 Dependabot PR，以及仓库 33 个含第三方依赖的
Cargo manifest 中的 115 个注册表包。版本事实来自 crates.io API；稳定版按
SemVer 比较，不按最后发布时间排序（例如 cap-std 的 3.x 维护发布晚于 4.0.3，
不应因此降级）。执行环境为 Rust 1.98.0，所有编译、工具缓存和临时文件均在 D:。

## 开放 PR 的决定

| PR | 内容 | 决定 |
|---|---|---|
| [#42](https://github.com/glory-rs/glory/pull/42) | syn 2 → 3 | 不直接合并。工作区工具声明统一到 3.0.9；另一处修改属于尚未迁移的 hot-reload。 |
| [#43](https://github.com/glory-rs/glory/pull/43) | anyrender 0.11 → 0.12 | 不合并。目标是退役的 native 包；当前 glory-blitz 已使用最新 0.14.0。 |
| [#44](https://github.com/glory-rs/glory/pull/44) | wry 0.55 → 0.56 | 不合并。目标是退役的 WebView desktop 包；当前原生宿主是 Blitz/winit，wry 稳定版也已到 0.57.1。 |
| [#45](https://github.com/glory-rs/glory/pull/45) | Salvo 0.94 → 0.95 | 不合并旧版本。保留的工具声明直接升级到最新 1.0.1。 |
| [#46](https://github.com/glory-rs/glory/pull/46) | anyrender_vello 0.12 → 0.13 | 不合并。旧 native GPU 包已退役；当前可选 CPU 宿主使用最新 anyrender_vello_cpu 0.18.0。 |

旧 PR 的 CI 调用了已经删除的 web-ssr/backend-command feature 和 workspace 外的
旧工具包。它们的红色状态不能替代当前语义架构的兼容性测试。本次主 CI 改为
现有语义检查脚本、固定 fork 测试、WASM、真实 Windows 窗口/UIA 与 Chromium 验收。
格式检查也只针对当前 workspace 和已迁移示例。实际跨平台验收边界仍见实施记录。

## 有效依赖范围与更新

当前 11 个 workspace 包的 36 处直接注册表依赖声明（34 个独立包）均已核对并使用
所选发布通道的最高版本。默认选择稳定版；winit 沿用项目已选用的 0.31 beta 通道。
未为了追逐不兼容的传递依赖大版本而添加全局 patch 或第二套引擎类型。

- AccessKit 0.25.0 → 0.25.1、consumer 0.39.0 → 0.39.1、xplat 0.2.0 → 0.2.1。
  锁解析同时更新 Windows/macOS/Unix 的适配器依赖。
- winit 0.31.0-beta.3 → beta.4，所有 winit 平台包保持同一 beta。
  Blitz fork 从当前维护提交派生，不以已分叉的上游 main 替换本地修复。
  新增 shell IME 转换测试覆盖启用、禁用、预编辑、提交和 surrounding deletion 的
  文本及字节偏移；去掉 beta.4 已不可达的旧事件兜底。
- 新 Blitz 固定提交为
  [c850a2aee1a8cb49eee1386120807751d7480559](https://github.com/glory-rs/blitz/commit/c850a2aee1a8cb49eee1386120807751d7480559)。
  四个直接 Blitz 依赖、共享 stylo_taffy、锁文件和 provenance 一致。
- 其他有效依赖已处于最新发布，例如 cssparser 0.38.0、Fluent 0.16.0/0.12.0、
  wasm-bindgen 0.2.129、Stylo 0.22.0、anyrender 0.14.0、CPU renderer 0.18.0。
  对宽松声明提高最低版本至当前核对值；传递依赖更新包括 ICU 2.4、smallvec、uuid 等。
- 主 workspace、counter、semantic-web 与五份独立基准的八个受版本管理锁文件均已
  重新解析。Dioxus 0.7.10、Leptos 0.8.22 保持最新稳定版，未切换到 alpha/RC。
- Playwright 更新至 1.64.1，浏览器驱动支持显式从 D: 工具目录加载。
  CI Actions 更新到核对时最新发布并固定完整 SHA；Rust 工具链 Action 固定 stable
  分支提交。CI 的 Rust 版本继续固定 1.98.0，保留可复现的验证环境。
- fork 检查器从过时的十包数量断言改为验证全部十一个预期包名。

根 workspace 保留的旧工具依赖声明也已更新，包括 syn 3.0.9、Salvo 1.0.1、
getrandom 0.4.3、base64 0.23.1、zip 9.1.0 等。它们不在当前生产依赖图中，
更新声明不表示旧 CLI、宏、WebView、路由、旧示例重新得到支持。
未激活的旧目录及其内联 manifest/npm 文件仍是提取参考，不把修改版本号当成迁移；
其 API 迁移、旧 Tailwind 工具链和清理属于架构报告已列出的后续工作。

Taffy 维护分支仍固定 dd7914fba2e6e0b5d8a2c8854d764a2ade817efd。组织 fork 的
上游镜像 main 已分叉，直接换到那个 main 会丢失 Glory 的修复并引入另一套 Stylo/
布局接口，因此不作为常规依赖刷新。所有六个引擎/布局包仍为单一类型身份。

## 验证

- 原工作副本：全部 11 包、all-features 测试 921 个通过；严格原生及 WASM Clippy 通过。
- 独立 PR 分支：all-features 测试 916 个通过，严格 Clippy 通过；未带入其他 UI/性能改动。
- 两个固定 fork 的库/附加测试合计 440 次通过（不同 feature 配置之间有重复执行，不作为独立场景数）。
- 真实 Windows shell、AccessKit shell 和独立 UIA 客户端各一组通过；UIA 覆盖两个隔离窗口、Invoke/Toggle/SetValue/Focus、密码和虚拟行显示及清理。
- Playwright 1.64.1 驱动 Chromium 155.0.8059.84：全部 27 组实际浏览器验收通过，包含 CSR、SSR adoption 与渐进流。
- counter/semantic-web 原生测试和 Web 构建通过；已变更示例/基准的 WASM Clippy 与 counter desktop Clippy 通过。
- actionlint 1.7.12 检查全部工作流通过；Git diff 空白检查通过。

日志位于 D:/BuildCache/glory-dependencies。GitHub 托管 runner 的结果以 PR Checks 为准。
操作系统 IME、读屏器播报、Android 设备和 Linux 窗口不由 Windows/Chromium 结果推断。
