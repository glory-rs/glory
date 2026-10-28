# v2 显示 API、DSL 与 benchmark 实施记录

实施日期：2026-10-28 至 2026-10-29。分支 `v2`；远端/本地起点 `cb7ebdd`。
本轮起点包含之前尚未提交的稀疏显示宿主和 DSL，不能把远端 SHA 当作实际 before。
可用接口见[指南](v2-display-keyed-content-guide-2026-10-28.md)，原始目标见
[实施计划](v2-display-completion-plan-2026-10-28.md)。

最终保留版本已完成 API/DSL、两套 benchmark 迁移与完整测试。
千行创建 Script 改善 21.4%，标准表格交换行 total 从 107.0 降至 42.7 ms。
仍有替换、局部更新、万行与部分 wall 回退，预设性能目标未全部达到。
当前结果在本文“最终保留版本”和“标准 HTML-table suite”两节；前面的实验结果单独保留。

## 已实现的应用层

- `KeyedList<K,T>` 只保存一份业务值，顺序通知与行值通知分开。成员身份包括 arena slot
  和 generation；移动、同 key 值替换保留身份，删除后重插使旧句柄失效。
  字段别名属于创建它的 Owner，多视图可以共享数据并独立卸载。
- 手写 `view(...).child(outlet, each(rows, factory))` 管理行 Scope、挂接、移动和退休。
  应用不再实现 RowHost、不维护第二份 label 表，也不捕获 WeakApp 做列表胶水。
  直接复用旧 `ui::Each` 的 Cache 协调，显示宿主与语义 Widget 宿主各自适配。
- XML 使用 `<Each>`，统一手写 `each` 和类型 `display::Each` 的概念；旧的 `ui::Each`
  仍通过命名空间区分。没有增加内置 `For` 同义名。
- `<Show>` 隐藏时退休分支，重新进入创建新状态；兄弟组件不重置。
  `<Content value={...}/>`、Option/Vec、片段、组件 children 使用真实内容范围，支持多根内容。
  `{expr}` 保持文本语义，不推断任意 Rust 返回值是否是视图。
- `field!(row.label)` 生成固定字段名与读写投影；不复制标签值到另一棵数据树。
  `class:danger={computed(...)}` 使用布尔槽，初值相同则省略写入，不反复构造完整 class 字符串。
- 可选 `#[component]` 生成 Props 与 Component 实现；具体同步 Props 继续由 Rust 检查。
  `display::prelude::*` 与 `App::launch` 减少根挂载样板。
  [普通计数器/列表示例](../examples/display-web/src/bin/counter.rs)包含组件 children、条件和列表。
- 具名模板增加本 workload 所需的 a/h1/table/tbody/tr/td、静态 id 和模板内表格层级检查。
  小写 HTML 不建立对应的 Glory Button/View Widget，不为每个静态元素建立响应式实例。

当前主 benchmark 的 XML 前端见
[xml.rs](../benchmarks/glory/src/browser/xml.rs)，手写前端见
[handwritten.rs](../benchmarks/glory/src/browser/handwritten.rs)，二者共用
[业务模型](../benchmarks/glory/src/model.rs)。统计、接受性导出和探针独立放置；正式计时不编入故障探针。
原独立 html-template 应用保持移除，历史源码/结果继续冻结。

## 实现成本与优化

先独立测量了公共 API 的初版，未把语法变短等同于提速。初版 HashMap 成员存储及中间行别名
增加成本，相关失败实验保留在 `first-api-trial`、`direct-fields-trial`、`arena-trial`。
最终成员值使用 core 已有的 generational Arena；字段直接投影，避免额外响应式行别名。
字段路径和别名表的零/单元素情形用内联存储，避免每个字段都分配一棵单节点 BTree。

宿主按 JavaScript turn 用 microtask 合并更新，移除固定等待 RAF 的延迟；显式 flush 保留重入拒绝。
文字使用共享暂存区，命令记录保存 UTF-8 区间及 UTF-16 长度，分包复用缓冲。
每包仍受 256 记录、4096 创建/退休物理节点、64 KiB 文字预算限制；超预算刷新分包。
失败只确认成功前缀，停止后缀，关闭并重建；不声称任意业务 panic 或 DOM 异常可事务回滚。

V8 精确 JS coverage 的独立调用计数（不参与正式计时）如下：

| 操作 | before 提交包 | 候选提交包 |
|---|---:|---:|
| 创建万行 | 196 | 157 |
| 更新千行中的每十行 | 1 | 1 |
| 清空千行 | 4 | 4 |

包数只能说明协议工作量，不能换算成同等比例耗时收益。物理 DOM 仍然存在，布局/样式/GC 仍由浏览器执行。

## 正确性与工具检查

本轮最终工作区 Rust 测试通过 **1114 项**，**2 项忽略**；Native/WASM 全工作区、全 feature、
全 target Clippy 均以 `-D warnings` 通过。引擎 fork 审计通过：3 个不可变 Git fork、
18 个引擎/平台包保持单一类型身份；13 个 Glory workspace crate，默认/Web 图不引入 DSL 编译器。
26 项真实下游编译诊断包含重命名 facade、组件 Scope 类型、Each/Show、字段和布尔 class 误用。

浏览器验收包括手写/XML 相同表现、多视图/多根范围、稳定后代身份、局部字段不扫描结构、
同 key 替换不重新运行工厂、删除重插重置状态、条件分支重置、反复清空后的活计数、
关闭/fault 清理、GC、焦点/选区、原生和 fallback 移动、内存增长与提交重入。
另外覆盖大 Unicode 标签跨文字分包、布尔 class 和 create/place/write/retire 的故障前缀。
普通 counter 示例也经过增删改、选择、分支和身份检查。

工作区初次全测有两项原有 Blitz 测试失败，已修正测试构造并重新全测：Windows 的 NUL 文件 URI
使用平台有效 file URI 后附 `%00`，确保检验确实到达元数据拒绝；异步权限测试用有界轮询等待，
避免假设 8 ms 内定时器必定完成。没有改变相应运行时行为。另保留了探针 feature 未启用及
独立绑定顺序假设造成的失败验收记录；正式通过的产物检查了实际 feature/导出。

## 两套测量的边界

自带 suite 保留 Glory/Leptos/Dioxus，不加入 React/Solid。五个比较产物是本轮前的真实 DSL、
新 DSL、等价手写、Dioxus 0.7.10、Leptos 0.8.22。固定 seeded 业务 oracle、相同行结构与 CSS；
每项在 fresh browser 中 3 次热身、12 正式样本，正反交替四轮，共 2160 个正式样本。
计时前核验完整数据、后代身份、万行几何和六处相同截图；所有慢样本保留。

js-framework-benchmark 使用独立的标准 table/tr/td fixture 和标准 Math.random 标签生成。
测试 Glory、Leptos、Dioxus、React hooks、Solid、Vue、Svelte、vanilla JS 八个 keyed 实现。
采用当前 checkout 启用的九个 CPU、三个 memory 和 size 组的三个结果，不启用上游已注释的旧用例。
使用上游默认样本数、CPU 降速和 runner，不用 smoketest 或 `--count` 缩短正式测试。
CPU 默认每项 15 个样本，选行按上游再加 10；memory/size 每项 1 个。后者不足以判断稳定的内存/启动排名。

两套 workload、采样方法和 CPU 降速不同，不拼接成一个总排名。版本来自实际锁定依赖，
不宣称为网上最新版本。当前 checkout 未包含 Makepad fixture，本轮未添加。

## 公共 API 首轮候选：保留的优化前四轮结果

[该阶段汇总](../benchmarks/display-keyed-api-stage-results-2026-10-29.json)；原始全部样本在 `paired-final/evidence.json`。
Script 为 CDP ScriptDuration 增量中位数，单位 ms；每版本/操作 48 个样本。

| 操作 | before DSL | 新 DSL | 新手写 | Dioxus | Leptos fixture | 新 DSL 变化 |
|---|---:|---:|---:|---:|---:|---:|
| create_1k | 7.574 | 6.158 | 6.809 | 4.917 | 15.748 | -18.7% |
| replace_1k | 9.778 | 11.093 | 10.253 | 8.557 | 20.116 | +13.4% |
| update_10th | 0.505 | 0.537 | 0.525 | 1.158 | 0.597 | +6.3% |
| select_row | 0.450 | 0.346 | 0.324 | 1.018 | 176.847 | -23.2% |
| swap_rows | 0.431 | 0.413 | 0.405 | 1.251 | 0.246 | -4.3% |
| remove_row | 0.349 | 0.365 | 0.377 | 1.079 | 0.540 | +4.7% |
| append_1k | 5.595 | 5.530 | 5.595 | 5.454 | 14.732 | -1.2% |
| create_10k | 55.349 | 53.813 | 56.834 | 41.993 | 161.204 | -2.8% |
| clear_1k | 5.305 | 4.983 | 5.346 | 3.190 | 21.202 | -6.1% |

千行创建和选行四轮均更快，万行创建三轮更快；替换和每十行更新四轮均更慢。
后两项超过原计划 5%/三轮的剖析触发线，已独立采样。替换的公共成员/字段管理仍有
额外成本；采样包含注册、字段路径树操作、Owner 退休及 DOM remove，不足以精确分摊单项耗时。
随后尝试单订阅者内联，core 回归及真实应用验收通过，但独立一轮未建立收益，已撤回。
`subscribers/inputs` 与 `subscribers-trial` 保留该实验，没有代替正式四轮结论。

两 RAF wall 中位数如下：

| 操作 | before ms | 新 DSL ms | 变化 |
|---|---:|---:|---:|
| create_1k | 46.00 | 41.65 | -9.5% |
| replace_1k | 42.40 | 39.15 | -7.7% |
| update_10th | 25.80 | 22.35 | -13.4% |
| select_row | 25.05 | 21.55 | -14.0% |
| swap_rows | 24.85 | 23.60 | -5.0% |
| remove_row | 23.80 | 23.35 | -1.9% |
| append_1k | 40.75 | 38.20 | -6.3% |
| create_10k | 317.40 | 309.30 | -2.6% |
| clear_1k | 25.10 | 20.45 | -18.5% |

全部 wall 中位数更低；microtask 去掉固定 RAF 等待是相关实现变化，不能全归因于 Rust CPU 变快。
**未达到**创建千行/万行/清空均进入 Dioxus 1.10 倍以内的工程目标。公共 API 与验证交付完成，
性能目标仍有缺口，不能宣布整体性能问题已解决。

Leptos 自带 fixture 的选行出现已知的长操作历史敏感现象（约 177 ms），不推广为框架的一般选行性能。
标准 suite 的 Leptos 使用其原生 Selector 写法，结果另列。

标准 fixture 也揭示了具体的更新策略差别：Dioxus 的 `use_set_compare_equal` 与 Leptos 的
`Selector::selected` 按选中 key 通知，Glory 当前的 `computed(model.is_selected(id))`
仍让全部行比较 selected，再跳过未改变的布尔 DOM 写入。减少 DOM 命令不等于消除响应图广播。
这是剩余选行成本的明确来源；本轮未用手写应用级选中行索引绕过公共 API 来改善成绩。

新 DSL/手写 WASM 为 460930/460451 B，before 为 363892 B；Dioxus/Leptos 为 456622/278790 B。
公共集合与结构 API 增加可达代码，WASM 增长 97038 B；JS 为 33528 B，before 为 33114 B。
不与覆盖完整平台能力的通用包直接排名。图 baseline 3→5，万行仍每行三个活节点（30003→30005）。
万行实例/动态目标为 10001/50007；清空不保留活行/投影/事件，arena 容量按峰值复用。

## BindingId 记录缩减阶段：保留的四轮结果

[该阶段汇总](../benchmarks/display-record-trim-results-2026-10-29.json)；原始样本 `delivered-final/evidence.json`。
下表是曾包含两项改动的独立完整四轮，不能与上节样本混池。Script 中位数，单位 ms。

| 操作 | before DSL | 最终 DSL | 最终手写 | Dioxus | Leptos fixture | DSL 变化 | 更快轮数 |
|---|---:|---:|---:|---:|---:|---:|---:|
| create_1k | 7.205 | 6.454 | 6.178 | 4.890 | 16.839 | -10.4% | 3/4 |
| replace_1k | 10.077 | 10.195 | 10.478 | 8.538 | 19.807 | +1.2% | 1/4 |
| update_10th | 0.517 | 0.538 | 0.512 | 1.252 | 0.613 | +4.1% | 1/4 |
| select_row | 0.428 | 0.354 | 0.334 | 1.021 | 175.480 | -17.4% | 4/4 |
| swap_rows | 0.403 | 0.364 | 0.414 | 1.229 | 0.235 | -9.8% | 4/4 |
| remove_row | 0.378 | 0.370 | 0.373 | 1.084 | 0.517 | -2.1% | 2/4 |
| append_1k | 5.607 | 5.698 | 5.536 | 6.098 | 14.880 | +1.6% | 2/4 |
| create_10k | 52.813 | 53.254 | 52.700 | 45.526 | 153.780 | +0.8% | 3/4 |
| clear_1k | 5.284 | 5.049 | 4.936 | 3.271 | 21.500 | -4.4% | 2/4 |

两 RAF wall 中位数：

| 操作 | before ms | 最终 DSL ms | 变化 |
|---|---:|---:|---:|
| create_1k | 47.80 | 40.00 | -16.3% |
| replace_1k | 44.20 | 39.35 | -11.0% |
| update_10th | 25.20 | 22.45 | -10.9% |
| select_row | 25.85 | 22.50 | -13.0% |
| swap_rows | 22.70 | 23.05 | +1.5% |
| remove_row | 23.55 | 22.50 | -4.5% |
| append_1k | 40.80 | 39.90 | -2.2% |
| create_10k | 316.55 | 305.60 | -3.5% |
| clear_1k | 20.40 | 21.80 | +6.9% |

该实验移除显示实例重复保存的 BindingId Vec：所有绑定已由同一 core Owner 拥有，
Owner 清理回调才退休显示实例，因此无需再次遍历并 dispose 同一批绑定。
它减少每行实例记录和分配，不改变图节点数、数据通知或 DOM 操作数量；但最终未保留。
该阶段的独立验收与初测保存在 `record-trim-acceptance` 和 `record-trim-trial`。

连续移动段只在开始时读取一次选区端点，避免在前一次 DOM 移动之后再次查询选区。
其他命令和分包均切断快照；每次相关移动后仍恢复焦点/选区。
新增验收覆盖两种移动 API、两个被交换行中的任一个持有焦点/选区，并核验每段仅查询一次。
故障前缀、内存增长、GC、完整身份和截图也继续通过。

表格隔离诊断使用同一旧标准 WASM，只替换宿主 JS，每模式两轮共 24 样本，4x CPU 降速。
原策略/快照策略 wall 中位数为 88.18/74.61 ms，CDP ScriptDuration 为 3.154/2.512 ms。
另外直接替换 `moveBefore` 为 `insertBefore` 未获收益，故未改移动 API 策略。
这组诊断与正式 suite 分开；标准 suite 的 Script 是 trace 脚本事件区间，其内部可包含同步布局。
不能把它当作自带 suite 的 CDP ScriptDuration，也不能将 Script 与 paint 相加。

## 各阶段证据目录与复现

本轮构建和原始证据根目录为 `D:/BuildCache/glory-display-completion-20261028`：

- `before/inputs`：本轮实际起点的 528 个冻结输入。
- `verified/inputs`：初版公共 API 候选的 542 个冻结输入，含 SHA-256；`completed/site`、`handwritten/site`、
  `probes/site` 分别是独立 feature 产物。
- `fixture/acceptance-final-full/evidence.json`：普通例子和结构/分包验收；
  `acceptance-complete-final/evidence.json`：六个产物的共同验收及专项探针。
- `workspace-verified.txt`、`clippy-native-verified.txt`、`clippy-wasm-verified.txt`、
  `engine-forks-verified.txt`：完整检查日志；旧失败日志不覆盖。
- `submit-counts/evidence.json`：独立精确调用计数。

构建自带 suite 使用 `benchmarks/build.ps1`，分别显式传 `-Handwritten`、`-Probes`；
不要把 switch 字符串数组当作 PowerShell 命名参数 splat。使用 `compare.cjs` 的目录布局和环境变量
选择独立的基线/候选产物，再用 `summarize.py` 汇总原始样本。
标准 fixture 使用 `scripts/build-js-framework-benchmark.ps1`，peer JS 应用按其锁文件构建，
启动上游 8080 server 后运行 `benchmarks/run-standard.py --out <新的 D: 目录>`。

新显示 API 的 SSR/hydration、Blitz 适配、完整 HTML/property/事件、受控输入/IME/选区等平台迁移
仍属于后续能力工作。本 benchmark 的通过不能代表这些能力已迁移。

## 实例记录实验的撤回依据

固定相同的新宿主 JS，只改变 Rust 绑定记录，正反两轮、每版 24 个样本、4x CPU 降速、相同八次清空热身后：
旧记录/缩减记录的清空 ScriptDuration 中位数为 44.293/50.341 ms，wall 为 48.95/56.00 ms。
两版都清空到零行，全部生命周期验收通过；记录更少没有带来更低耗时。原因未精确归因，
因此恢复原实例记录，保留已验证的选区快照。原始数据在 `table-clear-record-diagnostic`。


## 最终保留版本：四轮 2160 样本

[当前完整汇总](../benchmarks/display-keyed-results-2026-10-29.json)；全部样本在 `selection-only-final/evidence.json`。
保留版本恢复原实例记录，只保留选区快照及先前已验收的 API/宿主优化。下表为独立完整四轮，单位 ms。

| 操作 | before DSL | 当前 DSL | 当前手写 | Dioxus | Leptos fixture | DSL Script 变化 | 更快轮数 |
|---|---:|---:|---:|---:|---:|---:|---:|
| create_1k | 7.726 | 6.073 | 6.136 | 4.649 | 15.691 | -21.4% | 4/4 |
| replace_1k | 10.343 | 10.771 | 10.444 | 9.000 | 19.169 | +4.1% | 2/4 |
| update_10th | 0.511 | 0.534 | 0.524 | 1.281 | 0.600 | +4.7% | 2/4 |
| select_row | 0.403 | 0.319 | 0.331 | 1.034 | 179.854 | -20.8% | 4/4 |
| swap_rows | 0.428 | 0.407 | 0.393 | 1.259 | 0.255 | -4.9% | 3/4 |
| remove_row | 0.372 | 0.369 | 0.367 | 1.075 | 0.556 | -0.9% | 2/4 |
| append_1k | 5.464 | 5.411 | 5.500 | 5.914 | 14.598 | -1.0% | 3/4 |
| create_10k | 51.854 | 52.501 | 51.961 | 41.752 | 156.617 | +1.2% | 3/4 |
| clear_1k | 6.236 | 5.498 | 5.563 | 3.142 | 20.530 | -11.8% | 2/4 |

两 RAF wall 中位数：

| 操作 | before ms | 当前 DSL ms | 变化 |
|---|---:|---:|---:|
| create_1k | 46.60 | 39.65 | -14.9% |
| replace_1k | 43.55 | 38.15 | -12.4% |
| update_10th | 25.05 | 22.40 | -10.6% |
| select_row | 22.70 | 19.70 | -13.2% |
| swap_rows | 22.10 | 22.85 | +3.4% |
| remove_row | 24.95 | 23.20 | -7.0% |
| append_1k | 41.85 | 34.70 | -17.1% |
| create_10k | 304.10 | 294.50 | -3.2% |
| clear_1k | 21.80 | 20.90 | -4.1% |

千行创建/选行/清空的 Script 合并中位数改善 21.4%/20.8%/11.8%；替换和局部更新仍慢 4.1%/4.7%。
万行合并中位数慢 1.2%，交换的 wall 慢 3.4%；所有慢样本保留，不宣称每项都提速。
未全部达到原计划相对 Dioxus 1.10 倍的工程目标。后续重点仍是成员/字段登记退休和按 key 通知选择状态。
当前 DSL WASM 为 460910 B，before 为 363892 B；JS 34020 B，before 33114 B，功能扩展的体积成本继续保留。

## 标准 HTML-table suite：八个框架完整结果

[全部 120 个结果文件及原始值](../benchmarks/standard-results-2026-10-29.json)。
Intel Core Ultra 9 285H / Windows 11 Pro / Chrome 155.0.8059.86 / Node 24.18.0 / Rust 1.98.0。
Glory v2、Leptos 0.8.22、Dioxus 0.7.10、React hooks 19.2.0、Solid 1.9.3、Vue 3.5.39、Svelte 5.42.1 和 vanilla JS。

采集分阶段：七个未修改框架保留首次完整运行，Glory 在通用宿主改动后全部 15 项重跑；
不挑选较快项目替换。peer/harness 输入逐文件验证未变，最终冻结目录为 `official-selection-delivered`。
因此这是分阶段本机观测，不称为八个框架同批随机交替的统计排名。

首次完整运行五项遇到 Chrome trace 缺 commit，经六次完整项目复核补齐；最终 Glory 一项又发生同类错误，
整项复核后补齐。失败日志、初次结果和每次 trace 仍保留，成功项目和慢样本没有替换。

CPU total 中位数（ms）

| 项目 | glory | leptos | dioxus | react-hooks | solid | vue | svelte | vanillajs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 01_run1k | 45.50 | 48.80 | 45.10 | 48.60 | 47.00 | 46.00 | 48.30 | 41.90 |
| 02_replace1k | 55.00 | 55.80 | 52.60 | 58.60 | 47.30 | 51.70 | 48.30 | 44.70 |
| 03_update10th1k_x16 | 54.80 | 60.70 | 58.30 | 50.40 | 43.90 | 47.40 | 39.20 | 39.00 |
| 04_select1k | 24.60 | 21.80 | 17.60 | 20.00 | 16.70 | 15.50 | 21.40 | 18.40 |
| 05_swap1k | 42.70 | 41.60 | 72.40 | 264.80 | 44.50 | 51.00 | 56.70 | 53.10 |
| 06_remove-one-1k | 31.00 | 32.20 | 33.90 | 31.20 | 33.50 | 38.70 | 27.40 | 26.90 |
| 07_create10k | 559.30 | 550.20 | 567.80 | 780.50 | 483.20 | 534.80 | 489.30 | 434.70 |
| 08_create1k-after1k_x2 | 59.30 | 59.50 | 57.80 | 60.70 | 53.50 | 58.70 | 53.30 | 51.20 |
| 09_clear1k_x8 | 49.10 | 83.10 | 67.10 | 68.30 | 36.80 | 38.90 | 32.40 | 36.90 |

CPU Script 中位数（ms）

| 项目 | glory | leptos | dioxus | react-hooks | solid | vue | svelte | vanillajs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 01_run1k | 8.20 | 10.50 | 8.10 | 9.40 | 5.80 | 8.50 | 6.10 | 3.10 |
| 02_replace1k | 15.10 | 18.40 | 14.50 | 17.80 | 8.90 | 13.00 | 10.30 | 5.50 |
| 03_update10th1k_x16 | 5.00 | 5.10 | 7.50 | 10.10 | 4.00 | 6.30 | 4.20 | 1.80 |
| 04_select1k | 3.20 | 3.40 | 2.20 | 7.60 | 3.00 | 3.80 | 7.00 | 1.40 |
| 05_swap1k | 3.20 | 2.80 | 5.90 | 45.60 | 3.20 | 4.80 | 5.10 | 0.80 |
| 06_remove-one-1k | 1.20 | 1.50 | 2.80 | 3.20 | 1.00 | 5.80 | 1.60 | 1.20 |
| 07_create10k | 107.80 | 115.70 | 100.30 | 294.40 | 42.80 | 81.20 | 46.10 | 30.50 |
| 08_create1k-after1k_x2 | 10.20 | 10.40 | 9.90 | 10.70 | 4.80 | 9.00 | 5.40 | 2.70 |
| 09_clear1k_x8 | 42.80 | 73.80 | 58.70 | 60.10 | 28.90 | 30.20 | 21.60 | 28.70 |

页面归属内存（MiB；每项仅 1 样本）

| 项目 | glory | leptos | dioxus | react-hooks | solid | vue | svelte | vanillajs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 21_ready-memory | 1.80 | 1.77 | 1.91 | 1.19 | 0.60 | 0.87 | 0.67 | 0.55 |
| 22_run-memory | 5.66 | 5.61 | 6.29 | 4.58 | 2.83 | 4.07 | 3.03 | 2.02 |
| 25_run-clear-memory | 4.19 | 4.58 | 6.90 | 1.97 | 0.73 | 1.20 | 0.95 | 0.65 |

体积（KiB）与 first paint（ms；每项仅 1 样本）

| 项目 | glory | leptos | dioxus | react-hooks | solid | vue | svelte | vanillajs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 41_size-uncompressed | 474.30 | 189.70 | 422.30 | 190.30 | 11.50 | 64.30 | 26.60 | 11.70 |
| 42_size-compressed | 112.70 | 48.80 | 114.80 | 51.40 | 4.50 | 23.30 | 9.70 | 2.50 |
| 43_first-paint | 766.20 | 371.30 | 737.40 | 329.30 | 106.50 | 128.10 | 134.50 | 101.10 |


选区快照对应的标准交换行 total 为 107.0→42.7 ms，trace Script 为 52.5→3.2 ms。
Script 区间缩短包含同步布局的消除，不能换算成 Rust 计算加速倍数；Script 与 paint 也不能相加。
标准创建万行 total 为 Glory 559.3 / Dioxus 567.8 ms，Script 为 107.8 / 100.3 ms。
选择状态仍存在广播差距；体积/first paint 每项只有一个样本，保留数据但不据此判断稳定排名。

最终保留源码：`selection-only/inputs`；独立产物：`selection-only/site`、`selection-only-handwritten/site`、`selection-only-probes/site`。
最终共同/故障验收：`selection-only-acceptance/evidence.json`。Rust/WASM Clippy 复核：`clippy-wasm-selection-only-rechecked.txt`，依赖审计：`engine-forks-delivery.txt`。
