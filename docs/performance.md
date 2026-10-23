# Performance Notes

## Scope cleanup and sibling unlink — 2026-10-23

本轮继续优化 UI Scope 的逐节点清理及 `Memory::unlink`。普通 keyed `each`
按旧显示顺序退休：旧 `position` 通常首项即命中，但每次 `Vec::remove(0)` 都搬移
余下兄弟，连续清空形成二次方工作。另一个重复成本是 Scope 先收集完整子树再清理，
每个节点分别更新节点 Owner 和监听 Owner，空记录还需第二次哈希删除。

已实现：

- render 新增只读切片接口的 `Children`：前方删除推进游标，前插可复用空前缀；
  已移除的数值前缀超过 `live + 64` 才压缩，空序列立即清除。因此物理长度有界于
  `2 * live + 64`，连续前删摊销线性。Clone、迭代、Debug 仅暴露活跃身份。
  容量沿用峰值；不保留业务数据或所有访问过的行。任意中部删除仍为线性。
- `unlink` 直接判断首尾，并在修改前取得 next，修复前驱时省掉再次查询被移节点。
  Place-before-first 也走直接路径。根列表仍提供切片；公开 `Node.children`
  从 Vec 改为 Children，需要 Vec 快照的调用方使用 `to_vec()`。
- Scope 使用同一 DFS 栈遍历和清理，省去第二份完整身份数组；同一 Owner 的节点
  和监听兴趣一起更新，通过 occupied entry 移除空记录。其他 Scope 注册的监听、
  独立查询及活跃 Owner 边界保持原语义。
- 已提取整个 Owned 记录的当前 Scope 直接跳过对应索引项。Rust 1.98 的 std
  使用 hashbrown 0.17.1，缺失键的 `entry()` 会先 reserve(1)，即使不插入；
  代码避免这条无用路径。来源与哈希见 `stdlib-entry-evidence.json`。

用户回调顺序未调整，捕获依然在图/呈现借用外释放。算法回归用 Vec oracle 检查
混合前/中/尾编辑、32k 退休的压缩界和摊销搬移量，以及压缩后的移动/快照/代际复用。
新增 UI 回归覆盖本地和跨 Scope 监听注销、析构重入与存活外部节点身份。

按实现计算，每份 Memory 中连续前删的身份搬移数如下；它不是计时、RSS 或内存带宽测量：

| 行数 | 原 Vec 搬移身份数 | Children 搬移身份数 |
|---:|---:|---:|
| 1,000 | 499,500 | 737 |
| 10,000 | 49,995,000 | 9,529 |
| 32,768 | 536,854,528 | 32,183 |

基线 239 个、新版 240 个冻结输入；新增 Children，实现改动只在 render memory 和
UI app，其余差异是回归测试及一个 Vec 快照调用。两个版本在独立 D: target 全新
release 构建：opt-level=3、LTO、单 codegen unit、wasm-bindgen/CLI 0.2.129、
无 wasm-opt。应用、锁文件、种子、数据生成器、CSS、计时脚本均相同。
完整回归使用与最终被测构建逐字节相同的 240 个输入。

主对照仍是 `semantic-rows-v1`，完整挂载每行，三次预热、12 次采样、正反两轮，
每个版本/操作 24 样本，合计 **432 样本**。Chrome 155.0.8059.84，1280×900、scale=1。
默认调度出现较大波动且曾有并行编译，因此主对照将本轮 Chrome 进程固定在本机
已核实的 P 核（逻辑 0–15，mask 0xffff），优先级 AboveNormal；两版条件相同。
CPU 拓扑、每个 context 的进程设置和独立驱动均归档。没有更改其他任务的设置。
它与历史默认调度数字不可直接拼接；也不证明系统完全空闲。

中位 ms；Wall 为点击到两次 RAF，Script 为 CDP 脚本计数器差值，包含共同桥接开销。

| 操作 | Wall 前 → 后 | Script 前 → 后 | Script 降低 |
|---|---:|---:|---:|
| create_1k | 100.65 → 101.60 | 72.75 → 73.57 | -1.1% |
| replace_1k | 123.40 → 119.50 | 94.51 → 93.02 | 1.6% |
| update_10th | 12.20 → 10.75 | 0.70 → 0.66 | 6.0% |
| select_row | 8.05 → 8.05 | 1.04 → 1.03 | 1.3% |
| swap_rows | 10.80 → 11.10 | 0.60 → 0.57 | 4.7% |
| remove_row | 10.85 → 11.40 | 0.70 → 0.64 | 8.5% |
| append_1k | 106.80 → 104.75 | 76.21 → 73.22 | 3.9% |
| create_10k | 1395.00 → 1031.50 | 1027.60 → 752.14 | 26.8% |
| clear_1k | 44.85 → 33.50 | 43.06 → 31.24 | 27.5% |

清空千行的 Script 合并中位数 **43.06 → 31.24 ms（-27.5%）**；两个顺序分别
45.57 → 31.17、35.70 → 31.48 ms，方向一致但幅度有明显差异。
替换千行仅 **94.51 → 93.02 ms（-1.6%）**，两个顺序为 101.90 → 96.17、
91.42 → 91.18 ms，没有稳定的大幅收益。create_10k 的 runAll 分数含前序反复
创建/清空历史，不能仅凭此表把其差异归因于纯创建路径。

另行包装 DOM 方法/CPU 采样的六案例诊断不进入主计时：

| 诊断 | Busy sampled 前 → 后 | Owner 含回调前 → 后 | UI Scope 前 → 后 | Memory::apply 前 → 后 |
|---|---:|---:|---:|---:|
| replace_1k | 134.75 → 120.00 | 14.49 → 11.36 | 11.78 → 8.69 | 16.54 → 10.88 |
| clear_1k | 28.62 → 26.17 | 12.99 → 12.30 | 10.33 → 10.20 | 3.80 → 4.26 |
| clear_10k | 344.68 → 313.66 | 154.77 → 133.55 | 119.60 → 97.45 | 70.88 → 41.23 |
| clear_1k_after_10k | 32.65 → 33.05 | 13.61 → 13.26 | 11.46 → 9.00 | 3.32 → 5.19 |

万行清空的 Memory::apply 样本约 70.88 → 41.23 ms，Scope 约 119.60 → 97.45 ms。
旧 unlink 独立帧约 17.94 ms，新版未采到独立帧；内联影响归属，不能解释成零成本。
上述函数族是包含式、有重叠，不可相加，也不是重复计时中位数。

为定位替换，另外做了不包装 DOM 方法的正反两轮采样，16 个诊断/版本，
同样固定 P 核；这些也不混入主分数。包含式样本中位数：

| 替换诊断函数族 | 前 | 后 |
|---|---:|---:|
| Owner 含回调 | 14.32 | 12.55 |
| Scope 清理 | 10.97 | 9.22 |
| 独立 unlink 帧 | 0.53 | 0.53 |
| DOM sink | 49.86 | 47.32 |
| Element build | 17.15 | 17.32 |
| Memory apply | 17.82 | 13.80 |

替换使用全部新 key，仍需销毁旧行并创建 8,000 个元素、3,000 个文本节点、
写 32,000 次属性。诊断中 DOM sink 约 47–50 ms，Element build 约 17 ms，
Owner 含回调仅约 13–14 ms，独立 unlink 帧不到 1 ms。
因此本轮主要缩短的退休阶段只占替换的一部分；完整创建、映射、字符串转换和 DOM
提交继续占大头。函数族有包含关系，不能据此拆出相加的精确比例。
采样诊断本身也有开销，不能拿它的 Script 绝对值代替主对照。

附加保留了 1,296 个默认调度样本：初版候选、修正版、并行编译结束观察期各 432 个，
与受控主对照分开。初版候选替换明显变慢，后续发现并避开缺失 entry 容量准备；
但源码事实不等于证明它导致了全部回退，默认调度/热身/运行历史也存在明显波动。
这几组数据和全部不利结果均保留在 paired、final-paired、confirmed-paired，
主数据在 pinned-paired；不混合调度条件，也不只挑最短耗时。

验证结果：

- 每个计时样本核对数据摘要；更新/选择/交换/删除/追加保留 Element、Text Node 和
  语义身份；替换后旧后代断开；万行几何摘要和六个视口 PNG 检查点一致，含万行中部。
- 六个诊断案例的 DOM 方法/属性次数及图、Owner、节点数量一致；清空回到
  0 行/label、10 语义节点、3 图节点、13 Owners。
- Windows/Linux 各 **434** 项工作区测试、各 **289** 项 fork 测试通过；原生/wasm
  严格 Clippy、fmt、Linux 无默认 feature 和 paint 检查通过。
- Windows shell/accesskit 真实窗口及双窗口 UIA、Linux X11/Wayland × shell/accesskit
  四次窗口探针和 CPU raster 通过；这些不替代 Unix AT-SPI/屏幕阅读器验收。
- 浏览器 **24/24** 套件通过；64/1024 测量缓存各 70 次小数更新、真实行高、
  原有 0.1/0.15 容差、节点身份及各 73 张锚点截图通过，最终位置/画面一致。
- SSR fixture 四项测试、流式 Shell/Patch/End、资源交接、断连取消及慢消费者背压通过。

新增游标和已移除的数值前缀属于额外元数据，容量不主动收缩；以上节点数/身份/像素证明
不等于 RSS 降低。后续中间批量删除若成为主路径，需要另行评估有序结构/批量过滤；
本轮改善的是保持退休顺序的连续前删。替换的下一重点应是创建/HTML lowering/DOM
提交的分配与转换开销，并保持新 key 的身份和销毁契约。

冻结版本验收完成后，工作树另有并行新增的 input.rs 与 mod input 声明，提供两个输入辅助 API。
该独立新增未混入性能基线；已保留，并另冻结 241 输入的 integration/source，
补做 Windows 全工作区 434 项测试及原生/wasm 严格 Clippy。
随后新增的 widget_input.rs 两项测试也单独通过，未混入上述 434 项冻结套件。
原来的双平台/浏览器全套验收对应最终被测的 240 输入版本，不将它扩展为新增 API 的行为证明。

完整源码/构建哈希、四组原始分数、CPU profiles、DOM 计数、截图、进程设置和全回归：
`D:/BuildCache/glory-scope-unlink-20261023/evidence-index.json`。
`evidence.py` 重验所有来源、样本、身份/画面及回归；`profile-summary-pinned.py`
重算函数族。复现主对照用独立 before/final 构建，目录 HTTP 服务和已归档的
`bulk-pinned.cjs`/`pin-browser.ps1`；TEMP/TMP 设为该目录 pinned-tmp，计时使用原有
bench-harness.js/browser-run.js。其他完整验证入口为 run-windows.ps1、linux-base.sh、
linux-window-root.sh 和 scripts/test-semantic-browser.cjs。

## Owner and Binding retirement follow-up — 2026-10-23

本轮在 DOM 父表修复后冻结新基线，继续定位 Owner/Binding 销毁。清空万行的
基线 CPU 诊断中，`Owner::dispose` 含回调约 155.29 ms，其中 UI Scope 清理
约 127.71 ms；核心 `State::dispose_owner` 约 18.06 ms，`remove_node`
约 5.30 ms。后两者是包含关系，所有包含式统计均不可相加。
Owner 栈包含语义树、监听、查询和组件资源清理，不能全部归因于响应式图。

实际优化：

- `glory-core` 先用代际身份取出 Owner。重复退休直接返回，叶 Owner 不再先分配
  `vec![key]`，有孩子才扩展遍历栈。完整退休后执行回调、子先父后/反向注册顺序、
  借用外释放捕获，以及任务取消规则保持不变。
- `glory-ui` 将同一 Instance 的节点、监听、查询集中到一个 `Owned` 记录。
  最后一个兴趣退休时移除记录，后续 Scope 回调一次未命中即可返回。
  外部节点上的监听、外部目标的查询仍归注册 Scope，三类兴趣互不代替。
  `UiStats` 独立计数节点 Owner 和查询 Owner，不扫描所有 Owner 来计算。

Binding 仍从实际存活来源注销，立即取消调度成员，在图借用之外释放捕获。
此处没有全图 retain；双向边注销和用户析构是必要工作。队列代际校验、FIFO、
阶段顺序及 `2 * live + 64` 物理条目上限保持原契约。

两份冻结源码各 239 个输入，仅两个实现文件、两个回归测试文件不同。
各自从独立 D: target 全新构建，opt-level=3/LTO/单 codegen unit，
wasm-bindgen/CLI 0.2.129，无 wasm-opt。`semantic-rows-v1` 应用、锁文件、
生成器、种子、CSS、计时脚本完全相同，每行完整挂载。
Chrome 155.0.8059.84，1280×900，scale=1；正反顺序各一轮，独立 context，
每项三次预热、12 次采样：**432 样本，每个版本/操作 24 样本**。
回归冻结树与被测 release 的全部 239 个输入逐字节一致，无后续实现修补。

中位数，单位 ms。Wall 为点击到两次 RAF；Script 为 CDP 脚本计数器差值，
包含共用驱动开销。本机另有独立 Cargo 任务；本次对照不代表专用空闲机器。

| 操作 | Wall 前 → 后 | Script 前 → 后 | Script 降低 |
|---|---:|---:|---:|
| create_1k | 91.35 → 91.45 | 64.99 → 65.23 | -0.4% |
| replace_1k | 118.35 → 121.55 | 92.11 → 94.39 | -2.5% |
| update_10th | 11.75 → 12.35 | 0.74 → 0.68 | 8.7% |
| select_row | 8.50 → 10.05 | 1.13 → 1.07 | 5.4% |
| swap_rows | 10.90 → 9.60 | 0.58 → 0.56 | 4.1% |
| remove_row | 11.30 → 8.15 | 0.62 → 0.62 | 0.2% |
| append_1k | 100.35 → 100.80 | 71.88 → 70.11 | 2.5% |
| create_10k | 1061.25 → 1058.55 | 775.26 → 767.35 | 1.0% |
| clear_1k | 37.65 → 34.85 | 34.35 → 32.05 | 6.7% |

清空千行在两个顺序中均改善，分别约 10.4% 和 6.6%；合并后的 Script 中位数
**34.35 → 32.05 ms，降低 6.7%**。替换千行合并后慢 2.5%，第二轮 after
波动尤其大（116.04 ms，对照 93.00 ms），不能称为替换优化。
创建万行的 Script 差异约 1%，没有足够证据声称创建提速。
小操作的 RAF 差异也不作为架构收益。

另行包装 DOM 方法及 CPU 采样的诊断，不进入上述 432 个计时样本：

| 诊断 | Busy sampled 前 → 后 | Owner 含回调前 → 后 | UI Scope 清理前 → 后 |
|---|---:|---:|---:|
| replace_1k | 123.16 → 112.10 | 13.65 → 12.20 | 11.39 → 11.15 |
| clear_1k | 30.47 → 26.75 | 15.22 → 13.78 | 12.11 → 9.48 |
| clear_10k | 320.74 → 311.47 | 155.29 → 142.28 | 127.70 → 108.97 |
| clear_1k_after_10k | 34.07 → 33.03 | 14.36 → 14.64 | 10.11 → 10.88 |

万行 Owner 含回调样本降低约 8.4%，UI Scope 清理降低约 14.7%。
同次核心 `State::dispose_owner` 为 18.06 → 21.13 ms，`remove_node` 为
5.30 → 5.74 ms，核心部分未显示稳定降低。两项修改合并测量，未做单项消融，
不能给免分配快路径单独分配收益。编译内联/采样粒度影响函数归属，
诊断不能代替重复计时分布。

六个诊断案例的 DOM 方法、属性写入次数和节点/图统计完全一致。
每千行增加 8,000 语义节点、6,000 图节点、9,000 Owners；替换仍创建
8,000 个元素、写 32,000 次属性、删除 1,000 个行根。
清空回到 0 行/label、10 语义节点、3 图节点、13 Owners。
合并记录的单条载荷更大，活跃条目和节点数不等于 RSS；本轮不声称内存字节降低。

一致性与完整回归：

- 所有计时样本核对种子数据；更新、选择、交换、删除、追加保留存活 Element、
  Text Node 和语义身份，替换后旧后代断开，万行几何摘要一致。
- 六个视口 PNG 检查点逐字节相同，含万行中部；两份构建的 DOM 调用计数一致。
- Windows/Linux 各 430 项工作区、各 289 项 fork 测试通过；原生/wasm 严格
  Clippy、fmt、Linux 无默认 feature/paint 检查通过。
- Windows shell/accesskit 两次真实窗口探针和双窗口 UIA 通过；Linux
  X11/Wayland × shell/accesskit 四次真实窗口探针及 CPU raster 通过。
- Chromium 24/24 套件通过；64/1024 测量缓存各 70 次小数更新、真实行高、
  原有 0.1/0.15 容差、节点身份及各 73 张锚点截图通过，最终位置和画面一致。
- SSR fixture 四项测试及流式 Shell/Patch/End、资源交接、断连取消、
  慢消费者背压通过。
- 新增测试覆盖重复退休后的 Binding 槽位复用、回调顺序、捕获析构重入，
  以及目标退休后的独立查询完成、兴趣清空后的重新注册、退休请求不再回调。

剩余优先项是 UI Scope 中逐节点索引操作，以及语义 `Memory::unlink` 的
兄弟位置搜索/Vec 搬移；万行诊断后端 DOM sink 仍约 108 ms。
依赖边和捕获释放可继续细分，但当前纯核心占比不支持优先进行更大的生命周期重写。

原始样本、构建/源码哈希、CPU profiles、DOM 计数、截图和完整日志：
`D:/BuildCache/glory-owner-retirement-20261023/evidence-index.json`。
`evidence.py` 校验冻结源码、样本、像素、身份、回归及产物哈希；
`profile-summary.py` 重算包含式函数族时间。
复现使用该目录 before/release/source，以 `benchmarks/build.ps1` 指定独立
BuildRoot，启动目录 HTTP 服务，再执行 `bulk-compare.cjs`、`--integrity`
及 `summarize-bulk.py`。完整验证入口为已归档的 `run-windows.ps1`、
`linux-base.sh`、`linux-window-root.sh` 和 `scripts/test-semantic-browser.cjs`。

## DOM retirement follow-up — 2026-10-23

The parent lookup added for DOM keyboard/drag routing used `HashMap::retain`
for every disposed subtree. Clearing a keyed list disposes many row subtrees,
so each row scanned unrelated live parent entries and the table's retained
capacity. Repeated row teardown could approach quadratic work. A 10k run also
left a larger table allocation for later 1k clears; the main benchmark's setup
includes those expensive 10k clears outside the timed action interval.

`Dom::dispose` now removes parent and pending-size entries by each `NodeId`
in the mapper's complete semantic subtree. It includes parked descendants;
no source scan, new cache, ID lifetime change or capacity shrinking is involved.
The normal input/layout-queue filters and observer retirement remain in place.
The row workload does not subscribe to Resize, so the large regression here
comes from the parent lookup rather than a populated measurement cache.

The before/release snapshots differ only in `crates/web/src/browser/dom.rs`.
Both were freshly built in independent D: release targets, opt-level=3/LTO/one
codegen unit, wasm-bindgen/CLI 0.2.129 and no wasm-opt. The benchmark app, seed,
CSS, data generator and timing harness are byte-identical. Chrome 155.0.8059.84,
1280×900, scale 1; before→release then release→before, fresh contexts, three
warmups and 12 measured samples per operation/round: **432 samples**, 24 per
variant/operation. These are contemporaneous paired results, not reused values
from the earlier three-framework run.

Median ms. Wall is click-to-two-RAF; script is the CDP counter delta including
common bridge/harness work. This is a local full-DOM benchmark, not FPS,
presentation latency, RSS or an official HTML-table score.

| Operation | Wall before → after | Script before → after | Script reduction |
|---|---:|---:|---:|
| create_1k | 109.75 → 103.40 | 78.69 → 73.10 | 7.1% |
| replace_1k | 221.50 → 143.80 | 195.08 → 110.52 | 43.3% |
| update_10th | 11.20 → 11.10 | 0.68 → 0.70 | -3.0% |
| select_row | 8.05 → 8.10 | 1.10 → 1.12 | -2.0% |
| swap_rows | 10.85 → 11.10 | 0.59 → 0.64 | -8.1% |
| remove_row | 11.10 → 10.25 | 0.95 → 0.67 | 28.8% |
| append_1k | 113.50 → 110.15 | 73.07 → 77.07 | -5.5% |
| create_10k | 1131.60 → 1089.40 | 788.80 → 814.76 | -3.3% |
| clear_1k | 150.15 → 40.30 | 148.41 → 36.23 | 75.6% |

The improvement is concentrated in replacement and teardown. Creation,
append and tiny updates are essentially unchanged; do not treat their small
percentage differences as gains from this patch.

Separate diagnostic runs wrap DOM methods and use a sampling profiler. They
are excluded from the 432 timing samples and are not additional median scores:

| Diagnostic | Busy sampled ms before → after | DOM sink inclusive ms before → after |
|---|---:|---:|
| replace_1k | 206.09 → 141.09 | 141.75 → 77.74 |
| clear_1k | 103.15 → 26.16 | 80.23 → 8.01 |
| clear_10k | 10227.81 → 386.48 | 9528.39 → 119.39 |
| clear_1k_after_10k | 211.88 → 34.88 | 179.26 → 12.26 |

The 10k clear previously spent about 9.53 s inside the DOM sink; after the
change it spends about 119 ms there. Owner disposal is still present (about
177 ms inclusively in that optimized diagnostic) and is now a larger part of
the remaining cost. Inclusive families overlap and must not be added together.
The profiler resolves the adapter sink rather than a precise inlined retain
instruction; the paired source change and unchanged native-operation counts
are the evidence for the parent-table attribution.

All six diagnostic cases have identical native call/attribute counts and live
DOM/graph census across variants. Replacement creates 8,000 elements, writes
32,000 attributes and removes 1,000 row roots in both; clear-1k removes 1,000
roots, clear-10k removes 10,000. Speedup comes from less metadata work.

Correctness checks passed for both builds: seeded row data for all operations
and timing samples, surviving Element/Text Node/semantic IDs through update,
select, swap, remove and append, old descendants disconnected on replacement,
all 10,000 row rectangles equal, six viewport PNG checkpoints byte-identical
(including the middle of 10k rows), and cleanup back to zero rows/labels,
10 semantic nodes, 3 reactive nodes and 13 Owners.

Full regression on the frozen validation tree:

| Gate | Result |
|---|---|
| Windows workspace / fork tests | 428 / 289 passed |
| Linux workspace / fork tests | 428 / 289 passed |
| Browser | 24/24 suites passed |
| Windows real windows / UIA | shell + AccessKit windows and actual two-window UIA client passed |
| Linux real windows / paint | X11/Wayland × shell/AccessKit, CPU raster passed |
| Formatting / Clippy | validation workspace, native and WASM all targets, fixture passed |
| SSR / streaming | 4 fixture tests, actual progressive delivery, cancellation and slow-consumer backpressure passed |

The browser regression also reruns the real fractional-height lists with 64
and 1024 entries: both maintain zero anchor drift through 70 fractional changes,
live descendant/Text identities, equal settled PNGs and identical final logical
and physical Home positions. The original 0.1/0.15 px checks are unchanged.

The validation tree started as a copy of the measured release source. Seven
files were normalized with rustfmt. Full-feature Clippy then required equivalent
let-chains in existing native/Web drag and theme branches; no-shell Clippy
required gating private desktop drag state/initializers by `shell`. The fixes
are also in the working tree. `native-lint-fix.py`, `web-lint-amendments.json`,
`dom-lint.diff` and `drag-lint.diff` preserve the exact amendments. The initial
and second failed attempts remain in `windows-initial.txt`, `linux-initial.txt`,
`windows-second.txt` and `linux-second.txt`; only subsequent complete runs count
as accepted. Later parallel worktree edits are excluded.

After those gate fixes, `final/source` was built independently again. Its
benchmark/library input hashes exactly match the accepted validation tree.
The same workload passed another **108 samples**, six equal PNG checkpoints
and a full descendant/Text/semantic identity and 10k-geometry differential
against the measured release build. This is a final-source verification run,
separate from the 432-sample causal experiment above:

| Final-source verification | Wall median | Script median |
|---|---:|---:|
| replace_1k | 119.75 ms | 91.76 ms |
| clear_1k | 40.40 ms | 36.95 ms |
| create_10k | 1053.40 ms | 767.60 ms |


Everything is under `D:/BuildCache/glory-dom-retirement-20261023/`:
`before/source/inputs.json`, `release/source/inputs.json`, `validation/inputs.json`,
`paired/evidence.json`, `integrity/evidence.json`, `browser/evidence.json`,
`final/source/inputs.json`, `final-benchmark/evidence.json`, `summary.json`,
raw CPU profiles and final platform logs.
The [evidence index](/D:/BuildCache/glory-dom-retirement-20261023/evidence-index.json)
verifies sources, independent output directories, data/geometry/picture hashes,
method counts, accepted regressions and retained failed attempts.

## Current three-framework rerun — 2026-10-23

The current Glory snapshot was rebuilt alongside **Leptos 0.8.19** and
**Dioxus 0.7.9**, using fresh independent release targets on D:, opt-level=3,
LTO, one codegen unit and wasm-bindgen/CLI 0.2.129; no wasm-opt. All three
strict WASM Clippy and app formatting checks passed. The 15 shared/app build
inputs, including harness, data, CSS and lockfiles, are byte-identical to the
earlier semantic-rows-v1 comparison. Only Glory library sources differ.

Chrome **155.0.8059.84**, Windows/MSVC Rust 1.98.0, Intel Core i7-14700KF,
1280×900, scale 1. Two sequential rounds used fresh contexts in opposite order:
Leptos→Glory→Dioxus, then Dioxus→Glory→Leptos. Each operation warmed three
times and measured 12 samples per round: **24 samples per cell, 648 total**.
The table pools both rounds and reports medians. The browser had its own
process/profile; this is a local machine run, not an isolated lab measurement.
Sources were frozen before building. Later concurrent working-tree changes
did not enter these binaries; their paths are recorded in `validation.json`.

All three passed the independent seeded-data oracle, nine operations,
keyed swap/replacement checks, all 10,000 real row boxes and cleanup checks.
Every measured sample's final row hash/count agrees across frameworks and
rounds. All six create-1k checkpoint viewport PNGs are byte-identical.

Median **click to two foreground RAF callbacks**, ms, lower is better.
Setup/data validation is outside timing; this does not establish actual
display completion, FPS or an official HTML-table score.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 90.30 | 40.00 | 32.65 |
| replace_1k | 306.55 | 47.70 | 33.35 |
| update_10th | 14.35 | 15.40 | 11.25 |
| select_row | 9.10 | 267.70 | 8.10 |
| swap_rows | 10.60 | 10.65 | 9.70 |
| remove_row | 12.50 | 13.90 | 10.55 |
| append_1k | 177.95 | 89.65 | 37.95 |
| create_10k | 1625.25 | 739.20 | 341.85 |
| clear_1k | 152.65 | 21.60 | 9.10 |

Median CDP **ScriptDuration counter delta**, ms. This includes common
harness/binding-bridge work and is diagnostic; inclusive CPU metrics must not
be summed.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 63.93 | 14.87 | 4.75 |
| replace_1k | 266.76 | 23.06 | 8.09 |
| update_10th | 0.81 | 0.50 | 1.14 |
| select_row | 1.12 | 262.02 | 0.93 |
| swap_rows | 0.85 | 0.22 | 1.08 |
| remove_row | 1.18 | 0.41 | 1.07 |
| append_1k | 125.42 | 33.39 | 6.19 |
| create_10k | 1224.57 | 293.16 | 54.99 |
| clear_1k | 151.33 | 19.40 | 2.94 |

Per-round create-10k medians expose the repeatability limits:

| App | Round 1 wall / script ms | Round 2 wall / script ms |
|---|---:|---:|
| glory | 2067.90 / 1465.37 | 1519.95 / 1084.01 |
| leptos | 871.10 / 364.11 | 533.20 / 207.07 |
| dioxus | 353.60 / 57.01 | 321.55 / 52.91 |

Glory still trails on bulk creation/replacement/append/clear. Small updates,
reorders and removal are much closer in absolute cost; RAF waiting limits the
wall-time resolution for these short operations. There is no overall ranking.

Glory replacement varied substantially: 373.60 ms versus 209.50 ms
across the two rounds. Treat the pooled median as a local observation; do not
interpret small differences or historical deltas as a precise optimization effect.

Source inspection found a newly added cost in the current DOM adapter:
`Dom::dispose` calls `parents.retain(...)` for every disposed subtree, scanning
the whole live parent table. Disposing many row subtrees can therefore repeat
a full-table scan. This is a concrete profiling candidate for clear/replacement
and untimed setup cleanup; source inspection alone does not quantify its share
of the observed time. The frozen-source diff is retained as `parent-map-diff.txt`.
This rerun keeps that code intact; it does not substitute an older Glory binary.

The mixed-workload Leptos selection outlier is reported as observed. Separate
fresh-context diagnostics are excluded from the main 648 samples:

| App | Selection script, first 1k list | Selection script, after 30 replacements |
|---|---:|---:|
| glory | 1.19 ms | 1.54 ms |
| leptos | 1.27 ms | 6.78 ms |
| dioxus | 0.76 ms | 1.34 ms |

This establishes history sensitivity in the workload, not its root cause or
a general selection-performance ranking.

Ten-thousand-row census, outside timing:

| App | Elements | Text nodes | Comments | Attributes |
|---|---:|---:|---:|---:|
| glory | 80,008 | 30,006 | 4 | 320,037 |
| leptos | 80,008 | 30,006 | 1 | 120,009 |
| dioxus | 80,008 | 30,006 | 0 | 140,015 |

Glory's live 10k state: 80,010 semantic nodes, 60,003 reactive
nodes and 90,013 Owners. After clear it returns to the baseline
(10 semantic nodes, 3 reactive nodes, 13 Owners, zero rows/labels). Counts do
not measure RSS; no memory-byte claim is made.

Historical deltas describe different sessions, not a simultaneous causal A/B:

| Glory operation | Earlier wall → current ms | Earlier script → current ms |
|---|---:|---:|
| create_1k | 133.65 → 90.30 | 103.50 → 63.93 |
| replace_1k | 173.80 → 306.55 | 143.06 → 266.76 |
| append_1k | 169.55 → 177.95 | 129.07 → 125.42 |
| create_10k | 2809.90 → 1625.25 | 2156.60 → 1224.57 |
| clear_1k | 83.05 → 152.65 | 79.79 → 151.33 |

Use the earlier [paired bulk experiment](bulk-dom-optimization-2026-10-23.md)
for the controlled optimization comparison. The subsequent anchor correction
does not make this full-`each` workload a variable-height/virtual-list benchmark.

Frozen input manifest: `D:/BuildCache/glory-framework-rerun-20261023/source/inputs.json`.
Raw results: `round-1/evidence.json`, `round-2/evidence.json` under that build root.
The [evidence index](/D:/BuildCache/glory-framework-rerun-20261023/evidence-index.json)
contains all medians, per-round values, source/build/site/driver/PNG hashes,
independent target metadata and the separate selection diagnostic. No production
framework code was changed for this rerun. Runner changes add explicit ordering,
per-run screenshots/source preservation and an untimed DOM census.

## Fractional list anchor correction — 2026-10-23

The follow-up fixes the adaptive list's host scroll quantization residual.
Independently built before/final fixtures reproduce **0.375 → 0 px** drift with
both 64 and 1024 measurement entries, using actual fractional DOM row heights
and the unchanged 0.1 px anchor tolerance. Each configuration passed 70 repeated
fractional changes, live descendant/Text Node identity checks, 80 far seeks and
73 viewport PNG checkpoints. Corresponding settled images agree byte for byte;
both final logical and physical offsets are zero. All 24 browser suites pass.
This is correctness evidence, not a new bulk timing or presentation-latency run.
See the [fix, complete regression scope and frozen-source evidence](anchor-residual-fix-2026-10-23.md).

## Bulk DOM optimization — 2026-10-23

The paired follow-up kept the benchmark app/data/CSS/harness byte-identical and
used independently built before/after WASM, two reversed-order rounds and 432
samples. Median script time: create-10k **1127.54 → 730.45 ms (-35.2%)**,
replace-1k **129.69 → 88.48 ms (-31.8%)**, clear-1k **40.56 → 31.06 ms (-23.4%)**.
Ten-thousand-row Owners fell from 130,016 to 90,013, attributes from 560,061 to
320,037 and comments from 30,010 to 4; semantic nodes and bindings stayed live.
Six viewport PNG checkpoints and all 10,000 row rectangles agree, with surviving
row/descendant/text/semantic identities preserved across updates and moves.

See the [detailed implementation, results and evidence](bulk-dom-optimization-2026-10-23.md).
The full native workspace passed 419 tests and the actual Windows/UIA probes.
Browser suites: 23/24 passed; the adaptive suite's 0.375 px anchor discrepancy
also reproduces identically on the unoptimized snapshot. Do not describe that
historical run as green or combine this paired baseline with the historical timings below.
The subsequent anchor correction and its separate complete regression are linked above.

## Semantic browser comparison before bulk optimization — 2026-10-23

The three local apps have been migrated to **semantic-rows-v1**. Glory uses
`web`, `launch_with`, public handwritten semantic builders and the complete keyed
`each`; there is no `BrowserHolder`, old `Each`, template cloning, manual row DOM
patching, virtual scrolling or compatibility feature alias. Dioxus is pinned to
0.7.9 and Leptos to 0.8.19. All three use the same Rust input generator, seed,
visible element/text tree, CSS and nine operations. Framework attributes and
comment markers remain part of each implementation's measured cost.

This is a **local full-DOM workload, not an official js-framework-benchmark
table score**: the finite semantic API currently maps its table/row/cell roles to
divs, and all three adapters now use that same structure. The old official runner
fails explicitly instead of silently submitting a different DOM contract.

All three passed the independent seeded-data oracle, nine operation checks,
keyed-node preservation on swap, replacement/removal checks, 10,000 actual row
boxes and repeated clear/recreate checks. The viewport screenshots at the same
create-1k checkpoint are byte-identical. Every measured sample's final row-data
SHA-256 and count agree across frameworks. Glory returns to rows=0, labels=0,
10 semantic nodes, 3 live reactive nodes and 16 Owners after clear.

One round used Chrome **155.0.8059.84**, 1280×900 at scale 1, fresh contexts and
order Leptos→Glory→Dioxus. Each operation had 3 untimed warmups and 12 samples
(324 measured samples total). The browser process/profile was independent of
other tasks; temporary profiles and all build/test output were on D:.
Windows Rust 1.98.0/MSVC, three independent Cargo targets, opt-level=3, LTO and
one codegen unit; all use wasm-bindgen/CLI 0.2.129, with **no wasm-opt**. The
accepted frozen library/app sources are in `source-keyed/inputs.json`; later
working-tree edits cannot enter these binaries. All three WASM/all-targets
strict Clippy checks passed, as did formatting and runner syntax checks.
After the Dioxus key correction, Glory and Dioxus rebuilt. Leptos reused only
its own isolated target with byte-identical app/manifest/lock/shared-data inputs;
the compiler-artifact freshness flags and exact WASM hashes are preserved.

Median **click to two foreground RAF callbacks**, milliseconds, with setup and
correctness snapshots excluded. This includes waiting for frame callbacks; it
does not establish presentation latency or FPS:

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

Median CDP **ScriptDuration counter delta**, milliseconds. The common harness
and binding bridge have some cost; these are diagnostics, not the official
tracing runner's isolated script metric:

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 103.50 | 16.17 | 7.25 |
| replace_1k | 143.06 | 24.29 | 10.53 |
| update_10th | 0.72 | 0.65 | 1.67 |
| select_row | 1.59 | 248.11 | 1.14 |
| swap_rows | 0.68 | 0.26 | 1.50 |
| remove_row | 0.81 | 0.42 | 1.34 |
| append_1k | 129.07 | 25.28 | 11.98 |
| create_10k | 2156.60 | 194.23 | 95.41 |
| clear_1k | 79.79 | 18.15 | 4.87 |

The main gap is bulk work. Glory create-10k is 2809.90 ms versus Leptos 509.30
and Dioxus 554.90 ms; its script counter is 2156.60 ms versus 194.23/95.41 ms.
Create/replace/append/clear also trail. Fine updates, swap and removal are much
closer in absolute time; frame-waiting limits the resolution of their wall time.
No overall score or statistical-significance claim is made from this one ordered
round on a machine running other work.

**Selection is history-sensitive.** The mixed sequence's Leptos select result
(250.60 ms wall / 248.11 ms script) is anomalous, not a general Leptos selection
baseline. A separate fresh-page probe with alternating selection on a fixed
1k list found the following script-counter medians; it is not pooled into the
main table and uses its own counter boundary:

| App | After 1 create | After 30 replacements |
+|---|---:|---:|
| glory | 2.259 | 2.165 |
| leptos | 2.204 | 11.110 |
| dioxus | 1.120 | 1.260 |

This confirms history sensitivity in this test but does not identify a leak or
its root cause. Do not use the mixed selection outlier to rank the frameworks.

A separate 10k census (outside timing) shows equal visible element/text counts,
but meaningful implementation overhead:

| App | Elements | Text nodes | Comments | Attributes |
+|---|---:|---:|---:|---:|
| glory | 80008 | 30006 | 30010 | 560061 |
| leptos | 80008 | 30006 | 1 | 120009 |
| dioxus | 80008 | 30006 | 0 | 140015 |

Glory's 10k state additionally reports 80,010 semantic nodes, 60,003 live graph
nodes and 130,016 Owners. These are live-count observations, not RSS or proof
of the timing root cause. They motivate profiling per-node construction,
Owner/Binding allocation, mapper work and DOM metadata/markers before changing
the contract. The earlier million/100-million-row virtual-list work mounted
only a bounded window and cannot establish full-`each` throughput.

Generated WASM bytes after bindgen (not compressed, and not whole download size):
Glory 817,189; Leptos 278,497; Dioxus 457,457. No cross-framework final bundle
size ranking is inferred without accounting for JS and Dioxus's snippets.

Initial conformance rejected Dioxus's expression-wrapped row loop because swap
replaced DOM nodes. The accepted version puts the keyed div directly in the
RSX loop. Earlier packaging assumed an underscore binary filename; the build
script now takes the WASM path from Cargo's compiler-artifact record. These
diagnostics and the failed pre-correction run are retained, never mixed into
the accepted timings. The shared MCP browser was occupied, so this run used a
separate Playwright-launched Chrome with D:-backed temporary profiles.

Raw records and the preserved measured driver sources:
`D:/BuildCache/glory-framework-compare/browser-keyed-round/evidence.json`.
Build/source/probe/cleanup hash index:
`D:/BuildCache/glory-framework-compare/comparison-evidence-2026-10-23.json`.
The new runner is `benchmarks/compare.ps1`; see the benchmark README for setup.
No production framework source was changed by this benchmark migration.

## Historical measurements (previous architecture)

This file keeps reproducible local measurements that can be compared before
deeper optimization work. The first baseline is intentionally raw cargo wasm:
it does not include `wasm-bindgen`, minification, compression, CDN headers, or
Binaryen passes unless called out in the `Kind` column.

## Wasm Size Baseline

Date: 2026-06-11

Command:

```powershell
./benchmarks/wasm-size.ps1
```

Local tool availability during this run:

- `wasm-opt`: not found on `PATH`, so `wasm-opt -Oz` rows were not produced.
- `wasm-bindgen`: not found on `PATH`, so final bindgen package size was not
  measured.

| Example | Variant | Profile | Kind | Bytes | KiB | MiB |
| --- | --- | --- | --- | ---: | ---: | ---: |
| _test-size | minimal | debug | raw cargo wasm | 42772055 | 41769.6 | 40.79 |
| _test-size | minimal | release | raw cargo wasm | 1299394 | 1268.9 | 1.24 |
| counter | web-csr | debug | raw cargo wasm | 42048282 | 41062.8 | 40.1 |
| counter | web-csr | release | raw cargo wasm | 1350085 | 1318.4 | 1.29 |
| router-basic | web-csr+routing | debug | raw cargo wasm | 45077943 | 44021.4 | 42.99 |
| router-basic | web-csr+routing | release | raw cargo wasm | 1563804 | 1527.2 | 1.49 |
| todomvc-fullstack | web-csr+server-fn | debug | raw cargo wasm | 46334731 | 45248.8 | 44.19 |
| todomvc-fullstack | web-csr+server-fn | release | raw cargo wasm | 1693113 | 1653.4 | 1.61 |

Interpretation:

- The minimal `_test-size` fixture is 1.24 MiB raw release wasm, about
  49.5 KiB smaller than the counter release build.
- Routing adds about 213.7 KiB to the raw release wasm over the counter
  baseline in this sample.
- The fullstack/server-fn sample adds about 335.0 KiB over the counter
  baseline and about 126.2 KiB over the routing sample.
- These numbers are regression guardrails, not final download sizes. Final
  payload tracking still needs `wasm-bindgen`, `wasm-opt -Oz`, and compressed
  asset measurements in CI.
- `./benchmarks/wasm-size.ps1 -Json` now emits tool availability and skipped
  case metadata alongside rows, so missing `wasm-bindgen` and `wasm-opt` are
  machine-readable in reports.

## Official JS Framework Benchmark

The local `benchmarks/` apps are useful for fast iteration, but official
numbers should come from krausest's Chrome tracing runner. Use:

```powershell
./benchmarks/official-js-framework-benchmark.ps1
```

The script clones or reuses `JS_FRAMEWORK_BENCHMARK_REPO`, generates
`frameworks/keyed/glory-rs` and `frameworks/keyed/dioxus-rs` adapters from the
local benchmark apps, builds them with `trunk build --release`, runs
`npm run bench -- --framework keyed/glory-rs keyed/dioxus-rs`, then writes
status and copied result artifacts under
`target/benchmark-report/official-js-framework/`. It also writes
`official-js-framework-summary.md` and `.json` with median/range tables for the
official `total`, `script`, and `paint` metrics.

Use `-SkipBench` for adapter/build validation only, or `-ChromeBinary` when
Chrome/Edge is not discoverable. On this Windows machine Chrome exists at
`C:\Program Files\Google\Chrome\Application\chrome.exe`.

Use `-BaselineName name` to preserve a named baseline under
`target/benchmark-report/official-js-framework/baselines/name/`; pass
`-CompareBaseline name` on a later run to add baseline deltas to the summary.
For optimization A/B work that should isolate Glory from other framework
builds, pass `-GloryOnly`. Baselines are not replaced unless
`-OverwriteBaseline` is set.

Stable local A/B example:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -GloryOnly `
  -Benchmarks 01_,02_,03_,04_,05_,06_,07_,08_,09_ `
  -Count 5 `
  -Headless `
  -NoThrottling `
  -BaselineName before-e9

./benchmarks/official-js-framework-benchmark.ps1 `
  -GloryOnly `
  -Benchmarks 01_,02_,03_,04_,05_,06_,07_,08_,09_ `
  -Count 5 `
  -Headless `
  -NoThrottling `
  -CompareBaseline before-e9 `
  -BaselineName after-e9
```

Local official CPU smoke, 2026-06-11:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -SkipInstall `
  -Benchmarks 01_,02_,03_,04_,05_,06_,07_,08_,09_ `
  -Count 1 `
  -Headless `
  -NoThrottling
```

This is the official Chrome/Puppeteer tracing path with one measured sample per
CPU benchmark. It is a CI-style smoke baseline, not a publishable median run.
Result JSON and the built result viewer are under
`target/benchmark-report/official-js-framework/`.

| Benchmark | Framework | Total ms | Script ms | Paint ms |
| --- | --- | ---: | ---: | ---: |
| `01_run1k` | dioxus-rs v0.7 keyed | 28.1 | 6.1 | 21.4 |
| `01_run1k` | glory-rs local keyed | 43.8 | 22.0 | 21.4 |
| `02_replace1k` | dioxus-rs v0.7 keyed | 32.2 | 10.0 | 21.7 |
| `02_replace1k` | glory-rs local keyed | 62.6 | 41.8 | 20.5 |
| `03_update10th1k_x16` | dioxus-rs v0.7 keyed | 10.1 | 1.2 | 8.5 |
| `03_update10th1k_x16` | glory-rs local keyed | 9.1 | 0.7 | 8.1 |
| `04_select1k` | dioxus-rs v0.7 keyed | 2.4 | 1.2 | 0.8 |
| `04_select1k` | glory-rs local keyed | 3.4 | 2.4 | 0.7 |
| `05_swap1k` | dioxus-rs v0.7 keyed | 3.7 | 1.1 | 2.4 |
| `05_swap1k` | glory-rs local keyed | 3.1 | 0.5 | 2.3 |
| `06_remove-one-1k` | dioxus-rs v0.7 keyed | 31.4 | 11.9 | 18.8 |
| `06_remove-one-1k` | glory-rs local keyed | 6.2 | 0.5 | 4.0 |
| `07_create10k` | dioxus-rs v0.7 keyed | 307.5 | 66.3 | 238.7 |
| `07_create10k` | glory-rs local keyed | 643.0 | 388.5 | 251.7 |
| `08_create1k-after1k_x2` | dioxus-rs v0.7 keyed | 39.0 | 8.0 | 30.5 |
| `08_create1k-after1k_x2` | glory-rs local keyed | 61.0 | 30.2 | 30.3 |
| `09_clear1k_x8` | dioxus-rs v0.7 keyed | 7.0 | 3.3 | 0.5 |
| `09_clear1k_x8` | glory-rs local keyed | 16.2 | 15.4 | 0.5 |

The first full official run caught two conformance issues before producing the
green baseline: composite `Each` rows were not reattaching their descendant DOM
nodes during CSR moves, and the benchmark remove icon was not reliably
clickable in the official headless runner. Both are fixed in this checkout.

### Focused Create10k Rerun

E12 validation reran only `07_create10k` with five samples after optimizing the
Glory benchmark data generator and CSR delegated click registration:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -SkipInstall `
  -Benchmarks 07_create10k `
  -Count 5 `
  -Headless `
  -NoThrottling
```

| Benchmark | Framework | Samples | Total median | Total range | Script median | Script range | Paint median | Paint range |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `07_create10k` | dioxus-rs v0.7 keyed | 5 | 328.9 | 26.5 | 71.7 | 7.8 | 253.9 | 18.1 |
| `07_create10k` | glory-rs local keyed | 5 | 319.8 | 126.4 | 80.3 | 25.5 | 241.2 | 100.8 |

Glory now matches the official create10k total time and is faster on paint in
this focused run. The remaining script-only gap is about 8.6 ms and is tied to
the per-row reactive scope/static-subtree overhead tracked under E9.

E9 added automatic CSR compact-fill for ordinary builder element wrappers:
fresh CSR mounts now keep non-hydrated, self-static elements as native DOM
instead of allocating a View for each wrapper, while reactive/listener nodes
remain normal Views and dynamic descendants keep their fixed native parent.
The same focused command was rerun after that change:

| Benchmark | Framework | Samples | Total median | Total range | Script median | Script range | Paint median | Paint range |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `07_create10k` | dioxus-rs v0.7 keyed | 5 | 326.0 | 26.3 | 69.3 | 5.4 | 252.3 | 20.4 |
| `07_create10k` | glory-rs local keyed | 5 | 330.4 | 34.1 | 73.8 | 13.7 | 244.0 | 21.2 |

The script gap narrowed to about 4.5 ms in this run; total time remains close
and is sensitive to paint variance.

## Local Benchmark Report

Use this wrapper to generate a repeatable local bundle:

```powershell
./benchmarks/report.ps1
```

It runs:

- `cargo bench -p glory-core --bench command_wire`
- `cargo bench -p glory-core --bench event_handlers`
- `cargo bench -p glory-core --features web-ssr --bench each_reorder`
- `cargo bench -p glory-core --features web-ssr --bench scheduler`
- `cargo bench -p glory-core --features web-ssr --bench ssr_stream`
- `./benchmarks/wasm-size.ps1 -Json`
- Playwright CSR/hydration/fullstack projects when `npm` and `tests/playwright`
  are available. Missing app URLs are recorded in `playwright-status.json`
  instead of silently disappearing from the report.

The report summary, logs, wasm size JSON, and Playwright status JSON are written
to `target/benchmark-report/`; Criterion HTML remains under
`target/criterion/`.

The event-handler benchmark isolates 1k/10k same-DOM construction with and
without click handlers, plus command-stream dispatch lookup against empty and
populated handler registries. The scheduler benchmark covers a shared `Cage`
patching 100 and 1000 subscribed views plus command queue batch drain with and
without coalescing. The SSR stream benchmark compares full `render_string()`
output with collecting `render_stream()` chunks for 100 and 1000 row pages.

## Event Handler Baseline

Date: 2026-06-19

Command:

```powershell
cargo bench -p glory-core --bench event_handlers -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
```

Representative local results:

| Benchmark | Mean Range |
| --- | ---: |
| `event_handler_install/build_1000_rows_no_handlers` | 182.37-184.81 us |
| `event_handler_install/build_1000_rows_click_handlers` | 454.10-486.80 us |
| `event_handler_install/build_10000_rows_no_handlers` | 3.0247-3.7970 ms |
| `event_handler_install/build_10000_rows_click_handlers` | 4.2722-4.5484 ms |
| `event_dispatch_lookup/missing_click_10000_rows` | 75.931-77.746 ns |
| `event_dispatch_lookup/registered_click_10000_handlers_last_row` | 106.10-108.67 ns |

Interpretation:

- Command-stream click handler registration adds about 1 ms to a 10k-row
  synthetic build on this machine.
- Dispatch lookup/restore remains sub-microsecond even with 10k registered
  row handlers, so `create10k` tuning should prioritize bulk DOM command count
  and browser-side apply cost before inventing a separate row-delegation API.

## Scheduler And SSR Baseline

Date: 2026-06-11

Commands:

```powershell
cargo bench -p glory-core --features web-ssr --bench scheduler -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
cargo bench -p glory-core --features web-ssr --bench ssr_stream -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
```

Representative local results:

| Benchmark | Mean Range |
| --- | ---: |
| `scheduler_shared_signal/revise_100_subscribers` | 117.96-121.50 us |
| `scheduler_shared_signal/revise_1000_subscribers` | 1.2290-1.2758 ms |
| `command_queue_flush/take_raw_100_rows` | 32.814-36.248 ns |
| `command_queue_flush/take_coalesced_100_rows_3_text_writes` | 19.142-19.933 us |
| `command_queue_flush/take_raw_1000_rows` | 61.024-73.065 ns |
| `command_queue_flush/take_coalesced_1000_rows_3_text_writes` | 188.36-194.68 us |
| `ssr_render/render_string_100_rows` | 231.42-245.75 us |
| `ssr_render/render_stream_collect_100_rows` | 203.20-207.17 us |
| `ssr_render/render_string_1000_rows` | 2.0836-2.1884 ms |
| `ssr_render/render_stream_collect_1000_rows` | 2.1148-2.2274 ms |

Interpretation:

- Shared-signal patching scales close to linearly in subscriber count in this
  baseline.
- Raw command queue drain is effectively free because it is a `Vec` take.
- Coalescing has measurable cost and should stay opt-in for IPC/remote sinks.
- SSR stream chunk collection is comparable to full string rendering, and the
  holder now emits DOM-boundary `App` chunks instead of one giant app subtree
  chunk; real network soak tests can still be added at the adapter layer.
