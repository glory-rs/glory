# Glory 多维代码评审与后续改进报告

评审日期：2026 年 11 月 9 日。初轮源码基线：`d20a1392e358afaf3fe6cc17481bee38cb4a0fc0`，版本线 `0.7.0`。后续修复以 `50098a4` 为开发基线，通过 [PR 60](https://github.com/glory-rs/glory/pull/60) 提交；下列完成状态和新增证据描述修复结果，原始发现保留其基线含义。

## 修复完成状态

七项具体改进均已实现并补充回归。主评审协调三个 agent 分工修复，再交换未参与实现的专项进行代码审查。交叉审查没有发现阻止合并的运行时问题；发现的超时测试窄窗口已用更宽预算和 started/release 信号修正。最终远端 CI、review 和合并状态以 [PR 60](https://github.com/glory-rs/glory/pull/60) 为准。

| 项目 | 实现与回归状态 | 保留边界 |
| --- | --- | --- |
| F01 完成 | 最多 8 个连接处理线程，header/response 绝对期限，25ms socket 取消检查；真实慢滴、并发正常请求、满额和阻塞写回归通过；CLI 全部 33 项通过 | 限制 socket 等待，无法抢占同步本地文件系统调用 |
| F02 完成 | callback 前置 inbox 同时限 4096 条和 8 MiB，Tao 只收到合并 Wake；7 项 inbox 回归含真实 Renderer ready/ack/FIFO、洪泛、关闭和线程交错 | 溢出保留已接受前缀后明确终止；原生平台自身的 IPC 分配与真实窗口洪泛不是该预算的证明范围 |
| F03 完成 | 宽序列非边界编辑提升为 live NodeId 邻接索引；小/追加序列保留 Vec；工作量测试、顺序差分、跨父、悬挂、拒绝前缀、身份复用通过 | 2048 提升、1024 退回；以额外 O(live siblings) 索引内存换结构修改成本；公开每步读取连续 slice 仍 O(N) |
| F04 完成 | Text/Value 不再使 translation pending；保留结构、metadata、typed Style 和 reduced-motion 失效；新增真实 Chromium 回归在 0/1/1000 节点通过 | 相同回归在旧驱动上失败：10 次局部更新额外请求 10 个 RAF，修复后为 0；仅声明 typed Style 隐藏，未扩展任意 author CSS 的动画资格契约 |
| F05 完成 | 当前发布/CLI/README 指南统一继承调用者环境、按所属 workspace metadata 查 target；新增文档负向回归，46 项脚本测试通过 | 历史报告中的路径不改写；检查只覆盖明确列出的当前操作指南 |
| R01 完成 | 新增可选 StreamTimeouts request/idle，覆盖无 demand 与 Pending；区分超时原因；不断开真实连接也能清理并恢复 admission；Salvo 共 29 项通过 | 默认均 None；生产应用显式选期限；同步 factory/loader/序列化仍为合作式 |
| R02 完成 | Loader/FileStats 聚合 worker_panicked/worker_cancelled；覆盖 async 与 spawn_blocking；panic 后继续真实文件读取/资源处理并检查许可释放 | 不保存 panic 正文或逐任务历史；completed 仍是接受发布，不是 handler 完成 |

### 修复后的验证证据

本机活动 workspace 全 feature 测试 **1263 passed、0 failed、2 ignored**；两个显式平台验收测试不计为通过。独立 CLI **33 passed**；脚本 **46 passed**。**32 套 Chromium 验收全部通过**，包括新增 translation，以及既有 SSR adoption、stream、查询、输入、焦点、选择和清理；独立真实 TCP 顺序、断连与慢读背压检查也通过。其余整体验证命令及各检查状态记录在 [修复验证清单](quality-fixes-2026-11-09-validation.json)。

原生与 WASM 的 workspace all-features/all-targets 严格 Clippy、独立 CLI 和 WASM fixture 严格 Clippy 均通过。另显式运行的真实 Wry 窗口验收通过。该窗口首轮在固定宽度的浮点精确相等断言失败；原始 `50098a4` shell 与独立 plain DOM 都复现同样的 `359.9877319`，WebView2 的有效 DPR 为 `1.02`。只把两个固定尺寸断言改为有限正数且误差不超过 `1/64` CSS px，明确验证 359/361 这样的 1px 偏差仍失败，未放宽选择、焦点或滚动断言。失败、基线、独立探针及最终通过日志均保留。

新回归的红绿证据使用同一 translation fixture：旧 production driver 仅适配新增私有参数签名，出现 10 个额外 RAF 而失败；恢复修复后的源码与产物后，0/1/1000 节点的无关更新均不新增 RAF，并保留 animation/editor 身份、typed 隐藏/恢复、Suspend/Resume、reduced-motion 和卸载行为。

### 宽列表的工作量与性能取舍

确定性测试验证宽列表重排/前插的 promotion 只检查 N 个 live 节点，随后不再逐步移动或搜索连续兄弟；全循环结束前不物化 snapshot。追加和首部退休保持原 Vec 路径。当前 x64 Rust 布局中 Children 本体修复前后均为 32B；提升后的哈希索引及按需 slice 另占内存，没有 allocator/RSS 改善结论。

以下是共享主机上固定同一逻辑 CPU 的冻结 release 二进制对照，中位数单位为 ms。两侧使用相同数值 workload，准备和完整顺序/邻接校验均在计时外；它测量 semantic Memory，不测 DOM、布局或呈现。完整原始数据、二进制 hash、源码溯源和限制保存在 [性能证据](quality-review-2026-11-09-performance.json)。

| 节点数 | 操作 | 修复前 | 修复后 | 变化 |
| --- | --- | --- | --- | --- |
| 1000 | append | 0.1661 | 0.17035 | +2.6% |
| 1000 | prepend | 0.3839 | 0.3967 | +3.3% |
| 1000 | reverse | 0.39945 | 0.32955 | -17.5% |
| 1000 | shuffle | 0.7377 | 0.51195 | -30.6% |
| 1000 | head retire | 0.2030 | 0.20125 | -0.9% |
| 10000 | append | 2.3557 | 2.54905 | +8.2% |
| 10000 | prepend | 31.14545 | 5.37305 | -82.7% |
| 10000 | reverse | 21.6511 | 5.3226 | -75.4% |
| 10000 | shuffle | 50.53375 | 7.0185 | -86.1% |
| 10000 | head retire | 3.06735 | 2.9891 | -2.6% |
| 50000 | append | 12.49725 | 13.03485 | +4.3% |
| 50000 | prepend | 789.7053 | 27.9837 | -96.5% |
| 50000 | reverse | 557.56485 | 33.6980 | -94.0% |
| 50000 | shuffle | 1315.85315 | 48.3045 | -96.3% |
| 50000 | head retire | 16.5326 | 15.0697 | -8.8% |

保留 append 的回退，不声称整体提速。早期未固定 CPU 的 ABBA 曾测到小操作约 2 倍回退；同一冻结候选重新交替和同 CPU 复测未重复该统一幅度，同进程 old/new Children 微基准的 fast path 约在 -2.7% 至 +4.2% 内。异构核/调度影响只是解释假设，原始不利结果完整保留，主机仍有其他编译任务，亲和设置不等于独占 CPU。第一次 baseline benchmark 格式化前的原始文件字节未另存，数值 workload/计时区不变，但不提供不存在的原始字节 hash。

具体修复完成不等于“架构与 API 后续投入”中的长期工作全部完成。新增公共 Stats 字段及 StreamError::Timeout 可能要求外部穷举初始化或 match 更新；未增加 unsafe、I/O 默认授权、全局缓存根目录或 runtime 依赖。

## 初轮评审结论

Glory 已具备较完整的所有权、代际身份、显式 I/O 授权、失败前缀和宿主验收体系。本轮最值得优先处理的是开发 HTTP 服务的请求占用时间、Wry 入队前的资源预算，以及两处随页面规模放大的结构与控制器工作量。并发专项没有发现高置信的 P1/P2 并发正确性错误；生产流式 SSR 的超时策略和 native worker 故障观测仍有改进空间。

本轮没有确认 P0/P1 问题。该结论限于下述代码抽查与测试范围，不代表完整安全审计或全部平台验收通过。历史评审中的已修复问题没有重新计为当前缺陷。

## 范围和评审方式

评审对象是 `D:/Works/glory-rs/glory` 主仓库，开始时工作树干净。Cargo metadata 列出 15 个 workspace 包；独立 `tools/cli` 同时纳入评审。相邻 Apace、Takia、Wingy 仓库和 CI 验证副本不作为本次 Glory 源码发现的对象；引擎授权与调用边界通过 Glory 侧代码检查。

主评审和三个专项 agent 分工如下，发现由主评审再次核对调用链和行号：

| 评审角色 | 独立维度 | 重点范围 |
| --- | --- | --- |
| 安全 agent | 信任边界、输入校验、权限、资源耗尽 | HTML/SSR 转义、Wry IPC、Apace transport/files、CLI HTTP |
| 并发 agent | 异步取消、唤醒、锁与重入、Owner 生命周期 | core executor/resource、SSR/Salvo stream、native workers |
| 性能 agent | 算法复杂度、局部更新、内存、测量有效性 | semantic Memory、Each/list、browser 驱动、benchmark |
| 主评审 | 架构、工程质量、供应链、文档、独立验证 | manifests、CI/release、构建目录、测试执行和发现复核 |

证据分为三类：**静态确认**表示当前调用链直接支持该结论；**能力缺口**表示特定部署条件需要额外策略；**改进机会**表示有成本或维护收益，但没有证明其违背现有契约。本轮没有对新发现完成专门的故障注入或性能计时，现有测试通过也不等于新问题已被覆盖。

优先级：P1 为应立即处理的重大问题；P2 为影响特定规模、宿主或部署的功能与可用性问题；P3 为维护和诊断改进。优先级不等于漏洞评级。

## 各维度结论

| 维度 | 已有优势 | 下一步重点 |
| --- | --- | --- |
| 安全和隐私 | 链接协议限制、按上下文转义、显式文件/网络能力、密码保护 | F01 请求总期限；F02 IPC 全链路预算 |
| 并发正确性 | 本地 graph 与跨线程数据隔离，锁外回调和析构，代际 ID | 为共享 wake/close/cancel 状态机补交错验证 |
| 生命周期和资源治理 | Owner 退休、旧结果撤权、物理 I/O 许可保留、断连取消 | R01 长连接/永 Pending 的许可回收策略 |
| 算法效率 | 虚拟窗口、稀疏高度索引、LIS 快路径、字段级补丁 | F03 宽兄弟列表结构修改；F04 无关驱动重检 |
| 性能与内存 | 有界队列、独立资源预算、聚合 profiling、历史配对测量 | 增长曲线、分配/RSS、真实宿主帧与额外 RAF 验证 |
| 架构与代码质量 | core/render/html/host 分层，可选依赖有门禁 | 按状态机拆私有模块，扩大宿主共享差分用例 |
| 测试和可移植性 | Windows、Linux、MSRV、浏览器和真实流式测试入口齐全 | 自动化压力用例；Apple/移动端与读屏分别验收 |
| 供应链与发布 | 维护 workspace 清单、Actions SHA、发布来源校验和审计策略 | 保留例外依赖的迁移跟踪，建立每次发布证据清单 |
| API 和文档 | Widget、Each、Vary 及能力边界较清楚 | F05 清除当前指南的旧构建规则；补集合 API 成本说明 |
| 可观测性 | Runtime/UI/loader 的聚合统计较丰富 | R02 区分任务 panic、普通失败与已接受发布 |

## 优先改进项

### F01 开发 HTTP 服务缺少整个请求的绝对期限

**P2，静态确认，影响本机开发服务可用性。**

位置：[`tools/cli/src/serve.rs:45`](../tools/cli/src/serve.rs#L45)、[`143`](../tools/cli/src/serve.rs#L143)。

主循环同步调用 `respond()`，该函数将 socket 改为阻塞模式，为每次读取设置两秒超时，再循环读取到完整 headers。这个超时限制单次空闲等待，没有限制请求累计占用时间，也没有在循环中检查停止标志。

本地连接只需每秒发一个字节且不结束 headers，即可不断刷新读取等待。16 KiB 长度上限仍允许连接占用数小时；这期间其他请求、watch 重建和 Ctrl+C 标志的处理都要等待 `respond()` 返回。服务只监听 loopback，不能据此表述为已确认的公网攻击。

建议增加绝对 header/response deadline，将剩余时间传给每次 I/O，并以可取消、有并发上限的方式处理连接。不要只把每次读取超时调小。

验收：建立慢滴 socket，同时发送正常 GET 和停止信号；正常服务与退出必须在规定时间内完成，慢请求在总期限后释放。保留 Host authority、路径穿越、gzip/HEAD 回归。本轮 CLI 30 项测试通过，尚无该慢滴用例。

### F02 Wry 的宿主入队阶段缺少总预算

**P2，静态确认的预算覆盖缺口，实际平台洪泛影响待测。**

位置：[`crates/wry/src/shell.rs:311`](../crates/wry/src/shell.rs#L311)、[`crates/wry/src/lib.rs:241`](../crates/wry/src/lib.rs#L241)、[`protocol.rs:10`](../crates/wry/src/protocol.rs#L10)。

IPC callback 只检查单条消息不超过 8 MiB，然后把拥有的字符串交给 `proxy.send_event()`。Renderer 的待处理总字节和消息数检查发生在 UI loop 取出消息、调用 `receive()` 之后。因此下游有界队列不能证明 callback 到 event-loop 这一段也受 Glory 的总容量限制。

如果 WebView 的消息生产快于宿主消费，框架没有入队前的计费来限制这一段滞留的字符串。真实平台的事件交错、平台队列限额和最大滞留量尚未压测；本项没有证明远程攻击或绕过 Wry 的导航/CSP 隔离。

建议引入按数量和字节计费的有界 host inbox，callback 只写入 inbox 并发送一次合并 Wake。出队和关闭释放额度，满额采用明确失败策略；可以合并的几何观察和必须保序的 ack、编辑终态应有不同规则。

验收：暂停消费后持续送入消息，断言 retained count/bytes 的硬上限；恢复后验证 ack、终态输入、过期 session、关闭和额度释放。Wry 本轮通过的 16 项库测试没有实际 WebView 洪泛用例。

### F03 宽兄弟列表的批量插入和重排存在二次工作量

**P2，静态复杂度确认，耗时收益需要基准验证。**

位置：[`crates/render/src/memory.rs:1412`](../crates/render/src/memory.rs#L1412)、[`1654`](../crates/render/src/memory.rs#L1654)、[`memory/children.rs:18`](../crates/render/src/memory/children.rs#L18)、[`crates/ui/src/list/cache.rs:195`](../crates/ui/src/list/cache.rs#L195)。

`unlink()` 对非首尾兄弟线性找位置，`place()` 再线性找 `before`；底层 Children 使用 Vec，中间插入/删除还会搬移元素。对 N 个同父节点反转，或在宽列表前部逐项插入，O(N) 次结构操作中的线性查找/搬移可累计为 O(N²)。LIS 减少移动次数，不能消除每次 Memory 移动的成本。

本结论针对普通语义 Widget/Each 的 Memory 路径。追加、相邻 no-op、从头退休已有快路径；不能把所有创建/清空或 sparse display 路径都归为平方级。

先建立 1k/10k/50k 同父节点的追加、前插、反转、随机打乱和首部退休基准，再评估批量顺序更新、索引或分块表示。新增下标缓存也可能在插入后产生线性维护成本，不能仅凭数据结构名称判断收益。

验收应同时核对增长曲线和真实宿主时间，并保持逐操作成功前缀、next-sibling、代际身份、焦点/选区及回滚语义。分开记录匹配、Memory、mapper/host 和布局成本，保留快路径回退样本。

### F04 无关局部补丁会重检所有 translation 控制器

**P2，静态工作量确认，实际帧时间影响待测。**

位置：[`crates/web/src/browser.rs:218`](../crates/web/src/browser.rs#L218)、[`browser/translation.rs:139`](../crates/web/src/browser/translation.rs#L139)、[`189`](../crates/web/src/browser/translation.rs#L189)、[`semantic_changes.rs:18`](../crates/web/src/semantic_changes.rs#L18)。

每次 `Renderer::apply()` 都调用 `update_translations()`，包括无关的 Text/Value patch。驱动扫描 live 控制器，只要表非空就设置 pending 并调用 `frame.input()`；随后观察再为各控制器沿祖先链检查允许状态。M 个稳定控制器、树深 H 时，一次无关更新可增加 O(M·H) 检查以及宿主帧请求。帧请求可能合并，实际增加多少 RAF 需要计数；这不是无限 RAF。

配置和允许状态不变时已有逻辑会跳过 computed-style/animation 写入，应保留该保护。改进重点是减少无关扫描和 pending 设置。

建议把批次状态和 metadata 变更传入驱动，只因相关声明、结构、资格或显示样式变化重检。**不能机械套用现有 `StateChanges::inherited()`：Style 当前不设置 eligibility，祖先 `display:none` 等变化仍必须覆盖。** Presence、Collapse、Sheet 的类似扫描可以随后按实测决定是否处理。

验收：分别保留 0/1/1000 个稳定 translation，连续更新无关文本，统计控制器访问和真实 RAF；同时验证 metadata、Suspend/Resume、祖先样式隐藏和 reduced-motion 的正确终止/恢复。比较同一 release 构建与真实浏览器帧，不能从检查次数直接推导帧率提升。

### F05 当前构建指南仍要求使用已停用的目录

**P2，静态确认的文档与工具契约冲突。**

位置：[`docs/release-readiness.md:35`](release-readiness.md#L35)、[`tools/cli/README.md:204`](../tools/cli/README.md#L204)、[`scripts/check-current-docs.py:15`](../scripts/check-current-docs.py#L15)。

release-readiness 仍要求把构建产物、Cargo 缓存和临时文件全部放到旧全局目录；CLI README 的 WSL 说明也继续要求旧挂载。它们与当前 AGENTS、RELEASING 和 CLI 的 `Paths::apply()` 继承环境行为冲突。`validate_build_root()` 已明确拒绝旧目录；照现有指南配置会失败，或引导调用者重新创建已禁止使用的路径。

建议更新当前入口指南，统一为继承调用者环境与项目配置，在实际 workspace 用 `cargo metadata` 查询 target_directory。文档检查应验证当前操作指南，区分历史证据中的路径和当前命令指令，避免批量改写历史测量报告。

验收：Windows、WSL、macOS 的入口文档与当前代码逐项一致；加入旧规则的负向检查。本轮 `check-current-docs.py` 仍通过，说明现有检查没有覆盖这一冲突。本轮没有访问或创建旧目录。

## 可靠性和诊断能力改进

### R01 为生产流式 SSR 明确请求期限

**P2，部署条件相关的能力缺口。**

位置：[`crates/salvo-adapter/src/streaming.rs:51`](../crates/salvo-adapter/src/streaming.rs#L51)、[`157`](../crates/salvo-adapter/src/streaming.rs#L157)、[`crates/ssr/src/stream/mod.rs:55`](../crates/ssr/src/stream/mod.rs#L55)。

并发许可、每次 demand/reply 和帧背压均有界，但 adapter 没有总时限或等待 demand 的闲置期限。客户端保持连接而长期不产生 demand，或业务 loader 永远 Pending，可以长期占用 worker、runtime 和许可；达到 max_requests 后新请求持续 Busy。这是有界资源长期占用问题，不是无界缓存或断连取消失效。部署层若已提供有效的超时并会丢弃 body，可以缓解此风险。

建议提供显式可选的总期限和 idle 期限，覆盖等待 demand 和 next_frame，并记录超时原因。测试保持旧连接不断开，确认到期后 Owner/任务清理、许可回收和新请求恢复。同步 factory 或 loader 阻塞仍需要业务代码合作式返回，不能承诺超时可抢占同步执行。

### R02 保留 native worker 的失败分类

**P3，静态确认的可观测性缺口。**

位置：[`crates/apace/src/loader.rs:426`](../crates/apace/src/loader.rs#L426)、[`496`](../crates/apace/src/loader.rs#L496)、[`558`](../crates/apace/src/loader.rs#L558)、[`crates/apace/src/files/worker.rs:27`](../crates/apace/src/files/worker.rs#L27)。

JoinSet 的消费路径忽略 JoinError。自定义 handler/future panic 时，服务统计难以区分任务崩溃与普通失败。Tokio 的 panic hook 仍可能输出日志，因此不能说所有错误信息都消失。Loader 的 completed 在调用 handler 前增加，语义是接受发布，不应被运维理解为 handler 已成功结束。

建议增加固定聚合的 panic/cancel 指标与结构化诊断，明确 completed 的语义，不保留逐任务历史。通过注入 handler/future panic 验证后续任务继续处理、许可释放、统计分类正确，并保持回调和析构在锁外执行。

## 架构与 API 后续投入

以下为改进机会，不计入已确认缺陷。

1. **补充集合 API 的成本说明和批量入口。** `core/src/keyed.rs:338` 的 remove_key 有 key 索引，但顺序位置查找和 Vec 删除仍为线性；循环删除可累计为平方级。评估 remove_keys/retain 一次压缩顺序的接口。`ui/src/each/source.rs:170`、`192` 的 Cage/Bond<Vec<T>> 刷新会克隆全向量；文档应给出何时选择 KeyedList/字段投影，并用 Clone 计数样例展示区别。token 复制不等于业务值复制。
2. **按状态机提取私有模块。** app、Memory、runtime、browser events 聚合了较多职责。优先把查询登记与取消、生命周期、输入版本、宿主驱动分类提取成私有模块，保持公开 API 和契约测试稳定。文件行数只用于安排阅读成本，不是质量评分。
3. **扩大共享差分验证。** 现有 render conformance 和 HTML patch oracle 是良好基础。增加由同一 mutation 序列驱动 reference/SSR/browser/native 的测试，覆盖拒绝前缀、悬挂子树、回收身份、dirty editor、query barrier 和 capability 拒绝。宿主各自仍需真实布局与输入测试，不宜为了减少重复而合并不同物理语义。
4. **对跨线程的小状态机补交错验证。** 为 Executor Notify、ResourceWake、worker close/permit 增加 barrier 驱动的 wake/close/cancel 顺序测试，必要时做小规模模型检查。保持 Runtime graph 本地所有权，不把它整体改成共享锁模型。
5. **建立当前版本性能和体积基线。** 分开记录冷启动/稳定更新/结构重排、Script/两 RAF wall、布局/绘制、WASM/JS 体积、分配峰值和 RSS。历史报告不能代表本轮 HEAD。profile.release 的 opt-level=z 应与吞吐量优先配置按同一 workload 配对比较，先测量再决定是否增加配置。
6. **保持平台和供应链证据独立。** 现有 CI 已串接依赖审计、MSRV 和 Linux 检查，发布等待 reusable CI；本轮未发现这些门禁缺失。对当前审计中的显式例外保持迁移跟踪，定期更新数据库；Apple、移动设备、OS IME、读屏语音及不同浏览器分别记录实际结果。

## 建议执行顺序和完成条件

下表是建议批次，不是已承诺工期。

| 批次 | 工作 | 完成条件 |
| --- | --- | --- |
| 第一批 | F01、F02、F05 | 慢连接可超时/取消；IPC 入队前 count/bytes 有界；当前文档不再要求旧目录 |
| 第二批 | F03、F04 | 有修复前后的规模曲线/工作量；真实宿主回归通过；快路径和动画状态机无退化 |
| 第三批 | R01、R02 | 长连接不断开也能按配置回收许可；panic 分类可观测；关闭与锁外析构保持正确 |
| 持续投入 | 共享差分测试、批量集合 API、模块拆分、平台证据 | 每项建立独立验收，功能正确性与性能收益分别交付 |

不建议把这些工作合成一次大重构。每次修改应携带最小触发场景、失败前证据、修复后验证，以及对成功前缀、焦点/选区和资源释放的检查。

## 初轮实际验证

执行环境：Windows `x86_64-pc-windows-msvc`，Rust/Cargo `1.99.1`。产物目录由所属 workspace 的 metadata 查询：主仓库为 `D:/Works/glory-rs/glory/target`，CLI 为 `D:/Works/glory-rs/glory/tools/cli/target`。没有重定向 Cargo home、target 或 TEMP/TMP，没有 Docker 构建。

| 检查 | 结果 |
| --- | --- |
| 10 个可移植 crate 的 `cargo test --locked ... --all-features` | 797 passed，0 failed，0 ignored，包含文档测试 |
| 同 10 个 crate 的 all-features/all-targets 严格 Clippy | 通过，`-D warnings` |
| Wry all-features 库测试 | 16 passed，不含真实窗口测试 |
| Salvo all-features 测试 | 24 passed，其中 13 项 stream，包含真实 HTTP/TCP、慢读背压、断连取消 |
| 独立 CLI `cargo test --locked` | 30 passed；安全 agent 另复跑其中 5 项 serve 测试，不重复计数 |
| `python -m unittest discover -s scripts -p 'test_*.py'` | 43 passed |
| `cargo fmt --all -- --check` | 通过；稳定版 rustfmt 对 nightly 配置输出警告 |
| `python scripts/check-current-docs.py` | 通过，同时静态确认 F05 未被该脚本覆盖 |
| `git diff --check` | 通过 |

Rust 测试合计 867 项，Python 测试 43 项。命令、覆盖边界和日志摘要见随报告保存的 [验证清单](quality-review-2026-11-09-validation.json)。原始日志位于本机 `target/verification/code-review-2026-11-09/`，该目录不随 Git 提交。

初轮没有执行完整的 `check-semantic-ui.ps1 -Web -Window -Audit`、新的 RustSec 在线扫描、浏览器/真实窗口验收、Apace 全 feature 测试、Android/iOS/Apple 验收或新性能计时。初轮 867 项测试只支持该阶段列出的范围；后续新增的故障/压力、浏览器和性能验证见本文开头的修复状态与验证清单。
