# Glory 代码评审报告（2026-09-02）

评审范围：`D:\Works\glory-rs\glory`，branch `main`，含未提交的 `crates/liveview` / `.github/workflows/release.yml` / `docs` 改动。

方法：15 个 crate 的源码全量通读（8 路并行审计，其中横切面一路中断后由主会话补做），
所有"无调用者"结论均经全仓库 `grep` 复核（含 `examples/`、`docs/`）；关键结论跑了
`cargo check` / `cargo test` 实测。

> 上一轮 `_improve.md`（IMP-001 ~ IMP-011）的 11 项已全部完成并验证通过，本报告是新一轮独立评审，不重复其内容。

---

## 0. 当前门禁基线（本次实测）

| 检查 | 结果 |
|---|---|
| `cargo test --workspace --lib` | 通过，297 tests |
| `cargo clippy --workspace --all-targets` | 通过，仅 33 条机械性 warning（`Cage` Copy 上的 `clone`、`redundant reference`） |
| `cargo clippy -p glory-core --all-targets --features web-ssr -- -D warnings` | 通过 |
| `cargo test -p glory-core --features web-ssr` | 通过，109 tests |
| `cargo test -p glory-serverfn` / `-p glory-liveview` / `-p glory --features routing` | 全部通过 |
| `cargo check`（8 个 Salvo 示例） | **失败**，salvo 0.93/0.94 版本冲突，见 P0-9 |

**门禁全绿，但门禁覆盖面本身是本报告最核心的发现**：大量在 `_todos.md` / `_improve_todos.md` 中标记
`[x]` 的功能，其单元测试只覆盖了纯函数部分，端到端接线从未验证。下面 §4「实现了但没接入」
的 30 余项全部处于"有类型、有单测、无调用者"状态。

---

## 0.1 整改状态（2026-09-02 同日完成第一至第五批）

本报告的前三批修复已全部落地并带回归测试，第四、五批完成了优先级最高的部分。
修复后的门禁：

| 检查 | 结果 |
|---|---|
| `cargo test --workspace --lib` | 通过，15 个测试目标 |
| `cargo test -p glory-core --features web-ssr` | 通过，114 tests（新增 5 个回归测试） |
| `cargo test -p glory-serverfn --features "salvo axum actix"` | 通过，45 tests（新增 5 个） |
| `cargo test -p glory-liveview --features "salvo axum actix"` | 通过，19 tests（新增 4 个） |
| `cargo test -p glory-routing --lib` / `-p glory --features routing` | 通过，69 / 5 tests（新增 5 个） |
| `cargo clippy --workspace --all-targets` | 通过，**0 warning**（此前 33 条） |
| 8 个 crate 的 `-D warnings` 严格 clippy | 通过，含此前从未跑过严格检查的 desktop / native |
| `cargo fmt --all -- --check` | 通过 |

第二轮后新增的测试目标：`ssr_runtime_flavors`（3）、routing `navigation`（11）、
serverfn `server_fn`（46）、native `blitz_consumer`（27）、desktop（31）。

**已修复**（编号对应下文）：

- P0-1 布尔属性反转 — `None` 现在编码为 `SetProperty{value:"true"}`，JS 与 native 消费端按 DOM 属性类型回转；新增两个回归测试。
- P0-2 部分修复 — 见「仍未处理」。
- P0-3 调度器标志泄漏 — `BATCHING` / `RUNNING` 改 RAII guard，`gather` 的 tracking layer 与 `is_building` 也做了 panic 安全；回归测试已验证修复前会失败。
- P0-5 `.fill()` XSS — 字符串 filler 改走 `.text()`，同时修好了「反应式 filler 只是快照」的问题；示例中两处真实注入点一并修正。
- P0-6 raw-text 转义 — `<script>` / `<style>` 内容原样输出，只中和 `</`；旧测试固化的错误行为已更新。
- P0-7 / P0-8 隐藏子树 — `attach_child` 在父未挂载时不碰 DOM，`detach_child` 不再放过未挂载的 view，`suspend` 停止清空后代的 `parent_node`；回归测试覆盖「隐藏期间增删行」。
- P0-9 示例 salvo 版本 — 9 个示例升到 0.94，并新增 CI job 编译示例（此前 `examples/` 完全没有编译门禁）。
- P0-12 移动端 IPC — 补齐缺失的 match 臂，`IpcMessage` 标记 `#[non_exhaustive]` 使协议今后可演进。
- P0-13 `r` 键 panic — `SOURCE_CHANGES` 换成 std 锁。
- P0-14 assets 瞬态错误 — 降级为 warn，并跳过目录/已消失的路径。
- P0-15 LiveView — 心跳、客户端树 reset、会话上限、`Close` 立即释放、CSPRNG token、Origin 校验、消息大小上限全部落地。
- §2.1 / §2.2 CSRF 与 body limit — 新增 `RequestContext::ensure_same_site` 与共享的 `max_body_bytes`，三个 adapter 统一接入并返回 413。
- §2.3 编码不一致 — CSR 与 SSR 统一用 `percent_encoding`，路径段保留 unreserved 字符。
- §2.4 catch-all 双重解码 — `split_catch_all` 停止二次解码。
- §2.12 `Set-Cookie` 折叠 — 三个 adapter 改 `append`。
- §2.13 错误状态 — `Deserialization` 改判 400；新增 `ServerFnError::internal_logged` 用于脱敏。
- §2.14 可触发 panic — 浏览器导航、锚点解析、`match_route_pattern`、wasm 正则编译全部改为返回错误。
- §3.4 wasm 正则相等性 — `Regex` 保存源串，不再所有正则相等。
- §4 接线 — `DesktopCloseBehavior` 与 `LoadState` 补上导出（此前三个 API 对外不可调用）；build-error overlay 真正接入浏览器客户端并加了跨 crate 覆盖测试；CLI 侧的死 overlay 构建器已删除。
- §5 文档漂移 — `.get()` 订阅语义（三处）、hot-reload 描述、desktop `runtime` 默认值、CLI `--hot-reload` 帮助、serverfn "single entry point" 全部更正。
- §8.1 CI 缺口 — 补齐 routing / liveview / hot-reload / desktop / `glory --features routing` 的测试与 lint；`check-all-features` 扩展到 serverfn 与 liveview；`skip_feature_sets` 补上被 `compile_error!` 守卫的组合；新增编译 examples 的 job。
- **整改期间的额外发现**：CI 的 `Clippy CLI` 步骤（`cargo clippy -p glory-cli --lib --no-default-features -- -D warnings`）本身就在失败，`ext/exe.rs`、`compile/style.rs`、`command/bundle.rs`、`lib.rs` 共 15 处机械性 lint 早于本次改动存在。已一并修复。这说明 §8.1 的问题不止「缺步骤」，已有步骤也未必是绿的。

### 第二轮整改（同日续做）

在上表基础上又完成了一批，全部带经验证的回归测试：

- **P0-11 `glory new` 模板** — 实测复现了报告的诊断（`the package 'demo-app' does not contain this feature: web-csr`），
  修好 web 模板的 `[features]`，给 desktop / mobile 模板补上 `[package.metadata.glory]`（缺它时 CLI 直接 bail），
  模板版本号改用 `env!("CARGO_PKG_VERSION")`，并新增一个不依赖网络的模板自洽测试
  （元数据引用的 feature 必须存在于本包，且每个模板必须声明 glory 元数据）。
  bin 包查找的错误信息也区分了「包不存在」与「包没有 bin target」（后者是 mobile 项目的正常形态）。
- **P0-2 的可修部分** — `spawn_local` 现在按 tokio runtime flavor 分支。此前 `block_in_place`
  在 current-thread runtime 上必 panic，下游任何 `#[tokio::test]` 渲染含 resource 的页面都会崩；
  无 runtime 时 `Handle::current()` 同样 panic。新增 `ssr_runtime_flavors` 集成测试覆盖三种情形。
  （桌面事件循环执行器仍未做，见下。）
- **§3.4 派生路由提前返回** — 变体解析失败现在落到下一个变体，而不是中止整个 `from_url`。
  此前 `#[route("/users/<id>")]` 声明在 `/users/new` 之前会吞掉后者，声明顺序静默决定路由可达性。
- **§3.4 `stuff_keys` 泄漏** — 短路条件同时要求 key 已登记**且** stuff 仍在队列中。
  `Graff` 渲染时会消费 stuff 而 key 留存，所以返回同一路由时嵌套 outlet 永久空白。
- **§3.4 匹配失败的参数泄漏** — `PathFilter::detect` 与 `Router::detect` 现在回滚 params 而不只是 cursor。
- **§3.4 特异性排序** — 改为逐段字典序比较。此前按总分求和，`/<a>/<b>/<c>`（120）压过 `/x`（100）。
- **§3.1 `Bond` 依赖集** — 重算后按集合差做 unbind/bind，与「值是否变化」解耦。
  此前用 `with_partial_eq()` 时，分支切换到一个恰好等值的新依赖会让它完全不被订阅。
- **§3.1 `Each` 重复 key** — build 与 patch 两侧都折叠重复 key；build 改用 `store_in` + 延后 attach，
  消除持有 items 借用时执行用户 widget 的 `BorrowMutError` 风险。
- **§3.1 调度器丢更新** — 超过 8 轮时把待处理项放回队列再退出，而不是丢弃。
- **§3.5 serverfn 客户端** — WebSocket 加连接代际号（旧连接的 `onclose` 不再夺走新 socket），
  `reconnect` 先摘除旧回调，`close` 在无连接时直接落到 `Closed`，重连定时器尊重手动关闭；
  `reqwest::Client` 改为进程内共享；SSE 非法 `retry` 不再终止整个流；
  `id` / `event` 字段过滤换行，防止帧注入。
- **§3.6 desktop** — `Focus` 先 `set_visible(true)`（此前 `Hide` 后窗口无法再显示，进程挂死）；
  窗口关闭时结算其挂起的 `eval`（否则 future 永不完成）；菜单/托盘路由按窗口清理。
- **§3.6 JS 解释器与 native 消费端** — `Remove` 现在递归清理节点表与监听器表
  （核心只对子树根发 `Remove`，此前后代永久残留，native 侧还可能把回收的 blitz 槽位映射回旧 id）；
  JS 侧的 `Remove` 也对齐参考实现，忽略过期的 remove。
- **门禁** — 全 workspace clippy 警告清零（含此前从未跑严格检查的 desktop / native），
  并把 desktop、native、`backend-command`、runtime-flavor 测试都加进 CI。

**仍未处理**（工作量或设计决策超出本轮）：

- P0-2 的另一半：桌面事件循环缺少异步执行器，`eval()` 与 node query 的 future 在事件循环线程上仍无处驱动。
  需要给 runtime 加 `HostEvent::Wake` + `LocalPool`，是独立的架构改动。
- P0-4 `Cage` 泄漏。需要把框架内部的 `Cage::new` 全面改为 owner 化创建，属 generational-box 重构的一部分。
- P0-10 `#[server(stream)]` 派发。需要扩展 `ServerFnEntry` 以承载流式 handler。
- §4 表格中其余「有类型、无调用者」项（`OutboundBuffer`、ISR 缓存、`PreloadedState`、`reloadable_view!`、`ReloadHooks`、`glory-tui` 等）。
- §5 流式 SSR 的真正 over-the-wire flush、head 跨组件去重。
- §7 功能缺失清单。
- §8.2 发布版本策略。模板与示例都写 `0.3.1`，而源码 API 已经变了
  （例如 `BrowerHolder` → `BrowserHolder`、`Cage` 变成 `Copy`），
  所以 `glory new` 生成的项目对着 crates.io 上的 0.3.1 编译不过。必须先 bump 并发布。

---

## 1. P0：应当优先修复

### P0-1 布尔属性在所有命令流后端被反转（desktop / liveview / native 全中）

`crates/core/src/web/prop.rs:114-123` 在非 CSR 路径把 `true` 编码为 `set_property(name, None)`；
`crates/core/src/renderer/command.rs:715-733` 又把 `(_, None)` 映射为 `Command::RemoveProperty`。
于是 `true` 和 `false` 都变成"移除属性"。

- 复现：`input().attr("type","checkbox").checked(true)` 在 desktop-counter / LiveView 中永远不勾选；
  `button().disabled(true)` 不禁用。
- 消费端佐证：`crates/desktop/src/wry_interpreter.js:125-129` 执行 `el[name] = undefined`；
  `crates/native/src/blitz_consumer.rs:265-273` 执行 `clear_attribute`。
- 注意 SSR 的 `SsrNode::set_property(_, None)`（`renderer/ssr_dom.rs:78-80`）语义是"存在的无值布尔属性"，
  命令流把这个语义翻转了，所以 SSR 与命令流后端结果相反。
- 建议：`None` 编码为 `SetProperty { value: "true" }`，JS/native 端对已知布尔属性做 `value === "true"` 转换；
  或在协议中新增 `SetBoolProperty`。

*两路独立审计各自命中，置信度高。*

### P0-2 `spawn_local` 在桌面 / 原生 / 非流式 SSR 上阻塞调用线程

`crates/core/src/spawn.rs:24-64`：非 wasm 且非流式时 `spawn_local` 退化为 `block_on`。

三个后果：

1. **桌面死锁**：事件处理器里 `spawn_local(async { node.value().await })`（`docs/platform-apis.md:147-149`
   推荐写法）在 tao 事件循环线程上 `block_on`，而 `GloryWryQuery` 的回包必须由同一线程的
   `HostEvent::Query`（`crates/desktop/src/runtime.rs:1306-1313`）递送。`DesktopWindowHandle::eval()`
   返回 Future，但 runtime 不提供任何执行器轮询点。
2. **current_thread 运行时 panic**：`spawn.rs:53` 与 `:118` 的 `tokio::task::block_in_place` 只在
   multi_thread 运行时合法。下游用 `#[tokio::test]`（默认 current_thread）测试含 `resource_in` 的 SSR
   渲染会直接 panic。`cfg(not(test))` 只保护了 core 自己的测试。
3. **吞吐塌陷**：默认 SSR 路径每个含异步资源的请求都占住一个 worker 线程直到资源完成。

建议：桌面 runtime 增加 `HostEvent::Wake` + `LocalPool`；SSR 在 current_thread 下退化为
`futures::executor::block_on`，并按运行时 flavor 分支。

*三路独立审计命中。*

### P0-3 panic 后调度器标志位不复位，整个会话冻结

`crates/core/src/reflow/scheduler.rs:394-426`：`batching.set(true); let out = opt(); batching.set(false)`
没有 RAII guard，`opt` panic 时标志永久为 true。

链路：`ErrorBoundary > Switch`，case 子 widget 在 flip 时 `build` panic → `Switch::patch` →
`attach_child`（`scope.rs:448-458` 进入 `batch` 置位）→ build panic → unwind 穿过 `batch` →
`run()` 的 `catch_unwind` 捕获并渲染 fallback。此后 `BATCHING[holder] == true`，任何
`Cage::revise` → `schedule()` 因 `is_batching()` 直接返回（`scheduler.rs:435-439`），
LiveView / desktop 会话从此不再更新。

同类未复位的标志：`view.rs:150-152` 的 `is_building`；`reflow/mod.rs:94-99` `gather` 的
tracking layer（panic 后僵尸层留在 `TRACKING_STACK`，之后所有读取都会被它捕获）；
`scheduler.rs:487,490,494` 三处 `resume_unwind` 后 `RUNNING` 永远为 true。

现有测试 `error_boundary_catches_child_patch_panics` 里 patch 直接 panic、不经嵌套 batch，故未覆盖。

建议：全部改用 `Drop` guard 复位。

### P0-4 非 Owner 持有的 `Cage` 永不回收，SSR 每请求泄漏

`crates/core/src/reflow/cage.rs:381-387` 用 `Box::leak` 分配 slot；唯一回收入口 `invalidate()`
（`cage.rs:160-173`）只由 `Owner::drop` 触发，而 `Owner` 只登记 `ctx.cage()` / `own_cage` 创建的 cage。

框架自身到处直接 `Cage::new`：`widgets/suspense.rs:39`、`widgets/loader.rs:73,272`
（每个 Loader 一个 `Cage<LoadState<T>>`，**含完整加载结果 `T`**）、`reflow/effect.rs:140,187,190`
（每个 resource 一个 `Cage<Option<T>>`）、`scope.rs:344`、`cage.rs:118-128` 的 `Deserialize`。

后果：hackernews-salvo 这类 SSR 服务每个请求泄漏该请求所有 story 数据的 slot，进程内存单调增长。
`CAGE_FREE_LIST` 只对被 invalidate 的 slot 有效，对这些永远不生效。

AGENTS.md 把 generational-box 重构列为 roadmap，但当前状态是生产级内存泄漏。
建议：框架内部一律改用 `ctx.cage(...)`；为 `ServerHolder` 增加请求级 `Owner`。

### P0-5 裸字符串 `.fill()` 走 innerHTML，CSR + SSR 双端 XSS

`crates/core/src/web/widgets/mod.rs:721-761`：`IntoFiller` 对 `String` / `&str` / `Cow<str>` /
`Cage<T: AsRef<str>>` / `Bond<T>` 全部展开为 `span().html(...)`。
落点是 CSR 的 `set_inner_html`（`web/attr.rs:113-114`）和 SSR 的 `inner_html` 原样输出
（`renderer/ssr_dom.rs:267-268`，**不经** `escape_html_text`）。

仓库内现存实例：`examples/hackernews-salvo/src/views/story/show.rs:47,106`、`list.rs:158`
把远端 API 返回的字符串直接送进 innerHTML。

API 命名 `.fill(str)` 没有任何 raw 暗示，与 Leptos / Dioxus 把字符串子节点视作文本的惯例相反。
建议：`IntoFiller for str/String/Cow/Cage/Bond` 改为 `.text(..)`，raw HTML 必须显式 `.html()`。

### P0-6 SSR 对 `<style>` / `<script>` 的 `.text()` 做实体转义，CSS / JS 被破坏

`crates/core/src/renderer/ssr_dom.rs:269-279`：`inner_text` 一律转义 `& < >`。但 `<style>` / `<script>`
是 raw-text 元素，浏览器不解码实体。

复现：官方推荐写法 `style().text(scope.css())`（`web/style.rs:11`）配合
`scoped_css(":scope > button{...}")` → SSR 输出 `.gly-scope-x &gt; button{...}`，规则失效。
`examples/forms-showcase/src/main.rs:67` 线上就是这么写的。
CSR 走 `set_text_content` 是对的，所以这是 CSR/SSR 不一致，首屏无样式、水合后才修正。

**注意**：`crates/core/src/widgets/snapshot_tests.rs:284` 用 `assert!(html.contains("&gt; button"))`
把这个错误行为固化成了预期，修复时需一并更新。

建议：`leaf_inner_html` 对 `script` / `style` 原样输出，但把 `</script` / `</style`（大小写不敏感）
改写为 `<\/...`。

### P0-7 被 Suspense 隐藏的子树在 patch 时被重新插回 DOM

`crates/core/src/scope.rs:490-515` 的 `hide_child` 只把 view 的 `is_attached` 置 false 并移除节点，
订阅仍在；`reflow/scheduler.rs:479-484` 对 ROOT_VIEWS 中任何 view（不检查 `is_attached`）都调 `patch`；
`scope.rs:373-460` 的 `attach_child` 也不检查 `self.is_attached`。

- SSR/命令流复现：`Suspense` body 内含 `Each`，pending 期间 items cage 变化 → `Each::patch` 对新增行
  `attach_child` → `Element::flood` 把 `<li>` 插到 `render_node` 下 → fallback 与"隐藏"body 同时出现。
- CSR 额外路径：`web/widgets/csr.rs:154-156` 的 `patch` 里 `if node.parent_element().is_none() { parent.append_child(node) }`
  会把隐藏节点重新 append 到父容器**尾部**（位置也不对），且 `ctx.parent_node.unwrap()` 在非 root 为 None 时 panic。

建议：`patch` 开头检查 `is_attached` 直接 return；`attach_child` 在父未 attach 时只登记不操作 DOM。

### P0-8 未 attach 的 view 无法 detach，产生僵尸 view 与重复渲染

`crates/core/src/scope.rs:462-468`：`detach_child` 对 `!is_attached` 的 view 直接 `return None`，
既不从 `child_views` 移除，也不调用 `widget.detach()`。

- `Each` 复现：Suspense fallback 期间删掉 key `"b"` → `detach_children_bulk(["b"])` 保留了该 view →
  之后加回 `"b"` → 新建第二个 view → Suspense 解除时默认 `flood` 把 `child_views` 全部 attach →
  页面出现两个 `"b"`，旧 view 的事件处理器与 Effect 订阅都还活着。
- `Switch` 同根因（`widgets/switch.rs:87-95`）：隐藏期间翻转分支后旧 case 变僵尸，恢复时双显，
  再翻回时 `cached_view.take()` 为 None 又建第三个。
- `widgets/error_boundary.rs:115-121` 额外手写了一段 `child_views.shift_remove`，说明作者已知此坑。

### P0-9 示例 `liveview-salvo` 编译失败（已实测）

```
error[E0308]: mismatched types
  --> src\main.rs:89:15
   | .push(salvo_mount::router(Counter::new));
   | expected `salvo::Router`, found `salvo_core::routing::router::Router`
   = note: there are multiple different versions of crate `salvo_core` in the dependency graph
```

根因：`examples/liveview-salvo/Cargo.toml:10` 固定 `salvo = "0.93"`，而工作区在提交 `54f35f8`
升到了 `0.94`。全部 9 个 Salvo 示例都还写着 `0.93`。

实测确认，凡是启用了 glory 的 salvo 集成（`glory/salvo`、`glory-serverfn/salvo`、
`glory-liveview` 的 `salvo`）的示例都会拉进两份 `salvo_core` 而编译失败，共 **8 个**：
`hackernews-salvo`、`todomvc-fullstack`、`file-upload`、`ssr-simple-salvo`、`ssr-modes-salvo`、
`tailwind-salvo`、`realtime-notify`、`liveview-salvo`。
`counter` 只自己依赖 salvo、不启用 glory 的集成 feature，因此**幸免**（已实测通过）。

`hackernews-salvo` 的报错更直白：

```
error[E0277]: the trait bound `glory::web::holders::SalvoHandler: salvo::Handler` is not satisfied
```

`examples/` 被 workspace `exclude`，`cargo check --workspace` 检测不到。
但 CI 的 e2e job（`ci.yml:87-200`）会启动 `ssr-simple-salvo`、`todomvc-fullstack`、
`liveview-salvo` 三个受影响示例，**该 job 自 salvo 升级以来应当一直是失败的**，建议核对最近的
workflow 运行记录。这也说明 examples 缺少独立的编译门禁（见 §9 建议 18）。

### P0-10 `#[server(stream)]` 客户端调用必然 404

`crates/macros/src/lib.rs:515-542` 的 stream 分支只输出原函数与 wasm stub，**不产生
`inventory::submit!`**；`ServerFnEntry.handler`（`serverfn/src/lib.rs:1775-1783`）签名返回 `Vec<u8>`，
也无法承载流。三个 adapter mount 收到 `/__glory/fn/<name>` 都走 `dispatch_with_method` → `NotFound`。

同一分支还静默丢弃 `middleware` 与 `encoding` 选项：`#[server(stream, middleware = require_auth)]`
编译通过但鉴权根本不存在。

`_todos.md` 把 FS1/FS2 标为 `[x]`，宏文档（`macros/src/lib.rs:396-402`）也未提示客户端会失败。

### P0-11 `glory new` 生成的默认项目无法 build / serve

`crates/cli/src/command/new.rs:232-250` 生成的 Cargo.toml 没有 `[features]` 段，只把 `web-csr`
开在依赖 `glory` 上；但元数据写 `lib_features = ["web-csr"]`。
`config/lib_package.rs:44-56` 会拼出 `--features=web-csr` 传给 `cargo build --package my-app`，
cargo 对包内不存在的 feature 报错。同一项目 `builds_server()` 为真，
`bin_package.rs:43-50` 又会加 `--features=web-ssr`（模板同样没有）→ 二次失败。

对照 `examples/counter/Cargo.toml` 是在包内声明 `web-csr = ["glory/web-csr", ...]` 才能工作。
`crates/cli-harnesses/fixtures/web/Cargo.toml` 同病。

`desktop` / `mobile` 模板（`new.rs:377-430`）都没有 `[package.metadata.glory]`，
`config/mod.rs:484-492` 会直接 `bail!`；mobile 模板还只有 `src/lib.rs` 没有 bin target，
而 `Project::resolve` 无条件要求 bin。`.github/workflows/mobile-device-smoke.yml:40-52`
正是 `new --template mobile` → `build` → `bundle`，按当前代码必然失败。

### P0-12 `examples/mobile-counter` 与 CLI 移动模板在目标平台编译失败，且被主机检查掩盖

`examples/mobile-counter/src/lib.rs:223-236` 对 `IpcMessage` 的 `match` 只有 4 臂，
缺 `crates/desktop/src/lib.rs:101` 新增的 `GloryWryEval { .. }` → E0004 非穷举。
整个 `mod host` 被 `#[cfg(any(target_os="android", target_os="ios"))]` 门控，所以
`scripts/mobile-device-smoke.ps1` 的主机 `cargo check` 通过并记为 `completed`，是**假阳性**。
`crates/cli/src/command/new.rs:832-843` 生成的模板宿主同病。

### P0-13 serve 模式按 `r` + Enter 必定 panic

`crates/cli/src/command/watch.rs:62-88` 的 `spawn_controls` 是 `tokio::spawn` 的异步任务，
同步调用 `Interrupt::send_all_changed()`，后者在 `signal/interrupt.rs:41-46` 执行
`SOURCE_CHANGES.blocking_write()`。tokio 的 `RwLock::blocking_write` 在运行时 worker 线程上会 panic
（`Cannot block the current thread from within a runtime`）。JoinHandle 被丢弃，panic 被吞，
结果是第一次按 `r` 后 `r`/`v`/`/` 全部失效，而重建从未触发。

### P0-14 assets 目录的瞬态文件系统错误让 `glory serve` 直接退出

`crates/cli/src/compile/assets.rs:53-90` 的 `update_asset` 对任一 `fs` 失败返回 `Err`；
`command/watch.rs:162-179` 用 `try_join!(...)?` 把它当致命错误，`:53-57` 收到 Err 即
`request_shutdown` 并返回。

可复现场景：vim / VS Code 的原子保存（Create `.tmp` → Rename → Remove `.tmp`）产生对不存在文件的
`remove_file` → ENOENT → 进程退出；Windows 上目录本身的 `Modify(Any)` 事件 → `fs::read(dir)` → EISDIR。

### P0-15 LiveView 未提交改动引入三个高危默认行为

未提交的 `crates/liveview/src/lib.rs` 改动把 `SessionRegistry` 从"建筑块"接进了适配器
（`negotiate_hello` / `SessionManager` / resume token），方向正确，但缺两个前置能力：

1. **5 分钟踢人**：`idle_timeout_secs` 默认 300（`lib.rs:231`），只有入站消息 `touch`；
   客户端从不主动 ping（`liveview_client.js:50-51` 只被动回 pong），服务端也不发。
   5 分钟无操作后任一连接触发的 `reap` 删掉该 session，用户下次点击收到 `Close`，
   客户端置 `closed = true` **永不重连**（`js:52-54`）→ 页面死亡且无提示。
2. **重连 UI 重复**：token 过期或服务器重启后 `connect()` 走新建分支并发 `Mount`，但浏览器端
   `wry_interpreter.js` 的 `nodes` Map 与 `document.body` 仍保留旧树，协议里没有任何 reset 命令。
   `Insert{parent:0, Tail}` 把新树追加到旧树之后 → 页面出现两份 UI。
3. **session 无上限**：改动删除了 `SessionRequest::Close` 与 `impl Drop for SessionWorker`，
   socket 关闭不再释放 worker；无后台 reaper，无流量时永不回收。反复开关连接可在 5 分钟内
   堆积任意数量的 widget 树。

另外 resume token 由 `nanos + pid + 单调计数` 拼接（`lib.rs:515-524`），可推测；
`SessionManager` 是私有 struct，用户无法注入自定义生成器；同一 token 可被多个 socket 同时 resume，
两边 DOM 立即分叉。文档建议的 `options.resumeToken` 种子用法（`docs/liveview.md:159-163`）
会得到空白页，因为 `Resumed` 不回 `Mount` 且没有服务端 DOM 镜像可重放。

**建议在合并前至少补 1、2、3 三点。**

---

## 2. 安全漏洞

### 2.1 Server function 完全没有 CSRF 防护（高）

`crates/serverfn/src/lib.rs:1893-1901` 的 `request_encoding` 对任何 `Content-Type`（含 `text/plain`、
缺失）都回退到 JSON 解码；`:2056-2060` 的 `is_form_request` 让单参数函数直接接受
`application/x-www-form-urlencoded`。三个 mount（`:2949-2992`、`:3051-3090`、`:3137-3167`）
与 `dispatch_with_method`（`:1809-1853`）全程没有 `origin` / `sec-fetch-site` / 自定义 header 检查
（全仓库 grep `origin|sec-fetch|csrf` 在 serverfn/macros/docs 内零命中）。

攻击页面放一个自动提交的跨站表单即可携带 cookie 调用单参数 struct 函数；多参数函数可用
`enctype=text/plain` 构造 `[{"id":7,"z":"="}]`（serde 默认忽略未知字段）。

`#[server(method = "GET")]`（`macros/src/lib.rs:421-430`）只校验值是 GET/POST 不限制语义，
于是 `<img src="/__glory/fn/delete_todo?__glory_args=[7]">` 即可跨站触发；GET 响应还没有
`Cache-Control: no-store`（`lib.rs:341-349` 只写 content-type），中间代理可能缓存含用户数据的结果。

### 2.2 三个 adapter 的 body 上限不一致且错误码错误（高）

| Adapter | 上限 | 来源 | 超限响应 |
|---|---|---|---|
| Salvo | **64 KB** | `req.payload()` 走 Salvo `secure_max_size` 默认值 | 400 "invalid body"（应为 413） |
| Axum | 16 MB | `lib.rs:3072` 硬编码 | 400（应为 413） |
| Actix | 256 KB | actix `PayloadConfig` 默认 | 413 纯文本，不是 serverfn JSON，客户端只得到 `Request("HTTP 413")` |

同一份 server fn 代码换个 adapter 就有 256 倍的行为差异。建议统一在 mount 层做可配置 body limit，
超限一律 413 JSON。

### 2.3 CSR 下路由参数用 `encodeURI` 而非 `encodeURIComponent`（高）

`crates/core/src/web/mod.rs:25-36`：wasm 分支 `escape = js_sys::encode_uri`，
`unescape = js_sys::decode_uri`；host 分支用 `percent_encoding` 的 `NON_ALPHANUMERIC`。

后果：
- `encodeURI` 不编码 `/ ? # & = +`，浏览器端 `AppRoute::Files{path:["a/b"]}.to_url()` 得到两段路径；
  `User{name:"x?y=1"}` 生成的 URL 被注入 query。
- `decodeURI` 不解码 `%2F`，SSR 输出的 `hello%2Fworld` 到 CSR 端仍是字面量。
- 反向不一致：host 端会编码 `. - _ ~`，CSR 不会 → 同一个 `<a href>` 在 SSR 与 CSR 生成不同字符串，
  hydration 属性不匹配。`crates/glory/tests/routable_derive.rs:29` 断言 `intro%20page%2Emd`，
  这个测试只在 native 通过，wasm 上会失败。

建议两端统一用 `percent_encoding`（wasm 也可用），path segment 用 path-segment 集合。

### 2.4 派生 catch-all 参数双重解码（中，可绕过路径过滤）

`crates/routing/src/lib.rs:110-114` 的 `PathState::new` 已对每段 `unescape`；
`crates/macros/src/lib.rs:1386-1390` 又把 `params["**path"]` 交给 `typed.rs:370-376` 的
`split_catch_all` → `decode_route_param` 再解一次。

`/files/%252e%252e/x` → 第一次得 `%2e%2e` → 第二次得 `..`；段内 `%2F` 解出后被 `split('/')` 再拆段。
`typed.rs:547-556` 的 round-trip 测试走的是手写 `from_url`，对派生实现不成立。

### 2.5 `ServerAviator::with_redirects` 静默改写 URL（高）

`crates/routing/src/aviators/server.rs:68-79`：只要 `R::resolve_url(url).to_url() != raw_url` 就改写。

- `/users/7?page=2` 的 `to_url()` 是 `/users/7` → **整个 query 被丢弃**，`Locator::queries()` 为空。
- `crates/macros/src/lib.rs:1172-1174`：unit 变体 `#[not_found]` 的 `to_url()` 固定返回 `"/"`，
  而 `resolve_url` 对任意未匹配 URL 返回该变体 → SSR 把**所有 404 改写为 `/` 并以 200 渲染首页**。
- 它也不是 HTTP 3xx：浏览器地址栏不变，而 `BrowserAviator` 没有 redirect resolver，
  hydration 后 CSR 会把原 URL 交给 catcher，SSR/CSR 内容不一致。

### 2.6 desktop IPC 无来源校验（中）

`crates/desktop/src/runtime.rs:1465-1490` 注入 `WRY_INTERPRETER_JS` 并注册 `with_ipc_handler`，
但没有注册 `with_navigation_handler`（wry 0.55 提供），也不检查消息来源。
wry 文档明确 **Windows 上初始化脚本总是注入子框架**，`window.ipc` 由 WebView2 注入所有框架。

于是应用只要渲染了任何可点击的外部链接或 `<iframe>`，远程页面即可
`window.ipc.postMessage(JSON.stringify({GloryWryEvent:{...}}))` 伪造任意节点事件、
篡改 `target.value`，或伪造 `GloryWryQuery` / `GloryWryEval` 响应（token/id 单调递增可猜）。

配套问题：`glory://` 资源根在未配置时回退到**可执行文件目录**（`runtime.rs:1004-1015`），
开发期可读 `target/debug/` 下的 `.pdb`、`deps/`；`GLORY_SITE_ROOT` 在 release 中同样生效。
路径穿越防护本身（`resolve_asset_path:1032-1046`）是正确的，问题在根太宽。

### 2.7 LiveView / 热重载 WebSocket 均无 Origin 校验与消息大小限制（中）

- LiveView 三个适配器（`lib.rs:711`、`:861`、`:1061`）都用 `WebSocketUpgrade::new()`，
  未启用 salvo 0.94 提供的 `allowed_origins()` / `check_origin()`（salvo 文档明确警告 CSWSH），
  `LiveViewConfig` 也没暴露该选项。
- 无 `max_message_size`：salvo/axum 走 tungstenite 默认 64 MiB，客户端可发巨大 `patch` 消息，
  服务端完整反序列化成 `Vec<Command>`（`lib.rs:752`）后在 `handle_message` 里丢弃（`:124-129`）。
- CLI 的 `/live_reload`（`crates/cli/src/service/reload.rs:42-44`）绑定跟随 `site_addr` 的 IP，
  而 `examples/counter` 默认 `0.0.0.0:8000` → reload 服务也监听 `0.0.0.0:8001`，
  局域网内任意主机可连。

### 2.8 工具链下载无完整性校验（中）

`crates/cli/src/ext/exe.rs:86-146`：`reqwest::get` → 解压 → 直接执行，只依赖 TLS，
没有 sha256 / 签名钉死。`:702-742` 的区间版本模式还会取 GitHub API 返回的 `tag_name`
直接拼 URL 执行，构建产物不可复现。`command/bundle.rs:226-299` 的 WiX 工具链同理。

（已核对：`tar` 的 `unpack` 与 `zip` 的 `extract` 自带路径逃逸保护，**未发现 zip-slip**。）

### 2.9 Apple 公证密码进 argv 并被完整打印到日志（中）

`crates/cli/src/command/bundle.rs:508-515` 把 `--password <APPLE_APP_PASSWORD>` 放进 argv；
`:1085-1087` 的 `run_checked` 用 `log::info!("Running {} {}", name, format!("{:?}", command.as_std()))`
打印整条命令。macOS CI 的构建日志里会出现 App 专用密码，`ps` 也能看到。

### 2.10 `site_root` / `bin_target_dir` 可指向任意目录，而 build / clean 会递归清空它（中）

`crates/cli/src/config/project.rs:245-251` 的 `validate()` 只拒绝 `"/"` 和 `"."`，
`".."`、`"src"`、绝对路径全部放行；`config/dotenvs.rs:41` 还允许 `.env` 里的 `GLORY_SITE_ROOT` 覆盖。
`command/build.rs:33-35` 与 `command/clean.rs:29-45` 对该路径直接 `rm_dir_content` / `remove_dir_all`。

克隆一个 `.env` 里写着 `GLORY_SITE_ROOT=..` 的仓库并执行 `glory build`，即清空上级目录。

### 2.11 CSP nonce 是进程级静态值，且未覆盖流式脚本（中）

- `crates/core/src/config.rs:116` 从 `GLORY_CSP_NONCE` 读一次，整个进程所有响应复用同一 nonce。
  CSP nonce 要求每次响应唯一且不可预测，固定 nonce 的安全价值趋近于 0。
- `web/holders/server.rs:53` 的 `PlaceholderPatch` 脚本与 `:256-258` 的 `__gloryResource` 脚本
  **都没有加 nonce**，而 `config.rs:37-38` 文档承诺"nonce attached to Glory-managed SSR scripts"。
  严格 CSP 下 `__gloryResource` 永远不会写入 → 客户端 `resource_hydratable_in` 全部重新请求，
  文档承诺的"不重复请求"失效。
- `server.rs:255` 的内联 JSON 只转义了 `</`，未处理 `<!--`（可让 HTML 解析器进入
  script-data-double-escaped 状态）。

### 2.12 多个 `Set-Cookie` 头被静默折叠（中）

三个 adapter 都用 `insert` 而非 `append`（`serverfn/src/lib.rs:2997-3001`、`:3095-3099`、`:3172-3176`）。
`ServerFnError::redirect("/").with_header("set-cookie", a).with_header("set-cookie", b)`
只有 `b` 到达浏览器；登录同时下发 session + csrf cookie 会丢一个且无任何错误。
`docs/serverfn-adapter-recipes.md:169-170` 承诺"adapter mounts already forward headers"，
只对单值成立。

### 2.13 错误信息泄漏（中）

- `serverfn/src/lib.rs:96` 把 `serde_json::Error::to_string()`（含字段名、期望类型、偏移）
  直接装入错误并序列化到响应体；`:306-312` 还把客户端可控的 `Deserialization` 判为 **500**，
  导致畸形请求推高 5xx 告警。测试 `tests/server_fn.rs:141-143` 固化了这个行为。
- `ServerFnError::ServerError(String)` 的 `From<String>` / `From<&str>`（`:323-332`）让任何
  `map_err(|e| e.to_string())?` 把数据库/IO 错误原文送到客户端。
- `crates/core/src/widget.rs:28-41` 的 `BoundaryError::from_panic` 原样保留 panic payload，
  `widgets/error_boundary.rs:124-132` 把它 JSON→base64 写进 `gly-error-*` 属性交给页面。
  SSR 生产环境下任何子树 panic 的完整消息（可能含路径、SQL、内部值）会进入 HTML。

### 2.14 不可信输入触发 panic（DoS）

| 位置 | 触发 |
|---|---|
| `core/src/web/mod.rs:35` `decode_uri(s).unwrap()` | 页面上任意 `<a href="/x/%E0%A4%A">` 被点击 → `URIError` → 整个 wasm 应用挂掉 |
| `routing/src/aviators/browser.rs:179` `Url::try_from(href).unwrap()` | 用户生成内容里的坏链接（如 `href="http://[bad"`） |
| `routing/src/aviators/browser.rs:124-131` `unwrap_throw` | `goto` 传入跨域 / `javascript:` URL → `SecurityError` → wasm trap。函数明明返回 `Result` 却不映射 |
| `routing/src/filters/path.rs:872,684,687` | `PathFilter::new` 对非法模式 / 未注册 `:sign` 直接 panic；`typed.rs:146-149` 的 `match_route_pattern` 是公开函数且 pattern 是运行时参数 |
| `routing/src/regex.rs:22-24`（wasm） | `Regex::new` 永远返回 `Ok`，非法正则时 `js_sys::RegExp::new` 直接抛异常 → trap。JS 正则是回溯引擎且 `path.rs:456` 不锚定 → ReDoS |
| `core/src/widgets/loader.rs:220` | `serde_json::to_string(&state).unwrap()`，用户类型含 `f64::NAN` 即在 SSR 请求中 panic |
| `core/src/web/holders/server.rs:348` | `GloryConfig::load(None).await.unwrap()`，配置非法时启动 panic 且无上下文 |
| `core/src/web/widgets/node_meta.rs:55` | `body_meta().class("a b")` detach 时 `classList.remove("a b")` 抛 `InvalidCharacterError` |
| `cli/src/service/site.rs:101-102` | `Debug` impl 用 `blocking_read()`，async 上下文里 `{:?}` 一个 `Project` 就 panic（当前仅测试触发，属埋雷） |

### 2.15 其他

- **inventory 端点名冲突静默**：`serverfn/src/lib.rs:1817-1836` 线性取第一个匹配，
  `macros/src/lib.rs:507-508` 端点名默认只取函数名不含模块路径。两个模块各有 `list()` 时
  编译通过、运行时后者的客户端调到前者。
- **原生图片提供器**：`native/src/image_provider.rs:257-267` 在调用线程同步 `fs::read` 整个文件后
  同步解码，无大小限制 → 解压炸弹阻塞 UI / OOM；`resolve_asset_url`（`:73-86`）只剥前导 `/`，
  **不剥 `../`**，与文档"must not escape"承诺不符。
- **wasm streaming 客户端不取消 ReadableStream**：`serverfn/src/lib.rs:2913-2933` 流被 drop 时
  既不 `cancel()` 也不 `release_lock()`，fetch 继续接收直到服务器关闭。
- **`NdjsonDecoder` / `SseDecoder` 无最大行长**（`:505-515`、`:572-584`）：永不换行的巨大响应会 OOM。
- **`SseEvent::encode` 不过滤 `id` / `event` 中的换行**（`:1057-1092`）→ SSE 帧注入。
- **`FileSystemCache::write` 非原子**（`:2476-2486`）：并发读者可能读到半个 body。
- **desktop bootstrap HTML 无 CSP**（`runtime.rs:898`），`SetHtml` 直通 innerHTML。
- **缓存目录无并发保护**（`cli/src/ext/exe.rs:148-154`）：两个 glory 进程同时首次下载同一工具时，
  一方可能执行另一方尚未写完的二进制。

---

## 3. 功能缺陷（bug）

### 3.1 核心响应式

- **`Bond` 依赖集变化时订阅不更新**（`reflow/bond.rs:149-159`）。仅当 `value_changed` 时才把新 gathers
  `bind_view`，且从不 `unbind` 旧依赖。用 `with_partial_eq()` 门控时，若新依赖恰好与旧值相等，
  新依赖**完全不绑定** → 后续更新丢失。复现：
  `Bond::new(move || if *flag.get() { *a.get() } else { *b.get() }).with_partial_eq()`，
  flag 翻转时 `b == a` → `b` 未绑定 → `b.revise()` 不更新视图。
  另外 `bind_view` 对嵌套 Bond 每次调用都 `count + 1`，与 `unlace_view` 的减法不匹配。
- **调度器超过 8 轮后静默丢弃待处理更新**（`reflow/scheduler.rs:528-532`）：`warn!` 后 `break`，
  `pending_items` 被 drop 且已从 `REVISING_ITEMS` 移除，状态与 DOM 分叉直到下一次 revise。
- **多 holder 共享 `Cage` 时另一个 holder 的订阅被剪掉**（`cage.rs:82-84` + `scheduler.rs:512-516`）：
  `holder_id()` 取第一个订阅 view 的 holder，`run(holder_A)` 会把在 A 的 ROOT_VIEWS 里找不到的
  B 的 view_ids 从 cage 中删除。场景：进程级 `static`/`Lazy<Cage>` 被多个 SSR 请求或多个 desktop 窗口读取。
- **`Cage` 陈旧句柄读到别人的值**：`cage.rs:291-293` 的 `borrow`、`:130-143` 的 `Serialize`、
  `:255-257` 的 `subscriber_count` 都不检查 `is_current()`，slot 被复用后静默返回新占用者的值，
  而不像 `get()` 那样报 `Stale`。
- **`Each` 重复 key 产生孤儿 view**（`widgets/each.rs:193-207`）：重复且此前不存在的 key 会创建两个 view，
  `new_key_view_ids` 只记住后者，前者永久残留并渲染。文档（`each.rs:60-62`）称"duplicates collapse to
  a single view"，与行为不符。
- **`Each::build` 在持有 items `Ref` 期间执行子 widget `show_in`**（`each.rs:159-168`）：
  子 build 若 revise 同一 items cage 即 `BorrowMutError`。`patch` 路径已用 `store_in` 规避，`build` 未同步。
- **`Loader::patch` 持有 `state` 的 `Ref` 调用用户 callback**（`loader.rs:195-200`、`:333-338`）。
- **`Suspense` 的 `<glory-suspense>` wrapper 在 detach 时不移除**（`suspense.rs:89-113`），
  长会话下残留。
- **`ErrorBoundary` 在 CSR 上不生效**：`error_boundary.rs` 与 `scheduler.rs:482` 依赖 `catch_unwind`，
  而 `wasm32-unknown-unknown` 的 panic 一律 abort，捕不到。`docs/api-guide.md:132-138` 未提示。

### 3.2 SSR / CSR 一致性

- **`text()` + 子元素两端结果不同，且 CSR 响应式 text 会清空子元素**：SSR（`ssr_dom.rs:248-257`）
  有子节点时忽略 `inner_text`；CSR（`web/widgets/csr.rs:103-115`）先 `set_text_content` 再追加子元素。
  更糟的是 CSR patch 时 `Cage` 文本再次 `set_text_content`（`web/attr.rs:116`）会**删除所有子 DOM 节点**，
  而子 view 仍以为自己挂着。`p().text(counter).fill(span())` 第一次 revise 后 span 消失。
- **SSR 把 camelCase JS 属性原样当 HTML 属性输出**（`ssr_dom.rs:137-150`）：
  `label().html_for("name")` → `<label htmlfor="name">`（应为 `for`）；
  `meta().http_equiv(..)` → `httpequiv`；`accept_charset` → `acceptcharset`；
  `default_value` / `selected_index` / `validity` 等纯 JS 属性被输出成无意义属性。CSR 正确，SSR 错误。
- **`Command::Create` 无命名空间**（`renderer/command.rs:56-60`）：JS 用 `document.createElement`，
  而 CSR 用 `create_element_ns`。desktop / LiveView / native 上 `svg()` 渲染为 `HTMLUnknownElement`，
  尽管 core 提供了整套 `svg::*` 工厂。
- **`AttrValue` 不支持 `Lotus<T>`**（`web/attr.rs`），而 `PropValue`（`prop.rs:86-101`）与
  `ClassPart`（`class.rs:139-155`）都支持。于是 `.prop("value", lotus)` 可以，`.attr("title", lotus)`
  与 `.text(lotus)` 编译失败。
- **`IntoFiller for Cage/Bond` 只是一次性快照却做了订阅**（`web/widgets/mod.rs:743-761`）：
  值变了内容不变，但父 view 被无谓重新 patch。
- **`Classes` 在 `<html>` / `<body>` 上覆盖而非合并**（`web/class.rs:49`）。
- **事件名表含错误项**（`web/events/mod.rs:110,185`）：`doubleclick`（DOM 事件是 `dblclick`，
  此监听永不触发）、`graffchange`（`slotchange` 的拼写错误副本）。`mod.rs:259-267` 的测试
  把 `doubleclick` 当"覆盖 Dioxus 缺口"固化了。

### 3.3 CSR 事件系统

`crates/core/src/web/helpers/event.rs:40-43` 用 `Reflect::set(node, "$$$click", cb)` 存储处理器，
同一节点同事件第二次 `.on(click, ..)` **静默覆盖**第一个；而非冒泡事件（`:48-55`）是叠加。行为不一致。

泄漏：`event.rs:40,53,107` 用 `Closure::into_js_value`（等价 forget），每个带非委托监听
（`focus` / `blur` / `mouseenter`）的元素永久泄漏 Rust 堆；`web/widgets/csr.rs:528-531` 的
`visible` 用 `mem::forget` 保存 `IntersectionObserver` 且从不 `disconnect`。
`Each` 列表反复重建行时线性增长。

### 3.4 路由

- **`stuff_keys` 永不失效 → 嵌套 outlet 回访后空白**（`routing/src/lib.rs:204-215`）：
  `insert_stuff("content@settings")` 若 key 已登记直接 return，而 `remove_stuff` 与 `Graff::build/patch`
  都不清 key。复现：`/app/settings` → `/other` → 回 `/app/settings`，内容区空。
  `tests/navigation.rs` 只测了兄弟切换。
- **派生 `from_url` 用 `?` 提前返回 None**（`macros/src/lib.rs:1163-1167`）：
  `User{id:u64}` 声明在 `New` 之前时，`/users/new` 命中第一个模式、`u64` 解析失败 →
  **整个 `from_url` 返回 None** 落到 not_found，而不是继续尝试下一变体。
  `crates/glory/tests/routable_derive.rs:58` 把这个行为当作了预期。
- **`Router::detect` 失败分支不回滚 `params`**（`router.rs:111-120`）：
  `users/<id>/edit` 与 `users/<name>` 同时存在时，`/users/bob` 会得到 `{id: bob, name: bob}`。
- **特异性按段求和而非字典序**（`filters/path.rs:846-857`）：`/<a>/<b>/<c>` = 120 > `/x` = 100，
  于是 `/x/y/z` 被动态三段路由抢走。leptos / dioxus 都是逐段比较。
- **CSR 与 SSR 的 Locator 更新时序相反**：CSR（`aviators/browser.rs:98-108`）先 `receive` 再跑 handler，
  旧页面会先用新参数重渲染一轮（触发无意义 fetch，两轮 flush）；SSR/Memory（`aviators/mod.rs:36-51`）
  先 `scrape` 后 `inject`，goal 里 `truck.obtain::<Locator>()` 得到 `Err`。同一份路由代码两端行为不同。
- **`join_path!` 用 `PathBuf::push`**（`router.rs:16-26`）：子路径以 `/` 开头时替换父路径，
  `with_path("app").push(with_path("/about"))` 在 Salvo 表里成 `/about`，而 glory 匹配是 `app/about`。
- 其他：`goto` 同 URL 重复压栈并无条件滚顶；anchor 处理取 composed path 最外层锚点且同 URL + `#hash`
  不滚动；`CombWisp::validate` 的 `while index < len - 2` 漏检最后一对（`/<a><b>` 被接受）；
  `append_route_query_param` 在含 `#fragment` 的 URL 上把 query 追加到 fragment 之后；
  `MemoryAviator::back/forward` 失败时 cursor 已错位。

### 3.5 Server functions

- **wasm 客户端拿不到 `redirect`**（`serverfn/src/lib.rs:2732`、`:2879` 用 `RequestRedirect::Manual`）：
  fetch 规范下 manual 模式的 3xx 返回 opaque-redirect 过滤响应（`status == 0`、headers 空、body null），
  于是客户端得到 `Request("HTTP 0")`；而 reqwest 客户端（`Policy::none`）得到真实的 303 + JSON body
  并正确解码为 `Http{status:303}`。同一段代码 CSR 与 desktop 行为不同。
  测试 `tests/server_fn.rs:201-222` 只覆盖了 `handle()` 直接调用。
- **`ReactiveWebSocket::reconnect()` 产生重复连接**（`:862-869`）：先 `socket.close()`（异步）
  再立即写入新 socket；旧 socket 的 `onclose`（`:969-989`）稍后触发并 `take()` **把新 socket 取走置 None**，
  然后又开第三个连接。期间 `send()` 报"not connected"，最终两个活跃 socket 都在改同一批 Cage。
  每次重连还泄漏 4 个闭包（`:906` 只丢 JsValue）。
- **`close()` 在未连接状态永久停在 `Closing`**（`:851-860`）。
- **每次调用都新建 `reqwest::Client`**（`:2694-2697`、`:2846-2849`）：新连接池 + TLS 上下文，
  桌面应用高频调用会耗尽 fd。
- **`SseDecoder` 遇到非法 `retry` 值就让整个流报错**（`:616-622`），SSE 规范要求忽略。
- **`status_code()` 对客户端错误也返回 500**（`:306-312`）：UI 无法区分"网络断了"与"服务器 500"。
- **405 响应缺 `Allow` 头**；`dispatch_with_method` 每请求线性扫描 inventory。

### 3.6 桌面 / 原生

- **`DesktopCloseBehavior::Hide` 后无法重新显示窗口**（`runtime.rs:1231-1234`）：
  `WindowCommand::Focus`（`:1617-1620`）只 `set_focus()` 不 `set_visible(true)`，
  而文档（`:817-821`）说"Re-show it later via focus"。所有窗口都隐藏且无托盘时进程挂起且不可交互。
- **窗口关闭后该窗口的挂起 `eval` 永不 resolve**（`:1546-1574` 丢弃 slot 但不
  `eval_resolve(Err(Disconnected))`）。
- **菜单路由按 item id 全局键控**（`:580`、`:1710`）：两个窗口用同一 `MenuSpec` 时，
  第一个窗口的菜单项会路由到第二个窗口的 holder；窗口关闭后路由也不删除。
- **无 `preventDefault` 通道**：`wry_interpreter.js:150-152` 只 `emitEvent`，core 无对应 API。
  `form().on(events::submit, ..)` 在桌面会导航到 `about:blank` → 二次 `Ready` →
  而 `runtime.rs:1283-1297` 对重挂载只 `warn!("remount is not supported yet")` → **应用死亡**。
- **JS `nodes` Map 永不因 `Remove` 收缩**（`wry_interpreter.js:140-144`）：
  只有 `DetachEvent` 且节点已断开时才删除，绝大多数无监听器的节点在 `Each` 高频增删下无限累积。
- **原生子树移除后映射表与 blitz slab 双重泄漏**（`blitz_consumer.rs:340-349`）：
  core 只对子树根发 `Remove`，消费端只删根的映射，后代永久残留；
  blitz 的 `remove_node` 仅 detach 不 drop。`SetText` 释放了 blitz 节点却不清 `blitz_to_glory`，
  slab id 复用后可能把新文本节点映射到旧 glory id。
- **原生每个 UI 事件克隆整张映射表**（`blitz_consumer.rs:665-670`），鼠标移动时 O(节点数)。
- **不冒泡事件被当作冒泡处理**（`blitz_consumer.rs:736-741` 沿整条 chain 找监听者，不看 `bubbles`）。
- **`Insert` 锚点非子节点时行为三方不一致**：参考实现回退尾插，JS 抛 `NotFoundError` 中断整批，
  native 在 blitz 里 `unwrap` **panic**。
- **`flush` 失败无反馈**（`runtime.rs:1896-1898` 仅 `error!`；JS 第一条失败即 throw，
  后续命令丢失，`evaluate_script` 不回传异常，holder 认为已应用）。
- **多窗口共享全局资产清单**（`runtime.rs:1017-1029` 每窗口 `clear_asset_manifest()` 后重装）。

### 3.7 CLI

- **构建期间的编辑丢失**（`command/watch.rs:129` 快照 → 构建 → `:196` 清空）：
  构建期间再次保存同类文件时 `Interrupt::add()` 因已存在返回 false 不发信号，
  构建完成后集合被整体清空 → 第二次保存永远不会被构建。
  独立的样式任务（`:132-143`）在成功时也 `clear_source_changes()`，
  可能把主构建被中断后待重试的变更一并抹掉。
- **`CARGO_TARGET_DIR` 下产物路径与 cargo 输出不一致或直接 panic**：
  `ext/cargo.rs:76-78` 的 `unbase(workspace_root).unwrap()` 在 target dir 位于 workspace 之外
  （CI 常见的共享 target）时 panic；`compile/server.rs:58` 与 `front.rs:29,67` 硬编码
  `target/server` / `target/front`，与 `metadata.rel_target_dir()` 推导的路径可能分叉，
  结果是 `service/serve.rs:116-118` 记一条 debug "no exe found"，服务器永远不启动且不报错。
- **构建失败时 `run` / `serve --no-reload` / `end2end` 退出码为 0**
  （`command/serve.rs:8-10`、`command/end2end.rs:22-24` 直接 `return Ok(())`）。
- **`--https` 让 live-reload 失效**：`lib.rs:77-79` 只置 `site_https`，从不输出
  `GLORY_RELOAD_PROTOCOL`，core 默认 `ws://`，https 页面连 ws:// 被浏览器拦截；
  且 reload 服务本身不支持 TLS。
- **Tailwind 输入为绝对路径时输出覆盖源文件**（`config/tailwind.rs:15-26`）：
  `Utf8PathBuf::push` 遇绝对路径整体替换，`tailwindcss --input X --output X`。
  嵌入式 builder 无元数据模式下 `config_dir` 就是绝对的 workspace_root，必然触发。
- **`site_addr` 为 `0.0.0.0` 时 Windows 上每条 reload 消息先等 10 秒**
  （`service/reload.rs:91-95` 的 `wait_for_socket` 连接 `0.0.0.0:port`，Windows 不允许 → 20×500ms 全失败）。
  同一模式让端口占用检测假阴性，随后 salvo 内部 bind `unwrap` 在任务里 panic，reload 服务静默消失。
- **每次 debug 重建都做 brotli-11 / zstd-21 / gzip-9 三重压缩**（`compile/front.rs:114-130`），
  且是同步 IO 写在 async 任务里。debug wasm 常达 10–30 MB。
- **watch 模式下样式编译失败被静默吞掉**（`command/watch.rs:136-142` 只匹配 Success 分支，
  `Outcome::Failed` 与 lightningcss 解析错误既不打日志也不推 overlay）。
- **Linux aarch64 上下载 x86_64 版 wasm-opt**（`ext/exe.rs:381-383`）。
- **macOS 签名/公证参数构造有误**（`bundle.rs:441` 把 Team ID 当 codesign 身份；
  `:516-519` 的 API key 模式只传 `--key <id>`，而 `notarytool` 需要 `.p8` 路径 + `--key-id` + `--issuer`）。
- 其他：`end2end` 不等服务器就绪即启动测试；`end2end_cmd` 无 `end2end_dir` 时 `current_dir("")` 失败，
  且 `cmd.split(' ')` 不支持引号；`.env` 查找相对于用户 cwd 而非 workspace；
  对 `GLORY_ANDROID_*` 等合法变量发"not used by glory-cli"假警告；
  `broadcast::channel(1)`（`signal/reload.rs:5`）在 Style 紧接 Full 时丢消息；
  子进程未 `kill_on_drop`，CLI 异常终止时应用服务器成孤儿。

### 3.8 LiveView 客户端

- **`open` 事件重置退避 → 250ms 无限重连风暴**（`liveview_client.js:60-80`）：
  服务端 `read_hello` 失败后直接关闭 socket，于是 open → close → 250ms → open 循环。
- **断线重连期间 `send` 静默丢弃事件**（`js:23-27`），无队列，1–5 秒重连窗口内的点击全部丢失。
- **`RemoveProperty` 赋 `undefined` 再 `delete`**（`js:125-129`）：对 `input.value` 这类原型访问器，
  `undefined` 经 DOMString 转换成字符串 `"undefined"` 显示在输入框里，`delete` 对原型属性无效。
- **`Remove` 语义与参考实现相反**（`js:140-144` 不校验 parent 仍 `child.remove()`，
  而 `command_dom.rs:177-181` 及其测试规定过期 Remove 应忽略）。
- **actix 适配器丢弃 `Continuation` 帧**（`liveview/src/lib.rs:1092-1128` 的 `_ => {}`）：
  actix-http 默认 `max_size = 65_536`，超过 64 KiB 的文本消息被拆成 continuation 后**静默丢失**。

---

## 4. 实现了但没接入（有类型、有单测、无调用者）

这是本次评审最集中的问题。以下各项在 `_todos.md` / `_improve_todos.md` 中多数标记为 `[x]`。

| 模块 | 项目 | 位置 | 现状 |
|---|---|---|---|
| liveview | `OutboundBuffer` / `enqueue_patch` / `max_pending_patches` | `lib.rs:133-208,222` | LV2 标完成；无任何适配器调用，配置字段被传入 `SessionManager` 后无读取点。且当前协议是严格"一请求一回复"，根本没有服务端主动推送，折叠语义在接入前需重新审视 |
| liveview | `SessionRegistry::remove` | `lib.rs:362-364` | 客户端 `Close` / socket 结束时都不调用，session 要等 idle 超时 |
| liveview | `LiveviewRouter` trait | `lib.rs:394-401` | 只为 axum/actix 实现，salvo 未实现且无法自定义 path（`normalize_liveview_path` 被 salvo 完全忽略） |
| liveview | `Ping` / `Pong` | `lib.rs:46-47` | 双方都不主动发起，是 P0-15 的根因 |
| hot-reload | `ReloadHooks` + `ReloadStateStore` | `hot-reload/src/lib.rs:204-269`、`state.rs` | HR1 标完成；core / desktop / cli / examples 均未接线 |
| hot-reload | `reloadable_view!` / `FunctionRegistry` | `hot-reload/src/lib.rs:28-46` | 除 crate 自身 tests 与一个 example 外无使用；core widget 层没有任何注册点，CLI 扫描的是用户代码里不存在的标记 |
| hot-reload | `patch.js` | `hot-reload/src/patch.js:1` | 整个文件只有一行 `console.log`，而 README:18-21 称它是"dispatches glory:function-reload events 的客户端运行时" |
| cli | `build_error_overlay_js` | `cli/src/service/reload.rs:161-210` | CL6 标完成；服务端会发 `build_error` 消息，但浏览器脚本（`core/src/web/utils/ssr.rs:19-36`）只处理 `full/style/functions`，桌面端（`desktop/runtime.rs:966-972`）的 `ReloadMessage` 连该变体都没有，收到后打 warn。overlay 从未被注入 |
| cli | `--proxy` / `--tls-cert` / `--tls-key` | `cli/src/lib.rs:73-89` | 只把字符串塞进 `GLORY_PROXY_CONFIG` / `GLORY_TLS_*` 环境变量，全仓库（core/glory/desktop/serverfn/liveview/examples）无人读取。同样无人读取的还有 `GLORY_SITE_SCHEME` / `GLORY_SITE_URL` / `GLORY_TARGET` / `GLORY_LIB_DIR` / `GLORY_BIN_DIR` |
| cli | `crates/cli-harnesses` | 整个 crate | 自述"validate the manifest layouts that glory-cli must understand"，但测试只调 `cargo_metadata` 断言包名，从不调 `glory_cli::config::Config::load`，也没有指向 glory-cli 的 dev-dependency。fixture 本身还是坏的（同 P0-11） |
| serverfn | `RenderFreshness` / `IncrementalCache` / `FileSystemCache` | `serverfn/src/lib.rs:2263-2528` | FS6 标完成；`ServerHolder` / `SalvoHandler` 没有任何缓存查找/写回钩子，也不设置 `Cache-Control` |
| serverfn | `PreloadedState` + `script_tag` | `:2533-2578` | `docs/fullstack.md:38-39` 称"Hydrated clients can parse the same JSON"，但客户端侧没有任何读取实现 |
| serverfn | `is_multipart_request` / `decode_current_multipart` | `:2056-2076` | 实际不可用：`#[server]` 函数体拿不到原始 body，自定义路由不在 `with_request_context` 作用域内 → `request_context()` 永远 None → 恒返回 400。`docs/fullstack.md:155-163` 的示例无法成立 |
| serverfn | `WebSocketEndpoint` | `:1452-1513` | 文档说"adapter upgrades the connection"，但三个 mount 都没有 WS 路由；`examples/realtime-notify` 需用户自己写 upgrade 且无 Origin 检查 |
| serverfn | `SseDecoder` / `SseMessage` / `encode_sse_events` / `decode_json_lines` / `WebSocketFrame` | `:470-478,562-676,1094-1097` | 仅测试引用，没有任何客户端/服务端路径消费 |
| serverfn | `ServerFnEncoding::name/from_name` | `:49-57,69-78` | 无调用者；宏走的是枚举路径 |
| serverfn | cbor / postcard 编码 | — | GET 与 stream 路径硬编码 JSON（`:2650-2654`、`:2813`、`:2853`），wasm stream 无论返回类型都发 `accept: application/x-ndjson` |
| core | devtools 协议 | `devtools.rs:38-168`、`cage.rs:269`、`bond.rs:194`、`command.rs:428` | 有 `DevtoolsSnapshot` 类型和三个 snapshot 产生器，但**没有任何函数汇总它们**，`DevtoolsMessage` 也没有传输层。desktop 的 "devtools" 是 webview inspector，与此协议无关 |
| core | `reflow::untrack` + `UNTRACKING` | `reflow/mod.rs:148-182`、`scheduler.rs:355-382` | 全仓库无调用；且 `UNTRACKING` map 在 holder `Drop` 中未清理，一旦使用会按 HolderId 泄漏 |
| core | `renderer::AttributeValue` + `set_attribute_value` | `renderer/mod.rs:39-133` | 富类型属性枚举已实现，web 层仍全走 `Cow<str>`（AGENTS.md 标为 roadmap） |
| core | `LoadState` | `widgets/loader.rs:13-23` | 是 `Loader::state()` 的返回类型，但 `widgets/mod.rs:11-12` 不导出，`glory` 门面也没有 → 用户可以调用但无法在签名里写出该类型、无法 match |
| core | `DEPOT_URL_KEY` | `web/holders/server.rs:15` | 只写入不导出；routing 用字面量 `"glory::url"` 读，跨 crate 靠字符串对齐 |
| desktop | `DesktopCloseBehavior` | `runtime.rs:810-822` | **类型未从 crate 导出**（`lib.rs:105-112` 的 `pub use` 列表不含它，`mod runtime` 是私有的），于是 `close_behavior` / `with_close_behavior` / `set_close_behavior` 三个 API 外部**无法调用**。CHANGELOG:38-40 宣称已新增 |
| desktop | `CommandSink` / `RecordingSink` | `desktop/src/lib.rs:61-87` | 无 crate 外调用者，runtime 的 flush 也不走它 |
| desktop | `templates/tauri/` | — | 引用不存在的 `WryCommand` 类型，无任何代码或文档引用 |
| native | `glory-tui` | 整个 crate | `src/lib.rs:11` 只 `pub use ratatui;`，全文件无 `ratatui::` 调用，`render_outline` 只产 `String`。却在 `docs/release-readiness.md:89` 的发布清单里 |
| routing | `aviators/url_hash.rs` | 整个文件 | `mod.rs` 未声明该模块；文件引用的 `Aviator::push/replace/go/length` 不存在于 trait，一旦接入就编译失败 |
| routing | `LocatorModifier` 的 replace / scroll | `locator.rs:26-38` | 只有 `BrowserAviator` 认；trait 方法是 `goto(&str)`，无法通过 trait 或 `goto_route` 传递；`MemoryAviator::goto` 无 replace 语义 |
| routing | `base_path` | `browser.rs:22,42,185`、`server.rs:23,34` | 字段存在，全仓库无 setter |
| routing | `Locator` 的 scheme/authority/host/port/fragment | `locator.rs:60-67` | 五个私有 Cage 在 `receive` 里维护，但没有 getter。`fragment` 缺失直接阻断 hash 路由 |
| routing | `Router::layout` / `outlet` / `Outlet` | `graff.rs:92-96`、`router.rs` | 只在 docs/tests 出现，所有 examples 仍用 `Graff` + `insert_stuff`；且有 §3.4 的 key 泄漏 |
| routing | `with_redirects` | `aviators/server.rs` | 无 example 使用；Salvo 路由表也不为 legacy 路径建条目 |
| macros | `proc-macro-crate` 用于 `#[server]` | `macros/src/lib.rs:509-581` | `Store` 与 `Routable` 都用它解析 crate 路径，`#[server]` 却硬编码 `glory_serverfn::`，逼得 app crate 必须直接依赖 `glory-serverfn`（`glory/Cargo.toml:40-43` 用注释背书） |
| glory | `glory::ssr` | `glory/src/lib.rs:114-117` | 只再导出 `ServerHolder`，不含 `SalvoHandler` / `HtmlChunk`；文档反而叫用户去 `glory_core::web::holders::SalvoHandler` |

**Command 变体消费矩阵**（desktop JS / native / `CommandDom` 参考 / liveview）：
除 `SetHtml` 在 native 是 no-op（`blitz_consumer.rs:299-303`）外，所有变体都被三方消费。
但 `AttachEvent.bubbles`（`command.rs:104-108`）与 `Create.is_void`（`:56-60`）**在所有消费端被忽略**，
是线协议上的死字段；`EventData` 的 `clipboard` / `selection` / `scroll` / `resize` / `media` 五族
（`:255-267`）在 desktop JS（`wry_interpreter.js:61-105`）与 native 都不填充，
于是 `clipboard_text()` / `selection_range()` 在非浏览器后端永远返回 `None`。
liveview 客户端不自己解释命令，只把 `__gloryApplyWryBatch` 转发给 desktop 解释器，覆盖一致。

---

## 5. 实现了但没做完

- **流式 SSR 不是 over-the-wire 流式**：`web/holders/server.rs:145-215` 的 `streaming_chunks`
  先序列化壳层，**随后同步** `drive_deferred()` 把所有 deferred future 跑完，再把补丁 push 进
  同一个 `Vec<HtmlChunk>` 返回；`render_stream` 只是 `stream::iter(vec)`。首字节仍要等全部资源完成。
  占位/补丁格式做完了，传输层没做。`docs/api-guide.md:214` 与 `_todos.md` FS4 已承认。
  依赖 `futures-channel` 已在 `Cargo.toml` 声明但从未使用。
- **水合与流式补丁的竞态**（待确认，取决于时序）：wasm 通过 `requestIdleCallback` 立即启动，
  而 `__gloryResource` 脚本是最后一个 chunk。若 wasm 先完成 mount，客户端拿不到 hydrated 资源
  会重新 fetch，随后到达的补丁模板仍会被 `__gloryStreamHydrate` 换入 DOM，
  这些节点带 `gly-id` 却无 `gly-hydrating` 标记，不会被清理 → 服务端渲染体与客户端渲染体并存。
- **Head 去重只在单个 `DedupHead` 内生效**（`web/widgets/document.rs:100-116`）：
  多个组件各自 `head_mixin()`、以及框架自带的 `<meta charset>` / `<meta viewport>`
  都不参与。`document::charset("utf-8")` 必然产生重复 `<meta charset>`。
  文档称"mirror Dioxus"，而 Dioxus 是文档级去重。
  另外 `DedupHead::build/flood`（`:239-253`）的 `flood` 只调 `patch`（默认空）**从不 `attach_child`**，
  作为根 widget mount 时什么都渲染不出。
- **`.get()` 在 `Widget::build` / `patch` 中不订阅视图**：`AGENTS.md:53-56`、`docs/api-guide.md:31`、
  `lib.rs:27-30`、`reflow/mod.rs:6-7` 四处都声称通过 `TRACKING_STACK` 订阅，
  但 `gather()` / `push_layer` 的调用者只有 `bond.rs:47,128`、`effect.rs:57,71`、`loader.rs:127,139,184`，
  `View::build`（`view.rs:139-163`）与 patch 调用（`scheduler.rs:482-484`）都**没有**推入 tracking 层。
  用户自定义 Widget 在 build 里读 cage 再手写文本，值变化后永远不会 patch。
  所有 example 都靠 `.text(cage)` / `Bond` 的显式 `bind_view` 才能工作。**这是文档与实现相反，误导成本最高。**
- **`use_coroutine_in` 未 scope 化且在 SSR 上死锁**（`reflow/effect.rs:302-313` 的 `let _ = parent;`）：
  文档承诺"host widget detach 时句柄全 drop、协程自然终止"，实现完全没有把 sender 交给 Scope/Owner。
  更严重的是配合 P0-2，一个 `while let Some(msg) = rx.next().await` 的协程在 build 中启动、
  而 tx 尚未 drop → **SSR 请求在 build 阶段永久阻塞**。
- **`resource_hydratable_in` 客户端快路径丢失响应式**（`effect.rs:185-188`）：
  hydration 命中即 `return Cage::new(Some(value))` 不注册 Effect，
  此后依赖（如 URL 参数 cage）变化不会重新 fetch。文档说"Behaves exactly like resource_in, except…"。
- **`Switch::patch` 只处理"当前 case 已 attach"的分支**（`widgets/switch.rs:87-91`），见 P0-8。
- **`render_string()` / `render_stream()` 不可重入**：注释称"Non-draining: rendering is repeatable"
  只对 `replay()` 成立，`stream_ssr::finish()` 与 `take_resource_data()` 都是一次性消费。
- **`css_module!` 缺 `:global()` 且会改写 `url()` / `@import` 里的扩展名**
  （`macros/src/lib.rs:208-281`：任何 `.` + 标识符都被当 class，`url(a.png)` → `url(a.png__gly_xxxx)`），
  与同仓库 `web/style.rs:132` 的语义不一致。
- **`asset_folder!` 不追踪新增文件**（`macros/src/lib.rs:63-109` 在展开时枚举目录，无 `tracked_path`）；
  `asset!` 为存在性检查把整个文件 `include_bytes!` 进常量，大资源显著拖慢编译。
- **`doctor` 检查的工具与 CLI 实际依赖不一致**（`cli/src/command/doctor.rs:39-42`）：
  要求 `wasm-bindgen` / `wasm-opt` 二进制在 PATH，但 CLI 用的是库 `wasm-bindgen-cli-support`，
  `wasm-opt` 会自动下载；还把 rustfmt/clippy 缺失算作失败。干净机器上退出码 1 且指引错误。
- **`self-update` 是 stub**（`cli/src/command/self_update.rs:3-12` 只打印一段安装命令）。
- **bundle 的哈希资产对 Web 目标没有闭环**（`cli/src/command/bundle.rs:1400-1423`）：
  生成 `name.<hash>.ext` 副本并写入 `asset_map`，但从不改写 `index.html`、
  wasm-bindgen 生成的 JS 里的 `.wasm` 引用或 CSS `url()`。全仓库消费 `glory-bundle.json` 的只有
  desktop runtime，`core/src/assets.rs:155` 的 `install_asset_manifest` 没有任何 Web/SSR 侧调用。
  结果是 Web bundle 体积翻倍（原件 + 哈希件，各自再 `.gz/.br`），缓存友好性为零。
- **Cargo.toml / `.env` / `tailwind.config.js` 变更不触发重建**（`cli/src/service/notify.rs:18-44`
  的 watch 集合只含 src、js_dir、样式目录、assets；`Change::Conf` 无人产生）。
  改依赖、改 features、改 `.env` 都要手动重启。
- **`--hot-reload` 在浏览器端没有消费者**：服务端链路完整，但浏览器脚本
  （`core/src/web/utils/ssr.rs:35`）只是 `dispatchEvent(new CustomEvent('glory:function-reload'))`，
  core 内没有任何监听者。更根本的是 `FunctionReloadBatch`（`hot-reload/src/lib.rs:100-128`）
  **只携带 `function_id` / `source_path` / `line_number`，没有新代码**，运行中的 wasm 不可能拿到新闭包体。
  而且同一个 `.rs` 改动会同时触发 function-reload 与整页刷新（`watch.rs:224-226` +
  `notify.rs:95-101`），前者几秒后被后者覆盖。
  `docs/hot-reload-binary-patch-evaluation.md:14-18` 描述的"swaps the registered closure at runtime,
  Cage state survives"与实际不符。
- **原生网络图片只做"分类 + 日志"**（`native/src/image_provider.rs:273-278`）：
  `<img src="https://…">` 在原生永远空白，且只有 info 级日志。
- **桌面二次 `Ready` 不支持重挂载**（`runtime.rs:1283-1297`），见 §3.6。
  对照 `examples/mobile-counter/src/lib.rs:249-253`，mobile 宿主对每次 Ready 都重建 holder。
- **GPU 截图回归被 `#[ignore]`**（`native/tests/screenshot.rs:107`），常规运行的替代测试
  只断言 `get_node(0).is_some()`，不检查布局尺寸。
- **`config.rs` 两套默认值不一致**：`Default`（`:46-59`）是 `site_root="."` / `reload_port=8001` /
  `not_found_path="/404"`，`try_load_from_env`（`:105-118`）是 `"target/site"` / `3001`；
  字段文档写 8001，`not_found_path` 文档说 `site_root/404.html` 实为 `/404`。
  另外 `:153` 的 `Environment::with_prefix("GLORY").separator("_")` 会把 `GLORY_SITE_ADDR`
  解析为嵌套键 `site.addr`，可能无法覆盖扁平字段 `site_addr`（**待确认**，需实测 config 0.15 行为）。

---

## 6. 死代码

以下均已 grep 全仓库（含 examples/、docs/）确认无生产调用者。完整清单见各 §4 表格，此处补充未列出的：

**glory-core**：`reflow::selector`（`bond.rs:97-102`）、`Cage::borrow`、
`View::prepend_view/append_view/before_view/after_view`（`view.rs:193-229`）、`View::child`、
`Scope::provide_cage/cage_context/use_cage`（`scope.rs:340-371`，仅本文件测试）、
`Loader::observe` + `observing` 字段、`Loader.fut_maker` 的 `Option` 包装（永远 `Some`，三处 `unwrap`
与 `new` 的 `Clone` 约束都无意义）、`LoadState::is_idle/is_loading`、`spawn::queue_microtask`、
`is_dev/is_release`（`lib.rs:156-163`）、`RenderedElement` / `MockRenderer` 族、
`CommandNode::value/bounding_rect/scroll_offset/tag_name/is_void`、`HolderId::null`、
`Truck::inner/with_capacity/capacity/delete`、
`HtmlChunk::Placeholder` 变体（流水线在 `server.rs:168-172` 手写同样的字符串）、
`ssr_event_listener`、`location_pathname`、`debounce`、`undelegated` / `Custom` 事件、
`config.rs` 的 `site_root` / `not_found_path`（读入但 core 无消费者）、
`From<&str> for ReloadWebSocketProtocol`（会 panic 且无调用者）。

**死分支**：`console.rs:9-50` 的 `is_server()` 分支（模块只在 wasm+csr 编译，恒 false）；
`web/helpers/mod.rs:63` 同；`ssr_dom.rs:265-266` 的 `properties["text"]` 分支
（`set_property("text")` 已被翻译成 `SetText`，回放路径写不进 properties）；
`routing/aviators/browser.rs:160` 的 `state` 标记分支被 163 行的空 href 早退完全覆盖；
`cli/src/compile/change.rs` 对 `Change::Conf` 的所有判断。

**glory-routing**：`Url::set_fragment/set_query/set_path/set_port/set_host/set_password/
set_username/set_scheme`（CSR 版本无论成败都返回 `Ok`，与 doc 承诺的 `Err` 语义不符）、
`ParseError` 十个变体中九个从不构造、`Router::routes_mut/hoops/hoops_mut/filters/filters_mut/
with_hoop_when/hoop_when/WhenHoop/then/with_filter_fn`、`Filter::type_id/type_name`、
`PathWisp::type_id/type_name`、`Handler::type_id`、`ScrollMemory::peek/forget/clear/capacity/len/is_empty`
（`DEFAULT_SCROLL_MEMORY_CAPACITY` 导出了但 `BrowserAviator` 固定用 `Default`，
`ScrollMemory::new(cap)` 从后端不可达）、`Locator::route::<R>()`、`AviatorExt::goto_route`。
wasm 版 `Regex::as_str` 对 `RegExp` 调 `as_string()` 恒为 `None` 返回 `""`，
导致 `RegexWisp` 的 `PartialEq` 在 wasm 上**所有正则相等**（`filters/path.rs:423-428`）。

**glory-cli**：`location!` 宏（引用私有模块里不存在的 `Location`，且不填 `modules` 字段，
一旦使用必然编译失败）+ `Location` 结构体、`Watched::path_ext/path_starts_with/path_starts_with_any`、
`PathExt::relative_to`、`StrAdditions::with/to_created_dir`、
`PackageExt::{bin_targets, cdylib_target, target_list}`、`MetadataExt::package_for`、
`OutputExt::has_stderr`（内部还残留调试 `println!`）、`BuildProgress::{status,is_complete,is_failed}`、
`Overrides::is_empty`、`ChangeSet::is_empty`、`ProductSet::empty`、
`ExeMeta` 的 re-export（所有字段与方法均私有，对外无意义）。
`config/cli.rs:689-701` 的 `should_open() = open || !no_open ≡ !no_open`，`--open` 标志冗余。

**glory-liveview / hot-reload**：`SessionRegistry::insert`（生成可预测的 `lv-{id}` token，仅测试）、
`ResumeToken::into_string`、`hello_with_resume`、`liveview_client.js:39-42` 的 `"hello"` 分支
（服务端从不发送）、`ReloadableFunctionVisitor`（`pub struct` 但字段私有、无构造/读取 API）、
`hot-reload/Cargo.toml` 的 `syn` `printing` feature 与 `proc-macro2` `nightly` feature（后者已是 no-op）。

**glory-desktop / native**：`blitz_consumer.rs:480-481` 的 `#[allow(dead_code)] fn unused(_: Attribute)`
（只为压制 `use blitz_dom::Attribute` 未使用警告的 hack）；
`wry_interpreter.js:19-20` 的内部标签解码分支（Rust 侧只用外部标签，无生产者）。

**未使用的依赖**（源码 grep 无引用，未检查 benches）：`glory-core` 的 `futures-channel` /
`tokio-util` / `url` / `multimap` / `smallvec`；`glory-serverfn` 的 wasm `web-sys` features
`WorkerGlobalScope` / `RequestMode` / `Headers`（实现只认 `window()`，`RequestMode` 从未设置）；
`glory-tui` 的整个 `ratatui` + `crossterm`。

---

## 7. 功能缺失

与 Leptos / Dioxus / Phoenix LiveView 对比，且代码中已有迹象打算做的：

**响应式核心**
1. `on_cleanup` / Effect 清理回调。`Owner` 只登记 cage 的 invalidator，没有通用 cleanup 注册，
   于是 `use_coroutine_in` / `use_future_in` 无法在 scope 卸载时取消。
2. `Loader` / `resource` 的错误态。`LoadState` 只有 `Idle/Loading/Loaded`，`Fut: Future<Output = T>`
   没有 `Result` 通道，Leptos 的 `Resource<Result<..>>` + `ErrorBoundary` 联动在此不可能实现。
3. `ErrorBoundary` 的 `reset()` / 重试；fallback 自身 panic 直接 `resume_unwind` 不向上级边界传播。
4. 帧级合并 flush。`queue_microtask` 已实现但无人使用，CSR 下事件处理器外的连续 revise 仍是同步逐次渲染。
5. `Cage::set` / `update` 便捷 API（`CageLens` 有 `set`，`Cage` 本身没有）。

**Web**
6. 响应元数据（状态码 / 重定向 / 头）。`Scribe::render` 硬编码 200 text/html，
   组件无法设置状态或 header，not-found 只能靠 Salvo `Catcher`。这也是 ISR 接线的前置条件。
7. 独立的文本节点原语。所有文本都是元素的伪属性 `inner_text`，是 P0-5 / P0-6 / §3.2 一系列问题的共同根因。
8. 水合 mismatch 诊断（目前只有 `debug_warn`）。
9. 事件选项 `capture` / `passive` / `once`。
10. 每请求 CSP nonce 与 `Content-Security-Policy` 头注入。

**路由**
11. `Link` / `A` 组件（active class、`aria-current`、相对链接）；锚点拦截已有，无组件封装。
12. CSR 侧重定向（`#[redirect]` 目前只在 SSR 生效）与真正的 HTTP 3xx / 404 状态透传。
13. 路由守卫 / 可中止导航。`WhenHoop` 只能跳过自身，hoop 内调 `goto` 会重入 `locate` 但外层循环继续
    执行旧路由剩余 hoop + goal，覆盖掉重定向目标。
14. 懒加载 / 异步路由（`Handler::handle` 是同步的，RT3 仍是待办）。
15. hash 路由与 `Locator::fragment()` 访问器。
16. `Rc<dyn Aviator>` 注入（组件目前只能 `obtain::<BrowserAviator>()` 硬绑平台）。
17. 尾斜杠策略（目前一律忽略，无 strict / redirect 模式）。
18. typed 参数与 `Locator` 的集成。两者是两套表示（`MultiMap` vs `BTreeMap`），
    `match_route_pattern` 每次都重新 `PathFilter::new` + 编译正则，无缓存。

**Fullstack**
19. 流式 server fn 的服务端派发（P0-10）。
20. CSRF / 同源保护（§2.1）。
21. 有状态 / tower `Layer` 风格中间件（现在是裸 `fn` 指针，无法携带配置）。
22. 成功路径的响应控制。`Ok` 分支固定 200 + content-type，登录成功设置 cookie 必须走 `Err(redirect)`。
23. CORS / OPTIONS 预检（路由只注册 GET/POST，而 `set_server_url` 暗示跨 origin 场景）。
24. Web Worker 内调用（`Cargo.toml` 启用了 `WorkerGlobalScope` 但实现只认 `window()`）。
25. 端点名命名空间（`module_path!` hash）。

**LiveView**
26. 每连接上下文。widget 工厂是 `Fn() -> W + Send + Sync`，拿不到 Request / cookie / query / 认证信息，
    无法做"每用户的 LiveView"。这是 resume token 无法绑定认证的根因。
27. 服务端主动推送。`run_session` 只在收到消息时回复，没有定时器 / PubSub / 后台任务推 Patch 的通道。
    `CommandHolder::update()` 在 core 里已具备，但 liveview 不把 socket 写端交给 session。
28. 初始 SSR（dead render）+ 接管。页面在 WS 连上之前是空白。
29. `live_patch` / 导航、文件上传、JS hooks、`phx-debounce/throttle`、表单恢复。
30. 可观测性（liveview crate 零 tracing，连接建立 / 拒绝 / reap 都不可见）。

**桌面 / 原生**
31. 桌面异步运行时与事件循环集成（P0-2）。
32. `prevent_default` / `stop_propagation`（`bubbles` 字段已在协议里但无人消费）。
33. 命名空间元素（SVG / MathML）。
34. 窗口事件回传到 widget（resize / moved / focus / close-requested 可取消）；
    `DesktopWindowState` 已缓存 resize/focus 但不投递给 holder，`EventData.resize` 从不填充。
35. 桌面会话持久化 / 重挂载。
36. 原生 `SetHtml`、网络图片、剪贴板 / 拖放 / IME 选区。
37. 移动端原生生命周期桥接（`onTrimMemory` / `didReceiveMemoryWarning` 等已在模板 README 标为 not bridged）；
    `docs/mobile-native-recipes.md` 建议的"从原生回调注入 typed command 事件"当前没有 API 支撑
    （`CommandHolder::dispatch_event` 只能投递到已注册的处理器，而 mobile 宿主不暴露 holder）。

**CLI**
38. 内置静态文件 dev server。CSR-only 项目 `serve` 时只记一条 debug "no exe found"，什么都不提供——
    而 `glory new` 的默认模板恰恰是 CSR-only。
39. watcher 去抖 / 合并窗口（现在每个 fs 事件立即杀掉正在跑的 cargo 再重来；trunk/dioxus 都有 100–300ms 去抖）。
40. `--https` 的 TLS 终止与 reload 的 wss；`--proxy` 的实际代理。
41. bundle Web 目标的引用改写、HTML/JS 最小化。
42. 模板硬编码 `glory = "0.3.1"`，应改用 `env!("CARGO_PKG_VERSION")`。

---

## 8. 工程化与仓库卫生

### 8.1 CI 覆盖缺口

`.github/workflows/ci.yml` 只跑 4 个 crate 的测试：`glory-core`、`glory-cli`、`glory-serverfn`、`glory-native`。
**没有跑**：`glory-routing`（66 tests）、`glory-liveview`（12）、`glory-hot-reload`（18）、
`glory-desktop`（3）、`glory --features routing`（`_todos.md` RP2 明确要求加入必跑清单，但没加）。

`cargo check-all-features` 只覆盖 `glory-core -p glory -p glory-routing`，
`[package.metadata.cargo-all-features]` 也只在这三个 crate 里声明；
`glory-serverfn`（8 个 feature）、`glory-liveview`（3 个）、`glory-desktop`、`glory-native` 这些
重 feature crate 完全没有矩阵覆盖。

`crates/core/Cargo.toml:100` 的 `skip_feature_sets` 只排除了 `["web-ssr","web-csr"]`，
**没有排除** `["backend-command","single-app"]`，而 `lib.rs:69` 有 `compile_error!` 守卫该组合 →
`cargo check-all-features -p glory-core` 会命中它（待确认是否已在 CI 报错，或 all-features 恰好没生成该组合）。

`[package.metadata.docs.rs]` 只有 `glory-routing` 声明了（且是 `all-features = true`，
在互斥 feature 上碰运气）。其余 crate 没有 docs.rs feature set 配置。

### 8.2 发布阻塞（沿用 `_report.md`，仍未解决）

crates.io 已有 `0.3.1`，当前源码不能按同一版本重新发布。`cargo package -p glory` /
`-p glory-routing` / `-p glory-serverfn` / `-p glory-cli` 均失败（registry 上的
`glory-macros 0.3.1` / `glory-hot-reload 0.3.1` 不含当前源码使用的 API）。

未提交的 `release.yml` 改动把 matrix 并行改成了单 job 内按依赖顺序串行发布 + 等待 index 可见，
方向正确，解决了"下游 package 时上游还没进 index"的问题。但需要先做 version bump。

`Cargo.lock` 中仍有 yanked 的 `spin 0.10.0`（同时存在 `0.9.8`）。

### 8.3 示例与 fixture

- 9 个 Salvo 示例固定 `salvo = "0.93"`，工作区是 `0.94`，其中 8 个实测编译失败（P0-9）。
- `examples/mobile-counter` 在目标平台编译失败（P0-12）。
- `examples/router-basic/src/app.rs:76` 用 `Router::with_path("/")`，会触发
  `filters/path.rs:866` 的 warn；零 wisp 的 `PathFilter` 对任何路径返回 true 且不消费，
  若带子路由会变成隐式前缀通配。
- `examples/liveview-salvo` 依赖整个 `glory-desktop` crate 只为取一个 JS 常量
  `WRY_INTERPRETER_JS`，liveview crate 未 re-export 解释器。
- `examples/_test-size`、`project`、`workspace` 是 CLI 测试 fixture 而非真正示例
  （已在项目记忆中记录：不要给它们加依赖或 bin）。

### 8.4 文档漂移

除各章节已列出的外：

- `AGENTS.md:25` 把 hot-reload 描述为"AST-diff types for future hot reload"，
  但 AST diff / `ViewMacros` 已删除（`hot-reload/README.md:34-40` 自述，grep 无命中）。
- `AGENTS.md:53-56` / `docs/api-guide.md:31` / `lib.rs:27-30` 的 `.get()` 订阅描述与实现相反（§5）。
- `crates/cli/src/config/cli.rs:39` 的 `--hot-reload` 帮助文本"Requires rust nightly [beta]"，
  当前实现基于 syn 解析，稳定版可用。
- `crates/desktop/src/lib.rs:10-11` 称 `runtime` feature "on by default"，
  而 `Cargo.toml:33` 是 `default = []`。
- `crates/native/src/lib.rs:24-27` 称 URL 解析辅助函数"compile without any Blitz feature"，
  但 `mod image_provider` 被 `#[cfg(feature = "blitz")]` 门控。
- `crates/serverfn/src/lib.rs:11-13` 与 `docs/serverfn-extractor-decision.md:18-19` 称
  `handle` / `handle_with_method` 是"single entry point"，实际是 `dispatch_with_method`。
- `crates/core/src/web/attr.rs:11-14` 的 `AttrValue` 文档指向"glory_macro view macro"，
  而项目明确不提供 `view!`（Leptos 残留）。
- `crates/core/src/web/events/csr.rs:71-72` 文档引用不存在的 `crate::HtmlElement::on` / `crate::ev`。
- `crates/cli/src/command/config_cmd.rs:52-81` 的 `config --schema` 缺
  `site_https` / `tls_cert` / `tls_key` / `proxy`，schema URL `glory.rs/schemas/...` 不存在。
- `crates/core/src/web/utils/ssr.rs:14` 只要 `GLORY_WATCH` 存在（任意值）就注入热重载脚本，
  而 `crates/desktop/src/runtime.rs:948` 要求 `== "ON"`，语义不一致。

### 8.5 仓库卫生

- 根目录的 `debug.log`（2022）与 `dump.rdb`（2023，Redis dump）是本地遗留文件，
  已在 `.gitignore` 中且未被 tracked，**无需处理，但可以删除**。
- 10 个 `_*.md` 报告文件被提交进仓库（`_report.md`、`_todos.md`、`_improve*.md`、`_m*_tasks.md`、
  `_current_report.md`）。它们是 AI 协作的工作产物，与 `AGENTS.md` 一起构成上下文，保留合理；
  但其中多处 `[x]` 标记与实际可用性脱节（本报告 §4、§5），建议按本报告结果回归核验后再更新任务板。
- 未发现任何提交的凭据 / token / 私钥。
- `#[ignore]` 的测试只有一个：`crates/native/tests/screenshot.rs:107`（需要 wgpu adapter），理由充分。

---

## 9. 建议修复顺序

**第一批（正确性根因，改动集中）**

1. P0-1 布尔属性编码（`prop.rs` + `command.rs` + 两个消费端），一次修复三个平台。
2. P0-3 用 RAII guard 复位 `BATCHING` / `RUNNING` / `is_building` / tracking layer / streaming 线程局部。
3. P0-7 + P0-8 `patch` 开头检查 `is_attached`；`detach_child` 对未 attach 的 view 也走完整清理。
   这两条是 `Each` / `Switch` / `Suspense` 一系列重复渲染与僵尸 view 的共同根因。
4. P0-9 把 9 个示例的 salvo 升到 0.94；P0-12 补 `IpcMessage` 的 `GloryWryEval` 臂
   （examples 与 CLI 模板各一处）。
5. P0-13 `Interrupt::send_all_changed` 改用非 tokio 锁；P0-14 assets 瞬态错误降级为 warn。

**第二批（安全）**

6. §2.1 + §2.2：mount 层统一做 Origin / Content-Type 校验与可配置 body limit，超限 413。
7. §2.3 CSR / SSR 统一用 `percent_encoding`；§2.4 去掉 catch-all 的二次解码。
8. §2.5 `with_redirects` 只对 `redirect()` 命中改写，保留 query/fragment，unit not_found 不改写。
9. §2.14 把可被外部输入触发的 `unwrap` / `unwrap_throw` 全部改为 `Result` 或安全回退。
10. §2.9 公证密码改用 keychain profile 或 stdin；`run_checked` 打印前脱敏。
11. §2.10 `site_root` / `bin_target_dir` 规范化后必须位于 workspace 内。

**第三批（文本模型，影响面大但改法明确）**

12. P0-5 `IntoFiller` 改走 `.text()`；P0-6 `<style>` / `<script>` 原样输出 +
    `</script` 改写（同步更新 `snapshot_tests.rs:284`）；§3.2 引入独立文本节点原语。

**第四批（把标记完成的功能真正接线，或如实下调状态）**

13. §4 表格逐项处理：要么接线并补端到端测试，要么从 `_todos.md` 撤下 `[x]` 并在文档标注为"未接入"。
    优先级建议：`DesktopCloseBehavior` 导出（一行）、`LoadState` 导出（一行）、
    build_error overlay、`OutboundBuffer`、ISR 缓存、`#[server(stream)]` 派发。
14. P0-15 LiveView 改动在合并前补心跳、客户端树 reset、session 上限三项。
15. P0-11 修 `glory new` 三个模板 + `cli-harnesses` fixture，并让 harness 真正调用
    `Config::load` + 命令拼装。

**第五批（门禁）**

16. CI 补齐 `glory-routing` / `glory-liveview` / `glory-hot-reload` / `glory-desktop` /
    `glory --features routing` 的测试；`cargo check-all-features` 扩展到 serverfn / liveview /
    desktop / native，并为它们声明 `skip_feature_sets`。
17. `crates/core/Cargo.toml` 的 `skip_feature_sets` 补 `["backend-command","single-app"]`。
18. 给 examples 加一个 `cargo check` 的 CI job（它们被 workspace exclude，当前完全没有编译门禁）。
19. mobile smoke 脚本改用 `--target aarch64-linux-android` 交叉检查，消除假阳性。
20. 完成 version bump 与按依赖顺序的发布验证（`_report.md` P0 清单）。

---

## 附：本次评审未覆盖

- `benchmarks/` 目录（未纳入 workspace，未审查）。
- `tests/playwright/` 的测试内容（只核对了 CI 是否调用，未评审断言质量）。
- 运行态验证：浏览器 e2e、真机移动端、GPU 截图、桌面真窗口 smoke 均未实跑，
  相关结论基于代码阅读，已在各条目标注"待确认"处说明。
