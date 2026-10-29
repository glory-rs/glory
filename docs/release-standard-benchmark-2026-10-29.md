# 标准 js-framework-benchmark 发布前复核（2026-10-29）

本轮使用标准表格 workload 和上游默认 runner：CPU 每项 15 个样本，选择项 25 个；memory/size 各 1 个。保留全部成功样本。
8 个实现均通过 keyed 检查；驱动 20 项单元测试通过。119/120 个完整结果已保存，Glory 与 Dioxus 的全部 15 项各自齐全。

测试的 Glory 运行时为优化提交 `2bc52e3`（改名/升版前的 0.3.1 源码）。标准 fixture 原先遗留的 IntoView/DemoProps 已改为现行 Widget 工厂接口；业务操作保持一致。此数据不代替 0.6.0 的注册表消费与平台验收。

## CPU 结果

单位 ms，表内均为本轮均值；各项使用上游自己的默认 CPU throttle，不能跨项目类型比较绝对值。小差异不表示统计显著。

| 操作 | Glory 总耗时 | Dioxus 总耗时 | Glory script | Dioxus script |
|---|---:|---:|---:|---:|
| 创建 1,000 行 | 45.733 | 46.387 | 8.040 | 8.167 |
| 替换 1,000 行 | 49.460 | 59.773 | 13.113 | 14.200 |
| 更新每第十行（×16） | 56.740 | 58.300 | 5.093 | 5.500 |
| 选择行 | 22.336 | 27.936 | 3.932 | 1.832 |
| 交换行 | 45.127 | 61.467 | 2.760 | 5.220 |
| 删除一行 | 30.773 | 41.567 | 1.100 | 2.420 |
| 创建 10,000 行 | 507.260 | 525.607 | 90.467 | 98.793 |
| 追加 1,000 行（×2） | 54.133 | 55.393 | 8.520 | 9.267 |
| 清空（×8） | 60.193 | 55.693 | 47.193 | 48.207 |

本轮创建 1,000 行基本持平；替换总耗时低约 17.3%，清空总耗时高约 8.1%。选择项的 Glory script 时间仍高于 Dioxus。总耗时还包括浏览器布局/绘制，不能将差异全部归给 Rust 组件处理。

## Memory 与传输体积

| 项目 | Glory | Dioxus |
|---|---:|---:|
| 21_ready-memory（上游单位） | 1.867 | 1.892 |
| 22_run-memory（上游单位） | 5.797 | 6.281 |
| 25_run-clear-memory（上游单位） | 4.442 | 6.897 |
| 41_size-uncompressed（上游单位） | 892.300 | 422.300 |
| 42_size-compressed（上游单位） | 200.400 | 114.800 |
| 43_first-paint（上游单位） | 1219.700 | 874.900 |

## 采集边界与证据

唯一缺项为 VanillaJS 的 `08_create1k-after1k_x2`。初始运行和三次完整 case 重跑都遇到 No commit event。原始失败 trace 在点击后有 UpdateLayoutTree、Layout 和 PrePaint，却没有该 runner 要求的后续 Commit；该项没有用部分样本或其他计时终点补值。其余因 trace 采集失败而重跑的完整 case、原始 traces 和日志均保留。

三个独立进程观察区间分别采样 2,524、226、32 次，均未发现本机编译进程。

原始输入、结果、traces、重跑日志与汇总：`D:/BuildCache/glory-rebrand-20261029/official-suite/`。
关键文件：`inputs.json`、`summary.json`、`results/`、`traces/`、`retries/`；采集失败分析在同级 `missing-commit-analysis.json`。

此前 div/span 自测、WASM 归因和优化过程见 `framework-benchmark-render-optimization-2026-10-29.md`；其数值与本页的标准表格结果分别保留。
