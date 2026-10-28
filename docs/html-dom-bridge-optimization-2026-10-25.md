# HTML / DOM 映射与桥接优化实测（2026-10-25）

在冻结基线上的本轮优化保留 DOM 原型的相邻节点寻址、Entry 物理根引用复用、普通节点直接放置和 patch 借用句柄。正式同批千行创建 / 替换 / 万行创建 Script 降低 **10.3% / 7.6% / 6.2%**，两个相反顺序的轮次均改善。清空约 -1.3%，不声称销毁有明显加速。

标准应用、数据、布局和 Bond/Vary 用法均未修改；两份独立 Memory、完整语义身份、全部物理节点及属性屏障保留。三个额外候选因收益不稳定或回退而撤回。

## 证据边界与复现

- 基线 Git：`e7f9567`，冻结 511 个输入。所有有效性能候选从这份快照派生，只有本轮 2–3 个 Web 文件不同；15 个 benchmark 输入逐字节相同。
- 目录：`D:/BuildCache/glory-dom-bridge-20261025`。8 个 Glory 版本加两个对照应用，共 10 份性能对照 release target；另有一份当前工作区独立构建，共 11 份；Cargo metadata 验证输出在 D:，owned compiler artifacts 均为 `fresh=false`。构建无产物互拷。
- Chromium 155.0.8059.84，i7-14700KF，1280×900、DPR 1、前台页；主计时浏览器进程统一固定同一 P-core 掩码 65535 / AboveNormal。
- 第一批 `primary` 1,296 个样本用于撤回 Vec 桥接；最终 `final` 1,512 个样本包含 before、Entry、Entry+links、numeric、stack、Leptos、Dioxus。每项 3 次热身、每轮 12 样本，两轮反向顺序，每格 24 样本，不剔除慢样本、不跨批混池。
- 主计时没有 DOM/WASM monkey patch 或 Profiler。桥接计数、DOM 计数、externref census 和 CPU 采样分别执行；完整回归在主计时结束后启动。共享桌面环境仍有运行波动，不能把某一轮绝对值当成产品保证。
- 中途工作区出现组件 API / 文件功能等并发改动，三个初次候选构建因此失败；未将这些输入用于性能比较。`*-clean` 均重新从冻结 before 派生，具体差异见源码审计。

[构建审计](D:/BuildCache/glory-dom-bridge-20261025/independent-build-audit.json)、[源码审计](D:/BuildCache/glory-dom-bridge-20261025/source-audit-final.json)、[最终原始样本](D:/BuildCache/glory-dom-bridge-20261025/final/evidence.json)、[汇总及 p10/p90](D:/BuildCache/glory-dom-bridge-20261025/final-summary.json)。`links-clean` 是保留实现，`second` 是独立正确性检查，不含计时样本。

## 实际改动

### 1. 静态 DOM 寻址只走一条物理边

既有实现已整棵 `cloneNode(true)`，并一次借用 DOM Entry 表登记实例。本轮没有重新实现深克隆。

旧版为每个 Part 保存从根出发的 childNodes 下标路径，每次克隆后重走祖先。现在在声明原型建立时，记录该 Part 的前兄弟或父节点在先前 Part 中的下标；实例通过 `nextSibling` 或 `firstChild` 找到它。检查拓扑次序，整个查找为 O(物理节点数)，无需每段取得 live NodeList。

实测标准 RowPlan 每行 11 个物理 Part，原来 19 次祖先路径访问，改为 10 次相邻节点访问。寻址信息仍是文档内、弱声明身份关联的有限原型元数据，沿用 64 声明 / 4,096 节点 / 1 MiB charged payload 上限。没有保留业务行或访问历史。

### 2. Entry 直接借用已拥有的物理根

普通节点的根已在固定 Parts 槽位中。之前 Entry 再各持有一份 first / last，新增两个 externref 句柄；现在 Boundary::Part 记录根 Part，借用其 Node。Group 使用 Boundary::Range，保留两端 comment，不增加布局包装。

普通节点放置直接借用根 Node，省去单元素 Vec 和范围相等比较；Text / Attribute / Property patch 借用已有句柄，保持 Element 类型检查和错误路径。受控输入的值、revision、selection、composition 屏障仍执行。

本轮没有修改 HTML field lowering、独立 Memory 校验、preflight、成功前缀/查询水位、调度及业务订阅语义。SSR / stream / hydration 的 HTML 格式版本仍为 13。

## 正式同批九项结果

ScriptDuration 中位数，单位 ms；完整 Task / Layout / Recalc / wall 分布见原始汇总。

| 操作 | Glory before | Glory 保留版 | 变化 | Leptos | Dioxus |
|---|---:|---:|---:|---:|---:|
| 创建千行 | 22.183 | 19.896 | -10.3% | 13.789 | 4.374 |
| 替换千行 | 32.326 | 29.855 | -7.6% | 21.116 | 7.561 |
| 更新每十行 | 0.561 | 0.534 | -4.9% | 0.347 | 0.868 |
| 选行 | 0.648 | 0.643 | -0.8% | 169.824 | 0.733 |
| 交换行 | 0.618 | 0.610 | -1.3% | 0.174 | 0.908 |
| 删除行 | 0.672 | 0.659 | -1.9% | 0.272 | 0.805 |
| 追加千行 | 23.694 | 21.557 | -9.0% | 15.846 | 5.619 |
| 创建万行 | 246.186 | 230.928 | -6.2% | 175.016 | 49.419 |
| 清空千行 | 15.255 | 15.050 | -1.3% | 17.271 | 2.646 |


| 操作 | 正序 before / 保留 | 反序 before / 保留 |
|---|---:|---:|
| 创建千行 | 23.417 / 20.443 | 20.479 / 19.501 |
| 替换千行 | 33.922 / 30.513 | 30.505 / 29.177 |
| 创建万行 | 245.080 / 229.922 | 248.848 / 234.945 |

Leptos 的选行 169.824 ms 是连续九项流程中的既有运行历史异常，保留原值；不能据此概括普通 Leptos 选行或宣传数百倍优势。

| 操作 | before Task | 保留 Task | before wall | 保留 wall |
|---|---:|---:|---:|---:|
| 创建千行 | 49.649 | 46.463 | 48.500 | 45.300 |
| 替换千行 | 63.098 | 59.598 | 58.550 | 56.600 |
| 创建万行 | 514.920 | 493.994 | 504.200 | 487.750 |

wall 是 click→两个 RAF 的外部等待；各指标分别取中位数，不能相加推导或换算 FPS。当前 Glory Script 仍约为 Leptos 的 1.32–1.44 倍、Dioxus 的 3.95–4.67 倍，差距仍然明显。

## 调用、句柄和采样证据

千行状态的独立计数：

| 指标 | before | 保留版 |
|---|---:|---:|
| create 的 WASM→JS 导入调用 | 20,033 | 19,033 |
| replace 的导入调用 | 23,033 | 21,033 |
| clear 的导入调用 | 3,033 | 2,033 |
| 存活 Node 句柄 | 27,070 | 11,054 |
| 上述句柄指向的不同 Node 对象 | 11,025 | 11,025 |
| externref table capacity | 33,796 | 17,412 |

Boundary 新增判别信息，未实测 Entry 总堆字节；以下仅报告直接句柄和槽位，不推断总内存。存活 Node 句柄约少 59.2%，是重复引用减少，不是 DOM 节点或语义身份减少。table capacity 是槽位数，不是 RSS；JS 对象内部引用与原型持有的后代不包含在这项直接槽位 census 内。当前 wasm-bindgen 使用 externref 表，句柄 clone/drop 也有 WASM 内部表操作，不能将每次句柄复制都说成一次 JS 导入。

千行 create 的实际 DOM 写入两版均为：1,000 次深克隆、9,000 次 setAttribute（8,000 个完整身份 + 1,000 个动态 class）、1,000 次 insertBefore、2,000 次 nodeValue。没有减少场景要求的写入。WASM 文件 1,870,421→1,871,473 B（+1,052 B，约 +0.06%）。

实际 DOM 写入见 [DOM 计数](D:/BuildCache/glory-dom-bridge-20261025/dom-counts.json)，WASM 导入和句柄见 [桥接计数](D:/BuildCache/glory-dom-bridge-20261025/bridge-counts-final.json)，寻址次数见 [原型计数](D:/BuildCache/glory-dom-bridge-20261025/plan-path-counts.json)。

独立 unwrapped 100 µs CPU 采样、每场景 3 次：Mapper::apply inclusive 的千行创建 16.146→14.381 ms、替换 23.809→21.899 ms、万行创建 195.680→170.819 ms。万行 gloryClonePlan inclusive 86.281→70.685 ms；千行 create 的该 JS 子项反而 5.365→7.136 ms，短采样噪声较大，不把它解释成所有子项均提速。inclusive 互相嵌套，不能求和。采样仅用于定位，与正式 CDP 计时不混用。

[原始采样](D:/BuildCache/glory-dom-bridge-20261025/profile-retained/evidence.json)、[采样汇总](D:/BuildCache/glory-dom-bridge-20261025/profile-retained-summary.json)。

## 撤回实验

1. **Vec<JsValue> 批量返回。** 删除每行 11 次 Array.get 导入，create 导入从 20,033 降到 9,033，但加入 ABI 缓冲分配与句柄复制。生成的 `passArrayJsValueToWasm0` 对每个元素通过 `addToExternrefTable0` 反向调用 `wasm.__externref_table_alloc`。实测 JS→WASM 的该导出调用为 1,010→12,010，malloc 导出调用为 2→1,002；只统计单向导入会漏掉这些额外往返。第一批千行创建 20.628→22.013 ms（+6.7%），万行 242.685→263.841 ms（+8.7%）。不因调用数更漂亮而保留。
2. **u32 slot/generation 对传入 JS 拼完整身份。** 最终千行创建 19.476 ms 比 links 更快，但替换 30.190 ms、万行 235.698 ms 比 links 慢，正反顺序也出现反向变化。完整 u64 runtime 字符串和 u32 边界的辅助检查通过，仍因收益不一致撤回。
3. **有界栈缓冲保存数字对。** 小声明避免一次临时 Vec，较大声明有 spill；实测千行创建 / 替换 / 万行 20.484 / 31.100 / 240.209 ms，不如保留版，撤回。

最终仍用原有完整十进制身份编码和 js_sys::Array 返回；逐 Part Array.get 仍在。[反向桥接计数](D:/BuildCache/glory-dom-bridge-20261025/reverse-bridge-counts.json) 独立包装命名分配导出，只统计 JS 到导出的调用，不包括 WASM 内部调用。减少桥接必须同时衡量 ABI 分配、复制及代码生成，不能只数调用。

## 正确性和完整回归

完整回归分别在原冻结基线及追加的当前 `9615776` 快照执行；最终以下记录来自当前快照。当前 benchmark 入口由并发迁移改为 `widget`，额外独立构建通过，[当前源码审计](D:/BuildCache/glory-dom-bridge-20261025/current-source-audit.json) 验证三个 Web 实现文件与工作区完全一致；这份源集仅追加正确性检查，没有混入上方性能样本。

- Windows 全 workspace / all-features：**903 passed**；native / wasm32 all-targets 严格 Clippy、fmt、Blitz minimal / paint 检查通过。另有真实窗口及 UIA 各 1 项通过。
- Linux 全 workspace / all-features：**902 passed**；严格 Clippy、minimal / paint、CPU raster 通过；X11 与 Wayland 各 shell / accesskit 两项实际窗口验收，共 4 项通过。
- 两个引擎 fork / 六个包来源及锁文件核对通过，fork-owned tests 已在单独的干净 Git checkout 运行。主 workspace 始终为 10 个包。没有 vendor、Git rev 或引擎实现变更。
- 浏览器 **27/27** 在当前 fixture 一轮全绿；fixture native tests 6 项、独立 WASM/SSR fixture 构建和真实 shell/patch/end 传输通过。涵盖 SSR、stream、hydrate、受控输入、composition、focus、CSS、queries、Owner 清理等既有验收。
- 标准 benchmark 的全部存续 Element/Text 引用和语义 ID、被替换行退休、全部 10,000 行矩形、六个视口 PNG 哈希一致；当前工作区独立版本也与保留版本通过相同完整身份、几何和像素对照。清空后 UI 10 / graph 3。
- 新增 File 多 Part / Control 根的 CSR、SSR adoption、keyed move、清理与三部件引用检查；18 节点计划全部身份唯一、文本正确。保留原有原型污染隔离、Group fallback、输入选择和 composition 检查。
- 真实变高列表缓存 **64 / 1024** 各 70 次小数滚动检查：锚点 top 恒为 **-15.125 px**、ID 均为 `2.59.1`，包含 ±0.375 px；实际行高 24.375 / 32.375 / 40.375 px。maxRows 28、maxNodeSlots 105、maxGraphSlots 185，测量条目分别受 64 / 1024 约束；最终物理/逻辑视口、行几何和全部 PNG 哈希一致。原来的严格容差未修改。
- Windows/Linux CPU PNG 已生成并检查；两平台字体可能不同，不做跨平台逐像素等同或 OS IME / 屏幕阅读器验收声明。

回归编译 target 与临时文件均在 D:；Linux target 位于已验证 D: backing file 的 loop 文件系统。回归禁用 debug symbols / incremental，保留 debug assertions；这与独立 release 性能构建分开。

完整原始证据：[Windows tests](D:/BuildCache/glory-dom-bridge-20261025/current-check-logs/native-tests.txt)、[Windows metadata](D:/BuildCache/glory-dom-bridge-20261025/current-check-logs/windows-metadata.json)、[WASM Clippy](D:/BuildCache/glory-dom-bridge-20261025/current-check-logs/web-clippy.txt)、[Linux log](D:/BuildCache/glory-dom-bridge-20261025/linux-current.stdout)、[浏览器 27 组](D:/BuildCache/glory-dom-bridge-20261025/browser-current-final/evidence.json)、[真实传输](D:/BuildCache/glory-dom-bridge-20261025/browser-current-final-stream.json)、[当前 benchmark 完整身份](D:/BuildCache/glory-dom-bridge-20261025/current-integrity/evidence.json)、[当前 benchmark 六处像素](D:/BuildCache/glory-dom-bridge-20261025/current-acceptance/evidence.json)。

复核中处理的非性能问题：

1. 冻结 fixture 曾混入新 API 名称，恢复对应旧名称后通过；当前 fixture 中两个模块自身也导出 widget，改用 5 处限定的 glory::ui::widget 构造调用，消除名称遮蔽。
2. 新增 File 测试曾把 SSR 专用 data-gly-part 当作 CSR 必有属性；实际 CSR 不承诺它。改验 span/display:contents 结构、Main/Control/Picker 相同完整身份、实际对象引用及挂载位置，未改 renderer 或放宽几何/身份条件。首次 26/27、单项修正及最终全量 27/27 记录均保留。
3. 追加当前快照时曾复用检查目录，Cargo 旧 rmeta 造成不存在的 widget 导出错误。性能构建始终独立，不受影响；随后 Windows 和 Linux 均改用当前快照专属的全新 target，重跑全部检查通过。失败记录保留，不把它算成代码修复收益。


## 剩余优先级

1. 继续在 Mapper::apply 内区分 Memory、实例登记、动态字段首写和宿主 clone 成本；保留独立校验与成功前缀。现有 profile 是包含时间，不能直接把其总量归罪于某个集合。
2. 将新传输 ABI 或宿主批处理作为窄实验，必须记录分配/复制成本、属性屏障、query barrier 与逐前缀失败。本轮 Vec 实验已证明“更少调用”不是充分条件。
3. 标准 RowPlan 尚未声明固定 bench-row，首个响应式 class 写入有真实作用；不能按静态重复属性跳过。若把固定 class 或直接属性闭包迁入应用，应另建应用迁移 cohort，不与本轮内部优化混算。
4. 双份 Memory、Owner/Binding 和全部节点身份保持；没有证据支持通过删掉这些语义契约来追赶 Dioxus。后续创建/退休优化仍需同一 workload 的独立 A/B。
