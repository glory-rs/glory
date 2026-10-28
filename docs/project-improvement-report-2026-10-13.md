# Glory 项目完成度与改进报告

历史评估：本文的旧 Core/routing 源码链接对应下方记录的基线提交；这些文件
已于 2026-10-26 清理，可从 Git 历史查阅。当前状态见
[语义 UI 实施记录](semantic-ui-implementation.md)。

Glory 已形成可用于开发应用的 Web 主干，并有较完整的多后端架构。当前适合定位为 **Web beta 候选框架，桌面和 LiveView 为实验能力，原生和移动端继续验证**。功能广度已明显超过早期原型，但仍有状态回收、流式 SSR、发布门禁和平台运行验证方面的缺口，不宜宣称全平台稳定可用。

近期最有价值的工作是补齐运行正确性和发布闭环。尤其是本次复现的两项问题：holder 销毁后部分框架状态不释放；流式 SSR 在无 runtime 或单线程 Tokio runtime 下 panic。这些问题应先于新增平台和 API 处理。

## 评估范围与判断方法

- 评估日期：2026 年 10 月 13 日。
- 仓库：`D:\Works\glory-rs\glory`，分支 `main`，提交 `d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb`。
- 基线：15 个 workspace crate，workspace 版本 `0.3.1`；Windows MSVC，Rust/Cargo `1.98.0`。
- 核验覆盖核心响应式与生命周期、CSR/SSR、路由、serverfn、三个 SSR 适配器、CLI、热重载、渲染协议、LiveView、平台代码、CI 和发布配置。
- 完成度分为三个维度：**功能已实现、目标环境已验证、发布流程可复现**。有实现或单测不代表三者均已完成；暂缓的设计决策也不计作功能实现。

以下“复现确认”指在本机执行并观察到结果；“代码确认”指可直接从当前实现或配置确定；“待验证”指需要相应浏览器、设备、平台或发布环境的结果。没有用历史任务勾选率换算整体百分比，也没有将多次 feature 测试中重复运行的用例累计为独立覆盖率。

## 各领域完成度

| 领域 | 已形成的能力 | 验证程度与主要缺口 | 当前判断 |
| --- | --- | --- | --- |
| 响应式和组件 | Copy Cage、Bond 依赖跟踪与相等门控、Owner、batch、Store derive、Effect、resource、Each/Switch/Loader/Suspense | 核心测试通过；部分内部 Cage 未纳入回收，异步任务生命周期不完整，字段 lens 仍订阅根状态 | 主干较完整，生命周期阻塞稳定发布 |
| Web CSR | builder API、事件绑定、静态节点优化、细粒度更新、hydration 路径 | CSR + routing + server-fn 的 wasm 编译通过；浏览器行为测试的 CI 接线和深度需加强 | beta 候选 |
| SSR 和全栈 | 命令重放生成 HTML、资源 hydration、chunk/placeholder、serverfn 多种传输、缓存、SSG、三个框架适配器 | SSR 和适配器测试通过；流式 runtime panic 已复现，首字节提前输出尚未实现 | 普通 SSR 主干较完整，流式能力部分完成 |
| 路由 | ranking、类型化路由、query、redirect、Outlet、Memory/Browser/Server aviator、滚动记忆 | 单元和公开 API 集成测试通过；浏览器 history/scroll 深度验证与异步加载路由仍有缺口 | beta 候选 |
| CLI 和资产 | build/serve/watch/bundle/new/doctor、移动模板、平台打包路径、资产处理与体积分析 | 86 个 CLI 单测及 Clippy 通过；发布版安装、签名、公证、安装包运行仍需目标平台证据 | 开发链较完整，交付链待收敛 |
| 热重载 | function registry、替换批次、状态快照与恢复 hooks、错误 overlay | Rust 单测通过；现有浏览器用例只检查 reload 脚本注入，不能证明实际替换和状态保留 | 已实现，需要真实编辑闭环 |
| Desktop | wry/tao host、IPC、eval/query、多窗口、菜单托盘、热键、自定义资产协议 | 默认配置的 3 个测试通过；本次未运行 runtime 真窗口和安装包 | 实验可用性继续验证 |
| Native | Blitz 命令消费者、shell、布局查询、事件桥、图片 provider、AccessKit 接线、GPU 截图测试 | 已有像素渲染与 CI 截图任务；本次未跑 shell/GPU；当前截图断言范围有限，上游依赖仍含 alpha/beta 版本 | 实验阶段 |
| LiveView | 三个 WS 适配器、worker pool、会话恢复、随机 token、Origin 检查、heartbeat、客户端重连队列 | 19 个测试通过；容量并发、后台清理、消息限制一致性、队列字节预算、身份绑定仍需治理 | 实验阶段，公开部署前需加固 |
| Mobile | Android/iOS host 模板、build/bundle、webview 生命周期、设备 smoke 脚本与 CI | 工具链路径存在；iOS smoke 可吞掉失败，已有验证记录停留在较早日期 | 模板和工具链阶段 |
| TUI | CommandDom 的 ratatui 只读树显示 | crate 自述为 demo/debugging aid，未承诺完整交互后端 | 调试工具，维持现有范围 |
| 工程和发布 | fmt、Clippy、定向测试、浏览器 e2e、多平台 CI、MSRV/semver job、依赖顺序发布 | 本机关键检查通过；docs.rs/feature 配置冲突、发布与质量检查脱节、MSRV 继承和证据归档有缺口 | 有门禁框架，尚未闭环 |

### 历史评估中需要更新的判断

[_todos.md](../_todos.md) 的汇总表仍列出“缺全局状态、缺 Store derive、缺路由排序、原生不渲染像素”等，但后文和代码已经实现相关能力。其“59 项全部完成并勾选”同时包含 12 项 `[~]`，混合了已完成、部分完成和暂缓决策，不能作为产品完成度指标。

[_report.md](../_report.md) 的 7 月结论也需按当前代码修正：第一方 LiveView 适配器已接入恢复会话，token 已改为 OS 随机数，客户端已有 heartbeat；serverfn 已有跨站请求检查和统一 body 限制；feature CI 已覆盖 serverfn 和 LiveView。剩余问题应具体记录为容量、身份、清理和运行验证，而不是继续把这些能力列为“尚未实现”。

## 发布前优先处理的问题

优先级定义：P0 阻塞对应发布面的可靠性；P1 是下一里程碑的质量要求；P2 是中期产品改进。LiveView 的公开服务要求与 Web 核心发布要求分别验收，避免实验平台无限拖延核心发布。

### R1 P0 框架创建的状态未完整回收

**复现确认。** 用带 `Drop` 计数的 payload 构建 `ServerHolder`，销毁 holder 后观察到：

```text
scope-owned: payload_drop_count=1
app-context: payload_drop_count=0
resource: payload_drop_count=0
```

对照组使用 `ctx.cage(...)`，其值按预期释放。`ctx.provide_cage(...)` 和 `resource_in(...)` 的值没有释放。当前 `Cage` 为 Copy handle，底层 slot 由 `Box::leak` 分配，必须通过 Owner invalidation 才能释放值并进入复用池；丢弃 Truck 中的 handle 不会释放其 payload。

代码位置：[scope.rs](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/core/src/scope.rs) 第 340–346 行；[effect.rs](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/core/src/reflow/effect.rs) 第 140、190 行；[cage.rs](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/core/src/reflow/cage.rs) 第 361–389 行。`resource_hydratable_in`、Loader 和 Suspense 也存在直接创建 Cage 的路径，应一并检查其所有权，不能从两项复现推定其他路径已验证。

**影响：** SSR 请求重复创建 app 状态和 resource 时，值可能一直保留；CSR 长会话反复挂载组件也有累积风险。free-list 已修复“失效后不复用”的问题，但无法回收从未 invalidated 的值。

**改进：** app 状态交给 app/holder 生命周期的 Owner；组件资源交给 scope 或明确的任务所有者。保留现有 Cage/Bond/Vary API，给直接创建 Cage 的用户说明 Owner 使用方式。处理所有权时同时处理在途任务，避免回收后任务继续 `revise` stale handle。

**验收：** holder drop、组件 detach、替换同类型 app 状态后 payload 正确析构；重复挂载和销毁后 slot 复用数趋于稳定；stale task 完成不 panic、不影响新组件。

### R2 P0 流式 SSR 不支持已宣称的 runtime 范围

**复现确认。** `ServerHolder::new_streaming(...).mount(带 resource 的 widget).render_string()` 的结果：

| 调用环境 | 本次结果 |
| --- | --- |
| 无 Tokio runtime | panic，`there is no reactor running` |
| Tokio current-thread | panic，`can call blocking only when running on the multi-threaded runtime` |
| Tokio multi-thread | 成功 |

根因在 [spawn.rs](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/core/src/spawn.rs) 第 126–138 行：`run_deferred_batch` 的生产分支直接调用 `block_in_place` 和 `Handle::current()`。普通 SSR 已按 runtime 类型分支处理，streaming 分支没有同步处理。

现有 [ssr_runtime_flavors.rs](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/core/tests/ssr_runtime_flavors.rs) 的 3 项测试本次通过，但它们调用普通 `ServerHolder::new`。流式 snapshot 单测则走 `cfg(test)` 下的 LocalPool 分支，因而没有覆盖消费者实际调用的生产分支。

**改进：** 统一普通与流式路径的 executor 策略；用公开 API 集成测试覆盖三种 runtime，测试中引入异步 resource。同步 fallback 要区分立即可完成的 Future 和依赖 Tokio timer/I/O 的 Future，不能仅用字符串常量验证全部异步行为。

**验收：** 三种环境在明确支持范围内成功渲染；不支持的异步调用方式有明确契约，避免运行时突然 panic；包含延迟、失败和嵌套 Suspense 的用例。

### R3 P0 文档构建与 feature 矩阵配置冲突

**复现确认与代码确认。** [routing manifest](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/routing/Cargo.toml) 第 15–16 行仍是 `docs.rs all-features = true`，但 core 明确拒绝 `web-csr + web-ssr`。本次 `cargo check -p glory-routing --all-features --offline` 退出码为 101，命中互斥错误，说明文档配置指定了无法编译的组合。正式 docs.rs 构建仍需使用实际 rustdoc 命令验收。

[glory manifest](../crates/glory/Cargo.toml) 已有显式 docs.rs feature 集合，但它的 `cargo-all-features` 元数据被注释。[CI](../.github/workflows/ci.yml) 第 277 行却将该包纳入组合检查。依赖 core 的 skip 配置不能替代对 facade 自身配置的核查，本次直接构建 facade 的互斥组合同样退出 101；组合工具的完整执行结果仍需确认。

**改进：** routing docs.rs 使用明确、可编译的 SSR feature 集合；为 facade 声明完整互斥排除规则；明确区分“合法组合应成功”和“非法组合应因指定 guard 失败”。对 desktop/native 的重 feature 按目标平台建立可控矩阵。

**验收：** 按每个 crate 的 docs.rs 元数据执行文档构建；完整组合检查通过；负向 guard 用例只接受预期错误。

### R4 P0 发布流程没有依赖同一提交的质量结果

**代码确认。** [release.yml](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/.github/workflows/release.yml) 中 publish 仅 `needs: git-tag`。tag 在 package/dry-run 之前创建；[ci.yml](../.github/workflows/ci.yml) 和 format workflow 又排除 `release` 分支。当前发布工作流本身没有 fmt、定向测试、feature 和浏览器质量任务，因此不能保证待发布提交已通过完整门禁。

依赖顺序发布、package、dry-run 和等待 registry 可见已经存在，应保留。风险在于质量结果未绑定发布提交，以及上游发布成功、下游失败时缺乏完整的交付状态。

**改进：** 为发布接入可复用质量 workflow，对同一 SHA 验证后再打 tag 和发布；先准备发布 manifest，明确版本、crate 顺序和实验平台范围；保留逐包验证和断点续发，记录每包状态、package 产物和校验和。下一版本是否已占用，应在发版时核对 registry，不能沿用历史报告当作实时结果。

**验收：** 人为引入测试失败时无法打正式 tag/发布；全新环境可从发布包安装 CLI 并构建旗舰示例；部分发布失败可定位、恢复且有准确记录。

## 下一里程碑的质量改进

### R5 P1 实现真正提前输出的 SSR 流

**代码确认。** [server.rs](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/core/src/web/holders/server.rs) 第 145–214 行先构造 `Vec<HtmlChunk>`，其中 `drive_deferred()` 等资源完成，之后才返回 `stream::iter`。Axum/Actix 的适配入口还先收集 chunks 再交给响应 body。当前可以产生 placeholder/patch 和分块 body，但慢资源仍推迟响应交付，不能兑现资源未完成时客户端已收到 shell 的效果。

**改进：** 在专用本地 executor 驱动 `!Send` widget tree，通过有界 channel 向响应 body 交出 shell 和随后完成的 patch；加入客户端断连取消、deadline 和错误传播。优先完成一个适配器的网络契约，再复用到其他适配器。

**验收：** 延迟资源未完成时 HTTP 客户端已收到首个 shell 字节；快慢资源分别完成并输出 patch；断连释放 holder 和任务；测试经真实 HTTP body 验证，不能仅检查最终 HTML 或 chunk 数量。对应历史任务 FS4。

### R6 P1 完成 LiveView 的资源边界和适配器一致性

以下为**代码确认**，本次未进行并发压力或网络攻击复现：

| 问题 | 当前证据 | 改进与验收 |
| --- | --- | --- |
| session 上限检查存在并发窗口 | `SessionManager::connect` 在第 518–525 行检查容量、释放锁、异步 spawn，再插入 | 先原子预留容量，失败释放；并发建立 session 时总数始终不超过配置 |
| 过期清理依赖活动 | `reap` 从 connect/touch 路径触发，没有周期后台 reaper | 无新连接、无消息时也能按 TTL 释放断线 session；记录 active/idle/expired 指标 |
| 消息限制未统一接线 | 配置有 `max_message_bytes`；Salvo 接入 `max_message_size`，Axum/Actix 当前入口未接入同一配置 | 三适配器一致限制完整消息，包含 hello、文本与分片；超限行为明确 |
| patch 配置没有形成完整出站治理 | `max_pending_patches` 仅用于 OutboundBuffer helper；适配器循环未使用它；helper 超限时 `tail.extend(batch)` | 限制 pending bytes/commands 和慢消费者处理时长；不能仅限制批次数后任由单批增大 |
| 恢复凭据与身份关系未定义 | 随机 token 是 bearer credential；manager 按 token 恢复，没有用户身份绑定参数；客户端默认 token 留在内存 | 提供宿主 auth/identity hook，明确 token 轮换、注销、跨刷新和部署实例策略；错误身份不得恢复 |

主要代码：[LiveView runtime](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/crates/liveview/src/lib.rs) 第 149–176、491–561、951–977、1188 行；[客户端](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/crates/liveview/src/liveview_client.js)。已有随机 token、Origin 校验、heartbeat 和 session Close 清理继续保留。这些改进是 LiveView 公开服务的准入条件，核心 Web 发版可先将其标为 experimental。

### R7 P1 补上能发现运行问题的浏览器测试

**代码确认。** [wasm_behavior.rs](https://github.com/glory-rs/glory/blob/d2f92dfa13745377dc2d579d2ac95d9a1b7cc1cb/crates/core/tests/wasm_behavior.rs) 和 wasm CSR smoke 已存在，但当前 CI workflow 未显式运行这两套 wasm-bindgen 测试。普通 Rust 测试和 Playwright 示例 smoke 不会自动执行它们。其注释“在 CI 执行”与实际接线不一致。

当前 [hot-reload spec](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/tests/playwright/tests/hot-reload.spec.js) 仅断言脚本包含 `new WebSocket` 和 `/live_reload`；[LiveView spec](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/tests/playwright/tests/liveview.spec.js) 仅覆盖点击一次后的变化；routing/hydration 用例主要覆盖普通导航。

**改进：** 接入版本匹配的 wasm-bindgen runner，显式执行两套测试并断言用例数量非零；热重载测试实际修改样例源文件并验证更新和状态恢复；LiveView 覆盖断线、恢复、过期、新 session 重建；路由覆盖 back/forward、scroll、query 和 hydration mismatch。CI 所需 URL 缺失时应失败，避免将 skip 视作覆盖。

**验收：** 测试能够捕获真实 DOM、卸载和重连回归；失败时归档 trace、截图和服务端日志。Playwright 依赖增加锁文件并使用确定性安装；目前 CI 明确 `--package-lock=false`，且仅在输出中打印服务器日志。

### R8 P1 将平台冒烟从脚本存在提升为真实结果

**代码确认。** [mobile workflow](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/.github/workflows/mobile-device-smoke.yml) 第 106 行在 iOS smoke 命令末尾使用 `|| true`；启动模拟器也可能忽略失败。即使安装或启动失败，job 仍可能成功。现有 [mobile validation](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/docs/mobile-validation.md) 保存的是 6 月的本机记录，不能说明当前版本设备验证完成。

**改进：** 让 iOS smoke 的失败退出码或结果 JSON 控制 job 结果；检查正确模拟器已启动、`.app` 存在、bundle id 匹配。Android/iOS 继续增加计数器点击、前后台恢复、键盘输入和资源加载验证。Desktop 测真窗口、eval/query、资产协议和安装包；Native 测输入/焦点/IME、图片和卸载。

已有 Native GPU 截图任务值得保留，但当前 [screenshot.rs](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/crates/native/tests/screenshot.rs) 主要验证 buffer 非空和中心像素蓝色占优；增加多场景截图或布局断言，才能发现布局、文字和交互回归。

**验收：** 平台结果关联具体 SHA、工具链、设备/模拟器与产物；blocked 和 failed 不能包装成 validated。Mobile 的设备结果是其平台发版准入条件，非核心 Web 的强制阻塞。

### R9 P1 修复 MSRV 声明与 CI 验证范围

**代码确认。** workspace 声明 Rust `1.94`，但 `cargo metadata --no-deps` 显示仅 CLI 和 CLI harnesses 有 `rust_version=1.94`，其余 13 个 crate 为空；各 crate 不会自动继承 `[workspace.package]` 的该字段。MSRV job 当前只验证 core，未覆盖 SSR、routing、serverfn、CLI 等实际发布组合。

**改进：** 各发布 crate 声明 `rust-version.workspace = true`，或有意声明单独的最低版本并记录原因；直接用所支持的最低工具链检查核心发布 feature 集合，固定依赖解析策略。当前本机 Rust 1.98 通过不能代替 MSRV 证明。

**验收：** metadata 与承诺一致；最低版本对声明支持的目标通过；依赖升级导致最低版本提高时更新版本策略和迁移说明。

### R10 P1 收敛文档与公开承诺

**代码确认。** [CHANGELOG](../CHANGELOG.md) 第 15–16 行宣称 `GlobalCage`、`GlobalBond`、`global_cage!`、`global_bond!`，当前源码未找到这些 API；实际 app-wide 状态接口是 Scope 的 `provide_cage/use_cage/cage_context`。AGENTS.md 还同时保留“routing/CLI 旧测试坏”和“定向测试应通过”，以及 wasm 测试仍在 backlog 的说法。

**改进：** 在 README 建立统一 maturity matrix；更新 changelog、AGENTS、任务摘要和平台计划。任务状态区分 implemented、validated、release-ready、deferred；每个 completed 条目关联验收命令或设备结果。Scope 全局状态说明中的“build 内读取即可订阅”也需与实际跟踪规则同步。

**验收：** 用户照着新手路径能从安装到运行；公开 API 名称与源码一致；暂缓项仍保留其未实现状态；指南不会继续推荐已经过时的 feature 或测试规则。

## 中期产品改进

| 优先级 | 方向 | 具体改进 | 验收标准 |
| --- | --- | --- | --- |
| P2 | 状态粒度与异步 DX | CageLens 当前穿过根 Cage 读取和修改；按真实瓶颈设计字段失效语义；hooks 增加取消与显式 cleanup。`use_coroutine_in` 当前忽略 parent，仅依赖所有 sender 丢弃退出 | 无关字段更新不触发所声明的观察者；组件 detach 后仍持有外部 handle 时，取消行为符合契约 |
| P2 | renderer 协议 | 现有 wire/参考 DOM 测试扩为共享 conformance fixtures，在 Rust 参考解释器、desktop JS、LiveView 和 Native 消费者间运行 | 顺序、属性/property、事件、query、节点删除和错误输入语义一致；协议版本变动有兼容规则 |
| P2 | 性能与资产 | 把 [performance.md](performance.md) 的历史基线更新为按 SHA 归档的多样本结果；记录最终 bindgen/wasm-opt/gzip/Brotli 体积；推进 CSS 指纹、source map/manifest、按需字体处理 | 有可复现的 payload 和性能趋势；冷构建、更新、SSR 内存与长会话均可比较；无未经当前测量的性能承诺 |
| P2 | 安全契约 | 在已存在 escaping、跨站检查、body limit、随机 token 上补威胁模型与对应测试；明确 raw HTML、CSP nonce、auth hook、desktop eval/custom protocol 和资源根目录边界 | 文档定义信任边界，回归测试覆盖真实入口；不把安全责任隐含在未接线的 helper 中 |
| P2 | 示例与交付体验 | 选定 counter、router、SSR hydration、fullstack/upload 为核心旗舰示例；平台示例独立验收；验证从发布版 CLI 到 bundle/serve 的完整流程 | 全新环境可复现，README 给出 feature 分 target 构建方式，发行产物能安装和启动 |

Bond 改 Copy、wasm split、异步懒加载路由、二进制热补丁和原生权限桥继续按已有设计决策推进。它们有价值，但不应优先于已复现的生命周期问题，也不必为了对标其他框架引入 RSX/JSX DSL 或替换现有响应式原语。

## 建议实施顺序与发布准入

| 批次 | 范围 | 交付条件 |
| --- | --- | --- |
| 第一批 | R1 所有权回收、R2 流式 runtime、R3 docs.rs/feature 配置 | 新增针对性回归，关键包检查保持通过，消费端公开 API 复现修复 |
| 第二批 | R4 发布门禁、R7 浏览器覆盖、R9 MSRV、R10 文档 | 同一 SHA 有质量记录，最低版本和浏览器测试真实执行，发布说明与 API 一致 |
| 第三批 | R5 网络流式输出、R6 LiveView 治理 | 延迟/断连/并发/慢消费者测试通过；分别定义 Web streaming 与 LiveView 的准入条件 |
| 第四批 | R8 平台验证和 P2 产品改进 | 目标环境产物和交互证据齐全；按平台独立升级成熟度 |

Web 核心可以先以明确范围的 beta 发布：core、glory、routing、serverfn、macros、hot-reload、CLI 及选择发布的 SSR adapters。前提是修复其 P0，完成版本与 registry tarball 验证、旗舰示例、浏览器和最低版本检查。若真正流式输出仍未完成，应清楚标注 buffered chunk 行为。

Desktop、Native、LiveView、Mobile 应拥有独立准入清单。全平台稳定版本至少需要安装/设备/窗口验证、协议一致性、长期资源释放、恢复和故障行为证据，不能用 workspace 默认测试替代。

## 本次验证记录

### 已通过的检查

| 命令 | 本次结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过 |
| `cargo test -p glory-core --lib` | 74 项通过 |
| `cargo test -p glory-core --lib --features web-ssr` | 116 项通过 |
| `cargo test -p glory-core --features backend-command` | 74 项单元、8 项 command 集成和 3 项 doc 测试通过；12 项 doc 忽略 |
| `cargo check -p glory-core --features "web-ssr backend-command"` | 通过 |
| `cargo test -p glory-routing` | 69 项单元、15 项集成和 2 项 doc 测试通过；1 项 doc 忽略 |
| `cargo test -p glory --features routing` | 12 项测试通过 |
| `cargo test -p glory-serverfn` | 46 项集成测试通过；1 项 doc 忽略 |
| `cargo test -p glory-serverfn --features "salvo axum actix"` | 47 项集成测试通过；1 项 doc 忽略 |
| `cargo test -p glory-liveview --features "salvo axum actix"` | 19 项通过 |
| `cargo test -p glory-cli --lib --no-default-features` | 86 项通过 |
| `cargo check -p glory --target wasm32-unknown-unknown --features "web-csr routing server-fn"` | 通过，属于编译验证 |
| `cargo test -p glory-core --features web-ssr --test ssr_runtime_flavors` | 普通 SSR 三种 runtime 均通过 |
| `cargo test -p glory-actix -p glory-axum -p glory-salvo` | 三适配器共 11 项集成测试通过 |
| `cargo test -p glory-desktop -p glory-hot-reload` | desktop 3 项、hot-reload 18 项及其 4 项 doc 测试通过；desktop 未启用 runtime |
| `cargo clippy -p glory-core --lib --features web-ssr -- -D warnings` | 通过 |
| `cargo clippy -p glory-serverfn --all-targets -- -D warnings` | 通过 |
| `cargo clippy -p glory-cli --lib --no-default-features -- -D warnings` | 通过 |

### 已复现的问题及验证边界

- 通过独立消费端 probe 确认 R1 的 payload 不释放，以及 R2 在两种 runtime 下 panic。probe 使用真实库依赖，未启用库的 `cfg(test)`，未修改生产源码。
- `cargo check -p glory-routing --all-features --offline` 失败，确认 R3 的 docs.rs feature 配置冲突。
- `cargo check -p glory --features "web-csr web-ssr" --offline` 按预期被 guard 拒绝；这是正确的约束，需修正组合检查配置而非取消 guard。
- 本次未运行完整 cargo-all-features、Playwright/wasm 浏览器 runner、desktop runtime 真窗口、Native shell/GPU、Android/iOS 设备，以及 registry package/publish dry-run；相应结论保留为配置核验或待验证，不能据此声明平台或发行包已通过。
- 本次未读取云端 Actions 运行结果；workflow 存在与最近一次运行成功是两个不同条件。

本机原始日志、命令结果 JSON、复现源码位于 `target/assessment-2026-10-13/`，属于可清理构建产物，不进入版本控制。主要复现可用以下命令重跑，前提是该目录仍保留：

```powershell
cargo run --manifest-path target/assessment-2026-10-13/probe/Cargo.toml --target-dir target --offline
```

报告描述的是上述提交与本机验证范围。最值得投入的下一步是修复 R1/R2、接通可信发布门禁，再按平台独立验证；已有架构和功能为这些收敛工作提供了足够基础。
