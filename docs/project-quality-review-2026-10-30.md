# Glory 项目质量评审与改进报告

评审日期：2026 年 10 月 30 日。基线：`cd750c8c4f159119fbf5fede688166b42ad95073`，版本 `0.6.0`。范围为 Glory 的 14 个活动 workspace 包、独立 `tools/cli`、相关测试、发布流程和性能证据；相邻框架仓库不作为 Glory 自身的质量结论。

**修复结果：安全、routing 和确定性性能问题已修复，本机完整主门禁、31 套 Chromium 验收和真实流式传输检查全部通过。** 单选通知降低了执行量，同时增加节点和 WASM 体积；混合显示仍有线性 readiness 检查和单文档 64 App 上限。Linux CI 和 Apple 手动门禁已配置，本机结果不能代替这些平台的实际运行。

首轮使用三个并行评审 agent 分别审查安全、性能与生命周期、正确性；主评审检查工程质量并执行独立复现。随后按用户授权并行修复，修改运行时、测试、CI 和文档。下文原始发现描述的是上述基线，当前状态以本节为准。

## 修复状态

| 项目 | 当前实现与验证 | 保留边界 |
| --- | --- | --- |
| F01 P1 Link URL | 共享语义层校验相对 URL 与 http/https/mailto/tel；Create、prepared Target patch、HTML、SSR、stream 回归通过 | 自定义协议通过显式应用回调处理；URL 空白需编码；这是有意收紧的行为 |
| F02 异步链接 | 新增 `link_with`，显式使用 Owner 的 Host；CSR/SSR 真实点击、Pending、Block、Allow、supersede 和退休取消通过 | 原 `link` 保持同步 helper；无 Host 时返回错误，没有隐式执行器 |
| F03 schema | 完整节点声明采用长度分帧编码；Bootstrap 升级为 `/2`；父子消费边界变更拒绝通过 | 有意拒绝 v1；应用仍负责 codec schema 版本，校验不是认证机制 |
| F04 小批追加 | 去掉两次全索引复制，按新增 key 验证和原地追加；旧源码与修复版确定计数对照通过 | 重复 key 拒绝、成员身份与原有用户代码 panic 边界保留 |
| F05 批量取消 | membership 立即取消，FIFO 有界惰性压缩；FIFO、stale waker、锁外 drop 和检查量回归通过 | 数字条目按界限压缩，未宣称零存储或 OS I/O 可立即中断 |
| F06 匹配预算 | 新增共享段候选索引，保留原捕获/优先级/声明顺序；4096 路由、8 步预算回归与 781 种路径差分通过 | 重叠动态路径仍受预算保护 |
| F07 Widget 岛 | 有序 live 表、只泵 ready 岛、最多 8 轮 presentation 补充提交；1/32/64 岛真实 Chrome 验收通过 | 仍 O(W) readiness 检查；第 65 个 App typed 拒绝，未扩大预算 |
| F08 选择通知 | 新增 Owner 管理的 `Selector<K>` 和非拥有 `Selected<K>`；立即读取、batch、提前读取、碰撞与退休回归通过；canonical model 已迁移 | 直接 Selected 读取即时，组合缓存 Bond/Binding 在 flush 后更新；每 live key/Owner 多一个通知源及 cleanup |
| F09 开发服务器 | 文件或诊断访问前验证单一 Host、实际 loopback 地址及端口；真实 TCP 回归通过 | HTTP/1.1 必需 Host；HTTP/1.0 可省略；未声称已完成真实 DNS rebinding 利用测试 |
| F10 依赖审计 | 11 份维护锁文件审计通过；新增 PR/release/定时门禁和对应 Dependabot 范围 | 4 项上游停止维护警告仍可见，冻结 peer 对比 fixture 不属于发布审计范围 |
| F11 兼容性 | Rust 1.94 workspace 全 feature 检查、CLI 检查、core/routing 测试通过；下游 DSL 诊断接入 Web gate，workflows 通过 actionlint | Linux CI 尚待远端执行；Apple 需准备好的 runner，设备/读屏与 Safari/Firefox 验收仍独立 |
| F12 文档 | README 引用实际常量，修正发布状态和旧 Apace 链接；API guide 补 selector、链接策略与异步路由；一致性检查通过 | 历史报告继续描述其原始提交，不将历史数字替换为新数字 |

### 本轮确定工作量与性能取舍

| 对比 | 基线 | 修复后 | 方法 |
| --- | --- | --- | --- |
| 4096 次单行追加 key Clone | 16,781,312 | 8,192 | 同一计数测试，独立旧源码实际执行 |
| 4096 ready 任务同序列取消的队列检查 | 7,456,995 | 5,329 | 旧 retain 实际 oracle 对照新压缩计数，不含 BTreeSet 和普通 pop 成本 |
| 4096 消费者选行执行数 | 4096 | 2 | 两个真实 graph 的确定性对照；新 graph marked=3 |
| canonical 万行 graph 节点 | 30,005 | 40,007 | 相同框架，只有 model 使用 selector 与否不同；Owner/实例/目标/物理树相同 |
| canonical 清空后 graph 节点 | 5 | 7 | 应用仍活着；根 Owner 退休另有完整释放验收 |
| release WASM | 417,312 B | 456,516 B | 同配置、共同冻结源码，仅 model 策略不同；增加 39,204 B 即 9.39% |
| gzip WASM | 147,226 B | 157,700 B | 同上；全部 runtime JS 均为 34,861 B |
| 选行 Script 中位 | 0.341 ms | 0.1315 ms | 每侧 24 个交错样本，两轮方向均改善 |
| 同次选行的两 RAF wall 中位 | 25.85 ms | 26.15 ms | 同上，wall 未改善且略回退 |

选行配对是在主 gate 并行运行时进行的未降速诊断，不能推断统计显著性、创建/更新/清空的整体耗时提升。配对框架源码相同，只有 model 策略不同，两侧均包含最终 template 同步屏障和 selector 提前读取修复；没有混用前几轮、原评审或历史报告的二进制。既有数据 oracle、万行布局、截图、GC、故障前缀和焦点/选区探针全部通过，未调整 probe 常量掩盖节点增长。

独立复审另复现了两项 selector 边界：未 flush 的临时选择期间注册通道会漏通知；缓存 Bond 首次读到中间值后，净零 batch 可能保留错误缓存。两项均保留修复前失败用例。最终通知初值与上次发布态对齐，并只追踪实际提前读取的 live channel Weak，flush 合并补发后清空，Owner 退休立即移除。常规选行只通知旧/新 key；提前读取过的中间 key 也会得到必要通知，因此不能把任意 batch 都描述为只通知两个消费者。10 项 selector 回归、全 core 121 项普通测试和 6 项文档测试通过。

F07 的 1/32/64 岛各 1400 次 raw 更新均没有泵 idle 岛。共享更新分别泵 1/32/64 岛，单岛程序更新泵 1 岛；最高 ID 岛事件更新共享 Cage 后，同一 `flush()` 返回前较低 ID 岛也完成提交。旧的一次性 snapshot 在该场景确实失败，已增加有限 presentation 补充提交并保留失败证据。真实编辑、dirty Bounds query、两轮卸载恢复、第 65 岛拒绝及 fault-close 清理通过；debug 微秒数字不作为 release 提速结论。

### 本轮验证证据

- [本机验证总览 JSON](quality-review-2026-10-30-validation.json) 汇总命令、退出结果、性能测量和保留边界，随本次提交保存。其余 D:/ 路径为本机原始日志，GitHub 页面无法直接访问这些本地文件。
- [`check-semantic-ui.ps1 -Web -Window -Bench -Audit` 完整通过](D:/BuildCache/glory-semantic-ui/quality-fix-gate-final.log)：活动 workspace、CLI、引擎来源及 fork 测试、全 feature 测试、严格原生/WASM Clippy、package-only 边界、实际窗口、独立 Windows UIA 客户端、33 项下游编译诊断和 virtual_list benchmark。前几轮失败日志保留；最终 native AX fixture 采用合法 `/next` 并核对同一导出 URL，未放宽安全策略。
- [Chromium 31 套全部通过](D:/BuildCache/glory-semantic-ui/quality-semantic-browser-final-20261030/evidence.json)，包含 SSR adoption、stream、输入、焦点、选区、资源、查询、布局与清理；[真实 TCP stream 检查日志](D:/BuildCache/glory-semantic-ui/quality-fix-browser.log) 覆盖帧到达顺序、断开取消及慢读 backpressure。浏览器使用的冻结 WASM、stream fixture 与最终主 gate 产物 SHA256 相同，测试后已关闭自建服务。
- [确定工作量对照](D:/BuildCache/glory-semantic-ui/performance-fixes-20261030.json)。
- [F08 最终浏览器与体积证据](D:/BuildCache/glory-semantic-ui/selector-browser-20261030/browser-evidence-read-ahead/evidence.json) 和 [配对输入审计](D:/BuildCache/glory-semantic-ui/selector-browser-20261030/pair-inputs-read-ahead.json)。此前 `browser-evidence-final` 目录对应较早修订，不能作为最终产物数据。
- [F07 最终 Chrome 结果](D:/BuildCache/glory-semantic-ui/island-readiness-site/island-readiness-evidence.json)；`failed-100.json` 和 `failed-shared-barrier.json` 保留在同目录。复跑：`scripts/build-island-readiness.ps1 -Verify`。
- [路由 6 组浏览器验收](D:/BuildCache/glory-semantic-ui/quality-routing-browser-2026-10-30.json)：含 CSR、SSR、晚 URL、fragment 和两种便携异步链接场景。
- [Rust 1.94 全 feature 检查](D:/BuildCache/glory-semantic-ui/quality-fix-msrv-final.log)、[core/routing 测试](D:/BuildCache/glory-semantic-ui/quality-fix-msrv-tests.log)、[独立 CLI 检查](D:/BuildCache/glory-semantic-ui/quality-fix-msrv-cli.log)。
- [11 份锁文件审计汇总](D:/BuildCache/glory-semantic-ui/quality-fix-audit/summary.json)。RustSec 数据库提交 `ee1a49a2769a24a596afc1c20cb33748d2769586`，更新于 2026 年 10 月 29 日；没有忽略 advisory。
- [actionlint 1.7.12](D:/BuildCache/glory-semantic-ui/quality-fix-actionlint.log) 验证 workflows；发布脚本 11 项 Python 测试、当前文档一致性检查和 `git diff --check` 通过。actionlint 为静态检查，Linux/Apple workflow 尚未在远端执行。

停止维护警告为 `ttf-parser 0.25.1`、`zbus_names 4.3.5`、`zvariant 5.15.1`、`zvariant_derive 5.15.1`。它们不是本次已知漏洞命中；需跟进字体与桌面总线依赖的上游迁移。门禁拒绝已知漏洞、unsound 和 yanked，完整保留 unmaintained 警告。

## 首轮评审结论与验证范围

| 方向 | 当前质量 | 优先改进 |
| --- | --- | --- |
| 安全 | 文本及 JSON 转义、有限模板、显式原生 I/O 能力较好 | 普通 Link 缺少 URL 协议策略；开发服务器 Host 防护待补 |
| 性能 | 稀疏显示、窗口列表、共享声明和已测优化有明确边界 | KeyedList 小批追加、执行器批量取消存在平方级工作量 |
| 正确性 | 代际身份、Owner 退休、查询屏障和失败前缀有系统测试 | routing 异步链接及 bootstrap schema 存在可复现缺口 |
| 鲁棒性 | I/O、解码、任务与虚拟行有独立预算 | 路由声明上限与匹配预算不协调；需边界规模测试 |
| 架构与可维护性 | core、render、html 和宿主边界清楚；可选依赖有检查 | 多个宿主和两种显示路径需要共享契约测试，降低行为漂移 |
| 测试与持续集成 | 原生、WASM、浏览器及引擎来源检查较丰富 | 最低 Rust 版本、独立编译诊断、非 Windows 平台持续门禁不足 |
| API 与文档 | Cage/Bond/Vary、Widget 与 Each 已统一 | 同步和异步导航入口不配套，部分入口文档仍描述旧状态 |
| 供应链与发布 | 引擎版本、校验和与提交来源固定，Actions 固定 SHA | 缺少显式依赖漏洞门禁，独立 workspace 更新覆盖不足 |

优先级：P1 为应优先修复的安全风险；P2 为影响特定功能、规模或持续保障的问题；P3 为文档和维护改进。下文分别标注独立复现、源码确认、优化机会和验收缺口，不将它们视为同等强度的证据。

本次在 Windows、Rust `1.98.0` 上完成：

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| `scripts/check-semantic-ui.ps1` | 通过；包括 CLI 测试及 native acceptance、引擎来源和 fork 测试、格式、workspace 全 feature 测试、package-only 测试、依赖边界、严格 Clippy | [主检查日志](D:/BuildCache/glory-semantic-ui/quality-review-2026-10-30-check.log) |
| workspace 全 feature、全 target 的 WASM 严格 Clippy | 通过 | [WASM 日志](D:/BuildCache/glory-semantic-ui/quality-review-2026-10-30-wasm.log) |
| 发布脚本 Python 单元测试 | 11 项通过 | `python -m unittest discover -s scripts -p test_release.py` |
| 独立消费者复现 | 下文 F01、F02、F03、F04、F06 均通过断言 | [复现代码](D:/BuildCache/glory-semantic-ui/quality-review-probe/src/main.rs)、[输出](D:/BuildCache/glory-semantic-ui/quality-review-2026-10-30-probe.log) |

本次没有重新运行 Chromium 行为验收、真实窗口与辅助技术客户端、Apple 或 Android 验收，也没有进行完整 RustSec 锁文件扫描。既有报告中的浏览器、设备及 benchmark 数据均按历史证据引用，不能替代本次执行结果。WASM Clippy 证明编译与 lint 通过，不证明浏览器行为。

## 已复现或由源码确认的问题

### F01 普通 Link 接受可执行 URL

**P1，独立复现。** `LinkProps.target` 可以是 `javascript:`；完整描述校验接受它，HTML 映射原样生成 `href`，动态 `Patch::Target` 也接受同类值。

依据：[描述校验](../crates/render/src/props/view.rs#L89)、[初始 href 映射](../crates/html/src/mapping.rs#L503)、[动态 Target](../crates/render/src/props/live.rs#L396)、[动态 href 映射](../crates/html/src/patch.rs#L761)。

最小复现的核心输入为：

```rust
Descriptor::new(Props::Link(LinkProps {
    target: "javascript:globalThis.__glory_link_probe=1".into(),
    label: "open".into(),
}))
```

当应用把用户或远端内容传入 target、用户激活链接，而且 CSP 没有拦截该执行方式时，形成条件性 XSS。SSR 在 WASM 接管前同样会保留危险 href。属性转义无法消除 URL 自身的执行语义。

**改进：** 在共享语义层定义 URL 策略，并同时用于 Create 与准备中的 Target patch。默认拒绝可执行协议和控制字符混淆，明确相对 URL、http/https/mailto/tel 等协议的支持范围。应用自定义协议应通过显式策略授权。[富文本链接](../crates/render/src/rich_document.rs#L78) 已有协议白名单，可参考其校验方式。

**验收：** 覆盖大小写、前导空白、TAB/换行混淆、相对路径和正常协议；检查创建、动态 patch、SSR、CSR、stream 一致。拒绝 patch 后旧模型和 href 保持原值，后续合法更新可以继续执行。

### F02 便携路由链接无法完成异步 guard 导航

**P2，独立复现。** `routing::link` 的 Activate 固定调用同步 `router.push`。安装任意 async guard 后，即便 guard 立即返回 Allow，点击仍返回 `AsyncRequired`，当前位置不变。

依据：[Link helper](../crates/routing/src/link.rs#L13)、[同步准备拒绝异步 guard](../crates/routing/src/router.rs#L308)。Web 的语义链接路径会先[阻止默认行为](../crates/web/src/browser/events.rs#L518)；raw `data-glory-route` 驱动则已经[选择异步入口](../crates/web/src/routing.rs#L364)。

独立消费者挂载 helper、派发 Activate 后得到 `Some(Err(AsyncRequired))`，路径仍为 `/`。同步 API 返回此错误符合自身契约，问题在于公开链接组件与异步导航能力未配套。

**改进：** 提供使用显式本地执行器的异步 Link/helper，或让现 helper 获得明确的执行服务。导航任务必须属于注册 Owner，保留 supersede、卸载取消和结果回调语义。短期至少在 helper 文档中明确同步限制。

**验收：** 立即 Allow、延迟 Allow、Block、错误、快速连续点击、卸载和 back 场景；同时覆盖 Memory Activate、浏览器真实点击与 native 语义激活，保留修饰键和外链行为。

### F03 路由 bootstrap schema 漏记父子消费边界

**P2，独立复现。** `Routes::schema` hash 了叶子的完整 pattern 和链上的 ID，却没有记录每个节点消费的 pattern。以下两个声明获得同一 schema：

```text
声明 A   root layout("")    → page(":id")
声明 B   root layout(":id") → page("")
```

使用相同 root/page ID、相同 codec schema 和 `/42` 时，A 的 root 参数为空，B 的 root 参数包含 `id=42`；A 的 Bootstrap 在 B 上仍校验成功。复现得到相同 schema `3fc4ca57dae5e36b`。这是 hash 输入缺项，未涉及密码学碰撞。

依据：[schema 输入](../crates/routing/src/definition.rs#L368)、[Bootstrap 校验](../crates/routing/src/bootstrap.rs#L30)。

**影响与改进：** 服务端和客户端路由消费边界变化时，声明身份检查可能漏报，布局参数和相对路径语义可能漂移。为 schema 定义版本化编码，纳入每个节点的 pattern、参数归属和相关策略；codec 改动继续由应用显式版本负责。这不是认证绕过，后续 DOM hydration 仍可能发现其他结构差异。

**验收：** 在叶子 URL、ID 链保持一致时，父子消费边界改变必须拒绝；相同声明重复构造应稳定，尾斜杠、base 和 codec 变更应分别验证。

### F04 KeyedList 小批追加反复复制完整索引

**P2，独立计数复现。** `prepare` 克隆全部旧 key 索引，`extend` 随后再次克隆。向已有 N 个成员的列表追加一项，需要复制约 2N 个旧 key。

依据：[prepare](../crates/core/src/keyed.rs#L236)、[extend](../crates/core/src/keyed.rs#L301)。

独立消费者使用计数 Clone 的 key，从空列表逐条 `extend(vec![row])`，没有 UI 订阅者：

| 最终行数 N | key Clone 次数 |
| --- | --- |
| 100 | 10,100 |
| 200 | 40,200 |
| 400 | 160,400 |

当前实现为 `N × (N + 1)` 次，确认累计 O(N²) key 复制。String key 还会复制其字符串内容。这是确定的工作量问题，尚未据此推断实际帧延迟。

**改进：** 为追加建立增量验证和提交路径，先消除重复快照，再避免全索引复制。保留重复 key 验证、失败前无成员变更、稳定成员身份以及用户 Hash/Eq/析构在 graph 借用外执行的规则。

**验收：** 相同最终规模下比较每批 1、32、1024 行，记录 key Clone、分配、CPU 和完整 UI 时间；同时覆盖重复 key、错误返回、碰撞、退休与恢复。整批 replace 的好成绩不能替代小批追加测试。

### F05 执行器批量取消反复扫描 ready 队列

**P2，源码确认。** `Notify::remove` 每取消一个任务就在 mutex 内对整个 ready FIFO 执行 `retain`。N 个 ready 任务逐个取消时，累计约 `N(N+1)/2` 次条目检查。

依据：[remove](../crates/core/src/executor.rs#L46)。native shell 和 SSR stream 的默认任务限额为 4096，满队列依次取消约需 839 万次数字比较；这不是已测的耗时数字。

**改进：** 参考现有 core scheduler 的立即取消 membership 与有界 FIFO 压缩，或采用支持按 ID 删除的队列。不要通过保留无界 tombstone 换取表面上的 O(1) 取消。继续在锁及状态借用外 drop future，拒绝 stale waker，关闭后不重新开放。

**验收：** 128、1024、4096 个 ready/pending 任务的批量取消、正常完成和关闭，统计检查条目、存储上界与时间；验证 FIFO 和宿主唤醒行为。

### F06 合法路由表可以超过默认匹配预算

**P2，独立复现，属于扩展性和预算协调问题。** 默认允许 4096 个 route 声明，但一次 resolve 的所有叶子共用 4096 步预算。匹配器找到候选后仍继续扫描其余叶子。4096 条单段静态路由 `p0…p4095` 可以构造成功，匹配 `/p0` 却返回 `OverLimit("matching steps")`。

依据：[默认 Limits](../crates/routing/src/location.rs#L46)、[扫描与共享预算](../crates/routing/src/definition.rs#L416)、[步数检查](../crates/routing/src/definition.rs#L638)。该例成功叶至少消耗两步，其余叶各至少一步，总计至少 4097 步。共同前缀还会提高成本。

**改进：** 保留预算防护，为静态段和前缀建立索引；短期在构造和文档中明确声明上限与默认可匹配规模的区别，提供预算诊断。单纯大幅提高预算会放大线性扫描成本。

**验收：** 1、100、1000、4096 条路由，分别测试命中、未命中、共享前缀、动态段优先级和最大工作量；保持现有优先级、歧义检测和超限错误契约。

## 需要测量的性能提升方向

### F07 混合显示每次刷新遍历所有 Widget 岛

**P2，优化机会。** 直接 display App 的混合路径每次刷新都会复制所有 live Widget 岛的 Rc、按 ID 排序，再逐个调用 flush。只有一个 raw 文本变化时，也有 O(W log W) 排序和 W 次岛刷新。

依据：[混合 Widget 刷新](../crates/web/src/template.rs#L527)。纯 raw 行 benchmark 无法充分反映这个成本。

先增加刷新次数计数，再对比 1、100、1000 个控件岛的单 raw 字段更新、单岛编辑和全量更新。可先比较有序存储，随后评估 live/dirty 岛索引；保留 ID 顺序、共享 Runtime、查询屏障和退休删除。预期减少无效刷新，实际收益需测量。

### F08 选中状态仍广播给所有挂载行

**P2，公共 API 优化机会。** benchmark 每行都读取同一个 `selected` Cage。切换选中行时，所有相关行重新比较；过滤相等 class 只能减少 host 写入，无法消除 reactive graph 广播。

依据：[共享 selected 读取](../benchmarks/glory/src/model.rs#L66)、[逐行 computed class](../benchmarks/glory/src/browser/xml.rs#L17)。已有完成报告也保留了这项限制。

建议提供 Owner 管理的按 key selector，让旧 key、新 key 的消费者成为主要通知对象。把它作为框架 API 验证，避免只在 benchmark 业务代码中绕过成本。

验收 1k、10k 和资源允许时的更大挂载规模，统计 `marked`、evaluation、host writes 和 Script 时间；覆盖多视图、重复选择、选中行删除、碰撞 key、卸载与重新插入，清空后应无活 selector 记录。

### 性能目标需要同时约束速度与体积

既有[最终保留版报告](../docs/v2-display-completion-results-2026-10-29.md#L257) 记录：部分创建、选行和清空场景改善，但替换、局部更新和部分 wall 场景仍回退，尚未全部达到原有相对 Dioxus 的工程目标；DSL WASM 从 363,892 B 增至 460,910 B，约增加 26.7%。这是该报告的历史比较，不是本次新测或当前发布包体积。

后续性能门禁应同时记录 Script、wall、实际布局、分配和体积；固定功能、工具链、工作负载和 strip 配置。`opt-level='z'` 也应与应用场景下的 `3` 做配对比较，避免将体积配置当作全部吞吐场景的最优值。保留较慢样本，不能通过只计 DOM 写入、减少记录数或混用 div/table suite 宣称整体提速。

## 安全与工程保障缺口

### F09 开发服务器没有验证 Host

**P2，攻击面确认，未完成浏览器利用。** CLI Web server 只绑定 loopback，但请求处理没有校验 Host，`/__glory_dev/state` 还能返回编译错误正文。

依据：[监听地址](../tools/cli/src/serve.rs#L30)、[请求处理](../tools/cli/src/serve.rs#L135)。这构成需要评估的 DNS rebinding 攻击面，不能据此声称已经发生源码或凭据泄露。

建议默认仅接受实际监听端口上的 localhost/loopback Host，拒绝任意域名和异常 authority；需要自定义 host 时显式配置。增加陌生 Host、重复或缺失 Host、IPv6 与端口测试，并用当前目标浏览器验证 rebinding 防护。保留现有路径边界、请求大小和超时限制。

### F10 缺少显式依赖漏洞门禁

**P2，工程缺口。** 当前主 CI、release 和 canonical 脚本没有 RustSec/cargo-audit/cargo-deny 检查。Dependabot 只配置根目录，未显式覆盖独立 CLI 和 examples/benchmarks 的清单及锁文件。

依据：[主 CI](../.github/workflows/ci.yml#L1)、[主检查](../scripts/check-semantic-ui.ps1#L1)、[Dependabot](../.github/dependabot.yml#L7)。

建议对发布 workspace 和独立 CLI 分别审计锁文件，按维护范围覆盖示例；设置定期扫描及 PR/release 门禁。漏洞豁免应包含理由、负责人和到期日。来源、许可证与撤回版本策略应有明确输出；引擎 checksum/provenance 验证继续保留。[RustSec 官方](https://rustsec.org/) 提供审计锁文件的 cargo-audit，以及检查依赖、许可证和来源的 cargo-deny。

本项未认定任何具体依赖存在 CVE；固定版本和 checksum 证明来源一致，不能证明版本本身无已知漏洞。

### F11 CI 对兼容性承诺的覆盖仍不完整

**P2，验收缺口。** manifest 声明 Rust `1.94`，主 CI 使用 `1.98.0`；现有 workflows 均运行在 Windows。仓库有 Linux/Apple 检查脚本，但没有对应持续运行的 workflow。独立 DSL 编译诊断脚本也未接入当前主门禁。

依据：[MSRV](../Cargo.toml#L15)、[CI 工具链](../.github/workflows/ci.yml#L42)、[Apple 检查](../scripts/check-semantic-apple.sh#L1)、[下游编译诊断](../scripts/check-display-diagnostics.py#L1)。这不证明 1.94 或其他平台已经失败，而是这些承诺缺少持续验证。

建议分层建立门禁：

1. 每个 PR 检查 MSRV 的最小图和主要 feature 组合，保留当前稳定版全功能测试；将 renamed dependency、宏错误诊断等消费者编译案例接入 CI。
2. 为 Linux/Apple 增加与各宿主存储规则一致的持续检查，覆盖 native/wasm 和关键可选 feature；不依赖 all-features 掩盖缺失 feature 声明。
3. 定期运行更昂贵的真实窗口、辅助技术客户端及移动平台验收；补 Safari/Firefox 的 hydration、selection、IME/焦点和 stream 差异测试。

Cargo 将 `rust-version` 视为支持版本声明，并建议在支持版本上验证功能；见 [Cargo 官方说明](https://doc.rust-lang.org/cargo/reference/rust-version.html)。实际 OS IME、读屏播报和真实设备结果应继续单独标注。

### F12 入口文档存在版本与发布状态漂移

**P3，源码与文档对照确认。** README 仍描述 HTML format v3、Apace profile v3；当前 HTML wire format 为 14，Apace style profile 为 11。另一个独立版本轴 HTML style profile 当前为 10。release-readiness 仍写着发布工作需要另行建立当前 pipeline，以及十二包 runtime graph，但仓库已存在自动 release workflow 和 14 个活动 workspace 包。

依据：[README 的 HTML 描述](../README.md#L109)、[README 的 Apace 描述](../README.md#L327)、[实际 HTML wire format](../crates/html/src/lib.rs#L59)、[实际 HTML style profile](../crates/style/src/profile.rs#L383)、[实际 Apace style profile](../crates/apace/src/document.rs#L59)、[旧发布描述](../docs/release-readiness.md#L3)、[当前 release workflow](../.github/workflows/release.yml#L1)。文档应分别引用对应常量。

建议指定一份当前 API/能力索引，README 只保留稳定概念和入口；版本号、包集合和 feature 示例由 metadata 或常量校验。日期报告继续保留历史数据，并明确以提交为界。对 release-readiness、RELEASING 和 workflow 做一次同步校对。

## 应保留的设计及架构约束

core 不依赖 DOM/HTTP，html 做纯映射，宿主负责物理树；可选资源、i18n、routing 和 native transport 有边界检查。Owner、代际身份、查询屏障和成功前缀契约共同约束错误恢复。这些机制值得保留，不能以减少元数据为由绕过。

安全方面，文本及属性转义和 script raw-text 转义分开；模板采用有限标签和属性；原生 HTTP 默认无隐式授权，重定向重新检查 origin，文件挂载通过根目录能力解析。优化应维持显式 I/O 授权和各层预算，不能用单一 body/cache 数字代表总 RSS。

窗口列表按可见窗口加 overscan 工作，离窗释放 Owner/task，最多保留一个焦点 pin；顺序保留场景跳过 LIS。普通非虚拟 Each 完整协调其数据属于当前契约，不能把它本身的 O(N) 枚举误报成窗口列表退化。

主要维护成本来自同一契约的多个执行路径：同步 SSR、stream renderer、Web、native，以及 sparse display 和 Widget 混合。建议扩展共享 conformance 与差分测试，让字段、事件、query、拒绝前缀和退休规则在同一矩阵里验证。保持模块分层，先统一可验证的规则与测试，再决定是否抽取共享实现。

## 建议实施顺序与完成标准

| 阶段 | 工作 | 完成标准 |
| --- | --- | --- |
| 第一阶段 | F01 URL 策略；F02 异步链接；F03 schema；接入 F10 依赖审计 | 安全与 routing 复现转换成正式回归测试，跨宿主一致；现有 gate 和新增检查通过 |
| 第二阶段 | F04 增量追加；F05 批量取消；F06 路由索引与预算 | key Clone/队列检查条目不再平方增长；静态大路由表可诊断并可用；失败、退休和 FIFO 契约保持 |
| 第三阶段 | 测量 F07/F08，补 F09/F11/F12 和体积预算 | 有相同功能与配置的配对数据，保留回退样本；兼容性门禁与公开声明一致 |

首批性能数据建议覆盖：单条与分批追加、满 ready 队列取消、共同前缀路由表、混合控件岛单字段更新，以及单选广播。每项同时观察实际工作量和用户可见延迟，避免用代码行数、分配减少或个别最好结果代替完整收益。

上述阶段顺序为首轮评审建议。修复后的实际实现、证据和保留边界见开头的修复状态；后续优先完成远端平台门禁运行、上游停止维护依赖跟进，以及 selector 在完整业务负载中的收益与成本测量。

## PR 复核及库锁策略（2026-10-30）

- [#53](https://github.com/glory-rs/glory/pull/53) 的质量修复及远端门禁通过，已合并；[#48](https://github.com/glory-rs/glory/pull/48) futures-timer 与 [#49](https://github.com/glory-rs/glory/pull/49) arboard 升级的既有 CI 通过，已合并。
- 合并后的全应用图检查发现 mobile 示例另外精确固定 futures-timer 3.0.3；现同步至 3.0.4 并更新其应用锁，同时为九个维护中的应用/基准图增加 `--locked --all-features` metadata 门禁。
- [#50](https://github.com/glory-rs/glory/pull/50) 暂不合并：Syn 3 已移除宏实现使用的 `Signature.unsafety`，semantic-ui 编译报 E0609。参见[失败运行](https://github.com/glory-rs/glory/actions/runs/40895989648)。
- [#51](https://github.com/glory-rs/glory/pull/51) 暂不合并：accesskit 更新仅覆盖根锁，counter 的应用锁仍解析 0.25.1，与清单的 `=0.25.2` 冲突。参见[失败运行](https://github.com/glory-rs/glory/actions/runs/40979514275)。
- 根运行时 workspace 和独立 `tools/cli` 均为库，不再提交 Cargo.lock。应用、示例和基准锁继续提交；审计产物另存本次解析的准确锁文件。检查入口先生成缺失库锁，再以 `--locked` 执行后续步骤，不覆盖已有本地锁。
- 无锁首次解析必须尊重 Rust 1.94 声明：采用 `resolver.incompatible-rust-versions="fallback"`，避免默认解析选择要求 Rust 1.95 的 mime-infer 4.1.0 / multra 1.2.0。九个维护中应用图的完整 metadata 已在本地通过。
