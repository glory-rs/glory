# 统一 Each：验证与测量

当前 API 见[统一 Each](v2-unified-each-2026-10-29.md)。这次统一了 Widget 类型、
builder、DSL 和控制器，保留不同数据源的存储方式及原有 keyed/LIS Cache。
没有把 KeyedList 转成 Vec，没有给 keyed 行增加 item/index Cage 或数据 Owner。
向量行仍保留原来的值/索引快照和初始值行模式。

实现中用编译期 Owner 策略避免 keyed/seed 行携带可选向量状态；可失败的数据准备
在 Widget 工厂之前完成，工厂直接返回 Widget，避免在每行传递大 `Result<Widget>`。

## 正确性

- 统一后的 UI/Web/SSR/facade 相关测试：269 项通过。
- Sapid `scripts/check.ps1`：2701 次测试执行通过，包括 default/all-feature 重复覆盖，
  不是 2701 个独立测试；严格 Clippy 通过。
- Native shell/AccessKit/raster 引擎：165 项通过，2 项平台测试忽略。
- 33 个真实下游编译检查覆盖向量 key、children/seed、错误属性、工厂和重命名 facade。
- 手写/DSL 稀疏宿主、600 行状态/GC、万行数据 oracle、失败前缀、原生/fallback 移动、
  焦点/选区、混合树 CSR、真实 SSR hydration、分阶段 HTTP 增量流及最终接管通过。
- 向量和 KeyedList 重排均保留节点；向量标签/索引随数据更新；删除重插重置状态。
  非 Clone keyed 值直接投影字段；字段编辑不重新协调顺序。源释放返回错误并可清理。
- Sapid 同一份 DSL 示例编译为 SSR、CSR/hydration 和 native，两个数据源共用重排操作。
  源码面板对应的 149 项 showcase catalogue 已重新生成并验证。

万行浏览器场景在 before/after 中的活计数完全一致：

| 指标 | 数量 |
|---|---:|
| 实例 / records | 10,001 |
| targets | 50,007 |
| 响应式 graph 节点 | 30,005 |
| Owners | 10,002 |

静态密度探针仍为 2 / 201 个物理节点，对应同样的 1 个实例、2 个 target、1 个 record。
向量 headless 千行场景仍为 4,002 个 graph 节点和 2,004 个 Owner。

## 性能方法

基线是此次修改之前的真实未提交工作区快照，不能用 Git HEAD 代替。最终源码快照为
`D:/BuildCache/glory-each-unify-20261029/after2/inputs`；基线位于同目录的 `before`。
`inputs.json` 保存逐文件 SHA-256。最终测量时运行时代码与该快照一致。

浏览器使用自带 semantic-rows-v1 div workload，Chrome 155.0.8059.86，release WASM、
相同 CSS/数据、1280×900、相同 CPU affinity，每个变体使用新浏览器。先比较完整数据、
全部后代身份、万行矩形及六处截图，再做四轮正反交替、每项三次热身和十二次正式采样。
最终共 864 个正式样本，无并行编译。原始慢样本全部保留。
这是自带 workload，不是 js-framework-benchmark 的标准 table suite。

## 最终浏览器结果

下表为每个变体 48 个样本的中位数。CPU 是 CDP ScriptDuration；wall 包含两次 RAF
等待，并非像素呈现延迟。两列分别列出，不用 CPU 改善代替 wall 结果。

| 操作 | before CPU ms | after CPU ms | before wall ms | after wall ms |
|---|---:|---:|---:|---:|
| 创建千行 | 6.510 | 6.808 | 41.05 | 40.90 |
| 替换千行 | 11.281 | 11.018 | 40.75 | 39.15 |
| 更新每十行 | 0.573 | 0.553 | 20.40 | 22.25 |
| 选择行 | 0.354 | 0.350 | 21.25 | 21.60 |
| 交换行 | 0.396 | 0.446 | 21.30 | 23.25 |
| 删除行 | 0.394 | 0.400 | 23.10 | 22.60 |
| 追加千行 | 5.577 | 5.579 | 36.95 | 37.65 |
| 创建万行 | 54.626 | 55.122 | 305.60 | 294.70 |
| 清空千行 | 5.329 | 5.640 | 21.10 | 21.00 |

WASM 从 2,036,662 增至 2,040,503 字节（约 +0.19%），JS 字节数不变。
万行创建 CPU 约 +0.9%，千行创建约 +4.6%；交换增加约 0.050 ms，清空增加约
0.311 ms。部分 wall 指标升高，不能宣称所有指标零回退或全面提速；这里只确认
保留了复用、活状态规模和同量级成本，并完整记录本机微基准的差异。

## Vary/Vec headless UI

用同样的千行语义 Text Widget、原来的值/索引 Cage、同一更新/交换/清空流程，
对比旧三参数 Each 与统一 builder。四轮交替、三次热身、十二次正式采样；
每项每变体 48 个样本，保留节点身份和活计数断言。此结果不包含 native 布局/绘制。

| 操作 | before ms | after ms |
|---|---:|---:|
| 创建 | 4.495 | 4.719 |
| 更新每十行 | 0.506 | 0.509 |
| 交换 | 0.455 | 0.459 |
| 清空 | 1.682 | 1.642 |

创建中位数约 +5.0%，其余为约 -2.4% 至 +0.9%。这些本机计时不构成所有应用、
浏览器或设备的性能保证。不要把接口合并或少量分配变化自动换算成提速。

## 复现与证据

- 浏览器最终原始样本及汇总：`D:/BuildCache/glory-each-unify-20261029/trial4/`。
  早期候选的 `trial2` / `trial3` 保留用于追溯，不代表最终实现。
- 向量最终样本、生成的 before/after Rust 程序：同目录 `vectors2/`。
  可用 `benchmarks/compare-each-vectors.py --before <snapshot> --after <snapshot>
  --out D:/BuildCache/<directory>` 重新构建、验证和采样。
- 稀疏路径最终证据：`D:/BuildCache/glory-semantic-ui/each-sparse-final/`。
- 混合树证据：`D:/BuildCache/glory-semantic-ui/mixed-site/evidence.json` 和
  `mixed-stream-site/evidence.json`。
- Sapid 证据：`D:/BuildCache/sapid/each-fast-full-check.txt`、
  `each-fast-dsl-check.txt` 以及 `dsl/evidence.json`。
