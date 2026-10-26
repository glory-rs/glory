# 当前 Glory / Leptos / Dioxus benchmark 重跑（2026-10-26）

包含 Role 字段投影优化的当前 Glory 0.3.1，与仓库锁定的 Leptos 0.8.22、
Dioxus 0.7.10 全部重新独立构建。批量创建、替换和追加仍是 Dioxus 最快。
Glory 千行创建 / 替换 / 万行创建的 Script 时间分别为 Leptos 的约
1.32 / 1.32 / 1.22 倍，Dioxus 的约 4.21 / 3.66 / 4.32 倍。

本轮没有修改框架、业务实现、依赖版本、feature 或 workload，也没有将旧轮
样本混入。它是当前实现的横向重跑，不是 Role 优化的受控 A/B；后者仍见
[Role 专项报告](role-patch-lowering-2026-10-26.md)。

## 构建和测量

- 冻结 534 个输入，Glory 为 HEAD `6f08a1265d27d036a4ee692ae67ba29d2da71af6`
  加当前工作区改动。冻结输入和工作区输入在实验结束后逐文件 SHA256 一致。
- 三个独立 target 均在 `D:/BuildCache/glory-framework-rerun-20261026-roles` 下，
  Cargo metadata 已核对。release 的 35 / 234 / 124 个 compiler artifacts
  全部 fresh=false，各应用 WASM all-targets Clippy 通过。
- Rust / Cargo 1.98.0，wasm-bindgen 0.2.129，opt-level 3、LTO、CU1。
  Leptos 0.8.22、Dioxus 0.7.10 来自既有精确清单及锁文件，不宣称为此次联网
  查询所得的最新版本。
- Windows 11、i7-14700KF、Chrome 155.0.8059.84，1280×900、DPR 1、en-US/light。
  每个场景和轮次使用新 context，前台页面；每次页面准备后统一并验证浏览器
  CPU 掩码 65535 和 AboveNormal 优先级，记录在 cpu-affinity.jsonl。
- 构建、压缩和 Binaryen 验收在计时前结束；计时前后检查未见其他 Rust 构建。
  不将本机一次实验推广为跨机器排名。
- Glory → Leptos → Dioxus，再反向 Dioxus → Leptos → Glory。
  九项操作，每项每轮 3 次预热、12 个正式样本，每格 24 样本，共 648 个。
  保留所有慢样本。数据快照、身份、几何、截图验收不在计时窗口内。
- 主计时使用三者各自 release + bindgen 产物，保持既有 benchmark 流程；
  下文统一 Binaryen 后的体积另列，不能把两个产物阶段混为一谈。

## 九项结果

ScriptDuration 中位数，单位 ms，越低越好。Script 包含实际事件和 DOM 操作，
不包含下载、WASM 编译和首次加载，也不是用户交互延迟或 FPS。

| 操作 | Glory 0.3.1 | Leptos 0.8.22 | Dioxus 0.7.10 |
|---|---:|---:|---:|
| 创建千行 | 18.962 | 14.355 | 4.507 |
| 替换千行 | 28.825 | 21.877 | 7.865 |
| 更新每十行 | 0.468 | 0.331 | 0.869 |
| 选行† | 0.637 | 173.339 | 0.761 |
| 交换行 | 0.378 | 0.175 | 0.940 |
| 删除行 | 0.354 | 0.379 | 0.802 |
| 追加千行 | 20.956 | 16.720 | 5.670 |
| 创建万行 | 228.643 | 187.121 | 52.974 |
| 清空千行 | 14.050 | 16.471 | 2.720 |

Glory 的更新、交换、删除小操作在本批快于 Dioxus；删除相对 Leptos 的微小
差距不应据此视为稳定优势。Glory 的批量操作仍明显落后于 Dioxus，也慢于
Leptos 的创建、替换和追加。

计入等待两个 RAF 的 wall 中位数，单位 ms：

| 操作 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 创建千行 | 45.550 | 39.150 | 30.850 |
| 替换千行 | 57.850 | 45.550 | 33.200 |
| 创建万行 | 504.900 | 464.100 | 333.700 |

wall 不是实际画面呈现完成时间。Script、Task、Layout、RecalcStyle 分别取
中位数，不能相加；全部指标的 p10/p90 和逐轮中位数保存在机器可读结果中。
例如万行创建两轮 Script：Glory 233.478 / 223.103、Leptos 188.405 / 183.327、
Dioxus 53.677 / 50.404 ms，保留轮间变化。

## Leptos 选行异常的独立检查

† 主表保留连续九项流程中的 Leptos 173.339 ms。此前相同流程也出现过这类
运行历史异常，不能用这个值宣传一般选行性能相差数百倍。

另在全新 context 测两种选行场景：同一千行实例交替选择前两行；每次重建千行
后选择。每格 3 次预热 + 12 样本，共 72 个诊断样本，未并入或替换主表。
验证选中行、唯一选中状态及跨框架业务数据哈希。

| 独立选行场景 Script ms | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| 同一千行实例连续切换 | 0.665 | 1.141 | 0.700 |
| 每次重建千行后选择 | 0.576 | 3.544 | 0.830 |

这说明主流程的 173 ms 不代表新页面通常的选行成本。此次没有定位该框架内部
根因，不将它推断为泄漏或某个特定算法问题，也没有修改其实现来调整成绩。

## WASM 和传输体积

三个应用统一使用 Binaryen 132.0.0：
`-Oz --enable-bulk-memory --enable-reference-types`。Brotli q11、gzip 9。
单位均为字节。生成 JS 包含入口及其相对导入的 snippets，逐文件独立压缩；
不计三者共用的 harness、CSS 和 HTML。

| 指标 | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| bindgen WASM（主计时产物） | 1,846,098 | 279,406 | 457,470 |
| Binaryen −Oz WASM | 1,412,608 | 155,008 | 317,164 |
| −Oz WASM Brotli | 376,516 | 43,438 | 99,109 |
| −Oz WASM gzip | 513,230 | 54,567 | 125,714 |
| 全部生成 JS | 60,648 | 30,134 | 49,916 |
| −Oz WASM + JS Brotli | 385,450 | 48,747 | 108,490 |

Glory 的最终 WASM 仍明显更大。不能从文件大小推断运行内存，也不能将跨轮
不同冻结目录的字节差异归为某一次优化收益。统一 −Oz 产物没有用来替换本轮
主计时文件；其真实浏览器正确性也独立验收。

## 一致性和证据

- seeded oracle 验证全部业务数据和九项操作；648 个计时样本的行数与数据哈希
  在对应操作/样本位置跨框架、跨轮一致，没有通过删样本使其对齐。
- 完整存续行和后代 Element/Text 引用保持，Glory 的语义身份同时保持；替换
  后旧对象脱离文档。10,000 行的每个矩形均为真实正高度，并跨框架逐项哈希一致。
- 创建、替换、追加、万行顶部、中部和清空六处 PNG 的哈希在三者间一致，
  已查看万行中部截图。
- 清空后没有行 DOM。Glory 内部 UI 10 / graph 3 回到基线；另外两者没有导出
  等价内部统计，只验证其 DOM 清理，不宣称已证明其内部资源/RSS 回收。
- bindgen / Binaryen 两阶段的三应用共六项真实浏览器契约验收通过，无 JS 异常。
- 这个 workload 是 `semantic-rows-v1` 完整 DOM 应用，不是官方 HTML-table
  js-framework-benchmark 排名，也不代表所有应用、控件或平台。

[机器可读汇总](../benchmarks/three-framework-results-2026-10-26.json)保留每项
Script / wall / Task / Layout / RecalcStyle 的中位数、p10/p90、逐轮值、依赖版本、
构建审计和产物哈希。

本机全部证据位于 `D:/BuildCache/glory-framework-rerun-20261026-roles`：
冻结源码 `source-20261026-123102-446/inputs.json`；三个独立 Cargo target 和
metadata / Clippy / release 日志；`primary/evidence.json` 为完整正式样本，
`primary/selection-diagnostic.json` 为独立选行诊断；`bundle-size/report.json`
和 `bundle-size/browser/evidence.json` 为体积及其验收。
完整 driver 和测试源码副本位于 `primary/driver-source/`，输入和输出均保留，
没有覆盖历史测量或重设已有体积基线。所有构建、缓存、临时文件都在 D 盘。
