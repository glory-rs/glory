# Glory 改进报告

生成时间：2026-06-01
状态：主任务列表已全部完成并勾选。

## 主任务清单

- [x] **IMP-001：修复 `.text()` 在 SSR/CSR 下可注入 HTML 的 XSS 风险。**
  - SSR 文本与属性值现在会转义；CSR `inner_text` 改用 `textContent`；`.html()` 保持显式 raw HTML 入口。
  - 覆盖：`outer_html_escapes_text_and_attribute_values`、`outer_html_keeps_explicit_inner_html_raw`。

- [x] **IMP-002：修复 `glory-hot-reload` 严格 clippy 失败。**
  - 清理 `ViewMacros::patch` 的 `collapsible_match`。

- [x] **IMP-003：清理 `glory-core` 严格 clippy。**
  - 清理 `clone_on_copy`、`map_clone`、`collapsible_if`、`question_mark`、`explicit_auto_deref`、`derivable_impls` 等问题。
  - 同步清理 test/bench 目标中的 `Cage` Copy 迁移残留和 deprecated `criterion::black_box`。

- [x] **IMP-004：修复 `glory-cli` 测试模块落后于 `Opts` 结构的问题。**
  - 测试构造补齐 `BuildTarget`；deprecated `insta::assert_display_snapshot!` 改为 `assert_snapshot!`。
  - 补回 CLI 测试所需的最小 Cargo fixture，并更新快照/命令断言。
  - 额外清理 CLI 严格 clippy 暴露的机械性问题。

- [x] **IMP-005：修复 `glory-routing` 测试编译断点和死代码警告。**
  - 更新过时测试导入与 `Filter` 调用签名；移除旧 `MethodFilter` 测试；默认 feature 改为空，避免 workspace 中 SSR/CSR feature 冲突。
  - 导出 `LocatorModifier`，并清理 routing 严格 clippy。

- [x] **IMP-006：接入浏览器路由 programmatic navigation。**
  - `BrowserAviator::goto` 现在支持 `replace`、滚动语义，并在 history 更新后触发 `locate`。
  - anchor click 复用同一路径，避免只改地址栏不匹配路由。

- [x] **IMP-007：移除 SSR 路由 `goto` panic。**
  - `Aviator::goto` 改为返回 `Result<(), NavigationError>`。
  - `ServerAviator::goto` 走可失败的 `locate`，SSR 下 path-only URL 可解析。

- [x] **IMP-008：补齐 builder-pattern hot reload 接入路径。**
  - 详见 `_improve_hot_reload_tasks.md`。
  - 新增 `reloadable_view!`；CLI watch 停止接入不匹配的 `ViewMacros`/`view!` patch path，只发送 builder-style function replacement。

- [x] **IMP-009：补齐 desktop Wry JS interpreter 的命令消费。**
  - 详见 `_improve_desktop_tasks.md`。
  - Interpreter 现在消费 create、attribute、property、class、text、HTML、insert/remove、event attach，并通过 IPC 转发事件。

- [x] **IMP-010：校验 SSR 动态标签名/属性名。**
  - SSR 输出层过滤非法 tag/attr/property 名；非法 tag fallback 到 `div`。
  - 覆盖：`outer_html_skips_invalid_names`。

- [x] **IMP-011：完成 dev auto-reload 注入脚本 CSP nonce 设计。**
  - `GloryConfig` 新增 `csp_nonce`，支持 `GLORY_CSP_NONCE`。
  - SSR head、stream hydrate script、module script、watch reload script 使用转义后的 nonce。
  - 覆盖：`ssr_head_applies_escaped_csp_nonce`。

## 拆分任务文件

- [x] `_improve_hot_reload_tasks.md`：IMP-008 的 hot reload 子任务已全部完成。
- [x] `_improve_desktop_tasks.md`：IMP-009 的 desktop renderer 子任务已全部完成。

## 最终验证

- [x] `cargo test -p glory-core --lib`：通过，32 tests。
- [x] `cargo test -p glory-core --lib --features web-ssr`：通过，55 tests。
- [x] `cargo clippy -p glory-core --all-targets --features web-ssr -- -D warnings`：通过。
- [x] `cargo check -p glory-core --features web-csr`：通过。
- [x] `cargo check -p glory-cli --all-targets`：通过。
- [x] `cargo test -p glory-cli --lib`：通过，13 tests。
- [x] `cargo clippy -p glory-cli --all-targets -- -D warnings`：通过。
- [x] `cargo test -p glory-routing --lib`：通过，40 tests。
- [x] `cargo check -p glory-routing --features web-csr`：通过。
- [x] `cargo check -p glory-routing --features web-ssr`：通过。
- [x] `cargo clippy -p glory-routing --all-targets -- -D warnings`：通过。
- [x] `cargo test -p glory-hot-reload --lib`：通过，12 tests。
- [x] `cargo clippy -p glory-hot-reload --all-targets -- -D warnings`：通过。
- [x] `cargo test -p glory-desktop --lib`：通过，3 tests。
- [x] `cargo clippy -p glory-desktop --all-targets -- -D warnings`：通过。
- [x] `cargo check --workspace --all-targets`：通过。
- [x] `cargo test --workspace --lib`：通过。
- [x] `cargo fmt --all -- --check`：通过。
- [x] `git diff --check`：通过；Git 仅提示部分工作区文件下次触碰时会做 CRLF/LF 规范化，无空白错误。

## 当前结论

本报告识别的 11 个主问题已全部修复并勾选；拆分任务文件也已全部勾选。当前没有已知未通过的验证项。
