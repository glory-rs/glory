# 手写 Rust 静态构造计划与创建/替换优化

本轮实现显式、不可变的静态语义构造计划，沿用普通 Rust builders、Cage/Bond/Lotus 和有限语义节点。计划编译发生在应用声明阶段；行数据、事件捕获、受控编辑状态和 generational NodeId 在实例化阶段独立创建。

## API 与使用边界

```rust
use glory::prelude::*;

// 在行循环外声明一次。mark 只定位部件，不生成 DOM id/class。
let declaration = plan(view().fill((
    text().mark("title"),
    button().label("Remove").mark("remove"),
)))?;
let title = declaration.part("title")?;
let remove = declaration.part("remove")?;

// 每行在自己的 component Scope 中执行。
let instance = scope.instantiate(&declaration)?;
instance.text(&title, title_cage)?;
instance.on(&remove, activate, move |_| { /* 本行的处理 */ })?;
```

`plan(...)` 接受现有 handwritten builders，递归得到有限静态树。拒绝带 reactive 属性、事件捕获、node_ref 或动态控制流的声明；这些行为通过实例或计划外的 component/each/when 添加。静态结构没有解释指令、表达式求值或模板 VM，也不从第一条业务记录、已有 DOM 或用户编辑后的控件提取模板。显式声明的静态文字、classes、style 可以复用；运行时数据写入实例。

每份计划最多 256 个语义节点，一个根、父先于子。重复 mark、重复语义名称、非法内容关系在声明/预检阶段拒绝。PlanPart 关联原计划，外来 part 在运行用户 patch 闭包前拒绝。PlanInstance 只保存 WeakScope 和本次身份，不延长行 Owner 的生命。多个实例若由同一个 Scope 创建，生命周期属于该 Scope；需要独立退休时使用独立 component/child Scope。

静态后代不再为了 builder 填充而各建一个 UI Scope。它们仍保留完整独立 NodeId、可变描述符、查询目标和事件路由身份，事件/Binding 归调用者 Scope 的 Owner 管理。不同实例不共享 reactive 数据。受控 Value 绑定仍从活节点取得下一 revision，显式 patch 的 revision 规则保持原样。

## 分层与提交

| 层 | 新职责 |
|---|---|
| render | 不含 HTML 类型的 `Plan/PlanNode`，一次 `Mutation::CreatePlan` 表达一个完整的离线创建单元 |
| ui | builders 编译、显式 mark/part、Scope::instantiate、实例 Binding/事件注册 |
| html | 纯静态 Shape 与父索引编译；Sink 提供默认的有限 Create/Place 展开 |
| web | 文档所有的离线原型、固定 Part 路径、一次 cloneNode(true)、只挂载实例根 |
| ssr/stream/blitz | 使用默认展开，复用现有解释器；没有 HTML wire 升级或 GPU 依赖外溢 |

Scope 先在借用的 staged Memory 上验证整个计划的外部上下文，并逐描述符检查样式/adapter limits，之后才分配身份。Memory 的直接 apply 独立验证整批 runtime/slot/generation/name；晚到的非法身份不会创建本单元前半段。计划内关系在声明时验证，创建新节点时直接建立 children/next/parent，省去临时 Box、重复描述符克隆、unlink 和逐节点祖先查找。每个活模型仍有自己可写的 Descriptor，不采用整 Descriptor 的共享写时克隆。

一次成功 CreatePlan 将 receipt 推进一次，随后外部 Place 独立提交。内部统计仍计入所有语义节点及内部边。适配器拒绝后保留先前接受操作的查询水位；不承诺宿主异常后的 DOM 回滚。新实例的深克隆、属性初始化和受控 property 初始化在离线状态完成，之后登记身份，再按现有 Place 路径挂载根。正常移动、Suspend/Resume 和 patch 不重新实例化。

HTML 与 DOM 两级缓存分别限制为 64 份计划、4096 个 physical pieces、1 MiB 被计费 UTF-8 载荷。key 是显式计划身份，weak source 防止失效地址复用；新声明进入时清理已过期项。预算不足时正确回退，既不缓存访问过的行，也不扩大错误容差。计费载荷不是总堆分配/RSS。缓存只保存声明和离线静态原型，不保存行键、实例 NodeId 或实时控件。

含 Group 的计划目前使用通用展开，仍有注释范围、没有布局包装。深克隆原型不写入实例身份；src/href/value/checked/id/aria-label 延后到新克隆设置，避免用原型加载资源或继承控件状态。Value/Checked 的 property 屏障照常执行。hydration staging 不生成新 DOM，走现有完整验证与 adoption；之后新建实例才启用原型。

## 对照方法

独立构建四个 Glory 版本：original（任务开始时的库与 builders）；ordinary（最终库 + 原 builders 应用）；generic（新 Plan + 通用逐节点后端）；final（新 Plan + 整树深克隆）。ordinary 与 final 库源码相同，只改应用行构造；generic 与 final 除 `DEEP_CLONE` 常量外源码逐字节一致，应用也一致。这样既能观察整体前后变化，也能排除工作树同期语义/原生事件改动的影响。

四版保持 semantic-rows-v1 的数据生成、CSS、九项操作、热身/样本数及验收脚本。Rust 行构造迁移到 Plan 的总体收益包括 API、Scope/Owner、语义构造及后端改造，不能全部归因于 cloneNode。

原始源码、独立 target、metadata、编译 artifact、样本与验收归档位于 `D:/BuildCache/glory-static-plan-20261024`。

## 实际性能

Windows / i7-14700KF / Rust 1.98.0 MSVC / Chrome 155.0.8059.84，1280×900、scale=1、en-US、light。每组独立 D: target，release opt-level=3、LTO、1 codegen unit，wasm-bindgen 0.2.129，没有 wasm-opt。metadata 核对实际目录；四个应用 release artifact 均 fresh=false。

正式顺序 original→ordinary→generic→final，第二轮反向；每轮每项 3 次热身、12 个样本，共 **864 个主样本**，每格 24 个。所有本任务的构建/回归/插桩在正式计时前结束，工作树后续验证在计时后启动。Chrome 进程于计时外设为 P 核逻辑 CPU 0–15、AboveNormal 并读回验证。机器不是独占，前后宿主进程列表保留。冷原型创建有独立调用计数；正式数据是按同一 harness 热身后的中位数。

ScriptDuration 中位数，单位 ms：

| 操作 | 原快照 | 同库 builders | Plan 通用后端 | Plan 深克隆 | 相比同库 builders |
|---|---:|---:|---:|---:|---:|
| 创建千行 | 56.71 | 55.27 | 37.71 | **28.36** | **-48.69%** |
| 替换千行 | 83.18 | 83.79 | 61.29 | **46.38** | **-44.64%** |
| 更新每十行 | 0.66 | 0.64 | 0.65 | 0.64 | 基本持平 |
| 选行 | 1.06 | 1.02 | 0.87 | 0.78 | -23.77% |
| 交换两行 | 0.58 | 0.59 | 0.65 | 0.58 | 基本持平 |
| 删除一行 | 0.62 | 0.61 | 0.70 | 0.63 | 基本持平 |
| 追加千行 | 61.12 | 61.97 | 41.16 | **30.44** | **-50.88%** |
| 创建万行 | 656.60 | 646.39 | 442.79 | **362.41** | **-43.93%** |
| 清空千行 | 30.50 | 30.05 | 25.26 | 25.62 | -14.73% |

与 original 的总体 Script 对照：创建千行 -50.00%、替换千行 -44.24%、追加千行 -50.19%、创建万行 -44.81%。主要结论以同库对照为准，不能把同期语义或原生工作树改动混入 API 收益。

generic→final 仅改变后端常量，创建千行/替换千行/追加千行/创建万行的 Script 分别减少 **24.80% / 24.32% / 26.04% / 18.15%**。其余收益来自显式计划、Scope 减少及语义/HTML 构造复用的组合；这组实验没有进一步拆分这些因素。清空没有深克隆收益（25.26→25.62），不能认为销毁瓶颈已解决。

完整端到端中位数，含原 harness 的帧等待、布局和调度，单位 ms：

| 操作 | 原快照 | 同库 builders | Plan 通用后端 | Plan 深克隆 |
|---|---:|---:|---:|---:|
| 创建千行 | 84.80 | 81.75 | 66.05 | 54.10 |
| 替换千行 | 110.60 | 112.75 | 89.70 | 73.35 |
| 更新每十行 | 9.30 | 10.60 | 10.70 | 9.30 |
| 选行 | 7.90 | 7.70 | 10.50 | 9.60 |
| 交换两行 | 8.80 | 9.40 | 7.95 | 9.85 |
| 删除一行 | 9.15 | 10.55 | 8.40 | 9.95 |
| 追加千行 | 91.30 | 95.95 | 72.90 | 62.20 |
| 创建万行 | 909.10 | 903.60 | 709.00 | 627.60 |
| 清空千行 | 32.50 | 32.75 | 27.15 | 28.05 |

短操作的端到端数据受帧调度影响，并非全部改善，例如选行 7.70→9.60 ms、交换 9.40→9.85 ms。大操作两轮同方向改善：最终版两轮创建千行 26.87/28.78、替换千行 45.73/48.10、创建万行 347.23/376.22 ms；同库 builders 对应 55.19/55.32、84.42/83.62、655.14/639.58。generic 的替换两轮为 54.89/71.97，波动原样保留，不将单轮最好数当结果。

原始 WASM 大小 original / ordinary / generic / final 为 **940,360 / 995,439 / 988,347 / 1,006,656 bytes**。最终版比同库 builders 增加 **11,217 bytes（约 1.13%）**，比 generic 增加 18,309 bytes；另有生成 JS 模块。没有测量总 RSS，不将 Owner 数下降解释成内存字节下降。

## 宿主调用与正确性证据

以下是单独插桩的替换千行计数，未混入主计时：

| 指标 | 原 builders | Plan 通用后端 | Plan 深克隆 |
|---|---:|---:|---:|
| cloneNode | 8,000 | 8,000 | **1,000** |
| insertBefore | 8,000 | 8,000 | **1,000** |
| createTextNode | 3,000 | 3,000 | **0** |
| appendChild | 3,000 | 3,000 | **0** |
| setAttribute | 15,000 | 15,000 | **9,000** |
| UTF-8 decode 次数 | 20,006 | 21,007 | **5,007** |
| 活语义节点 | 8,010 | 8,010 | **8,010** |
| 活响应图节点 | 6,003 | 6,003 | **6,003** |
| Owner | 9,013 | 2,013 | **2,013** |

首次创建包含静态原型的 8 个元素、3 个文本、10 次内部 append；后续实例的物理后代由浏览器深克隆产生，因此不能把 createTextNode=0 说成没有文本节点。每次替换仍以原方式物理删除 1,000 个旧行根。两种 Plan 的 Owner 相同，深克隆独立收益不来自 Owner 再次减少。

四版全部主样本的行数/摘要一致；独立 seeded oracle 校验数据和九项操作。四版各六个画面检查点，共 **24 张 PNG** 同检查点逐字节一致。额外深身份探针对四版检查每个存续 Element、Text Node 和 data-gly-node；替换后旧后代全部断开；万行全部真实矩形的摘要一致。清空恢复到 0 行/0 label、10 个语义节点和 3 个图节点，未保留访问过的行。

新增测试覆盖：原始整批晚失败的身份拒绝与成功前缀；声明内容/外部祖先预检先于身份分配；实例数据/事件独立及退休；foreign part 不执行用户闭包；HTML 计划与普通构造逐操作和序列化相同；Group、受控 Input/Check、文本规范化；缓存弱键淘汰及预算；共用 conformance 检查 reference/SSR/stream/Blitz 的批次水位与退休。新增浏览器计划套件验证节点保留、选择区、输入法 composition 协议、实例污染隔离、Group fallback、SSR adoption 草稿和之后新建实例。

冻结版完整验收：Windows/Linux 各 **465** 项工作区测试、各 **289** 项干净 fork 测试；fmt、原生/WASM 严格 all-target Clippy、Linux no-default/paint、双平台 CPU raster，通过。Windows shell/accesskit 实窗与实际 UIA、Linux X11/Wayland × shell/accesskit 四组实窗通过。SSR fixture 双平台各 **6** 项测试；浏览器 **26/26**，真实 stream Shell/Patch/End、资源交接、断开取消和慢消费者背压通过。64/1024 测量缓存保留真实小数行高与原容差，锚点均 **-15.125→-15.125 px**。

测试中发现原创建桥接验收将生成模块编号写死为 inline0；新增模块后编号改变。改为按实际 export 找模块，没有放宽产品断言，之后重新跑完 26 个套件。期间同步出现的六个表单/原生文件变化另存 `post/diff.json`，保持正式测量源码不动；工作树后续回归单独归档，避免将不同版本混称同一构建。OS composition 协议测试不代替实际 OS IME 或读屏器验收。

计时后的当前工作树补充回归完成：Windows/Linux 各 **466** 项工作区测试，严格原生/WASM Clippy、Windows 三组实窗/UIA、Linux 四组实窗及双平台各 6 项 SSR fixture 测试通过；最终浏览器再次 **26/26**，真实流式检查通过。post 与冻结版的两个测量预算各 70 次真实小数行高变化、各 73 张图片，图片哈希逐张一致，锚点不变。`post/inputs.json` 与当前 crates 源码逐项核对，任务自身的计划实现未在计时后更改。

`verify.py` 重算全部中位数，验证严格源码差异、四个 fresh release、D: target、样本/PNG/身份/几何与全浏览器输入。`summary.json` 包含每轮原始聚合及所有五项 CPU/端到端指标；原始计时保留在 formal/*-samples.json，插桩计数/CPU profile 在 probe/。这些数据仅比较本轮四个 Glory 版本，没有用旧的 Leptos/Dioxus 数据声称新的跨框架排名。

完整证据索引：[evidence-index.json](/D:/BuildCache/glory-static-plan-20261024/evidence-index.json)。保留首次失败、修复后的完整复验、冻结与 post 两份 browser 输入/输出和旧 fixture site；索引排除可再生 target/tmp/cache，仅对实际保留的源码、构建记录、发布 WASM/JS、原始样本、profile 和验收图片逐项记哈希。自建 HTTP/stream 服务完成后均核对进程所属并停止。

## 保留的成本与后续边界

深克隆减少宿主调用，并未消除真实 DOM 的创建与最终布局。八个语义节点仍各有 generational identity、描述符和 DOM Entry，两份 Memory 的职责保留。Binding/事件仍在本行 Owner 下独立注册；内部静态节点不再各持一个填充 Scope，但 keyed each 与 component 的行级 Owner 仍存在。不能从调用次数推算 RSS 或将所有收益归因于浏览器原生克隆。

本轮只实现明确声明的创建单元，没有将连续任意 mutation 编码为字节流，也没有跨查询/动作/受控输入屏障合并。Group 深克隆、压缩数字索引或进一步共享静态描述符需要另行量化与契约验证。普通 builders 继续可用，声明里不适合静态化的部分仍可在实例外用普通 control flow 构造；跨平台适配器可以逐步增加自己的批量创建实现。
