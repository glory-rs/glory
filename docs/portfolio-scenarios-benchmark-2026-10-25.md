# 稳定界面上的持续局部更新：Glory / Dioxus（2026-10-25）

**差距可以缩小，但要区分状态划分。** 在长期挂载的模拟持仓看板中，Glory 的单行/1% 更新显著快于 Dioxus 父组件统一管理列表的版本；Dioxus 改用行组件和行级 Signal 后仍更快。5,000 行单行更新，两者 Script 为 0.179 / 0.120 ms，约 1.49 倍，已小于旧批量创建场景的 4–5 倍差距。初始化和大面积更新仍是 Glory 的弱项。

本轮新增独立应用场景，没有优化或修改框架实现，也没有替换原 semantic-rows-v1 测试。不同工作负载的绝对耗时不能混成优化前后结果。

## 场景和公平对照

应用是一个真实挂载、带滚动区的模拟持仓看板：每行有资产编号、价格、数量、金额、涨跌样式和选中状态，顶部显示总金额。全部 1,000 / 5,000 行都挂载，没有虚拟列表、隐藏替身或降低精度。数据是确定性整数计算，不接入网络或真实金融行情。

三个版本运行[相同 Rust Market 业务控制器](../benchmarks/scenarios/model.rs)，相同报价变动、数量和金额计算、总额增量维护、更新目标及 DOM/CSS。控制器和 UI 状态副本在三者中都存在。

| 实现 | 状态组织 | 更新路径 |
|---|---|---|
| Glory | Each 的 live item Cage；每行四个直接 Binding；总额单独绑定 | 通过只含活行的索引表写命中的 Cage，不扫描完整源 |
| Dioxus root | 父组件的集合 Signal；keyed 行模板；总额独立组件 | 一次借用修改命中项，然后父组件重新生成列表的动态值 |
| Dioxus row | keyed 行组件；每行局部 Signal；总额独立组件 | 通过活行索引表只更新命中组件，Registry props 用 Rc 身份比较 |

因此，root 版是粗粒度组织的参照，**row 版才是判断 Glory 与同样采用局部状态的 Dioxus 差距的重要对照**。Dioxus row 没有因总额刷新而被迫重跑父列表，Glory 也没有把 Each seed 只读一次再多建一份行 Cage。

选中状态也放在行内，控制器只修改旧行和新行；没有让每行订阅一个全局 selected。Glory 使用现有直接 Binding API，Dioxus row 使用对应的局部 Signal。这些应用组织方式不同于原批量 benchmark，所以不能把结果称作对原实现的性能优化。

八项操作：打开看板、更新单行、更新 1%、更新 10%、更新全部、一次事件接收十批 1% 行情、切换选中行、关闭看板。选行只改变新旧两行。每批行情目标会旋转；突发项不等同于反复修改同一字段，不能用其结果证明重复失效消除的收益。

持续更新各自在新看板上热身，然后保持同一组节点连续更新；不把重建计入每次局部更新。打开/关闭另外计时，避免隐藏生命周期代价。

源码：[Glory](../benchmarks/scenarios/glory/src/main.rs)、[Dioxus 两种组织](../benchmarks/scenarios/dioxus/src/main.rs)、[共享样式](../benchmarks/scenarios/style.css)、[独立 JS oracle](../benchmarks/scenarios/harness.js)。

## 构建和测量

- Glory 0.3.1，Dioxus **0.7.10**，wasm-bindgen **0.2.129**；release opt-level 3 / LTO / 单 codegen unit，未使用 wasm-opt。
- 输出全部位于 `D:/BuildCache/glory-portfolio-20261025`。三个最终独立 target：glory-final/target、dioxus-root/target、dioxus-row/target；Cargo metadata 核对实际 D 盘路径，35 / 124 / 124 个 release compiler artifacts 全部 fresh=false，无 WASM 互拷。
- 每版 wasm all-targets 严格 Clippy 通过；Glory 依赖沿用两条 hydrate-only dead_code 警告，没有把它们当作本轮改动。两个新 benchmark package 独立于主 workspace。
- i7-14700KF / Chrome **155.0.8059.84**；1280×900、DPR 1、en-US/light，统一 affinity 65535 / AboveNormal。
- 每个实现、规模和轮次独立 context。顺序 Glory→Dioxus root→Dioxus row，再反向；每项 3 热身 + 12 样本，每格合并 24 个样本，共 **1,152 个正式样本**，不删慢样本。
- CDP Script/Task/Layout/Style 差值；wall 为 click→两个 RAF。oracle、DOM 身份检查、stats、哈希和截图均在计时窗口外；没有 DOM monkey patch 或 CPU profiler。
- 编译及测量使用冻结源码。最终冻结 **512 个输入**均保持原样，benchmark 输入与当前工作区一致。测量期间 `crates/web/src/browser/events.rs` 又有并发点击处理顺序修改，未进入已测二进制，另存差异；不把本轮当作该并发修改的性能或行为验收。

[源码与独立构建审计](D:/BuildCache/glory-portfolio-20261025/source-build-audit.json)、[冻结输入](D:/BuildCache/glory-portfolio-20261025/source/inputs.json)、[并发修改边界](D:/BuildCache/glory-portfolio-20261025/concurrent-workspace-diff.txt)。

## 正式结果

ScriptDuration 中位数，单位 ms，越低越好。root / row 表示上文两种 Dioxus 状态组织。

| 1,000 行 | Glory | Dioxus root | Dioxus row |
|---|---:|---:|---:|
| 打开 | 16.464 | 3.813 | 4.514 |
| 单行报价 | 0.181 | 0.534 | 0.092 |
| 1% 报价（10 行） | 0.249 | 0.545 | 0.131 |
| 10% 报价（100 行） | 0.853 | 0.670 | 0.389 |
| 全量报价 | 6.879 | 2.032 | 2.975 |
| 十批 1% 行情 | 0.870 | 0.681 | 0.406 |
| 切换选中行 | 0.160 | 0.513 | 0.089 |
| 关闭 | 6.663 | 2.348 | 3.046 |

| 5,000 行 | Glory | Dioxus root | Dioxus row | Glory / Dioxus row |
|---|---:|---:|---:|---:|
| 打开 | 89.603 | 19.986 | 25.594 | 3.50 |
| 单行报价 | 0.179 | 2.237 | 0.120 | 1.49 |
| 1% 报价（50 行） | 0.534 | 2.368 | 0.283 | 1.89 |
| 10% 报价（500 行） | 4.239 | 3.057 | 1.646 | 2.58 |
| 全量报价 | 42.244 | 9.585 | 16.437 | 2.57 |
| 十批 1% 行情 | 3.424 | 2.866 | 1.699 | 2.02 |
| 切换选中行 | 0.174 | 2.208 | 0.105 | 1.66 |
| 关闭 | 40.472 | 11.799 | 16.515 | 2.45 |

关键项 p10–p90，没有截掉不利样本：

| 5,000 行 Script 范围 | Glory | Dioxus root | Dioxus row |
|---|---:|---:|---:|
| 单行报价 | 0.168–0.190 | 2.200–2.337 | 0.106–0.153 |
| 1% 报价 | 0.517–0.551 | 2.291–2.534 | 0.265–0.315 |
| 全量报价 | 38.926–51.648 | 9.032–17.362 | 15.635–23.759 |

两轮单行报价各自中位数 Glory **0.177 / 0.179**，Dioxus root **2.228 / 2.258**，row **0.131 / 0.114**；1% 各自为 **0.540 / 0.528**、**2.361 / 2.379**、**0.281 / 0.286**。这些方向并非只出现在有利的单轮。

加入等待、样式和布局后的 wall 指标不能与 Script 混称：

| 5,000 行 click→两 RAF，ms | Glory | Dioxus root | Dioxus row |
|---|---:|---:|---:|
| 打开 | 195.50 | 119.55 | 141.50 |
| 单行报价 | 10.75 | 12.70 | 11.95 |
| 1% 报价 | 10.65 | 13.20 | 12.95 |
| 全量报价 | 128.60 | 95.85 | 102.15 |

短操作 wall 明显受 RAF 等待影响，不能据此宣称 Glory 比 Dioxus row 实际显示更快。单行 Script 差约 0.059 ms，也不能变成可感知延迟或 FPS 结论。大部分行在滚动区外，本轮不声称每帧绘制全部行。

[全部正式样本](D:/BuildCache/glory-portfolio-20261025/measured/evidence.json)、[全部指标、p10/p90 和逐轮统计](D:/BuildCache/glory-portfolio-20261025/summary.json)。

## 为什么场景改变后差距缩小

1. **建树/建图成本被移到生命周期开始。** 稳态行情不再反复创建两份 Memory、Owner、Binding、DOM Entry 和身份；Glory 可以复用这些状态。但打开/关闭表明这些成本仍存在，短命或反复重建的界面并未因此获益。
2. **工作量随实际更新对象走。** 单行更新从 1,000 到 5,000 行，Glory 0.181→0.179 ms、Dioxus row 0.092→0.120 ms，root 则 0.534→2.237 ms。源码和结果都支持局部状态避免整表重算的解释；这不是 Glory 独有的优势。
3. **更新密度升高后，桥接和 DOM 提交重新显著。** 5,000 行的 1% 场景 Glory 比 root 快约 4.43 倍；到了 10%，Glory 已慢于 root。这里没有测出准确的交叉阈值。Dioxus row 的细粒度组件更新和批量解释器可同时工作，Glory 不会因采用细粒度响应式就自动超过它。
4. **粒度本身有成本。** Dioxus row 的打开比 root 慢，单行/1% 更新却更快，全量更新又慢于 root。同一框架的两种组织也存在取舍，不应只拿其中较慢的一种作为竞争对手。

本场景 Glory 1,000 / 5,000 行的图节点为 6,004 / 30,004，Owner 为 2,017 / 10,017，语义节点 5,014 / 25,014；持续更新时保持稳定，关闭后回到 graph=4、owners=17、nodes=14、registry=0。没有额外放弃节点身份来缩小差距。

## 正确性和失败处理

- 六组应用/规模全部通过独立 JS oracle：每个资产编号、价格、数量、金额、涨跌/选中 class 和总额；正式样本之间的数据哈希一致。
- 比较所有行及单元格的完整实测矩形，行高确为 28 px，汇总文字的 Range 边界和数字字体设置也一致。
- 每组 mount / dense / select 后滚动 / clear 四个 PNG，共 **24 张**，对应检查点在三实现间逐字节一致，未放宽容差。
- 单独的有序身份验收遍历每行每个 Element/Text：1,000 行 **9,000 个**、5,000 行 **45,000 个**物理节点；六种持续更新后逐位置仍是同一对象，Glory 身份字符串不变。关闭后旧节点断开，再打开不复用旧 DOM 对象或旧 Glory 身份寿命。
- live registry 关闭后为空；Glory 图/Owner/语义节点回到基线。Dioxus 内部图和 JS 峰值节点引用释放不由这项检查证明，仍需参考[上一份回收策略分析](dioxus-performance-analysis-2026-10-25.md#6-清空计时还包含一个重要的回收策略差异)。

最初验收在正式计时前发现 293 个像素不同。不是随机截图噪声：Glory 的基础 `[data-gly-kind=text]{font:inherit}` 覆盖了相同 specificity 的 `.total`，重置 tabular-nums，导致汇总数字实际文字宽度不同。两套应用统一使用 `#main .total` 后通过；另外补充数字字体和 Range 几何断言。没有移除框架样式、改字体内容、修改浏览器栅格化参数或排除差异区域。失败目录 results/、final/ 均没有正式轮次，保留记录但不进入本表。

[预检修正记录](D:/BuildCache/glory-portfolio-20261025/preflight-correction.json)、[补充有序身份验收](D:/BuildCache/glory-portfolio-20261025/integrity.json)、[验收驱动](../benchmarks/scenarios/integrity.cjs)。本轮验证范围为三个应用的编译检查和浏览器验收，没有重跑平台全回归。

## 对应用选择和后续优化的含义

更适合发挥 Glory 特点的是**长期存在、少量字段持续变化、状态能定位到局部节点**的界面，例如设备监控、持仓/订单局部状态、后台表格单元格编辑。后二者及表单联动是基于机制的候选场景，不是本轮新增实测。

如果 Dioxus 也合理拆分局部状态，本轮仍未测出 Glory 的纯执行速度优势；可以确认的是稀疏更新的差距缩到约 1.5–1.9 倍，而非旧批量创建的 4–5 倍。初始化更重、更新时仍有约二倍甚至更多差距，说明后续仍应优化 HTML/DOM 桥接、实例登记和创建/退休，而不是把“细粒度响应式”当作已经解决性能差距的证明。

不能用稳态项抹去初始化代价，也没有用各项中位数拼成声称实测的完整会话时间。本轮仅测浏览器，不外推至 Blitz、GTK、原生布局、网络吞吐或真实业务数据库。

## 复现

入口及依赖锁见 [scenarios/README](../benchmarks/scenarios/README.md)。原有 benchmark 文件和框架 API 不受影响。

```powershell
# 使用一个尚未构建过的目录：
./benchmarks/scenarios/run.ps1 -BuildRoot D:/BuildCache/glory-portfolio-repro
python benchmarks/scenarios/summarize.py D:/BuildCache/glory-portfolio-repro/results/evidence.json --out D:/BuildCache/glory-portfolio-repro/summary.json
```

有序身份补充脚本 `integrity.cjs` 需在同一 site/ 的只读服务存活时运行，使用 `--base` 和 D 盘 `--out`。本轮服务端口 29045；任务结束后关闭自有浏览器和服务。原始 CPU 绑核记录及三个构建日志均保存在上述 D 盘根目录。
