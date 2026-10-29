# Glory / Leptos / Dioxus: current rows benchmark

Latest local optimization: [creation, replacement, clearing and artifact attribution](../docs/framework-benchmark-render-optimization-2026-10-29.md).
It compares the unchanged Widget/Each workload before/after and against Dioxus
and Leptos, with wide-template, failure-prefix, reuse and mixed-host checks.
`analyze-display-size.cjs` reports raw/compressed WASM sections and the actual
JS import graph without rewriting measured artifacts.

Latest published-commit run: [Widget and Each results](../docs/framework-benchmark-published-widget-each-2026-10-29.md)
for Glory `dae6f2c`, Leptos 0.8.22 and Dioxus 0.7.10: four alternating rounds,
1,296 samples, fresh release builds and data/identity/layout/pixel acceptance.
The report retains the Leptos selection anomaly and the rejected concurrent-build run.

The current keyed public API, completed checks and measurements are documented in
[the implementation report](../docs/v2-display-completion-results-2026-10-29.md).
Glory's XML and handwritten frontends share `model.rs`; application code no
longer implements RowHost, scopes or a separate label registry. `Each` owns
mounted row lifetimes; the standalone html-template app remains retired.

The sibling js-framework-benchmark now has an independent standard HTML-table
Glory fixture. Build its Rust implementations with
`scripts/build-js-framework-benchmark.ps1`; after building JS peers and starting
the upstream server, `python benchmarks/run-standard.py --out <new D: directory>`
freezes inputs and runs all enabled upstream cases for eight keyed frameworks.
Those results use a different workload and must stay separate from this suite.

Final 2026-10-29 self-suite: four alternating rounds, 2160 samples. XML create-1k
Script improves 21.4%, selection 20.8%, clear 11.8%; replace/update/10k retain
4.1%/4.7%/1.2% regressions. The full report includes wall regressions, all peers
and the standard table suite's complete CPU/memory/size matrix.
If Chrome trace collection alone fails with `No commit event found`, preserve
the run and use `python benchmarks/finish-standard.py <run-directory>` to rerun
whole failed cases. It rejects other failures and preserves earlier traces.

Glory 主入口 [glory/src/main.rs](glory/src/main.rs) 已迁移到新的显示 DSL。
默认构建测 XML；`--features handwritten` 用同一业务模型和 keyed Cache 对比手写 API；
`--features acceptance` 增加专项探针，不进入计时构建。
独立 `html-template` 应用已合并移除，避免维护两份相同 workload。
见[迁移和同批测量](../docs/benchmark-display-dsl-migration-2026-10-28.md)。
该迁移阶段的四轮 1728 样本：千行／万行创建 Script 为 **7.275／57.860 ms**，万行两 RAF wall 为 **321.00 ms**。
相对同批旧主入口分别降低 59.6%／64.2%／22.9%；保留小操作 wall 回退与 DSL/手写差异。

下列报告按历史阶段保留，旧入口的数字不代表当前 DSL 主入口。

[2026-10-28 稀疏 HTML 模板原型](../docs/v2-html-template-prototype-results-2026-10-28.md)：
旧 v2／保留相同响应图的新模板／合并绑定／Dioxus 四轮 1728 样本。
模板版千行／万行创建 Script 降低 55.1%／67.3%，万行两 RAF wall 降低 31.3%。
实例改为根和动态目标；原有逐语义节点能力不属于新 profile，编辑器／SSR／native 尚未迁移。
完整数据、DOM 身份、万行布局和六处截图一致，并保留小操作 wall 与合并绑定清空的回退。
当时的独立原型现已归入主 benchmark；历史源码与结果仍由报告中的冻结快照保存。
当前使用[独立驱动](compare.cjs)、[专用验收](display-acceptance.js)和[汇总程序](summarize.py)。

[2026-10-28 v2 提交准备与暂存日志优化](../docs/v2-plan-submit-cost-optimization-2026-10-28.md)：
同机原 v2／候选／Dioxus 四轮 1296 样本。清空千行／创建万行 Script 改善 27.1%／4.4%，
千行创建和替换基本持平；万行两 RAF wall 回退 1.7%，不宣称整体应用提速。
对象表引用减少约 66%，WASM 减少 4886 B；31 浏览器验收和 548 项相关 Rust 测试通过。
报告保留早期清空回退、消融、全部慢样本及 Blink 旧布局片段造成的 GC 验收修正。

[2026-10-28 v2 Plan 浏览器批量提交原型](../docs/v2-plan-host-batch-prototype-2026-10-28.md)：
完整覆盖受限 Plan 的创建、初始化、挂接和退休，千行创建导入调用 5,034→154。
四轮 1,296 个同批样本中，万行创建两 RAF 耗时改善 5.0%，替换、选行、清空等回退；
未确认整体收益，保留在 v2 原型，不据此引入 DSL。主表采用页面内两 RAF 计时，
CPU 仅作整轮读取，不能与旧报告逐操作 ScriptDuration 混用。

[2026-10-26 渲染流程架构改进](../docs/render-pipeline-implementation-2026-10-26.md)：
统一 UI 退休清单和索引维护、共用纯 Entry 初始化，并明确控制器更新依赖。29 组浏览器、
共享层/Blitz/Salvo 回归通过；保留两组 A/B 的环境干扰和回退，不声称整体提速。
−Oz WASM 减少 5,984 B，JS 大小不变；三框架、看板及主配对共 3,144 个正式样本。

[2026-10-26 跨 mutation 字段提交试验（已撤回）](../docs/field-submit-plan-2026-10-26.md)：
过桥 **5,034 → 3,065**、解码调用 **5,012 → 2,027**，但第二版四轮 A/B 的创建只快
约 1.5%，替换 / 选行 / 清空分别慢约 **4.4% / 23.6% / 13.9%**。保留两版实验、
三框架结果、前缀恢复和内存增长测试，恢复了试验前实现，没有默认启用这条路径。

[2026-10-26 类型化 Plan 宿主目标](../docs/plan-host-goals-2026-10-26.md)：
JS 直接完成字段写入、离树放置和移除；千行创建过桥 **11,034 → 5,034**，
反向 externref 分配 **1,010 → 10**，真实 DOM 工作保持一致。四轮 864 样本中
创建 / 万行创建 / 清空约快 **2.3% / 4.3% / 8.7%**，保留更新和交换回退；
另测三框架 648 样本。28 浏览器、内存增长和故障前缀测试通过，记录 WASM +5,509 B。

[2026-10-26 Plan 过桥批处理](../docs/plan-bridge-batching-2026-10-26.md)：
千行创建 WASM→JS **19,034 → 11,034（−42%）**，反向分配、真实 DOM 工作不变；
共享原始物理部件表、按需取回，保留完整身份和独立校验，并验证部分销毁的 GC。
四轮 864 样本中创建基本不变，替换 / 清空约快 **3.0% / 2.2%**，追加约慢 0.8%；
另测三框架 648 样本，千行创建 Glory / Leptos / Dioxus 为
**17.510 / 13.994 / 4.298 ms**。明确记录内存权衡、WASM +3,318 B 和选行异常。

[2026-10-26 批量路径热点与质量优化](../docs/bulk-lifecycle-quality-2026-10-26.md)：
合并四个控制器的批次元数据提取、复用 Memory 退休身份清单；四轮绑核 A/B 共 864
样本，千行创建 / 替换 / 清空改善 **4.5% / 5.4% / 2.8%**，万行改善 **6.7%**。
另重建三框架测 648 样本，千行创建 Glory / Leptos / Dioxus 为
**18.351 / 15.202 / 4.472 ms**。保留小操作回退、初次漂移数据、Leptos 选行异常，
以及 Dioxus 清空后活动解释器保留峰值离树行引用的独立观察。

[2026-10-26 当前三框架重跑](../docs/framework-benchmark-rerun-2026-10-26.md)：
包含 Role 优化的 Glory、锁定的 Leptos 0.8.22 / Dioxus 0.7.10，三个全新独立 target，
正反两轮 648 个样本。千行创建 Script 为 **18.962 / 14.355 / 4.507 ms**；
统一 −Oz WASM 为 **1,412,608 / 155,008 / 317,164 B**。数据、完整身份、万行几何
及六处画面一致；保留 Leptos 连续流程选行异常和另外 72 个独立诊断样本。

[Role 字段依赖优化与前后测量](../docs/role-patch-lowering-2026-10-26.md)：
Role 更新复用固定五属性投影，保留完整差分和独立旧规则校验；WASM 更新内核耗时
下降 31%–90%，完整应用 WASM 减少 6,496 B，Brotli 增加 252 B。内核时间不代表整页帧率。

[体积差距的具体代码分析与去重](../docs/framework-benchmark-size-analysis-2026-10-26.md)：
最终函数归因、同源码 Rust 3/z 诊断、HTML role/数值/方向映射去重及同路径 A/B。
保留性能配置和完整能力；去重的 WASM 减少 6,215 B，Brotli 增加 94 B，明确记录质量收益与体积边界。

当前应用的[WASM 体积实测（2026-10-26）](../docs/framework-benchmark-wasm-size-2026-10-26.md)：
独立重建 Glory 0.3.1 / Dioxus 0.7.10 / Leptos 0.8.22，保留性能配置。
统一 Binaryen `-Oz` 后 WASM 为 **1,425,214 / 317,164 / 155,008 B**；
WASM+全部 JS 的 Brotli 为 **385,157 / 108,490 / 48,747 B**。
原始发布流程、调试名称、同口径压缩及六项浏览器验收分别记录；Git 历史中的旧体积表不适用。

本轮[Plan 准备与批次身份校验优化](../docs/plan-batch-preparation-optimization-2026-10-25.md)：
八份独立构建，最终三版本同批 1,800 样本；千行创建 / 替换 Script 改善 **7.5% / 4.2%**，
5,000 行看板初始化改善 **6.0%**。万行轮间不稳定、全量更新 3–6% 回退均保留；
Windows/Linux 908/907、27 浏览器、64/1024 真实行高和实窗/UIA 回归通过。

新增[持续局部更新应用场景](../docs/portfolio-scenarios-benchmark-2026-10-25.md)：
模拟持仓看板，1,000 / 5,000 行，Glory 与 Dioxus 父列表/行组件两种组织，
三个独立构建、1,152 个正式样本、完整数据/身份/几何及逐像素验收。
5,000 行单行更新 Script 为 **0.179 / 2.237 / 0.120 ms**；行组件版 Dioxus 仍更快，
初始化和大面积更新也保留 Glory 的不利结果。该场景补充下方批量测试，不替换原 workload。

最新三框架独立重跑见 [2026-10-25 最新稳定版对照](../docs/framework-benchmark-latest-stable-2026-10-25.md)：
Glory 当前源码、**Leptos 0.8.22 / Dioxus 0.7.10**（registry 稳定版复核），三个全新 target。
相同九项、正反两轮 **648 个**正式样本，数据、完整后代身份、万行几何及六处截图一致。
Glory / Leptos / Dioxus 的千行创建 Script 为 **19.391 / 13.241 / 4.388 ms**，
替换千行为 **29.980 / 20.257 / 7.548 ms**，万行为 **230.614 / 168.170 / 44.815 ms**。
报告保留 Leptos 连续流程选行异常，另列 72 个独立诊断样本和正常选行结果，不混入主表。

针对上述当前构建的[深入 Dioxus 分析](../docs/dioxus-performance-analysis-2026-10-25.md)，
另做桥接/DOM 计数、CPU 采样和显式 GC 后的主机引用观察；探针结果不混入正式时间。

上轮实测见 [HTML / DOM 映射与桥接优化报告](../docs/html-dom-bridge-optimization-2026-10-25.md)：
10 份独立性能构建，首批 1,296 / 最终 1,512 个正式样本，分批统计。
保留版千行创建 / 替换 / 万行创建 Script 为 **19.896 / 29.855 / 230.928 ms**，
相对同批 before 改善 **10.3% / 7.6% / 6.2%**；完整 DOM 写入、两份 Memory 和节点身份保留。
原型寻址每行 19→10 次物理边访问，直接 Node 句柄 27,070→11,054。
报告含 Leptos/Dioxus 全九项、双向桥接计数、撤回 Vec/数字对/栈缓冲实验，
以及当前工作区额外独立构建与完整回归（903/902、27 组浏览器、64/1024 真实行高）。
主 A/B 的 15 个应用输入不变；当前 API 命名迁移作为另一源集仅核验正确性。

上轮实测见 [核心响应图与提交优化报告](../docs/reactive-runtime-optimization-2026-10-24.md)（2026-10-24 测量、10-25 复核）：
逐项消融 **1,944 个**主样本，最终同批对照另 **1,296 个**，不混池。
默认 Glory 千行创建 / 替换 / 万行创建 / 清空的 Script 为
**20.975 / 30.986 / 247.109 / 15.317 ms**；选行约快 30%，替换约快 5%，
批量创建收益不稳定。保留两份 Memory、完整身份、真实几何和逐像素检查。
报告同时记录原生高扇出、WASM Node 体积增加、空帧/多依赖回退、缓冲保留成本和撤回实验。
标准 benchmark 继续用原来的 Bond/Vary，直接闭包 API 作为独立迁移对照。

上轮实测见 [2026-10-24 双份 Memory 声明共享报告](../docs/shared-declarations-2026-10-24.md)：
六份全新独立构建、复制声明/共享声明/内联消融对照，最终 **1,296 个**正式样本。
Glory 千行创建 / 替换 / 万行创建 / 清空的 Script 为
**19.59 / 30.43 / 234.37 / 15.45 ms**，相对同批 before 降低约 16% / 17% / 13% / 12%。
同批 Leptos 为 15.30 / 22.86 / 187.55 / 17.59 ms，
Dioxus 为 4.52 / 7.85 / 60.93 / 8.96 ms。全部数据、存续节点身份、万行几何和画面一致；
报告保留消融慢样本、绑核辅助脚本中断/恢复、完整回归和未获益项，不合并不同源码批次。

上一轮优化实测见 [2026-10-24 节点存储与 DOM 元数据报告](../docs/node-storage-dom-metadata-2026-10-24.md)：
五个全新独立 target，Memory-only 拆分对照，四轮 **2,160 个**正式样本。
Glory 千行创建 / 替换 / 万行创建 / 清空的 Script 为
**23.90 / 36.87 / 256.21 / 16.86 ms**，相对本轮 before 降低约 14% / 20% / 25% / 31%。
双份 Memory、完整节点身份和全部 DOM 属性仍保留。报告包含三框架全表、
未获益的小操作、首轮慢样本与补跑、源码边界及完整回归证据。

上一轮三框架实测见 [2026-10-24 静态构造计划后对照](../docs/framework-benchmark-static-plan-2026-10-24.md)：
三个全新独立 target、相同九项、正反顺序 648 样本。Glory / Leptos / Dioxus
千行创建 Script 为 **28.47 / 14.21 / 4.46 ms**，万行总耗时为
**620.65 / 437.40 / 298.25 ms**。数据、存续节点身份、完整万行几何和 18 张画面检查点一致。
完整主表保留选行的运行历史效应，并附 72 个独立诊断样本；本轮未改实现或 workload。

静态计划实现及同库 before/after 见 [静态构造计划报告](../docs/static-construction-plan-2026-10-24.md)。

此前优化及源码分析见 [2026-10-24 创建提交/Dioxus 报告](../docs/bulk-create-dioxus-2026-10-24.md)：
独立构建、相同九项、正反顺序 1,080 样本；保留的 Glory 创建桥接将万行 Script 从
702.08 降到 654.75 ms，当前三框架万行总耗时为 909.45 / 465.10 / 313.70 ms。
历史结果和未保留实验均有明确边界，主计时 workload/harness 未改。

The previous [three-framework rerun after DOM creation optimization](../docs/performance.md#three-framework-rerun-after-dom-creation-optimization--2026-10-23)
freshly builds all three from a frozen 251-input snapshot and measures
648 samples in opposite order under the same P-core affinity. Create-10k wall
medians are 955.10 / 439.20 / 297.35 ms for Glory / Leptos / Dioxus.
The report includes all nine operations, CPU counters, per-round variation,
independent selection diagnostics and exact source/identity/geometry/pixel evidence.

The earlier [HTML mapping/DOM creation optimization](../docs/performance.md#html-mapping-and-dom-creation--2026-10-23)
compares fresh independent Glory before/after builds with 432 unchanged-workload
samples. Script medians improve 17.0% for create-1k, 7.8% for replace-1k and
11.4% for create-10k. Final attributes/identities remain intact: finite native
prototypes reduce replacement setAttribute calls from 32k to 15k and UTF-8
decodes from 78,006 to 34,006. The report records 25 browser suites, complete
native regression, real-height cache checks, and the concurrent-source boundary.

The previous [three-framework rerun after teardown optimizations](../docs/performance.md#three-framework-rerun-after-teardown-optimizations--2026-10-23)
freshly builds Glory, Leptos 0.8.19 and Dioxus 0.7.9 independently from a frozen
243-input snapshot. Two opposite-order rounds produce 648 samples, with the
same P-core affinity for all three. Median create-10k wall time is
1012.15 / 451.30 / 306.95 ms; clear-1k is 33.70 / 19.25 / 9.35 ms.
Data, surviving DOM/Text identities, all 10k row rectangles and six viewport
checkpoints agree. The report includes CPU counters, per-round variation,
Leptos's history-sensitive selection diagnostic and the source boundary for
concurrent changes. The host also ran other project builds; this is a local run.

The earlier [Scope/unlink follow-up](../docs/performance.md#scope-cleanup-and-sibling-unlink--2026-10-23)
compares independent builds under the same P-core affinity: 432 primary samples,
with 1,296 earlier default-scheduling samples retained separately. Clear-1k script
median improves 27.5%; replacement improves only 1.6%. See the report for variance,
node/pixel checks and full regression.

The [Owner/Binding retirement follow-up](../docs/performance.md#owner-and-binding-retirement-follow-up--2026-10-23)
records the next 432-sample paired run, identity/pixel checks and full regression.
Clear-1k script median improves 6.7%; replacement has no demonstrated gain.

The subsequent [DOM retirement follow-up](../docs/performance.md#dom-retirement-follow-up--2026-10-23)
compares fresh Glory before/after builds with 432 unchanged-workload samples,
removes repeated parent-table scans, and records complete regression evidence.

The previous rerun independently rebuilt all three apps and measured two
opposite-order rounds (648 samples). Its historical table, CPU counters, untimed
node census, selection diagnostic and immutable evidence are in
[Performance Notes](../docs/performance.md#current-three-framework-rerun--2026-10-23).
The earlier one-round table below remains historical.

The [bulk DOM follow-up](../docs/bulk-dom-optimization-2026-10-23.md) compares
Glory before/after framework optimizations with this same unchanged workload:
432 samples, reversed-order rounds, independent builds and identity/pixel checks.
Its paired results and the then-existing adaptive anchor failure are recorded
separately from the earlier three-framework table below. The anchor failure was
subsequently [fixed](../docs/anchor-residual-fix-2026-10-23.md). Use `bulk-compare.cjs`
and `summarize-bulk.py` for the paired run; the report includes exact commands.

The active workload is **semantic-rows-v1**. It compares Glory's XML display DSL
and full keyed Cache with Leptos 0.8.22 and Dioxus 0.7.10. Glory's explicit
display profile is **html-display-v1**, with independent class/label bindings
and no intermediate class Bond (3 graph nodes per row). All three
use the same seeded Rust generator, div/button/span element structure and CSS.
Framework-specific attributes and comment markers remain measured overhead.
The operations are create 1k/10k, replace, update every tenth label, select,
swap, remove, append and clear. Every row is mounted; this is not a virtual list.

These are **local comparison results**, not official HTML-table scores. The
old self-suite adapter script is disabled for this div DOM contract; the sibling
standard-table fixture described above is separate.
Historical official tables below do not describe the current architecture.

## Run

Use PowerShell on Windows with Rust's `wasm32-unknown-unknown` target, Node and
the Playwright package under `tests/playwright/node_modules` installed. Supply
wasm-bindgen-cli **0.2.129** at
`D:/BuildCache/glory-framework-compare/tools/bin/wasm-bindgen.exe`.
The framework library versions and bindgen cohort are locked in the three
checked-in Cargo.lock files; the CLI version must match.

```powershell
./benchmarks/compare.ps1
```

This freezes the current owned crate/app inputs with SHA-256, builds each app
in an independent D: Cargo target (release opt-level=3/LTO/one codegen unit),
runs strict WASM Clippy, generates web bindings without wasm-opt, starts a
loopback server, launches an independent headless Chrome and performs semantic
conformance before timing. It closes its browser and owned server on exit.
The default browser is installed Google Chrome; `-Browser` selects an executable.
All targets, temporary browser profiles and results must stay under D:/BuildCache.

```powershell
# Reuse already built sites; each run needs a new output directory.
./benchmarks/compare.ps1 -SkipBuild `
  -OutDir D:/BuildCache/glory-framework-compare/my-round
```

To check order sensitivity, reuse one build and run a separate reversed round:

```powershell
./benchmarks/compare.ps1 -SkipBuild -Order 'leptos,glory,dioxus' -OutDir D:/BuildCache/glory-framework-compare/forward-round
./benchmarks/compare.ps1 -SkipBuild -Order 'dioxus,glory,leptos' -OutDir D:/BuildCache/glory-framework-compare/reverse-round
```

For a one-app preview use `./benchmarks/run.ps1 glory` and visit the printed URL.
The module exposes `window.__BENCH__.runAll()`; structured three-app measurements
should use compare.ps1 so the CPU counters and conformance checks are included.
The old Trunk cache/output path is no longer used by the supported runner.

For a controlled XML/handwritten/Dioxus run, build independent targets from a
single snapshot with `build.ps1 -Apps glory`, `-Handwritten` in a second BuildRoot,
and `-Probes` in a third. `compare.cjs` records full identities/10k geometry,
six identical screenshots, paired per-operation CPU/wall samples and build manifests.
Its default directory layout and the exact commands are in the migration report.

## What is validated and timed

- An independent JS seed oracle verifies row IDs/labels and all nine operations.
- Swap must preserve keyed DOM identities. Every one of the 10k rows has a box.
- Clear/recreate checks that Glory's live keyed projections and graph return to
  baseline. Other frameworks' internal reactive graphs are not inspected.
- Same-checkpoint viewport PNGs are byte-identical; final normalized row-data
  hashes agree for every timed sample. This is not a claim about every pixel
  in every possible state or browser.
- Each operation warms 3 times and measures 12 samples. Setup and row-data
  checks are outside the click-to-two-RAF interval. This is not presentation/FPS.
- CDP script/layout/style/task CPU counter deltas are recorded as diagnostics;
  they include common harness overhead. Inclusive metrics must not be summed.

## First current-architecture round: 2026-10-23

Chrome 155.0.8059.84, 1280×900, 324 measured samples. Wall medians in ms:

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 133.65 | 43.35 | 49.80 |
| replace_1k | 173.80 | 49.75 | 47.30 |
| update_10th | 14.40 | 14.60 | 22.60 |
| select_row | 11.00 | 250.60 | 9.45 |
| swap_rows | 12.30 | 10.80 | 11.15 |
| remove_row | 14.65 | 12.80 | 15.95 |
| append_1k | 169.55 | 66.65 | 67.70 |
| create_10k | 2809.90 | 509.30 | 554.90 |
| clear_1k | 83.05 | 20.70 | 10.50 |

In that frozen 2026-10-23 round, Glory trails substantially on bulk mount/replacement/append/clear;
create-10k is about 2.81 s versus 0.51/0.55 s. Its script time is the largest
gap. Small updates are much closer. The Leptos mixed selection result is
history-sensitive; fresh-list diagnostics do not reproduce it as a general
selection baseline. There is no overall ranking from this one ordered round.

See [current measurements and caveats](../docs/performance.md) for CPU counters,
the separate selection probe, live node census, build details and raw evidence.

Historical command-backend benchmark instructions and results are preserved in [Git history](https://github.com/glory-rs/glory/blob/d31a8591dc626a303b1106285dff11f8e6162a28/benchmarks/README.md). Their runners and native reproduction program have been retired; use the semantic commands above.
