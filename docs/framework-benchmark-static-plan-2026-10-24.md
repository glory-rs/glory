# 静态构造计划后的 Glory / Leptos / Dioxus 对照

本轮将当前 Glory、Leptos 0.8.19、Dioxus 0.7.9 从同一冻结源码分别全新构建，再运行相同的 semantic-rows-v1。静态计划使 Glory 的批量创建差距缩小，但创建、替换、追加和清空仍落后于另外两者。增量操作接近 Dioxus，不能据此推断所有 UI 场景的排名。

## 主结果

两轮相反顺序，每轮每项 3 次热身、12 个样本；每格合计 24 个样本，九项共 **648 个主样本**。以下均为中位数，单位 ms。

### Script CPU

| 操作 | Glory 静态计划 | Leptos 0.8.19 | Dioxus 0.7.9 |
|---|---:|---:|---:|
| 创建千行 | **28.47** | 14.21 | 4.46 |
| 替换千行 | **46.75** | 21.64 | 7.90 |
| 更新每十行 | 0.63 | 0.36 | 0.89 |
| 选行（完整混合流程） | 0.78 | 170.05 | 0.75 |
| 交换两行 | 0.58 | 0.18 | 0.91 |
| 删除一行 | 0.62 | 0.29 | 0.81 |
| 追加千行 | **30.78** | 15.98 | 5.71 |
| 创建万行 | **352.18** | 179.41 | 48.76 |
| 清空千行 | **24.66** | 16.44 | 2.73 |

Glory 的创建千行 Script 为 Leptos 的 **2.00 倍**、Dioxus 的 **6.38 倍**；替换千行为 **2.16 / 5.92 倍**；创建万行为 **1.96 / 7.22 倍**；清空仍为 **1.50 / 9.03 倍**。主要差距仍在批量构造和销毁，不是每个局部更新都慢于 Dioxus。

### 端到端

沿用 click→两个前台 RAF 的原 harness，包含布局、调度与帧等待；这不是实际显示完成时间或 FPS。

| 操作 | Glory 静态计划 | Leptos 0.8.19 | Dioxus 0.7.9 |
|---|---:|---:|---:|
| 创建千行 | **54.70** | 38.85 | 30.05 |
| 替换千行 | **75.70** | 45.00 | 32.15 |
| 更新每十行 | 11.40 | 11.50 | 11.15 |
| 选行（完整混合流程） | 8.10 | 174.05 | 9.50 |
| 交换两行 | 9.90 | 10.05 | 10.15 |
| 删除一行 | 11.40 | 8.10 | 10.65 |
| 追加千行 | **61.90** | 43.30 | 34.30 |
| 创建万行 | **620.65** | 437.40 | 298.25 |
| 清空千行 | **28.15** | 17.55 | 10.30 |

创建万行的总耗时，Glory 是 Leptos 的 **1.42 倍**、Dioxus 的 **2.08 倍**，小于 Script 的差距。相同实际 DOM 布局成本占据了三者的大量时间：万行 Layout CPU 分别 **145.06 / 156.32 / 153.74 ms**，RecalcStyle 为 **64.35 / 55.06 / 53.55 ms**。这些指标来自不同 inclusive counter，不能相加或把 wall−Script 全称为绘制时间。本轮没有测量 GPU/合成耗时。

### 两轮变化

| 框架 | 创建千行 Script 两轮 | 替换千行 Script 两轮 | 创建万行 Script 两轮 | 万行端到端两轮 |
|---|---:|---:|---:|---:|
| Glory | 30.53 / 27.57 | 46.58 / 47.96 | 342.87 / 365.67 | 609.25 / 626.55 |
| Leptos | 14.28 / 14.09 | 21.70 / 21.44 | 186.19 / 177.38 | 447.20 / 435.75 |
| Dioxus | 4.48 / 4.46 | 7.90 / 7.91 | 49.37 / 47.19 | 302.75 / 293.35 |

主表合并每格全部 24 个原始样本取中位数，没有把两轮中位数简单平均。大操作两轮排名一致，较小的毫秒以下差异不应泛化到其他负载。

## 与静态计划之前的差距

上一轮[创建提交/Dioxus 报告](bulk-create-dioxus-2026-10-24.md)中，Glory 千行创建 Script 为 55.61 ms、替换 79.87 ms、万行创建 654.75 ms；当前为 28.47 / 46.75 / 352.18 ms。两轮跨框架报告的创建千行 Script 比例从约 **3.83× Leptos / 12.47× Dioxus**，缩小到本轮的 **2.00× / 6.38×**。

这些是不同轮次的历史参考，并非本轮新增的 before/after 因果实验。静态改造本身的收益已经在[静态构造计划报告](static-construction-plan-2026-10-24.md)中通过相同库、原 builders / 通用计划 / 深克隆计划的独立对照验证；本轮只重新测量三个当前框架的差距。Leptos/Dioxus 本次也重新构建、重新计时，没有复用旧结果。

## 选行的运行历史效应

主流程的 Leptos 选行 **Script 170.05 / 总耗时 174.05 ms** 原样保留，不视为通常的选行成本。额外在新上下文中检查两个场景，每格 3 次热身、12 个样本，共 **72 个诊断样本**，不进入主表：

| 框架 | 首次千行列表 Script / 总耗时 | 连续建表 30 次后 Script / 总耗时 |
|---|---:|---:|
| Glory | 0.90 / 11.70 | 0.92 / 11.70 |
| Leptos | 1.31 / 11.55 | 6.45 / 11.70 |
| Dioxus | 0.72 / 11.70 | 0.75 / 11.70 |

30 次建表是首次创建加 29 次替换；诊断随后在既有列表上交替选行，主流程则每次 sample 都先重新建表。两者是不同运行历史。它们表明高值不能泛化；本轮没有重新定位 Leptos 内部根因，也没有修改其实现或删除不利样本。

## 构建、正确性与边界

源码冻结于 **2026-10-23T22:36:08.732815Z（本地 2026-10-24）**，260 个输入。Glory 的全部 crates 与上一轮 post 完整回归逐文件哈希一致；三份 benchmark 应用及共享 workload 也未改变，深克隆常量为 true。本轮没有修改框架或 benchmark 实现，只增加结果文档。

收尾核对发现冻结后工作树另有样式相关的并发修改（style 的 lib/profile/sheet values、HTML styles/patch tests、Blitz document/style_profile 测试）。这些后续变化不属于本次构建；本次数字严格对应上述冻结的静态计划版本，不宣称覆盖变化后的样式实现。`concurrent-overlay/` 保存当时捕获的文件及前后哈希，`workspace-boundary.json` 记录具体边界；三个应用和共享 benchmark 输入仍未改变。

三个全新的独立 **D:** target，分别运行严格 WASM all-target Clippy 和 release 构建；metadata 验证实际 target，应用 compiler-artifact 都是 **fresh=false**。release opt-level=3、LTO、1 codegen unit；wasm-bindgen 库/CLI 0.2.129，无 wasm-opt。没有 vendor、fork 或依赖更新。

| 产物 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 原始 WASM bytes | 1,004,970 | 278,497 | 457,457 |
| 站点 JS bytes（含共同 harness 与生成模块） | 46,928 | 34,357 | 55,831 |

体积未压缩，不是下载/冷启动性能测试；不将它换算成总 RSS。Windows / i7-14700KF / Rust 1.98.0 MSVC / Chrome **155.0.8059.84**，1280×900、scale 1、en-US、light。每个上下文固定逻辑 CPU 0–15、AboveNormal 并读回验证，配置工作在计时外完成。顺序 **Glory→Leptos→Dioxus**，第二轮反向。所有本任务构建在计时前结束，宿主不是独占机器，前后进程列表保留。

- 独立 seeded oracle 校验全部九项操作、行 ID/label/选中状态；全部 648 个主样本的 count/hash 跨框架与跨轮次一致。
- 三者均检查存续 Element 与 Text Node 的对象身份；Glory 还检查 data-gly-node。更新、选行、交换、删除、追加保留原节点，整批替换后旧后代全部断开。
- 所有 10,000 行都有真实布局框，完整矩形摘要一致。未隐藏行、未虚拟化、未省略真实节点。
- 三者各六个画面检查点，共 **18 张 PNG**，同检查点逐字节一致，包含万行中段滚动与清空。
- Glory 清空后的行/label/语义节点/响应图恢复基线；没有声称检查 Leptos/Dioxus 内部的 reactive graph 或总堆泄漏。
- 构建、正式计时、身份检查与选行诊断使用独立阶段；诊断不混入主样本。浏览器关闭，自建 loopback HTTP 服务核对 PID/命令行后停止。

这是本地 **semantic-rows-v1** 的完整 keyed div/button/span 比较，不是官方 HTML table benchmark，也不是所有 UI/原生/SSR/启动场景的总体排名。Glory 的有限语义身份、元数据与 Owner/Binding 成本仍参与测量；没有为了对齐其他框架而剥离这些行为。本轮未做新的 profile，不能只凭 Script 数值将剩余开销精确分摊给各模块。

## 原始证据

根目录：`D:/BuildCache/glory-plan-compare-20261024-0636`。

- `source/inputs.json`、`source-boundary.json`：冻结源码、锁文件与上轮边界。
- `workspace-boundary.json`、`concurrent-overlay/`：冻结后并发样式变更的单独记录，不改写已测源码。
- `build/*-metadata-final.json`、`*-build.jsonl`、`*-clippy.txt`：独立 target、fresh 应用 artifact 与严格检查；`build/site/` 保留实际发布 WASM/JS。
- `formal/*-samples.json`、`formal/evidence.json`、`summary.json`：全部原始主样本、五项指标、每轮中位数和比例。
- `integrity/`、`formal/*.png`：深身份、完整几何、数据与画面证据。
- `selection/`：72 个独立诊断样本。
- `run.ps1`、`driver/`、`pin-browser.ps1`、`verify.py`：完整执行与核验方法。

运行 `python D:/BuildCache/glory-plan-compare-20261024-0636/verify.py` 可重算主表并验证上述源码、构建、原始样本及图片。重新构建或测量时使用新的 D: 根目录，避免覆盖本次证据。完整归档哈希：[evidence-index.json](/D:/BuildCache/glory-plan-compare-20261024-0636/evidence-index.json)。
