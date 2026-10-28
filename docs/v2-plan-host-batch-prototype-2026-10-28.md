# Glory v2 Plan 浏览器批量提交原型

日期：2026 年 10 月 28 日。分支：`v2`。基线：`f046098d795027e2d15e8c6b587188327ea1b7fc`。

后续同机受控优化见[提交准备与暂存日志报告](v2-plan-submit-cost-optimization-2026-10-28.md)。
下文保留初始原型的构建与测量；后续结果另列，不混合样本或计时方法。

**结论：保留为 v2 原型，不据此引入 DSL 或合入主线默认路径。** 桥接调用减少约 97%，但四轮共 1296 个样本没有建立整体性能收益。万行创建四轮同方向改善，合并中位数快 5.0%；千行创建的合并改善不稳定，其余多项操作回退。

本轮使用现有 Plan 实现创建、普通字段初始化、离树挂接和完整实例销毁的浏览器提交段。共享声明、每个语义节点的完整身份、UI 与 Mapper 两份 Memory、Cage/Bond/Vary 的字段依赖均保留。DSL 和静态身份压缩均未引入。

## 实现与支持范围

`App::commit` 已收集语义修改。本轮在其后的 Web 执行层增加三个部分：

- [Memory 暂存批](../crates/render/src/memory/submission.rs)：保存触及字段的原有值、创建身份的旧水位、原有邻接关系以及本批移出的退休节点。字段逆操作直接转移所有权；不克隆整棵 Memory，不复制完整活描述符。
- [Mapper 准备与确认](../crates/html/src/submission.rs)：顺序预检、调用规范字段 guard 和纯 HTML lowering，暂存 HTML key 变更，按宿主确认的完整语义前缀结算。
- [Web 操作编码与执行](../crates/web/src/browser/submission.rs)：生成有限记录和字符串区，一次调用 JS 执行一段修改，随后登记对应的物理 Entry 和父关系，清理已确认的退休状态。

每个操作记录为 10 个 u32；语义 mutation 结束标记单独计数，Keep/等值更新即使没有 DOM 编辑也有自己的提交边界。字符串位置明确使用 UTF-16 单位。每段最多 256 个语义操作、4096 个计费模型节点、4096 条物理记录和 64 KiB 新字符串。超预算形成有序回退，不缩小单个业务字段的支持范围。

当前批量创建接受无 Semantics 的 View/Text/Button/Link Plan，且物理形状满足普通原型条件；初始化覆盖 Text、Label 和 Classes。新控件、Select/Rich、Group、Suspend/Resume、其他字段及原型预算回退继续使用原路径。包含 Layer/FocusScope 或参考输入队列的模型保守退出这条准备路径。

离树放置可在 JS 内完成。已连接节点移动回到原有 selection/IME/moveBefore 路径。批量销毁要求完整的原始实例，且没有编辑控件或 Resize 兴趣；部分退休、外来后代和被聚焦的子树形成有序回退。**因此这是完整覆盖上述 Plan 生命周期的原型，尚未把所有控件和所有有限操作塞进单个入口。**

## 物理部件与生命周期

一个提交段中新建的实例共用 JS 部件数组，各自保存有限 offset 与不可变 Layout。静态原型仍由文档所有，业务值和实例身份不会进入声明缓存。日常字段操作传数字地址；必要查询和宿主逻辑才取得真实 Node。

完整批量销毁在 JS 移除根并清空该实例的部件区间，确认后 Rust 清理 Entry、观察器关联和待处理输入。标量销毁及部分退休也会清空自己的区间或成员；一个存活实例不会借共享数组保留已经退休的兄弟。分段数组只跟本段中的活实例关联，不保存访问历史或业务 key。

初始化保持原始操作顺序，未增加跨 mutation 的重排或最终值折叠，也未合并原有 UI/Mapper 生命周期。多个行根仍各自物理挂接；本轮没有增加 Dioxus 式的整组根替换。因此宿主调用减少与真实 DOM 工作减少需要分别计量。

## 成功前缀与输入状态

准备阶段只暂时推进 Mapper。JS 为每个完整语义操作计数；异常停止当前包，返回最后完成的语义边界。Rust 保留确认前缀，逆序恢复未执行后缀及其 key、统计、generation 和 metadata sharing 状态。单个语义操作内部已经发生的物理写入仍可能保留，App 进入 Faulted，不承诺 DOM 原子回滚。

如果准备后面的操作时发现语义错误，前面的合法段仍先执行；该段若更早发生宿主异常，保留原有错误先后关系。连接状态导致的回退在对应操作写入之前返回，原标量路径随后处理该操作。查询仍先刷新此前提交，受控 property/revision/selection/composition 继续走现有逻辑。

JS 在首次 DOM 操作前复制本段有界数字记录，字符串和对象表已经由 JS 持有。原生回调造成 WASM memory.grow 时，后续执行不再依赖失效的借用 TypedArray。原生监听器仍只排队，业务处理器不能重入正在借用的文档。

## 正确性验证

新增五项专项 Rust 测试，覆盖所有成功前缀、重复写入、丢弃和 unwind、metadata sharing 恢复、退休槽 generation 复用、晚语义拒绝，以及跨实例移入/移出后代后的父子和兄弟邻接恢复。最后一项在工作区全量测试之后添加，并在最终源码专项测试中通过。

[新增浏览器验收](../examples/semantic-web/submission-acceptance.js)使用公开 Plan API，验证四行创建/初始化/挂接的一次 JS 执行、执行中 memory.grow、节点身份保留、部分退休、共享数组 GC、草稿/UTF-16 选区/组合态保留、聚焦子树退休回退、包内异常、后缀停止与 Faulted 清理。

最终原型通过 31 套真实 Chrome 验收，以及本轮重建 stream_fixture 的传输检查。正式计时的三份构建逐样本核对相同数据 oracle。另一次[独立验收](/D:/BuildCache/glory-v2-plan-batch/acceptance-1793160410204/evidence.json)通过存续 Element/Text 身份、万行真实矩形和六个画面检查点对照；其中候选 WASM 为 4cc860…，随后仅给 Layout 辅助方法添加 wasm32 条件编译并补充 cfg(test) 测试，最终计时与调用计数使用 e51ab7…。

最终二进制的截图复验没有完成：一次误将软件合成与早期默认 GPU 配置的 PNG 相比，包括清空后的静态界面在内六张哈希均不同，不能据此判断实现差异；随后同配置重跑在基线的截图阶段超时，记录见[失败证据](/D:/BuildCache/glory-v2-plan-batch/acceptance-1793165513201/evidence.json)。因此不宣称最终二进制已完成三方像素一致性复验。该失败未混入性能样本，也不替代最终二进制的浏览器协议和数据断言。

原 Unicode selection 验收直接点击先于异步 WASM 初始化存在的普通按钮，首次整套运行读到了空 CSR 根。新增挂载等待后保留全部原断言，最终整套复验通过。

### 全量门禁的边界

原始 `check-semantic-ui.ps1 -Web` 在两个原生测试失败处停止：

- `rejected_uri_payloads_do_not_open_paths_or_replace_a_selected_grant`：URI 拒绝类型得到 Type，测试要求 Metadata。
- `requests_are_lazy_cancel_releases_delivery_but_not_the_os_prompt_and_old_ids_cannot_publish`：测试要求 Ready，实际仍为 Pending。

两项均在未修改的冻结基线单独复现，所属实现和测试未在本轮修改。排除这两项后，工作区 1090 项通过、2 项既有忽略；最终源码另行通过上述五项专项测试、native/WASM 全 feature/target 严格 Clippy 和 fixture WASM 严格 Clippy。**完整 canonical 门禁仍为失败，不将这些分项结果描述为完整门禁通过。**

一次并行复验还触发系统错误 1455，无法 mmap 原生 rlib；保留日志后改为 CARGO_BUILD_JOBS=1，串行构建复验通过。相关失败、基线复现及重跑证据保留在本轮 D 盘目录，没有通过修改原生断言来消除失败。

## 测量方法

基线与候选使用同一绝对源码路径 `D:/BuildCache/glory-v2-plan-batch/before/inputs`，先构建、归档原始源码，再在该路径安装候选实现。两者分别使用 `before/glory/target` 和 `after/glory/target`；Dioxus 0.7.10 在 `before/dioxus/target` 独立构建。最终三个应用 compiler artifact 均为 fresh=false，没有互拷 WASM。候选修订在其自己的 target 增量复建，依赖缓存命中不算新的独立构建。

业务数据、benchmark Rust 源码、CSS、harness、依赖锁和 release 配置不变：opt-level=3、LTO、单 codegen unit，wasm-bindgen 0.2.129，不运行 wasm-opt。基线保留全部 844 个跟踪输入；候选仅修改九个 runtime 实现入口/模块，运行时代码与工作树逐字核对。Mapper 文件的最终第五项测试属于额外的 cfg(test) 验证。

机器为 i7-14700KF，Chrome 156.0.8078.48 固定到逻辑 CPU 0–15、AboveNormal 并读回确认；视口 1280×900、DPR 1、en-US、light。正式计时使用 headless、`--disable-gpu`，每个变体每轮单独启动浏览器。主计时与调用计数、GC、截图和深身份检查分开执行。每次变体运行记录宿主 CPU 负载和 Cargo/rustc 进程，不能把共享机器当作独占环境。

主表使用原始 `bench-harness.js` 的 click→两个 RAF 耗时，业务动作、setup、snapshot、3 次热身与 12 次样本保持原样。每个变体正反四轮，共 48 个样本/操作；CPU 计数只在整轮前后读取，包含 setup、热身和数据核对，不能解释为某个单操作的 ScriptDuration。

早期一个画面驱动在 Context 关闭阶段未能推进；逐样本读取 CDP Performance 的两种驱动也在基线阶段超时。分段诊断显示 CDP 查询和部分宿主交互有秒级额外等待。最终改为独立浏览器进程，取消逐样本 CDP 读取，使用原 harness 的页面内计时循环。失败尝试和阶段记录保留，未混入最终样本；不能将其不完整的中位数拼成性能结果。

Dioxus 的独立深身份检查保留相同操作和节点/文本/矩形断言，清空阶段只检查 DOM，因为它没有 Glory 的 RuntimeStats 接口。其余两份 Glory 构建继续要求清空后回到原有节点、图和行状态基线。

全部构建、临时文件、源码、原始样本与证据位于 `D:/BuildCache/glory-v2-plan-batch`；每个构建入口通过 cargo metadata 核对实际 target。浏览器和测试服务均使用本任务拥有的进程与端口。

## 独立调用计数与退休引用

同一 harness 热身后的 WASM→JS 导入调用总数：

| 操作 | 基线 | 原型 | 原型提交段数 | Dioxus |
|---|---:|---:|---:|---:|
| 创建千行 | 5,034 | 154 | 20 | 10 |
| 替换千行 | 6,034 | 178 | 24 | 12 |
| 清空千行 | 1,034 | 58 | 4 | 10 |
| 创建万行 | 50,034 | 1,210 | 196 | 12 |

千行创建减少 96.9% 的导入调用，万行减少 97.6%。段数由有界的语义/字符串/节点预算决定；不会为了宣称一次刷新必定一次调用而取消预算。每段执行之外仍有对象表、返回结果、事件和帧等固定工作。

另一独立 GC 观察先保存万行根的 WeakRef，再清空并强制 GC 四次：基线和原型各有 0 个原行根存活，Dioxus 0.7.10 仍有 10,000 个原行根存活；三者文档中的行数都为零。探针不强持有节点，也不持有导入调用参数或返回对象。这个结果说明本轮清空后的宿主引用释放范围存在差异，不是总 RSS 或内存泄漏趋势的测量。

计数与引用证据：[counts evidence](/D:/BuildCache/glory-v2-plan-batch/counts-1793161785143/evidence.json)。它们使用最终候选 WASM 的同一 SHA-256，与正式计时分开运行。

## 正式前后与 Dioxus 对照

每格 48 个样本，单位 ms，为 click→两个 RAF 的中位数；变化列相对本轮基线，负数表示原型更快。

| 操作 | 基线 | 原型 | 变化 | Dioxus 0.7.10 |
|---|---:|---:|---:|---:|
| 创建千行 | 65.05 | 62.55 | −3.8% | 49.20 |
| 替换千行 | 67.75 | 69.45 | +2.5% | 50.60 |
| 更新每十行 | 27.40 | 28.40 | +3.6% | 33.30 |
| 选行 | 26.70 | 29.50 | +10.5% | 33.50 |
| 交换行 | 27.05 | 29.00 | +7.2% | 33.30 |
| 删除一行 | 26.95 | 27.90 | +3.5% | 33.40 |
| 追加千行 | 59.35 | 60.10 | +1.3% | 49.55 |
| 创建万行 | 525.70 | 499.25 | −5.0% | 339.45 |
| 清空千行 | 19.85 | 21.40 | +7.8% | 33.25 |

万行四轮基线→原型分别为 **515.25→487.95、532.05→494.15、527.20→516.35、525.60→514.85 ms**。方向一致，但收益从前两轮约 5–7% 缩到后两轮约 2%。

千行创建前三轮改善，第四轮 **65.45→79.10 ms** 回退；合并 p90 从 **74.50→83.72 ms**。替换第四轮也有 **65.50→86.65 ms** 的回退。所有这些慢样本保留，不能用合并中位数声称千行创建已稳定提速。计时前后 CPU 负载记录范围为 8–64%，还有不属于本任务的 dormant Cargo 进程；本轮是共享机器配对结果。

整轮 ScriptDuration 包含所有 setup、热身、动作和数据核对：四轮中位数基线 **10,580.76 ms**、原型 **10,101.73 ms**、Dioxus **3,344.00 ms**。原型合并降低约 4.5%，但最后一轮 **10,559.12→10,755.36 ms** 回退，因此这一指标也没有四轮一致的收益。它不能分摊为表中各操作的 CPU 时间。

这组 Dioxus 对照沿用原 benchmark 的父列表组件组织；没有在本轮重跑独立行组件版本。局部更新的表格差异同时受响应式组织和 RAF 相位影响；Dioxus 清空后的强引用保留范围也与 Glory 不同。不能把这张表当作所有应用或完整资源退休的统一排名，更不能把两 RAF 耗时换算成显示 FPS。

原型 raw WASM 为 **1,913,143 B**，相对基线 **1,845,213 B** 增加 **67,930 B（3.68%）**；生成 JS 合计 **72,781 B**，相对 **69,114 B** 增加 **3,667 B**。Dioxus raw WASM / JS 为 **457,458 / 55,831 B**，均来自本轮各自构建；没有运行 wasm-opt 或混入早期压缩产物。

汇总：[JSON](../benchmarks/plan-batch-results-2026-10-28.json)。完整正式样本和逐轮环境：[raw evidence](/D:/BuildCache/glory-v2-plan-batch/wall-1793163411424/evidence.json)。

## 实验后的取舍

这轮确认了完整生命周期的有界宿主协议在技术上可实现，也确认了成功前缀与现有输入协议可以保留。它没有确认足以覆盖体积和回退成本的整体性能收益，因此本轮停在原型。

后续若继续，优先分别测量短批次绕过、已连接移动提前选择标量路径、暂存日志/实例登记分配和退休准备成本；本轮没有进行这些原因的消融试验。现有每语义节点的实例、身份、两份 Memory 和属性写入仍保留，继续减少调用数不足以解释与 Dioxus 的全部差距。

DSL 的前端工作继续按[可行性调查](v2-dsl-and-host-batching-investigation-2026-10-28.md)独立评估。为同一 Plan 换一种语法不会自动消除上述成本；静态子树身份压缩仍需要先定义查询、交互、AX 与生命周期边界。

## 复跑入口

冻结与构建使用现有 `benchmarks/snapshot.py`、`benchmarks/build.ps1`。先归档未修改的 baseline 输入，以同一绝对 SourceRoot 构建 before 和 after，各用独立 BuildRoot；Dioxus 在 before 独立构建。`cargo metadata` 和应用 compiler artifact 的来源记录位于 [build audit](/D:/BuildCache/glory-v2-plan-batch/build-source-audit.json)。

准备好本轮目录布局后，可使用整理后的入口：

```powershell
node benchmarks/plan-batch-compare.cjs `
  --root D:/BuildCache/glory-v2-plan-batch `
  --out D:/BuildCache/glory-v2-plan-batch/comparison-new `
  --rounds 4
```

本轮实际执行的完整驱动快照与原始数据一起保存在上述 raw evidence 目录；仓库入口保留其页面内计时、整轮 CPU 读取、独立浏览器、正反顺序、绑核和逐样本数据摘要核对，删除诊断模式。该入口已另外完成一轮 before/after/Dioxus 复跑，额外 324 个样本仅验证复跑入口，没有并入主表。浏览器安装路径可通过 `--browser` 指定。

源码、构建和验证记录总索引：[evidence index](/D:/BuildCache/glory-v2-plan-batch/evidence-index.json)。
