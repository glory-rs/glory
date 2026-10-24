# 最新稳定版三框架 benchmark（2026-10-25）

本轮使用当前 Glory 代码，以及联网核实的 **Leptos 0.8.22 / Dioxus 0.7.10**，全部重新独立构建。Glory 批量创建/替换仍慢于两者；按 Script 中位数，千行创建/替换/万行创建分别约为 Leptos 的 **1.46 / 1.48 / 1.37 倍**，Dioxus 的 **4.42 / 3.97 / 5.15 倍**。每十行更新、交换、删除这些小操作，Glory 本轮快于 Dioxus，Leptos 的对应项更快。

数据、存续 DOM/Text 身份、Glory 完整语义身份、全部万行几何和六处逐像素检查均通过。本轮只升级独立 benchmark 的依赖并重跑，没有继续优化 Glory 或改动业务/布局代码。

## 版本与冻结输入

| 框架 | 实际版本/代码 | 关键运行依赖 |
|---|---|---|
| Glory | 0.3.1，HEAD `9615776964d6c4f59b982f906c7dea488113c0c3` + 当前工作区源码 | 保留本轮 DOM 相邻寻址、根句柄复用和直接插入 |
| Leptos | **0.8.22** | reactive_graph **0.2.16**，tachys **0.2.19**，leptos_macro 0.8.19 |
| Dioxus | **0.7.10** | core / web / signals / interpreter-js 均为 **0.7.10** |

截至 **2026-10-25 06:30（Asia/Shanghai）** 的 crates.io registry 复核，这两版仍是各自最大未撤回稳定版。GitHub Leptos releases/latest 指向 0.9.0-rc0，不能把它当稳定版。crates.io API 的 max_stable_version、未撤回版本筛选与发布方包页面相互核对：[Leptos 0.8.22](https://docs.rs/crate/leptos/0.8.22)、[Dioxus 0.7.10](https://docs.rs/crate/dioxus/0.7.10)、[Leptos RC 发布页](https://github.com/leptos-rs/leptos/releases/tag/v0.9.0-rc0)。原始 registry 回复与开始/结束复核见构建目录的 versions/。

两个 manifest 改为精确锁定新稳定版本，各自 Cargo.lock 更新兼容依赖并随清单保留为可复现输入；Dioxus 仍关闭 devtools/logger/document 等未参与本测试的默认项。框架 API、数据模型和事件处理算法保持原样。所有框架仍共享 wasm-bindgen **0.2.129**，web-sys **0.3.106**，CLI **0.2.129**。

冻结 **511 个** workspace/应用输入，后续构建及测量期间 SHA-256 未改变。相对上一轮当前工作区快照，三份 main.rs、共享 data.rs、bench-harness.js、semantic.css 均逐字节一致；版本与锁文件另有明确变化。旧轮结果不并入本轮样本，也不能把跨轮差异全部归因于框架版本升级或某一内部优化。

## 构建及计时方法

- 全部输出：`D:/BuildCache/glory-framework-latest-20261025`。三个独立目录 glory/target、leptos/target、dioxus/target；Cargo metadata 核对实际输出目录，无产物互拷。
- 三份 WASM release 的 **35 / 234 / 124 个 compiler artifacts 全为 fresh=false**。每个应用的 WASM all-targets Clippy 检查通过。Glory 依赖沿用的两条 hydrate-only dead_code 警告不影响应用检查；本轮未将其作为性能修复。
- rustc **1.98.0**、cargo **1.98.0**；wasm32-unknown-unknown、opt-level 3、LTO=true、codegen-units=1、enable-interning 一致。
- i7-14700KF，Chromium **155.0.8059.84**，1280×900、DPR 1、en-US/light。主计时浏览器统一 P-core 掩码 **65535**、AboveNormal；每个框架/轮次新建前台 context。
- 顺序 Glory→Leptos→Dioxus，再反向 Dioxus→Leptos→Glory；相同九项，每项 3 次热身 + 12 样本，最终每格 **24 样本**，共 **648**。不删慢样本。
- 主计时没有 DOM/WASM monkey patch 或 Profiler；所有自有构建在计时前结束。CDP 记录 Script/Task/Layout/RecalcStyle，数据 oracle 与快照排除在计时窗口外。wall 为 click→两个 RAF，不代表实际显示完成、用户真实鼠标交互延迟或 FPS。

[冻结源码清单](D:/BuildCache/glory-framework-latest-20261025/source/inputs.json)、[业务输入审计](D:/BuildCache/glory-framework-latest-20261025/workload-audit.json)、[独立构建审计](D:/BuildCache/glory-framework-latest-20261025/independent-build-audit.json)、[解析依赖版本](D:/BuildCache/glory-framework-latest-20261025/versions/resolved.json)。

## 正式九项结果

ScriptDuration 中位数，单位 **ms**，越低越好。

| 操作 | Glory | Leptos 0.8.22 | Dioxus 0.7.10 |
|---|---:|---:|---:|
| 创建千行 | 19.391 | 13.241 | 4.388 |
| 替换千行 | 29.980 | 20.257 | 7.548 |
| 更新每十行 | 0.501 | 0.316 | 0.871 |
| 选行† | 0.592 | 168.768 | 0.786 |
| 交换行 | 0.590 | 0.155 | 0.910 |
| 删除行 | 0.600 | 0.307 | 0.791 |
| 追加千行 | 21.035 | 15.524 | 5.358 |
| 创建万行 | 230.614 | 168.170 | 44.815 |
| 清空千行 | 14.975 | 15.937 | 2.688 |

† Leptos 选行是标准连续九项流程中的运行历史异常。原始值保留，但不能用来宣传普通选行性能相差数百倍；独立诊断见下节。

| 批量操作 | Glory Script p10–p90 | Leptos Script p10–p90 | Dioxus Script p10–p90 |
|---|---:|---:|---:|
| 创建千行 | 18.604–20.378 | 12.243–14.522 | 4.244–4.764 |
| 替换千行 | 28.151–30.936 | 19.163–21.766 | 7.274–7.965 |
| 创建万行 | 214.646–245.785 | 150.158–179.349 | 41.977–54.296 |

### 加入浏览器工作后的差距

同批 click→两个 RAF 的 wall 中位数，单位 ms：

| 操作 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 44.200 | 36.650 | 28.450 |
| 替换千行 | 55.300 | 42.750 | 30.950 |
| 创建万行 | 509.100 | 426.550 | 289.650 |

万行的 Glory/Dioxus 比值在 Script 中约 5.15 倍，在这项 wall 指标中约 1.76 倍；不能把脚本倍数当成完整画面呈现倍数。CDP 的 Task、Layout、RecalcStyle 与逐轮数值全部保存在汇总中，各指标独立取中位数，不能相加。

未压缩 WASM 文件大小：Glory **1,870,640 B**，Leptos **279,394 B**，Dioxus **457,458 B**。这不是运行内存；本轮计时也不包含下载、编译和首次加载成本。

[全部 648 个原始样本](D:/BuildCache/glory-framework-latest-20261025/primary/evidence.json)、[九项全部指标及 p10/p90](D:/BuildCache/glory-framework-latest-20261025/primary-summary.json)。

## Leptos 选行的独立诊断

主流程 Leptos 168.768 ms 在最新稳定版仍出现。另开新 context，分别测试同一千行实例上连续切换选择、以及每次重建千行后选择；每格 3 次热身 + 12 样本，总计 **72 个诊断样本**，不并入主表。

| 独立场景 Script 中位数 ms | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 同一千行实例反复选行 | 0.649 | 1.144 | 0.701 |
| 每次重建千行再选行 | 0.561 | 3.563 | 0.751 |

这确认 169 ms 不能代表新页面千行选行的一般成本，之前的连续操作历史与结果有关。当前证据不足以将根因归为某个内部集合、泄漏或算法，不据此更改官方库或 workload。主表保留原值，正常选行诊断另列，避免挑选样本或偷换场景。

[诊断原始数据](D:/BuildCache/glory-framework-latest-20261025/selection/evidence.json)、[独立汇总](D:/BuildCache/glory-framework-latest-20261025/selection-summary.json)。

## 数据、身份和画面验收

- 独立 seeded oracle 检查创建、更新、选择、交换、删除、追加、替换、万行与清空后的完整业务数据；所有正式样本数量和数据哈希按同一操作/样本位置跨框架一致。
- 对全部已保存的行及后代 Element/Text 对象做引用比较：更新、选择、交换、删除其他行、追加时存续对象不替换；Glory 的完整 data-gly-node 身份同时保持。替换后旧对象全部脱离文档。
- 三者 **10,000 行完整矩形**哈希及首尾边界一致，每行都有正高度；不是只抽测首尾几行。
- 创建、替换、追加、万行顶部、万行中部、清空共 **六处 PNG** 在三者间逐字节哈希一致；已实际检查截图。
- 清空后各框架无行/标签 DOM；Glory 的 UI **10** / graph **3**。Leptos/Dioxus 未导出相同内部统计，不把 DOM 清空宣称为其内部资源/RSS 清理证明。
- 页面无 JavaScript 错误。未改任何几何或身份容差。本轮范围是 semantic-rows-v1 浏览器应用，不是官方 js-framework-benchmark 排名，也不能概括所有控件、原生平台、OS 输入或无障碍能力。

[完整后代身份及万行矩形](D:/BuildCache/glory-framework-latest-20261025/integrity/evidence.json)、[验收汇总及六处像素哈希](D:/BuildCache/glory-framework-latest-20261025/verification.json)。

本轮改动限于两个 benchmark manifest / lockfile 和报告入口；Glory 的 DOM 实现保持已完成的优化。内部优化前后归因见 [HTML / DOM 专项](html-dom-bridge-optimization-2026-10-25.md)，该轮基线及依赖版本与本次不同，数字不混用。
