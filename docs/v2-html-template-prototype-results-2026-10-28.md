# v2 稀疏 HTML 模板原型：实现与性能

日期：2026-10-28。分支 `v2`，HEAD／本次 fetch 后的 `origin/v2` 均为
`cb7ebdda33b51dd531ea33b51372112ca0d134`。修改尚在工作区。

后续已实现[具名手写 API 与 XML DSL](v2-view-api-and-xml-dsl-2026-10-28.md)。
本报告的“尚未实现 XML”和性能数字描述的是当时冻结的原型；新 API／DSL 已另行验收，未复用这些数字声称新性能结果。
随后[主 benchmark 已迁移并合并重复应用](benchmark-display-dsl-migration-2026-10-28.md)，下方旧命令属于历史版本。

**这个方向已经带来明显收益。** 保留相同响应图的直接 HTML 模板版，千行／万行创建
Script 中位数分别降低 **55.1%／67.3%**，万行两 RAF wall 降低 **31.3%**。
四轮中的每一轮、九项操作的 Script 中位数均优于本次 before。
这是受限浏览器显示原型的结果，不能据此宣布完整 Glory 运行时或全部平台已迁移。

## 1 已实现的变化

- [共享 HTML 声明](../crates/html/src/template.rs)：标签、静态文本、属性、动态目标路径是数据。
  当前支持 div/span/button、文本、有限普通属性、click 和子实例容器，拒绝其他 profile。
- [实例与绑定](../crates/web/src/template.rs)：每个 Scope 一个模板实例，记录模板、根归属、
  动态槽、处理器和绑定。原生 HTML 标签不调用 Glory Button/View builder，
  不经过逐元素 UI Memory、Mapper Memory 或浏览器语义 Entry。
- [浏览器执行器](../crates/web/src/template_host.js)：冷声明建立原型，后续深克隆；
  JS 只索引实例根和声明目标，委托 click；创建、字段、挂接和退休进入同一顺序操作流。
- 保留 Cage/Bond/Vary。`bind` 支持常量一次初始化与响应值；
  `bind_values` 用一个纯观察者计算一组字段，仅发送变化的槽。
- [当时的 benchmark 冻结源码](D:/BuildCache/glory-html-template-20261028/verified/inputs/benchmarks/html-template/src/main.rs)复用旧版本的 Generator、行标签 Cage、
  selected Cage、操作和 `glory_ui::list::Cache` keyed/LIS 协调；没有改成虚拟列表。
  用同一 Cache 的新 RowHost 直接挂接模板实例，跳过旧语义 Widget/App 构造。

普通版每行仍是 label Cage、selected 派生 Bond、class Binding、label Binding。
`grouped` 实验版改为 label Cage 和一个 class/label 联合 Binding；这是另一项变化，单独计量，
没有默认启用。XML 宏尚未实现，这一阶段手写模板数据，用来先验证实例表示的收益。

## 2 万行现场：减少的是框架记录

| 项目 | before：本轮开始时的 v2 | 模板版 sparse | 合并绑定 grouped |
|---|---:|---:|---:|
| UI 语义节点 | 80010 | 不建立 | 不建立 |
| Mapper 逐语义节点 Memory／浏览器 Entry | 各 80010，结构与 UI 对应 | 不建立 | 不建立 |
| 模板实例／Rust 活记录 | 旧逐节点路径 | 10001 | 10001 |
| JS 动态／事件／容器目标引用 | 旧逐节点路径 | 50007 | 50007 |
| 响应图节点 | 40003 | 40003 | 20003 |
| Core Owner | 10013 | 10002 | 10002 |
| 真实元素 | 80008 | 80008 | 80008 |
| 真实 Text 节点 | 30006 | 30006 | 30006 |
| 注释节点 | 4 | 0 | 0 |
| 属性数量 | 320037 | 140015 | 140015 |

新路径每行一实例、五个目标：ID 初始化文本、行 class、label 文本、两个 click。
表头与列表容器额外是一实例、七目标。不是为每个静态 cell/span 建立身份。
记录表只覆盖当前活实例；模板缓存最多 64 个声明，不按业务 key 保存历史实例。

新 stats 中的 `semantic_nodes:0` 是 profile 声明，并非从旧 UI App 读取的计数；
上述“不建立”来自入口和结构审计。`instances/targets` 则根据 JS 成功前缀更新，
`records` 是 Rust 当前表长度。各项是记录数量，不是活堆字节或 RSS。
DOM census 的元素、文本、属性数量来自真实浏览器；新版本也与 Dioxus 的这些数量一致。

独立密度检查实际生成 **2／201 个物理节点**，两者均只有 **1 个实例、2 个目标**。
Rust 侧的两份模板也保持一条实例记录；Unicode 更新、静态后代身份和清理同时验证。
共享原型和真实静态节点依然有成本，这个检查不表示整棵 DOM 内存不增长。

## 3 同批性能

本次 before 是开始任务时的工作区快照，**已包含上一轮提交准备优化**，不是未修改的远端提交。
Dioxus 使用同一锁文件中的 0.7.10，独立重建；不使用 sibling 开发仓库的另一版本。

Chrome 155.0.8059.86，Intel Core Ultra 9 285H，Windows，1280×900、DPR 1、headless、
`--disable-gpu`。本任务拥有的浏览器进程设置 affinity 65535、AboveNormal；
每个版本每轮使用新浏览器，四轮按正序／逆序交替。
每操作三次热身、十二次正式样本，四版本九操作共 **1728 样本**。
所有样本均保留；测量边界前后核实没有 cargo/rustc 构建。未做显著性检验。

表格为各版本每操作 **48 个样本的合并中位数**。Script 是逐操作 CDP ScriptDuration 增量，ms。
百分比相对本次 before，负值为耗时降低。

| 操作 | before | sparse | grouped | Dioxus | sparse 变化 | grouped 变化 |
|---|---:|---:|---:|---:|---:|---:|
| 创建千行 | 17.643 | 7.921 | 6.869 | 4.989 | -55.1% | -61.1% |
| 替换千行 | 27.581 | 10.190 | 9.894 | 8.891 | -63.1% | -64.1% |
| 每十行更新 | 0.837 | 0.533 | 0.548 | 1.122 | -36.3% | -34.5% |
| 选行 | 1.075 | 0.566 | 0.557 | 1.008 | -47.3% | -48.1% |
| 交换 | 0.665 | 0.385 | 0.377 | 1.274 | -42.2% | -43.3% |
| 删除一行 | 0.619 | 0.329 | 0.320 | 1.073 | -46.8% | -48.2% |
| 追加千行 | 17.001 | 5.497 | 5.166 | 5.417 | -67.7% | -69.6% |
| 创建万行 | 160.844 | 52.584 | 50.487 | 41.707 | -67.3% | -68.6% |
| 清空千行 | 18.987 | 4.970 | 6.657 | 3.421 | -73.8% | -64.9% |

两 RAF wall 中位数，ms；它不是屏幕呈现延迟或 FPS。

| 操作 | before | sparse | grouped | Dioxus | sparse 变化 |
|---|---:|---:|---:|---:|---:|
| 创建千行 | 56.80 | 47.25 | 47.70 | 41.50 | -16.8% |
| 替换千行 | 60.95 | 44.15 | 42.20 | 35.10 | -27.6% |
| 每十行更新 | 22.30 | 24.90 | 25.60 | 23.65 | +11.7% |
| 选行 | 24.70 | 25.15 | 23.80 | 22.55 | +1.8% |
| 交换 | 21.45 | 24.25 | 24.65 | 23.20 | +13.1% |
| 删除一行 | 24.55 | 24.10 | 23.65 | 21.55 | -1.8% |
| 追加千行 | 52.75 | 40.20 | 40.25 | 40.65 | -23.8% |
| 创建万行 | 427.25 | 293.45 | 306.15 | 298.05 | -31.3% |
| 清空千行 | 28.05 | 20.80 | 24.60 | 21.70 | -25.8% |

每轮万行 wall 中位数如下。虽然合并中位数中 sparse 略低于 Dioxus，但单轮比较只有一轮更低，
**不能据此声称万行 wall 已稳定超过 Dioxus**。布局、GC 和帧边界分布需要一起看。

| 轮次 | before | sparse | grouped | Dioxus |
|---|---:|---:|---:|---:|
| 1 | 430.55 | 303.30 | 307.30 | 280.50 |
| 2 | 408.00 | 298.35 | 303.45 | 278.80 |
| 3 | 405.65 | 298.90 | 306.60 | 274.10 |
| 4 | 437.20 | 285.65 | 291.05 | 306.40 |

保留的不利结果：

- sparse 的更新／交换 wall 分别增加 11.7%／13.1%，不能把 Script 下降解释为所有交互都更快。
- grouped 相对 sparse 的清空 Script 增加约 33.9%，四轮里有三轮更慢；
  万行 wall 也更高。观察者数少一半并不保证每项更快，暂时保留为 opt-in 实验。
- 万行创建 Script 与 Dioxus 的差距由约 3.86 倍缩到 1.26 倍，仍慢约 26.1%；
  千行创建、替换、清空也仍有差距。小更新的 Script 优势不代表完整框架排名。
- 这里只测热身后的操作；首次模板建立通过功能验收，但未单独给出冷启动／首次加载性能结论。

## 4 这说明什么

**主要收益先来自实例与提交路径的改变，绑定合并是次级变量。** sparse 和 before 都有
40003 个响应图节点，已经得到上述大幅改善。它同时省掉重复 Memory/Entry 处理、
逐静态元素登记、框架身份属性和旧 UI／Mapper 初始化与退休路径。
这不是只删一张表的单变量实验，不能把 67.3% 进一步精确拆给其中某一项。

grouped 将图节点减到 20003，万行 Script 相对 sparse 只再降低约 4.0%，且有回退。
下一步 XML 前端应生成这条已验证的模板／槽路径；不能展开回旧的完整逐元素 Plan。
与 Dioxus 相同的启发是共享模板、实例只定位必要目标；Glory 继续用直接依赖触发字段更新。
具体发布源码对照见[架构与 Dioxus 审计](v2-html-template-runtime-architecture-2026-10-28.md)。

## 5 提交、归属与故障契约

实例只能挂在 Scope 父实例的 Children 目标内，根 Scope 挂到文档根；该 profile 不支持跨 Owner 重挂。
一 Scope 一实例，事件回调只捕获 WeakApp／Instance 等弱句柄；避免捕获 App 形成业务循环。
实例 ID 在文档内递增且不复用，Core Scope 另有 generational 身份；拒绝外来、退休句柄与错误目标。
普通字符串仅作为文本或受限属性值；不支持 HTML 注入、URL 加载属性或编辑器属性协议。

事件只在原生监听器中排队，下一次 RAF 执行回调、Core flush、host 提交。
直接调用 set/place/mount 后可显式 `App::flush`；它们本身不承诺自动启动一次观察。
每包最多 256 操作、4096 创建／退休节点权重、64 KiB 字段 UTF-8 字符串；
声明最多 256 节点／目标、深度 64；待提交操作和事件分别限制为 1048576／4096。
字段位置以 UTF-16 单元编码，JS 在执行前复制借用的 WASM word view，防止 native hook 的 memory.grow 使其失效。

JS 返回完成的操作前缀。Rust 只据此前缀结算宿主实例／目标计数；失败后文档 fault，
不重试后缀、不继续接受事件。Rust desired records／业务值可能已前移，
与已创建但尚未挂接的宿主实例数量分别报告。失败操作自身可能已经产生部分原生副作用，
不声称具有事务回滚。该原型没有旧语义协议完整的 receipt/query 恢复能力；恢复方式是关闭并重建文档。
RAF 的错误保存在文档中，后续显式 flush 返回错误；不在宿主调用内重入业务。

`App::close` 取消 RAF、解绑监听器、清空宿主表和模板、退休 Scope；原生清理失败会返回错误。
close 后存活的 App 仍有一个 Core Runtime 根壳，实例／目标／记录／图节点为零；
最后 App drop 后 Runtime 和根壳释放。不要把测试中 close 后的一个根 Owner 误判为行泄漏。

## 6 验证与体积

五个相关 crate 的 all-feature Rust 单元／集成／文档测试 **600 项通过**；
相关 crate 的 wasm all-feature/all-target Clippy，以及两个实验配置和验收配置的独立 Clippy 均通过。
新路径另通过[浏览器专项验收冻结版本](D:/BuildCache/glory-html-template-20261028/paired2/driver-source/html-template-acceptance.js)：

- 与旧 v2／Dioxus 相同的 seeded 数据、顺序、事件结果、完整后代元素／Text 身份、万行布局盒、六处截图。
  旧版本的语义身份检查仍保留；新 profile 验证自己的实例／目标清理，不伪装成旧 API 等价。
- 原子移动和 insertBefore 回退都保持焦点、文字节点及选区。实测发现 Chromium moveBefore
  仍会重置 DOM range，已补上两条路径的选区恢复。
- 静态密度增长不增加实例目标；Unicode／非 BMP 文本、无效槽、跨文档／跨 Owner、
  重复挂载、错误绑定、超大字段、旧句柄、反复子 Scope 创建／清理和根重新认领。
- 批量创建时主动 memory.grow，重入 flush 被拒绝，千行仍完整提交。
- 第 300 次创建失败：已创建 299 行、尚未挂接，连同面板计为 300 个实例；
  第 300 次挂接失败：299 行已连接，1000 行实例已创建，后续不执行。
  第 20 次字段失败仅前 19 项生效；第 17 次退休失败仅退休 16 行。
  四种故障都验证前缀计数、停止后缀和可清理。
- 清空后等待渲染生命周期，再执行四次强制 GC，旧行根和 Text 的 WeakRef 均消失。

不依赖源码推测来宣称新输入法、受控 input/textarea、property 版本、AX 桥、查询、
SSR/hydration/stream 或 Blitz native 已完成。现有语义控件路径继续存在；新 profile 尚未接入这些能力。
XML／组件 Props 编译前端也仍待实现。

独立 `opt-level=3`、LTO、单 codegen-unit 的当前行应用产物：

| 版本 | WASM B | JS B，含各自执行器、不含共同 harness |
|---|---:|---:|
| before | 1908161 | 68558 |
| sparse | 353446 | 33236 |
| grouped | 344847 | 33236 |
| Dioxus | 456622 | 51608 |

小体积来自新受限入口没有拉入完整语义组件／Mapper／控制器运行时等代码；
不代表保留全部旧能力后仍有同样缩减。使用当前批次的产物，不拼接历史不同路径的体积。

## 7 证据和复现

- [仓库结果汇总](../benchmarks/html-template-results-2026-10-28.json)：四轮中位数、方向、范围、所有验收快照及产物 SHA。
- [1728 个原始样本、环境与冻结输入](D:/BuildCache/glory-html-template-20261028/paired2/evidence.json)。
- [专项密度／故障／回收验收](D:/BuildCache/glory-html-template-20261028/verified-acceptance/evidence.json)。
- [before 输入](D:/BuildCache/glory-html-template-20261028/before/inputs/inputs.json)、
  [新路径输入](D:/BuildCache/glory-html-template-20261028/verified/inputs/inputs.json)。
  对应独立 target；`verified-probes` 的 acceptance-only Rust 导出不进入测量产物。
- [Rust 日志](D:/BuildCache/glory-html-template-20261028/runtime-tests.txt)、
  [WASM Clippy](D:/BuildCache/glory-html-template-20261028/runtime-clippy.txt)。

`smoke`、`preflight`、`focus-debug`、`sparse-debug-prefix` 等先前诊断目录保留，不混入正式样本。
初版验收曾错误地把 close 后的 Runtime 根壳要求为零、把创建前缀当成已经挂接的行，均已按契约修正；
正式计时前即完成修正。`paired1` 是驱动根路径检查失败，未产生计时样本；正式数据只有 `paired2`。

以下为当时冻结版本的命令；当前主入口和 flags 见[迁移报告](benchmark-display-dsl-migration-2026-10-28.md)。
当时冻结输入后使用 `benchmarks/build.ps1 -Apps html-template`；
`-Grouped` 和 `-Probes` 分别在另外的 BuildRoot 构建。所有编译、Cargo cache、临时文件位于 D:。
驱动目录布局为 before、verified、verified-grouped、verified-probes，各包含 site；
设置 `GLORY_TEMPLATE_BUILD_ROOT`／`GLORY_REPO` 可更改驱动根与仓库位置。

```powershell
node benchmarks/html-template-compare.cjs new-run 4 before,sparse,grouped,dioxus
node benchmarks/html-template-compare.cjs new-acceptance 0 probes,sparse,grouped
python benchmarks/html-template-summary.py D:/BuildCache/glory-html-template-20261028/new-run/evidence.json --out D:/BuildCache/glory-html-template-20261028/new-run/summary.json
```

本地标准 benchmark 的原有对照是 Glory／Leptos／Dioxus，本次新增模板变体；没有加入 Makepad。
本次检查本地 `js-framework-benchmark/frameworks` 也未发现 Makepad 实现；这不是对其远端未来版本的断言。
当前测试仍是仓库自己的完整 div/role 行工作负载，不是官方 js-framework-benchmark 分数。
