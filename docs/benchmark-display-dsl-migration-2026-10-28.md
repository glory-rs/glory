# Glory 主 benchmark 迁移到显示 DSL

日期：2026-10-28，分支 `v2`。本轮更新 benchmark 应用与工具，没有改动运行时。

## 当前入口和去重决定

[benchmarks/glory](../benchmarks/glory/src/main.rs) 现在默认使用 `glory::display` 和 XML `view!`：
工具栏、表体与整行都走共享模板／动态槽，不再用语义 Widget/Plan 创建全部节点。
保留原 Generator、每行标签 Cage、selected Cage、全量 keyed Cache、六个按钮和九项操作。
class 计算直接观察 selected，不再创建中间 Bond；每行标签 Cage 加两个字段 Binding，共三个图节点。

**不保留独立 `benchmarks/html-template` 应用。** 它与主入口重复维护行模型和操作，
现已合并，移除其 Cargo.toml／lock／main／index 及构建入口。保留其有价值的内容：

- `glory --features handwritten`：使用[具名手写声明](../benchmarks/glory/src/browser/declarations.rs)，
  与 DSL 共用 Model、Rows、Cache、回调和统计，不再复制一套业务逻辑。
- `glory --features acceptance`：包含迁入的[原型探针](../benchmarks/glory/src/probes.rs)，
  并启用 flush/close 测试导出；正式计时构建不包含这些导出或探针代码。
- [display-acceptance.js](../benchmarks/display-acceptance.js)：保留密度、移动、内存增长、
  故障前缀和 GC 验收。未初始化 class 的失败现场改按真实 `role=row` 计数，没有放宽成功前缀数量。
- 旧原型、grouped 实验及旧工具的冻结源码和结果继续保存；不因删除重复应用而删除不利数据。
  [原型历史报告](v2-html-template-prototype-results-2026-10-28.md)、
  [本轮开始时的完整源码](D:/BuildCache/glory-benchmark-dsl-20261028/before/inputs/inputs.json)、
  [旧工具归档](D:/BuildCache/glory-benchmark-dsl-20261028/before/tooling)。

新主入口继续使用 `semantic-rows-v1` 这一共同 div/role 工作负载；Glory 的显示契约明确为
`html-display-v1`。静态节点没有独立 Glory 语义身份，不能冒充旧完整语义 API／编辑器的等价实现。
本地测试不是官方 js-framework-benchmark 排名。

## 验收

默认 DSL、手写 API、旧主入口、Dioxus 0.7.10 独立构建。
共享 oracle 的 seeded 行数据和事件结果一致；更新／交换／删除／追加保留全部存活后代 Element/Text
身份；所有万行有实际布局盒，完整几何摘要和六处截图一致。
两种新前端在每个验收阶段的完整状态统计也相同。

新主入口万行现场为：10001 实例、50007 目标、30003 响应图节点、10002 Owner；
物理元素 80008、Text 30006。清空回到 1 实例、7 目标、1 条记录、3 图节点、2 Owner。
旧主入口仍为 80010 语义节点、40003 图节点、10013 Owner；Dioxus 内部图未做同口径计数。

独立 acceptance 构建通过静态节点 2→201 而实例／目标不增长、Unicode、错误／外来／退休句柄、
两种移动路径的焦点／选区、memory.grow 与拒绝重入；创建／挂接／字段／退休四种中途失败的前缀、
停止后缀、清理均通过。退休根与 Text 在渲染生命周期和四次强制 GC 后被回收。
共享完整后代身份检查保留；清空检查改为完整状态回到启动基线，并额外验证新显示 profile 的固定根状态。

默认、handwritten、acceptance、全部 feature 组合的 WASM all-target Clippy 均通过，Rustfmt 通过。
所有 528 个最终冻结输入逐一对照当前代码；默认计时 app.js 中确认无 probe/flush/close 测试导出。
[独立验收原始结果](D:/BuildCache/glory-benchmark-dsl-20261028/acceptance/evidence.json)。

## 同批测量

before 是本轮开始时的旧语义主 benchmark，包含此前工作区运行时改进及 Vary 改名，
不是未修改的远端提交。新旧运行时源码没有在本轮加入优化；差异是应用从语义 builders/Plan
迁移到显示 API，以及移除中间 class Bond。不能把收益单独归因于 XML 的语法。

Chrome 155.0.8059.86、1280×900/DPR 1、headless/disable-gpu；本任务浏览器进程 affinity
65535、AboveNormal。每版本每轮新浏览器，正反交替四轮，每操作 3 次热身、12 个正式样本，
共 **1728 个正式样本**。前后检查无 cargo/rustc 构建；保留所有慢样本，不做显著性声明。
下表为每操作每版本 48 样本的合并中位数，Script 是逐操作 CDP 增量，ms。

| 操作 | 旧主入口 | Glory DSL | 手写对照 | Dioxus | DSL 相对旧入口 |
|---|---:|---:|---:|---:|---:|
| 创建千行 | 18.018 | 7.275 | 7.318 | 4.617 | -59.6% |
| 替换千行 | 27.318 | 9.640 | 10.084 | 8.334 | -64.7% |
| 每十行更新 | 0.883 | 0.507 | 0.533 | 1.144 | -42.6% |
| 选行 | 1.145 | 0.420 | 0.413 | 1.042 | -63.3% |
| 交换 | 0.641 | 0.412 | 0.426 | 1.229 | -35.7% |
| 删除一行 | 0.612 | 0.379 | 0.374 | 1.133 | -38.2% |
| 追加千行 | 16.922 | 5.536 | 5.568 | 6.456 | -67.3% |
| 创建万行 | 161.544 | 57.860 | 53.383 | 41.802 | -64.2% |
| 清空千行 | 22.553 | 5.543 | 5.175 | 3.814 | -75.4% |

相对旧主入口，四轮中九项操作的 Script 中位数均改善。
万行 Script 仍比 Dioxus 高约 38.4%，不能宣布整体领先。

两 RAF wall 中位数，ms；不是呈现延迟或 FPS：

| 操作 | 旧主入口 | Glory DSL | 手写对照 | Dioxus | DSL 相对旧入口 |
|---|---:|---:|---:|---:|---:|
| 创建千行 | 57.85 | 46.35 | 46.20 | 40.55 | -19.9% |
| 替换千行 | 58.90 | 40.55 | 43.50 | 34.70 | -31.2% |
| 每十行更新 | 23.15 | 24.20 | 24.15 | 22.80 | +4.5% |
| 选行 | 21.25 | 23.65 | 25.35 | 23.10 | +11.3% |
| 交换 | 22.55 | 25.60 | 25.40 | 23.15 | +13.5% |
| 删除一行 | 22.35 | 25.15 | 24.55 | 21.60 | +12.5% |
| 追加千行 | 52.40 | 40.65 | 42.95 | 40.85 | -22.4% |
| 创建万行 | 416.15 | 321.00 | 307.50 | 296.20 | -22.9% |
| 清空千行 | 28.60 | 19.95 | 21.50 | 22.55 | -30.2% |

创建／替换／追加／清空的 wall 四轮方向均改善；小操作 wall 没有相同收益，表中回退保留。
两种前端的业务、模板目标与响应图相同，不表示性能完全相同：本批 DSL 的万行 Script 比手写
高约 8.4%，四轮都是更高；清空高约 7.1%。其余操作差异小且方向不同。
保留 handwritten feature 有独立回归价值，不据此给差异作未经剖析的单点归因。
此前 raw html-template 的另一批性能不能与本批拼接，尤其不能仅从图节点减少就推断更快。

产物均为独立 opt-level=3/LTO/单 codegen-unit，无 wasm-opt：

| 版本 | WASM B | JS B，含输出目录执行器，不含共同 harness |
|---|---:|---:|
| 旧主入口 | 1907239 | 74577 |
| Glory DSL | 363804 | 33114 |
| 手写对照 | 362679 | 33114 |
| Dioxus | 456622 | 51608 |

新入口没有链接旧完整语义 widgets/Mapper/控制器的大量代码，这不是所有旧能力仍保留时的体积比较。

## 复现与证据

- [仓库汇总](../benchmarks/display-dsl-results-2026-10-28.json)：全部操作、轮中位数、最慢值、方向、验收和产物 SHA。
- [1728 原始样本与环境](D:/BuildCache/glory-benchmark-dsl-20261028/paired/evidence.json)。
- [旧源码清单](D:/BuildCache/glory-benchmark-dsl-20261028/before/inputs/inputs.json)、
  [新源码／工具清单](D:/BuildCache/glory-benchmark-dsl-20261028/after/inputs/inputs.json)。
- 新版本的三个独立 target 位于 after、handwritten、probes；旧版本和 Dioxus 位于 before。
  每份 build.jsonl 记录实际 feature、WASM 路径与 manifest，使用相同 bindgen 0.2.129。

以下根目录 layout 是本次已完成的迁移对照，before 使用上述冻结的旧源码。
通用三框架入口 `benchmarks/compare.ps1` 和 `benchmarks/run.ps1 glory` 也自动构建新的主入口。

```powershell
$root = 'D:/BuildCache/glory-benchmark-dsl-20261028'
$bindgen = 'D:/BuildCache/glory-v2-dioxus-20261028/tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe'
# 新的一次构建应先 snapshot.py 冻结当前输入，不能覆盖已有历史目录。
./benchmarks/build.ps1 -Apps glory -BuildRoot "$root/after" -SourceRoot "$root/after/inputs" -Bindgen $bindgen
./benchmarks/build.ps1 -Apps glory -Handwritten -BuildRoot "$root/handwritten" -SourceRoot "$root/after/inputs" -Bindgen $bindgen
./benchmarks/build.ps1 -Apps glory -Probes -BuildRoot "$root/probes" -SourceRoot "$root/after/inputs" -Bindgen $bindgen
node benchmarks/compare.cjs new-acceptance 0 before,glory,handwritten,dioxus,probes
node benchmarks/compare.cjs new-paired 4 before,glory,handwritten,dioxus
python benchmarks/summarize.py D:/BuildCache/glory-benchmark-dsl-20261028/new-paired/evidence.json --out D:/BuildCache/glory-benchmark-dsl-20261028/new-paired/summary.json
```

设置 `GLORY_BENCH_BUILD_ROOT`／`GLORY_REPO` 可指定根与仓库。
所有编译、Cargo cache、临时文件、浏览器资料和测试产物均在 D:。
