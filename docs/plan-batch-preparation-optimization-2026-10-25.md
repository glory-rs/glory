# Plan 批量构造的准备与校验优化（2026-10-25）

保留共享声明摘要与有序 slot 校验快路。相同应用、独立构建的最终对照中，千行创建 Script **20.349→18.827 ms（-7.5%）**，千行替换 **30.104→28.829 ms（-4.2%）**，5,000 行持仓看板初始化 **87.492→82.236 ms（-6.0%）**。这三项两轮方向一致。

万行创建合并中位数降低 5.0%，但第二轮没有改善，不能认定稳定提速。持续更新未得到普遍改善，尤其看板全量更新有约 3–6% 的中位数回退。下文保留这些结果。

本轮落实在“同一声明的准备只做一次、整批身份准备完后证明唯一性”这一层。现有 `Scope::batch` / `Runtime::batch` 和 App 帧提交行为没有变化，DOM 仍按现有解释器执行；没有新增事务 API、DSL、模板 VM 或依赖。

## 保留的实现

### 共享不可变声明与摘要

[plan.rs](../crates/render/src/plan.rs) 用 `Rc<Declaration>` 集中保存节点和有限的不可变事实：

- Layer / FocusScope 声明数量，以及每个初始 FocusScope 的子树大小。
- 穿过透明 Group 后仍需检查外部文档父节点的声明位置。
- 依赖外部 Listbox 的边界和外部 option 标记；内部归属仍在声明构造时完整检查。
- 带 Semantics 的节点位置。保持保守检查：不仅包含 id，也保留 combobox / active-descendant 等当前关系检查。

每次实例化不再重复计算这些事实或分配相关临时上下文向量。实际父节点、名字占用、当前容量和适配器限制仍在实例化/放置时检查；没有把某个 Memory 的检查结果缓存为全局许可。

`Plan` / `WeakPlan` 的共享句柄由 Rc slice 改为 sized Rc。实例仍各自拥有 Props 和字段覆盖；两份 Memory 独立保存可变状态。普通 Create 继续使用 owned Descriptor，没有从业务数据推断静态声明，也没有缓存访问过的行。

这个表示也让声明读取多了一层间接寻址，可能影响热更新；本轮没有单独拆出它的因果占比，不能把全部收益或回退归给某个字段布局。

### 整批 slot 的唯一性证明

[Memory::CreatePlan](../crates/render/src/memory.rs) 先检查当前批次的 slot 是否严格递增或严格递减。严格单调本身证明 slot 不重复，因此常见的新建/回收运行不再需要一个临时 HashSet。

乱序批次回退到原有 HashSet 检查。每个节点仍逐一检查 runtime、当前节点占用、generation 水位和需要的名字/关系；错误优先级、拒绝整份 CreatePlan 和已成功前缀的 applied 水位保留。没有排序、改写调用者的节点顺序，辅助信息只存在于当前调用栈。

检查顺序与事实复用见 [memory.rs](../crates/render/src/memory.rs)、[focus_scope.rs](../crates/render/src/focus_scope.rs)、[choice.rs](../crates/render/src/choice.rs)。

## 撤回的直接组装实验

另一个版本预计算每个节点的子列表和 next-sibling，在完整身份检查后直接构造整棵新子树，避免逐边回查并修改父节点；子列表也按实际数量分配。

功能及身份检查通过，但同批对照没有稳定优于仅摘要版本，千行创建/替换也没有明确收益。因此已经撤回，`Children` 存储和原有逐边链接保持原实现。非深度优先声明的兄弟顺序、移动、销毁和新实例独立性测试保留为行为回归。

## 独立构建和测量边界

- 基线为 Git `e6b3926` 的冻结工作区。before / after（仅摘要）/ assembled（撤回）/ ordered（保留）各有原九项应用和持仓应用两个独立 release target，共 **8 份构建**；每份均 35 个 compiler artifacts、全部 fresh=false。Cargo metadata 核对实际 D 盘 target，没有 WASM 互拷。
- 每份冻结 **529 个输入**。最终与基线仅四个 render 实现文件和两个测试文件不同；全部 benchmark 输入、CSS、业务算法和检查容差逐字节相同。编译和测量均读取冻结源码。
- 输出：`D:/BuildCache/glory-plan-facts-20261025`。Rust 1.98.0、wasm-bindgen 0.2.129，release opt-level 3 / LTO / 单 codegen unit，未运行 wasm-opt。
- i7-14700KF / Chrome 155.0.8059.84，1280×900、DPR 1、en-US/light。前台独立 context，统一 affinity 65535 / AboveNormal。共享机器存在其他 Cargo 检查活动，不能把小差异当作无噪声的产品保证。
- 最终对照都包含 before / after / ordered，按此顺序再反向。每项每轮三次热身、十二样本，每格 **24 样本**。九项 **648**，持仓两个规模 **1,152**，最终共 **1,800**；不删慢样本。
- 先行的 1,200 个样本、直接组装消融的 648 个样本单独保存，不并入最终表。早期 driver 路径配置失败没有进入计时轮次。
- 正式计时没有 DOM/WASM monkey patch 或 Profiler；oracle、身份、几何、统计和图片检查均在计时窗口外。wall 是 click→两次 RAF，不是实际显示完成或 FPS。
- Plan 在应用启动时显式声明，点击计时不包括它的一次性构造成本。本结果针对复用声明的大量实例；不外推为一次性 builder 或冷启动的同等收益。

[构建和输入审计](D:/BuildCache/glory-plan-facts-20261025/source-build-audit.json)、[最终原九项样本](D:/BuildCache/glory-plan-facts-20261025/ordered-bulk/evidence.json)、[最终看板样本](D:/BuildCache/glory-plan-facts-20261025/ordered-portfolio/evidence.json)、[全部统计及逐轮 p10/p90](D:/BuildCache/glory-plan-facts-20261025/final-summary.json)。

## 原九项正式结果

ScriptDuration 中位数，ms；负百分比表示保留版更快。

| 操作 | before | 仅摘要 | 保留版 | 保留版变化 |
|---|---:|---:|---:|---:|
| 创建千行 | 20.349 | 19.498 | 18.827 | -7.5% |
| 替换千行 | 30.104 | 30.057 | 28.829 | -4.2% |
| 更新每十行 | 0.531 | 0.555 | 0.544 | +2.4% |
| 选行 | 0.631 | 0.606 | 0.600 | -4.8% |
| 交换行 | 0.601 | 0.622 | 0.620 | +3.2% |
| 删除行 | 0.635 | 0.623 | 0.620 | -2.3% |
| 追加千行 | 21.964 | 21.635 | 21.000 | -4.4% |
| 创建万行 | 241.931 | 231.790 | 229.926 | -5.0%† |
| 清空千行 | 14.693 | 13.585 | 14.281 | -2.8% |

† 万行两轮 before→保留版分别 **248.503→231.113** 和 **228.097→228.819 ms**。第二轮略慢，基线轮间差异明显；合并中位数下降不等于稳定改善。清空的降幅同样较小，不宣称已经解决销毁成本。

千行创建两轮 **20.513→18.703 / 20.099→18.949**；替换 **30.237→28.284 / 29.876→29.131 ms**。额外的有序校验快路让替换优于仅摘要版本；不能把仅摘要阶段的所有差异都归给减少扫描。

同批 click→两 RAF 的 wall：千行创建 **46.30→45.20**，替换 **57.20→55.55**，万行 **503.90→480.90 ms**。这些值也受调度和浏览器工作影响，各指标中位数不能相加。

## 持仓场景正式结果

场景及两个 Dioxus 组织的既有对照见[场景报告](portfolio-scenarios-benchmark-2026-10-25.md)。本轮只对 Glory 基线及候选做同批 A/B，没有重新测 Dioxus，也没有将旧轮 Dioxus 时间拼成新的跨框架成绩。

| 1,000 行 Script，ms | before | 保留版 | 变化 |
|---|---:|---:|---:|
| 打开 | 15.379 | 15.425 | +0.3% |
| 单行报价 | 0.161 | 0.157 | -2.2% |
| 1% 报价 | 0.238 | 0.230 | -3.2% |
| 10% 报价 | 0.811 | 0.811 | +0.0% |
| 全量报价 | 6.666 | 7.046 | +5.7% |
| 十批 1% | 0.810 | 0.823 | +1.7% |
| 切换选中行 | 0.160 | 0.151 | -5.3% |
| 关闭 | 6.370 | 6.344 | -0.4% |

| 5,000 行 Script，ms | before | 保留版 | 变化 |
|---|---:|---:|---:|
| 打开 | 87.492 | 82.236 | -6.0% |
| 单行报价 | 0.172 | 0.176 | +2.3% |
| 1% 报价 | 0.521 | 0.521 | +0.1% |
| 10% 报价 | 4.080 | 4.142 | +1.5% |
| 全量报价 | 37.799 | 38.957 | +3.1% |
| 十批 1% | 3.317 | 3.340 | +0.7% |
| 切换选中行 | 0.171 | 0.178 | +4.4% |
| 关闭 | 36.161 | 36.520 | +1.0% |

5,000 行打开两轮 **86.955→81.374 / 87.826→83.361 ms**，wall 合并 **192.70→184.55 ms**。大规模初始化有一致改善，小规模初始化没有。

本轮选择保留批量构造收益；不能宣称热更新全面提速。稀疏更新接近原值，全量更新存在回退，额外的声明间接寻址和代码生成/分配变化值得后续单独定位。没有把几微秒差异直接转换成可感知延迟，也没有把各项中位数拼成声称实测的整段会话。

## 正确性和回归

- 最终两套 benchmark 均通过独立数据 oracle；全部正式样本跨版本的数据哈希一致。
- 原九项完整存续 Element/Text、Glory 全身份、10,000 个行矩形一致；持仓补充逐位置检查 9,000 / 45,000 个物理节点，关闭/再打开不复用旧对象或旧身份寿命。
- 最终三个版本原九项六处、持仓两规模各四处检查点，共 **42 张 PNG**，对应画面逐字节一致，没有放宽容差。
- 新测试覆盖透明文档边界的当前父上下文、不同 Memory 的 Layer/FocusScope 容量、实例覆盖独立性、非深度优先声明及移动后的兄弟顺序、升/降/乱序身份、重复 slot、复用世代和外部 runtime；原成功前缀与查询水位测试沿用。
- Windows workspace/all-features **908 passed**，严格 native/wasm all-targets Clippy、fmt、Blitz minimal/paint 检查通过；真实窗口及 UIA 检查通过。
- Linux workspace/all-features **907 passed**，严格 Clippy、Blitz minimal/paint、CPU raster 通过；X11 / Wayland 各 shell / accesskit 实窗验收，共四项通过。两个引擎 fork、六个包的来源及 fork-owned 测试在双平台均通过，依赖与锁文件未改变。
- **27 组**浏览器验收全部通过，含 CSR/真实 SSR adoption、stream、受控输入、queries、资源、CSS/flex，以及 **64 / 1024** 测量缓存的真实变高列表。两个缓存的最终逻辑/物理视口和画面一致，容差及行高来源均未改变。

[原九项身份检查](D:/BuildCache/glory-plan-facts-20261025/ordered-integrity/evidence.json)、[看板有序身份](D:/BuildCache/glory-plan-facts-20261025/ordered-portfolio-integrity.json)、[Windows 完整日志](D:/BuildCache/glory-plan-facts-20261025/windows-full.txt)。

[Linux 完整日志](D:/BuildCache/glory-plan-facts-20261025/linux-complete.txt)、[浏览器完整结果](D:/BuildCache/glory-plan-facts-20261025/browser/evidence.json)、[64/1024 真实行高验收](D:/BuildCache/glory-plan-facts-20261025/browser/adaptive-acceptance.js.json)。实窗/UIA 证据不替代真实 OS IME、读屏器或 Unix AT-SPI 客户端验收。

Linux 初次运行缺少 fontconfig pkg-config 路径，使用已有 D 盘 native-lib 后重跑；随后 D 盘的 96 GiB 构建镜像触及容量限制。确认镜像离线、D 盘 backing 和余量，检查文件系统后扩至 **112 GiB**，保留全部缓存再继续。没有改测试或第三方源码，也没有将编译移到 C 盘。[存储处理记录](D:/BuildCache/glory-plan-facts-20261025/linux-storage-expand.txt)。

## 剩余工作

此轮减少的是准备、校验与部分实例表示成本。HTML 降低、逐节点桥接和 DOM 属性写入仍存在，不能据这轮结果推断已追平 Dioxus。下一轮需要独立评价固定用途的宿主批传，或定位热 DescriptorView 的读取成本；不能直接用通用指令 VM 替代现有语义、身份与屏障契约。

复现入口仍为 `benchmarks/build.ps1`、`benchmarks/scenarios` 和标准平台脚本；本轮冻结源码、适配的 A/B 驱动、全部中间候选、失败环境日志均在上述 D 盘根目录，实验批次明确分开。
