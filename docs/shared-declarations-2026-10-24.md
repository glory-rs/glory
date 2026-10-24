# 双份 Memory 的声明共享与独立实例字段

本轮将显式静态 Plan 的声明字段与实例可变字段分开，保留两份 Memory、独立预检与 direct apply 校验、全部语义 NodeId、DOM 身份属性和查询水位。应用继续使用纯手写 Rust builders，没有 DSL、模板 VM、vendor 或依赖变更。

## 存储与更新

此前每个 CreatePlan 节点在 App 的 staged Memory 和 HTML Mapper 的 committed Memory 中各克隆一次完整 Descriptor；Style 本身有大量可选字段，默认值也占据每个节点的内联空间。上一轮分页索引解决了哈希和扩容搬移，但没有消除这份重复初始化。

现在原始 Descriptor 仍是构造声明的数据结构。活节点使用不公开可变入口的 LiveDescriptor：

- 普通 Create 保存独占的 Box<Descriptor>，不隐式冻结业务初值或建立全局缓存。
- Plan 实例保存独立 Props、Plan 引用、声明索引和可选覆盖字段。Style、classes、options、role、name、focus、item、semantics 从显式不可变声明借用。
- 修改元数据只分离该字段。例如 classes 更新不会克隆 Style；Style 第一次修改才分配自己的覆盖值。options 没有 live patch，始终从声明借用。
- Input 值、版本和其他 Props 仍独立属于每份 Memory。没有对整份 Descriptor 或 Props 做写时克隆；修改一个输入字段不会因此复制无关的大文本。

DescriptorView 是短生命周期的只读字段视图。Render::preflight、Subtree::New 和 PreparedPatch 使用它，HTML / SSR / stream / web / Blitz 读取同一有效值。需要真正生成快照时才显式 to_owned；正常补丁降低仍只读准备好的字段与 inverse，不重新构造两份 Shape。原始 Descriptor 的 builder API 保留，适配器的读取接口属于本次架构更新。

prepare_patch 记录覆盖字段的存在位。拒绝、guard drop 或 unwind 都先交换恢复字段，再撤销本次才创建的覆盖值；已经独占的字段恢复原缓冲区。成功提交仍只接受一个 mutation。两份 Memory 各自验证完整有效字段与内容约束，共享声明不代表共享可变状态或绕过校验。

被测源码的原生 64 位 size_of 探针中 Descriptor 仍为 672 字节，Style 为 480 字节，Memory::Node 从 760 降到 176 字节（记录大小降低 76.8%）。这只说明节点内联记录变小：Plan 声明、独占 Props 字符串、首次修改的覆盖 Box 和普通 Create 的 Box 都另占堆空间，不能换算成 RSS，也不能直接当成 wasm32 节点大小。同期功能会继续扩展字段，这些数字不混用其他源码版本。

代价包括一次字段视图选择、Plan 引用计数，以及普通 Create 的描述符间接访问。仅显式 Plan 获得声明共享；本轮不宣称所有工作负载都更快或所有文本都零复制。声明随最后一个实例/计划所有者释放，不持有已访问虚拟行的实例数据。

## 验证与测量边界

证据根目录为 D:/BuildCache/glory-descriptor-fields-20261024。before 冻结 287 个编译/benchmark 输入；共享实现增加 view.rs 和 declarations.rs，合计 289 个。全部测量版本的 15 个 benchmark 应用、锁、数据、CSS 与 harness 输入一致。编译缓存、Cargo target、浏览器临时目录、下载目录、Linux 构建与 fork 测试均位于 D 盘。

每个被测版本均从各自源码快照使用全新独立 target 构建 release wasm；Leptos 0.8.19 和 Dioxus 0.7.9 也重新独立构建。cargo metadata 记录实际 target，保留构建 JSONL、WASM 与驱动哈希。没有复用另一版本的产物，也没有修改 workload、删节点或放宽画面容差。

对照包含原实现 before、仍复制完整声明的 owned 消融版、共享声明 after，以及只增加视图内联属性的 inline 候选。owned 与 after 仅实例化路径不同，用于区分“小节点布局”和“声明共享”的收益；owned 是诊断构建，不是最终实现。第一次五版测量保留 1,080 个样本作为定位证据，不与之后不同构建的样本合并。

CPU 采样、DOM census、选行历史诊断单独运行，不混入正式计时。inclusive 栈类别可能重叠，内联会改变符号归属，不能相加当成精确账单，也不能将采样分配器时间解释成分配字节。

新增测试直接比较两份 Memory 和 Plan 中的静态字段地址、独立输入字符串与版本；检查 classes 的单字段分离、错误和 unwind 回滚、已有缓冲区恢复、最后一份 Memory 释放后的声明/options 析构。HTML 字段降低差分 oracle 对普通 Create 与 CreatePlan 都运行，覆盖补丁默认重置、canonicalization、拒绝和成功前缀。

## 正式计时与消融对照

最终保留 inline。Chrome 155.0.8059.84，1280×900、DPR 1，进程限制在 CPU 0–15，AboveNormal；每组 3 次热身、12 次记录，六版正序/反序各一轮，共 1,296 个原始样本，每格 24 个样本的中位数，不裁剪。表内为 **Script / 总耗时，ms**；Script 是相同 CDP 区间的 ScriptDuration，总耗时还包括样式、布局与 settle 等待。

| 操作 | before | owned | shared | inline | Leptos | Dioxus | before→inline Script |
|---|---:|---:|---:|---:|---:|---:|---:|
| create_1k | 23.35 / 51.20 | 25.21 / 55.65 | 21.27 / 47.65 | 19.59 / 46.00 | 15.30 / 42.50 | 4.52 / 29.85 | 16.1% |
| replace_1k | 36.70 / 65.60 | 71.94 / 135.55 | 31.27 / 58.90 | 30.43 / 57.35 | 22.86 / 48.40 | 7.85 / 32.00 | 17.1% |
| update_10th | 0.60 / 11.55 | 0.90 / 18.25 | 0.57 / 10.75 | 0.56 / 12.30 | 0.41 / 13.10 | 0.93 / 12.45 | 7.1% |
| select_row | 0.94 / 8.65 | 1.25 / 7.65 | 0.90 / 9.40 | 0.86 / 8.85 | 180.16 / 183.45 | 0.79 / 8.10 | 9.3% |
| swap_rows | 0.69 / 9.95 | 0.64 / 10.20 | 0.62 / 10.40 | 0.62 / 9.90 | 0.19 / 9.70 | 0.95 / 9.85 | 9.1% |
| remove_row | 0.68 / 11.10 | 0.62 / 10.85 | 0.64 / 8.95 | 0.66 / 10.15 | 0.29 / 11.30 | 0.83 / 10.20 | 2.9% |
| append_1k | 25.22 / 54.40 | 25.59 / 56.95 | 23.04 / 53.50 | 22.20 / 55.30 | 17.99 / 48.05 | 5.90 / 38.85 | 12.0% |
| create_10k | 270.61 / 571.50 | 271.01 / 578.95 | 242.61 / 522.85 | 234.37 / 508.60 | 187.55 / 458.90 | 60.93 / 370.40 | 13.4% |
| clear_1k | 17.53 / 19.40 | 17.86 / 21.15 | 16.48 / 18.70 | 15.45 / 17.35 | 17.59 / 20.05 | 8.96 / 11.00 | 11.9% |

before→inline 的千行创建 / 替换 / 万行创建 / 清空 Script 分别降低 16.1% / 17.1% / 13.4% / 11.9%，总耗时降低约 10.2% / 12.6% / 11.0% / 10.6%。千行追加虽然 Script 下降 12.0%，总耗时从 54.40 到 55.30 ms；update 的总耗时也略增。没有把小操作或绘制等待全部说成获益。

同次测量中，Glory 创建 / 替换 / 万行 Script 仍分别为 Leptos 的约 1.28 / 1.33 / 1.25 倍、Dioxus 的约 4.33 / 3.88 / 3.85 倍。清空 Script 小于本次 Leptos，但仍高于 Dioxus；这些是当前冻结程序与此 workload 的结果，不是框架所有应用的固定倍率。

shared→inline 仅修改 view 函数的内联属性，千行创建 / 替换 / 万行 / 清空 Script 再下降约 7.9% / 2.7% / 3.4% / 6.3%。这与声明共享的改动分开构建、分开测量，没有用调试构建作为 before。

消融不能只看一格大差值：最终 owned 的替换中位数为 71.94 ms，两轮分别 84.35 / 56.38 ms，明显比第一次五版实验的 35.63 ms 慢。全部慢样本和两轮均保留，当前没有足够证据将这段额外变慢归因于某一个分配器/GC 机制，因此不宣称“共享声明单独使替换快一倍”。

第一次实验中 before / owned / shared 的千行创建为 23.91 / 22.04 / 20.57 ms，替换为 34.80 / 35.63 / 29.98 ms，万行创建为 250.36 / 255.18 / 231.40 ms，清空为 17.26 / 16.59 / 15.15 ms。它提供了“紧凑节点布局”和“避免声明复制”分别起作用的对照；不与最终 1,296 个样本合并计算收益。Dioxus 清空在两批分别为 2.69 / 8.96 ms，也说明跨批绝对值会变化，表中保留实际值。

最终计时中，绑核脚本在第 8 组开始前枚举到已退出的 Chrome 子进程而中断。修复仅跳过已经退出的进程，活进程绑核失败仍报错；掩码、优先级和每项测量函数不变。已经完成的 7 组 756 个样本原样保留，恢复同一顺序测完剩余 5 组 540 个样本，没有删掉慢值或重测挑选较快结果。中断、恢复日志和两份驱动均保留。两批主计时期间本任务没有并行构建、CPU profile 或其他浏览器诊断。

Leptos 选行的完整历史效应继续保留在主表。单独的 72 样本诊断中，1 / 30 次替换后选行 Script 为 Leptos 1.22 / 6.31 ms、shared Glory 0.98 / 0.91 ms、Dioxus 0.70 / 0.75 ms。这没有复现主表的完整幅度，不能用诊断值替换正式结果，也不将差异认定为本轮声明存储造成。

## CPU、身份与代价

独立的 72 份无 DOM 包装采样涵盖六版、四个操作、每项 3 次；GC 在采样区间外。以下是各类别采样中位数，ms；内联及采样波动会影响函数归属，类别不能相加。

| 万行创建采样 | before | owned | shared | inline |
|---|---:|---:|---:|---:|
| staged Memory apply inclusive | 35.20 | 24.26 | 17.58 | 19.40 |
| committed Memory apply inclusive | 29.52 | 33.15 | 17.02 | 24.82 |
| allocator self | 27.95 | 23.44 | 10.65 | 14.50 |
| cloneNode self | 24.69 | 20.52 | 25.61 | 26.77 |
| setAttribute self | 36.81 | 33.27 | 31.34 | 32.39 |

采样揭示了非内联 view 的额外调用成本，促成最后的内联实验；并不声称内联让所有 profile 类别都下降。正式计时是保留它的依据。最终千行替换的 Owner dispose 仍有约 4.72 ms inclusive、removeChild 约 3.17 ms self；清空对应 4.42 / 4.51 ms，剩余销毁成本没有消失。

最终六版的数据哈希、六个画面检查点（共 36 张 PNG）一致。另行逐个比较保留行的 Element 与 Text 对象、完整 Glory token；更新、选行、交换、删除、追加不得替换存续对象，替换后旧对象必须断开。全部 10,000 行真实矩形的摘要一致；清空恢复 10 个语义节点、3 个图节点、13 个 Owner。

DOM census 中 Glory 四版千行创建/替换都保留 8,008 Element、3,006 Text、4 Comment、32,037 个属性与 8,008 个完整身份 token；模型为 8,010 节点、6,003 图节点、2,013 Owner。Leptos 与 Dioxus 的 Element/Text 相同，但 Comment 为 1 / 0，属性为 12,009 / 14,015。声明共享没有靠删除身份、属性或注释减少工作。

未压缩 bindgen WASM 大小为 before 1,044,064 B、owned 1,068,967 B、shared 1,069,198 B、inline 1,075,362 B；最终比 before 增加 31,298 B（约 3.0%），比 shared 增加 6,164 B。Leptos / Dioxus 分别 278,497 / 457,457 B。本轮没有 wasm-opt；这是这些 benchmark 程序的未压缩文件大小，不是框架的固定下载负担。

后续优先量化独立 Props 初始化、Owner/Binding 生命周期与 DOM 登记的剩余成本；物理属性、事件和语义身份都有协议用途，不能直接删掉换速度。若继续减少 Plan 引用/分配，应先证明生命周期与两份独立校验仍完整。普通 Create 和频繁修改大量元数据的场景尚需单独量化，不将本次静态行列表收益推广到它们。

## 完整回归

性能候选的 Windows 与 Linux 工作区各 522 项测试通过；Windows 原来忽略的 2 项、Linux 的 1 项窗口测试继续按脚本单独验收，没有删测试来获得通过。native / wasm32 全工作区 all-features / all-targets 严格 Clippy 通过。两个固定 Git fork 的 289 项验证另计，不混入工作区测试数。

Windows 的真实窗口、AccessKit 窗口、实际 Windows UIA 客户端验收通过；Linux X11 / Wayland 各自运行 shell 与 AccessKit，共 4 项真实窗口验收通过。CPU raster、原生 fixture 6 项测试、独立 wasm fixture release 构建及严格 Clippy 通过。这不新增 Android、OS IME 或屏幕阅读器语音的验收声明。

浏览器运行全部 27 组套件以及真实 HTTP stream 检查。变高列表分别使用 64 / 1024 测量缓存，保留真实 Resize 行高、原容差、80 次远跳和每份 70 次小数高度扰动。锚点增长前后均为 -15.125 px；每份 73 张截图、节点身份与最终 Home 位置单独记录，没有用固定行高替换测量，也没有掩盖 0.375 px 回归。

第一次 Windows 完整脚本在 fixture 编译时发现一处返回临时视图引用的 E0515；改成直接返回已借用的 props 后，从失败步骤继续，严格 Clippy、独立构建和浏览器全套全部完成。Linux 启动包装脚本的 CRLF 问题也在实际构建前修正，启动失败日志保留。上述失败均未从原始证据中删除。

性能候选与同期功能有明确源码边界。菜单、current、Grid、inline-flex 等同期修改在单独快照内三方合并，保留其行为后通过 546 项测试与 native / wasm32 严格 Clippy。交付时再次按当前文件内容三方合并，检查写入前哈希，避免覆盖并行修改；这些功能没有混入 before/after 性能收益。

交付后的实时工作区 native / wasm32 严格 Clippy 曾完成；示例另有一处多行旧字段访问，改用 view 后 fixture 严格 Clippy 通过。随后整库测试恰逢新的 Tooltip / PointerWithin 功能写入：Blitz 已使用新枚举，但该轮 render 编译尚未包含它们，报 E0599。这次实时工作区尝试不是通过的完整回归，不能冒充被测候选或 546 项合并快照的验证结果；失败日志独立保留，没有删除同期功能来取得通过。上面的完整回归结论仅覆盖列出的冻结源码。

同步完成后，实时工作区 all-features / all-targets 编译恢复通过。再次冻结 462 项源码/资源输入，校验捕获期间没有文件变化；该最终合并快照的 native / wasm32 all-features / all-targets 严格 Clippy 与 **566 项工作区测试全部通过**。它包含同期新功能，只作为合并验收，不替换被测性能程序或前述 Windows/Linux/浏览器回归边界。

完整原始索引为 D:/BuildCache/glory-descriptor-fields-20261024/evidence-index.json（配套 evidence-index.sha256）；verification.json 核对冻结输入、独立 target / fresh 构建、WASM、全部样本、对象身份、几何、截图、census 和测试结果。final-timing-summary.json 与 final-table.md 是最终同批汇总，timing-summary.json 为初次定位批次；两者分别保留。Windows/Linux/浏览器、最终合并和失败/恢复日志均在同一证据根目录。仅本任务拥有的测试服务器已关闭，未操作其他运行中的进程。
