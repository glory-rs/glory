# 语义 UI 重构实施记录

依据：[架构报告](semantic-ui-rendering-architecture-report-2026-10-21.md)。用户已授权破坏兼容的实现；最终不保留旧 API、Node alias 或 wire 兼容层。此文件记录实际证据，未勾选项不代表已经交付。平台与组织条目仍须有实际证据才计为交付；历史阶段记录由后续节更新。

第三方代码维护已按2026-10-23用户要求改为glory-rs组织fork及完整Git revision，见[引擎依赖维护](engine-forks.md)。仓库中不保留vendor。下文早期本地源码目录、包名和校验脚本是当时的历史记录，已被当前Git依赖及scripts/check-engine-forks.py替代，不是当前复现入口。

2026-10-25 [Plan 批量准备优化](plan-batch-preparation-optimization-2026-10-25.md)保留共享声明摘要和严格单调 slot 的批内唯一性证明，乱序保留 HashSet 回退；两份 Memory、完整身份和当前上下文校验不变。千行创建/替换 Script 改善 7.5%/4.2%，5,000 行看板初始化改善 6.0%；万行不稳定、密集更新回退另列。最终同批 1,800 样本，Windows/Linux 908/907、27 组浏览器及 64/1024 真实行高验收通过。直接组装子树实验未保留。

2026-10-24 原生 viewport top-layer 扩展已由 genuine Blitz fork、driver 和独立实际
consumer 验证：Dialog/AlertDialog Manage、非 modal Dialog Preserve、十二种锚点定位、
modal 焦点/AX/滚动及关闭清理均有证据。全部 native targets 195 通过计数/45 结果报告
（含零测试 examples；库 93 通过/2 忽略）及严格 Clippy 通过，记录在
`D:/BuildCache/sapid/native-layers-final-all.log` 和 `native-layer-final-clippy.log`。
有限能力边界、完整 fork SHA 和复现见 [native-top-layers.md](native-top-layers.md)。

## 完整交付范围

- [ ] R0：八组件、内容模型、类型化 Patch、生命周期、事件、查询屏障与能力错误契约。样式profile、基础目标/内容扩展、事件几何/来源、批处理恢复，以及输入版本/规范值已接入；原生Password呈现、其余props、焦点回读和结构预检等审计仍待完成，见组件契约审计。Portable stylesheet尚未提供；按报告v4，在声明该能力前完成文本校验，不作为HTML首发的隐含能力。
- [x] R1：style/render/core 分层，代际 Instance/Owner/Binding、直接依赖、动态重绑、任务取消、内存后端。证据见下方 R1 验证。
- [x] R2：纯 Rust UI builder、each/when、延迟创建、无盒子的 group、统一门面。另已接入公开的定高 list 组件，证据见下方 R2 验证。
- [ ] R3：共用 HTML mapper、Web DOM/事件/IME/资源、多 Runtime、样式与真实浏览器验收。DOM、组合事件协议、定高/变高列表及 owned 异步资源已验收；外部资源加载策略、操作系统输入法仍待补。
- [x] R4：SSR/document/快照/hydrate/真实流式输出、取消与背压。首发契约已在 Windows Chromium 与 loopback HTTP 验收：渐进 DOM/CSP、资源恢复、流结束接管、慢消费者与断开取消；证据和明确适用范围见下方 R4 渐进 HTML 一节。
- [ ] R5：Blitz 无窗口文档与 shell、Windows/Linux 验收。文档/CPU 像素、shell、焦点/选区/导航和受控编辑/组合协议、资源内核、桌面 HTTP/包/文件挂载 Loader、共享请求/完成背压及固有 PNG 高度锚点已有证据；可选AccessKit桥和Windows UIA的编辑/激活/虚拟行显示请求也已实测，范围见末节。真实OS IME、Unix AT-SPI客户端、读屏器/完整文本接口及Android资源仍待完成，不能以合成输入或局部预算代替它们。
- [ ] R6：Android Activity/Surface/IME/恢复/打包与设备验收。
- [ ] R7：惰性验证、字段依赖、样式缓存、性能与资源观测。Blitz 帧级视觉 revision/缓冲复用、固定聚合阶段计时/重复基线，以及绘制前有预算的本地观察已有实现；引擎内部/呈现时间分解、字段依赖、样式缓存仍待完成。
- [x] 清理旧依赖、feature、示例、CLI 和文档；只留新公开架构。2026-10-26 删除剩余 9 个未支持的旧包及相关入口；历史评估保留 Git 固定链接。平台与其余架构目标不因此标为完成。

R8 的其他工具包按报告作为后续按需适配项，不用空 crate 或未运行的平台宣称已支持。

## 大数据滚动专项（首发功能要求）

- [x] `list` 控制流采用窗口化挂载，滚动不扫描/克隆全部数据，普通 each 仍用于非虚拟列表。已有语义后端及真实 Chromium 滚动证据。
- [x] 定高模式定位 O(1)、额外索引空间 O(1)；变高模式测量更新及偏移定位 O(log n)。公开组件已接稀疏 Fenwick 与真实 ResizeObserver；密集索引仍可独立使用。
- [x] 滚动工作量取决于可见窗口与 overscan，节点/订阅/任务数量有界；不缓存所有离屏组件。已验证公开组件及全局来源的行派生值回收。
- [x] 显式窗口行预算：默认 4096（含 overscan，焦点 pin 最多额外一行）；初始超限不读源/建行，动态超限释放行与任务，恢复后继续运行。
- [x] 稳定 key、重排、数据更新、prepend、测量修正与滚动锚点；`.locate` 由数据适配器提供索引查找，框架不扫描全表。
- [x] 列表有界焦点 pinning：窗口外最多保留一行；定高/变高真实 DOM、选区、组合协议、任务寿命及 indexed prepend/删除已验证，详见末节。
- [x] 列表逻辑行键盘导航、焦点回退与语义位置/总数元数据；定高/变高的真实按键、DOM 属性和 Chromium AX role/name/focus 已验证，见末节。
- [ ] 列表真实平台辅助技术验收，包括屏幕阅读器对位置/总数的朗读与浏览/交互模式；DOM/AX 检查不能代替它。
- [x] 每帧合并滚动/视口/行测量，避免读写布局交错；支持真实滚动容器与视口变化。
- [x] 固有 PNG 迟到增高/缩小保持锚点；End 在首次占位测量后持续保持可见；滚动确认区分量化与真实小幅移动。Web/Blitz 及受控位置回写均有验收。
- [x] 原生绘制前有预算地消费本地几何反馈；变高列表避免中间绘制，预算耗尽让出宿主，异步 Web/I/O 不同步空转。CPU 像素、实际宿主与交替 release 对照见末节。
- [x] 超长列表使用逻辑坐标与受限物理滚动范围，验证首尾跳转与坐标精度。浏览器证据为 1 亿行、24 亿逻辑像素，算法证据另含 240 亿像素。
- [x] 至少百万条数据的算法、实际浏览器 DOM 数量、滚动行为与清理验收；性能报告区分算法、响应式和布局成本。当前浏览器验收不声称帧率。

## 当前实施

2026-10-26 按用户要求比较 Prisma Client Rust 的项目内 CLI 安装方式后，
新增独立的 `tools/cli` 工具 workspace，仅提供 `glory-cli` 库接口，尚未发布。
每个应用以私有 `tools/glory` 小程序调用它，与应用共用 Cargo.lock；生成
`bin/glory.ps1`、WSL `bin/glory` 和 Cargo alias。入口在编译工具前设置
D: Cargo/cache/temp 和原生 target，应用编译使用独立的 app-target，避免
外层 Cargo run 占用构建锁。普通项目推导入口、features、index/public；
仅覆盖项写入 package.metadata.glory，不新增配置文件。

本轮提供 doctor/print/build/run/serve/check/fmt/bundle/init，检查框架版本
与 Git 来源，按已解析版本安装项目内 wasm-bindgen。Web serve 支持源文件/
资产监听、刷新、错误显示和失败保留上次输出；Web/server 发布目录带 SHA-256
清单。counter 已接入共享锁与项目命令，其旧 build.ps1 改为转发入口。
此实现不恢复删除的旧 CLI、RSX/hot-patch 或全局工具安装；不宣称交付原生
安装器、Android 打包、SSG、Binaryen 优化或 SSR 开发代理。
最终方案、Prisma 比较和操作见 [工具说明](../tools/cli/README.md)。

CLI 的 15 项测试和严格 Clippy、counter 的 3 项测试、Web release bundle
以及真实 SSR 小项目的 run/server bundle 均通过；两个 bundle 的全部文件
哈希核对通过，发布文件中无开发刷新脚本。真实 Chrome 在 D: 临时 counter
副本验证 CSS 刷新、编译失败时旧应用仍可点击、已接受输出不被覆盖、错误源码
不阻止 doctor、修复后刷新，pageErrors 为空。WSL 仅检查入口语法和当前
`//localhost/D$` 本地 D 盘挂载识别，没有把它记为 Linux 编译/窗口验收。
根 runtime metadata 仍为 12 包、217 targets，依赖/features/target 定义
逐项一致；工作区全部 feature 测试 956 通过、2 项既有忽略，fmt 与全部
feature/target 严格 Clippy 通过。工具作为独立项加入统一检查与格式 CI。
提交前按用户要求统一复核全部改动，修正先前阻断门禁的格式差异，并为两个
仅供 hydration 使用的 Web helper/import 补上 feature 限定。
实验产物的 `check-semantic-ui.ps1 -Web` 曾完整通过：CLI 15 项、fork 440 项、
960 项测试通过（2 项既有忽略），fmt、native/WASM 全 feature/target 严格
Clippy 和 Web fixture 构建通过。使用 D: 项目缓存内的 wasm-bindgen 0.2.129，
未依赖全局的旧版本。counter 的 3 项测试和 Web/desktop 严格 Clippy 通过；
Web 的无默认 feature 与全 feature WASM 严格 Clippy 分别通过。
该实验产物的 29 组真实 Chrome 验收全部通过，覆盖跨 mutation 字段提交的
内存增长和故障前缀、Plan 局部退休、输入/接管/stream 等路径。
跨 mutation 字段提交随后因两版测量回退撤回，生产代码恢复到此前的 Plan
宿主目标；上述 960 项/29 组是实验产物的证据，不作为当前启用该实验的声明。
最终字段提交报告和完整基准 JSON 保留实验、撤回结论与来源摘要。
撤回后的生产源码再次通过 `check-semantic-ui.ps1 -Web`：CLI 15 项、fork
440 项、工作区 956 项测试通过（2 项既有忽略），fmt、native/WASM 全
feature/target 严格 Clippy 与浏览器 fixture 构建通过。逐文件恢复摘要与
检查期间的源码快照一致。当前生产产物的 28 组真实 Chrome 验收全部通过，
对应证据在 `D:/BuildCache/glory-project-cli/restored/browser-foundation-final`；
没有将已删除的实验浏览器探针算入当前验收数量。
日志、metadata、浏览器与哈希验收证据位于
`D:/BuildCache/glory-project-cli`，全部构建缓存和临时文件也位于 D:。

2026-10-26 继续完成旧源码退出：删除 CLI/cli-harnesses、macros、serverfn、
hot-reload、desktop/native/liveview/tui 共 9 个非活动包，剩余旧示例、旧基准
wrapper、16 份旧指南以及旧发布/移动端 CI；同时删除门面未加载的 cfg 与 CSS
测试残件。本轮删除 191 个文件，根 workspace.dependencies 的全部声明经核对
均无活动消费者，已整体移除。现有语义基准、测量证据和预先存在的源码修改保留；
历史评估的 37 处旧路径改为已核对存在的 Git 固定链接，旧测量指令移入 Git 历史。

前后 metadata 保持相同的 12 个包、217 个 target，依赖、features 和 target
定义逐项一致；其余活动 Rust 源码哈希不变。`check-semantic-ui.ps1` 通过：
fork 440 项、工作区 954 项测试通过（2 项既有忽略），fmt 和全 feature/target
严格 Clippy 通过。工作区 WASM 全 feature/target 严格 Clippy、counter 的 3 项
测试及 semantic-web 的 WASM 全 target 编译检查通过；9 个保留的独立示例/基准
清单均通过 locked metadata。Playwright 保留 counter 项目，测试发现通过。
本次未执行真实浏览器/窗口/OS 输入验收。审计、日志和构建缓存均位于 D:，证据根
目录为 `D:/BuildCache/glory-legacy-cleanup-20261026`。

2026-10-26 按用户要求移除 Truck 与关联旧架构：删除未加载的旧 Core
scope/view/node/reflow/web/renderer、旧 HTML/command 测试和基准、旧 routing
包及五个依赖它的示例；同步清理 Routable/Store 旧派生、旧浏览器测试入口、
失效路径和指南。现有新运行时源文件与工作区包/依赖/原有 target 保持；
`core/tests/task_host.rs` 继续由 runtime 测试引入，不重复登记。当前语义 UI/SSR/Web 测试承担新的行为契约，
不保留旧 wire 兼容测试，也不声称已交付旧路由功能；CLI/serverfn/其他旧 adapter
仍属于未支持的提取参考。此次是 Truck 调用链清理，不将整体旧架构退出项勾为完成。

最终 metadata 对照保持 12 个包、217 个 target，包依赖、features 和 target
定义全部一致。`check-semantic-ui.ps1` 通过：独立 fork 440 项测试、工作区全部
feature 测试、fmt 与全 target 严格 Clippy；撤去重复测试登记后的最终工作区
复验为 952 通过、2 忽略，`task_host` 只在 runtime 中执行一次。WASM 工作区
全部 feature/target 编译检查及保留的 Playwright 项目发现通过；此次没有重跑
浏览器或实际窗口/OS 输入验收。日志、前后 metadata、删除清单和未触及源码哈希
位于 `D:/BuildCache/glory-truck-cleanup`，编译缓存与临时文件也全部在 D:。

2026-10-25 新增 `each_seed` 明确同 key 只消费首次 T，省去 item/index Cage 和
data Owner；普通 `each` 保持活更新语义。普通 Part 销毁/挂起省去单元素范围 Vec，
退休队列直接查活 Entry 表，省去临时 HashSet。JS Part 数组/按需句柄实验虽然减少
桥接调用，但交错计时未显示稳定收益，已撤回；新增部分退休/查询/恢复验收保留。
两份 Memory、所有语义身份、受控输入和成功前缀保持；跨行批量创建/挂接及动态字段
一次初始化仍未实现。计数、对照和验证范围见 [性能记录](performance.md#plan-physical-parts-and-seed-rows--2026-10-25)。

2026-10-24 创建提交继续优化：固定 JS 参数合并浅克隆、class/style 与身份/live，
其他属性与受控输入保持原路径。独立 1,080 样本中千行创建/替换/万行 Script 降低
3.0%/3.8%/6.7%；属性打包及 Memory 间接存储实验均未保留。流式 fixture 增加生成
JS 模块的有限路径服务，双平台各 452 工作区/289 fork、25 浏览器、64/1024 真行高、
实窗/UIA/stream 全回归通过。Dioxus 的静态子树、编码提交及后续优先级见
[专项分析](bulk-create-dioxus-2026-10-24.md)；后续静态计划尚未实现。

2026-10-23 HTML/DOM 创建优化：规范 String 保留分配，活 DOM Part 内联三个槽，
文档最多缓存 64 个仅含有限静态属性的浅元素原型。动态值/身份/class/style/URL 不入原型，
未知/满表回退，清理释放；保留最终属性、受控输入屏障和 SSR/stream 格式。
独立构建 432 样本的创建千行/替换/万行 Script 降低 17.0%/7.8%/11.4%，
双平台各 448 工作区/289 fork、25 浏览器套件、64/1024 真实行高和窗口/UIA/流式回归通过。
并行修改与测试修正的边界见[HTML/DOM 创建报告](performance.md#html-mapping-and-dom-creation--2026-10-23)。

2026-10-23 Scope/unlink 继续优化：Children 采用有界前缀和摊销压缩，连续前删避免
重复搬移；Scope 融合 DFS 与节点/监听 Owner 注销，保持回调顺序和借用外释放。
独立构建、P 核受控的 432 样本中，清空千行 Script 43.06 → 31.24 ms，
替换千行 94.51 → 93.02 ms，后者收益很小。434 项/平台、fork 各 289 项、
浏览器 24 套件、64/1024 真行高锚点及原生窗口/流式回归通过。
详见[Scope/unlink 报告](performance.md#scope-cleanup-and-sibling-unlink--2026-10-23)。

2026-10-23 Owner/Binding 继续优化：核心重复退休/叶节点免分配遍历栈，UI 将节点、
监听和查询集中为一个 Owner 记录，最后一个兴趣退休时移除。432 样本对照中，
清空千行 Script 中位数 34.35 → 32.05 ms（-6.7%），替换千行 92.11 → 94.39 ms，
本轮不声称替换提速。Windows/Linux 各 430 项、fork 各 289 项、浏览器 24 套件、
64/1024 真行高锚点、原生窗口/UIA 和流式取消/背压均通过。
详情见[Owner/Binding 退休报告](performance.md#owner-and-binding-retirement-follow-up--2026-10-23)。

2026-10-23 DOM 父节点表销毁改为按 mapper 完整子树中的 NodeId 删除，尺寸缓存同步
按键清理；每个行子树不再重复扫描父表。同期独立构建的 432 样本对照中，替换千行
221.50→143.80ms、清空千行 150.15→40.30ms，创建/追加基本不变。存活后代与 Text
身份、万行几何及六个 PNG 检查点一致。随后修正基线中的 Clippy 条件与桌面 feature
隔离，再以相同最终源码补跑 108 样本及完整门禁：Windows/Linux 各 428 项工作区测试、
浏览器 24/24、实窗/UIA/CPU 绘制及流式检查通过。该冻结快照与并行后续改动的边界、
原始采样和失败重跑记录见[销毁修复报告](performance.md#dom-retirement-follow-up--2026-10-23)。

2026-10-23 批量 DOM 修订已取消 Element.fill 的中间 content Scope，保留每个子组件
独立 Owner。Memory 保存存活节点的逻辑 next sibling，使无变化位置检查不再扫描
全部兄弟；Scope 内容节点不加入容器的 movable roots。Core 完整退休 Owner 子树后，
UI 清理沿已经退休的语义祖先合并 Dispose，遇到存活 Owner 停止；用户清理回调顺序
不变，已释放身份不重复提交。CSR 创建直接持有 Part/Node 引用，省去解析用的
key/part 属性和文本注释，Group 范围与 SSR/stream/hydration v3 格式继续保留。
data-gly-live 只用于 VirtualPart；普通节点不创建停放 fragment，真正 Suspend 时
按需创建。详见[批量优化报告](bulk-dom-optimization-2026-10-23.md)：432 个配对样本、
节点/画面验证与当时仍可复现的 0.375 px 基线锚点问题均单独记录。

随后修复变高列表的宿主滚动量化余差：行样式 origin 使用 `segment_origin + bias`，
新目标请求原始物理投影，不重复扣除旧余差；动作去重不再吞掉真实小数变化。
64/1024 测量缓存均保留真实行高和原 0.1px 容差，70 次连续变化保持锚点与全部存活
后代（含 Text Node）身份，收敛后 PNG 一致；24 套浏览器验收全部通过。
极远逻辑段另有 0.125px 投影变化回归，完整门禁与独立快照见
[滚动余差修复报告](anchor-residual-fix-2026-10-23.md)。

已建立 style/render/core/resource/ui/html/web/ssr/blitz/glory 十个活动 workspace 包。依赖主线为 style ← render ← core ← ui；HTML mapper 只依赖 render/style，web/ssr/blitz 在外层组装，可选 resource 依赖 core/serde。门面默认不引入 Web/GPU/serde/Blitz，web、ssr feature 可同时开启；resources feature 可独立使用，只向已选择的宿主转发集成。原生 blitz feature 另行选择，不为默认/HTML/SSR 引入引擎。旧 core 入口与门面已经替换，旧 feature 和全局 Node alias 不再编入活动代码。旧适配器和工具目录暂时移出 workspace，旧 core 未引用源文件作为行为提取参考，最后仍需清理；这是实现过程的中间状态，不是最终兼容方案。

已实现并验证的子项：

- 类型化八类节点、有限 props/事件/查询、Keep/Set/Clear；Group 无样式盒子，交互控件嵌套和无效输入配置有诊断。
- RuntimeId + slot/generation 节点身份、明确 Place/Suspend/Resume/Dispose；暂停子树仍归父节点所有，永久删除清理所有后代。
- 内存参考解释器不伪造几何/显示完成；查询响应携带已应用水位；暂停/销毁后的已排队事件不再送达。
- 定高窗口 O(1) 索引空间与定位；变高 Fenwick 索引、O(log n) 测量与偏移定位。密集索引 splice 为 O(n)；稀疏索引只重建已有测量，均不在滚动路径调用。
- 窗口行稳定 key + LIS 复用，数据 revision 更新保留行内容，离屏立即销毁；相同窗口与 revision 不读取数据源。
- 测量/prepend/删除锚点修正；有界物理滚动坐标与重定位。定高/变高 DOM、每帧调度、有界焦点 pinning、逻辑键盘导航和位置/总数元数据已接线；真实平台辅助技术仍待验收。

测试覆盖百万条惰性数据源：600px 视口、24px 行高、前后各 4 行 overscan，最多 33 行/34 个参考节点；逐行滚动仅新建/销毁一行；跨列表反复跳转后 ID 存储仍有界。500 次随机数据编辑验证顺序和实例复用；2000 次变高测量与朴素模型逐项对照。

第一阶段验证为 19 个测试与三包检查；最新 R1 验证已扩大为下面的五包活动 workspace。CPU 算法基线保留原测量范围，不能把它视为新 UI 或实际平台性能证据。

复现入口：[check-semantic-ui.ps1](../scripts/check-semantic-ui.ps1)。使用 `-Bench` 运行 CPU 基线，结果写入 D 盘构建目录；性能证据与适用范围见 [列表基线](semantic-ui-list-baseline-2026-10-21.md)。

## R1 验证与行为

`crates/core/src/lib.rs` 已切换为新 arena/runtime，不再导出旧 reflow/web/Node；core 只依赖 render → style。全局 thread-local 仅用于本地 Copy handle 查找和当前求值上下文，所有值、图、Owner、任务与调度队列都在各自 Runtime 中。

已验证：直接 source → Bond → Binding 图、输出相等阻断消费绑定、动态依赖退订、菱形图稳定读取、纯计算写入诊断、循环检查、panic 后恢复、嵌套 batch、正常 Effect 稳定化与反馈循环诊断。反馈限制按同一观察者重复次数计算，不会把 100,001 个独立绑定误判成死循环。

`get()` 改为返回拥有的 clone；大数据用 `with(...)` 借用单个索引，不需要数据类型实现 Clone。信号/绑定/任务句柄不可跨线程传递；代际槽位溢出时退役，不循环复用。

Owner 覆盖信号、派生、Binding/Effect、子 Owner、Instance、清理函数与任务；销毁先失效身份和订阅，再在图借用之外执行取消/析构。Host 注入 spawn/wake/clock，缺失 Host 时明确报错，不执行 block_on。取消实际销毁宿主 future，同时撤销过期任务提交权限。

`crates/ui/tests/list_runtime.rs` 将新 core 接入百万条列表参考解释器：只修改一个可见行时增加一次 Binding 执行和一次文本 Patch；离屏任务取消，重新进入窗口后保留业务数据。近千次跳转后，33 行窗口的活跃/分配上限为 100 个图节点、35 个 Owner 槽位、34 个 Instance 槽位、33 个任务槽位；清空窗口后只保留业务数据源和根作用域。

实际命令：

```powershell
./scripts/check-semantic-ui.ps1
cargo tree -p glory-core --edges normal
cargo check --workspace --target wasm32-unknown-unknown
cargo check --workspace --target aarch64-linux-android
cargo check --workspace --target x86_64-unknown-linux-gnu
```

已通过 43 项运行测试、1 项 !Send 编译失败文档测试、活动 workspace 的 fmt/严格 Clippy，以及上述三目标编译检查。所有构建继续使用 D 盘；这些目标检查只证明无平台依赖的基础层能编译，未运行 Android/Linux 图形界面。门面运行示例为 `cargo run -p glory --example reactive`：只打印一次 parity，验证相等门控阻断消费端。

## R2 验证与行为

已经实现 `glory-ui::{App, Scope, Widget}`、八个 builder 与 `row/col/component/each/when/list`，门面 prelude 重导出这些入口。`glory_core::Scope` 只持有逻辑实例/所有权；UI Scope 在其外面附加语义父节点与挂载生命周期，不含 HTML/DOM/GPU 类型。

构造 builder 不分配呈现节点；mount 才创建。每个动态 prop 安装独立 owned Binding，并比较最后注入值；重复配置同一 slot 只有最后一次生效。输入版本、禁用、失效身份与控件事件类别在业务回调前验证。Activate 从子内容找到最近的处理器，同时检查禁用祖先，不重复执行动作。组件依然显式绑定，裸读取不订阅外层结构控制器。

Core 调度新增结构阶段，按“结构 → prop binding → Effect”稳定化。Each 的 key 重排使用 LIS、复用行实例与业务状态，位置与 item Cage 刷新后才注入 props；空组件和多根组件的插入顺序有测试。Each 明确为全量序列，仍会取得完整快照；大数据使用按索引的 List。

动态工厂在组件 Owner 中运行，根场景提供 `App::mount_with`。Cage::map 在构造上下文中归当前 Owner；上下文之外仍归来源 Owner，预先创建的全局派生值保持其声明寿命。Each/List/When 工厂、Widget::build、任务轮询与 TaskToken::commit 均进入对应所有权上下文；纯计算不能每次求值再分配一批响应式节点。测试覆盖全局来源在反复挂载、分支切换和远距离滚动时的派生值回收。

App 的语义模型先验证有限操作，再向任意 Render 批量提交，无 JSON 中转。Query 先提交帧；等待者按节点索引，目标销毁后异步完成 NodeGone，过期回复不会重复调用。Focus/ScrollTo 只在后端声明能力时提交，动作在本帧 mutations 之后、后续 query 之前执行。后端应用失败可能留下前缀，App 随即进入 faulted 状态，不假装回滚。

公开定高列表用法（纯 Rust）：

```rust,ignore
use glory::prelude::*;

app.mount(list(
    100_000_000_usize,
    |index| index,
    |index, _key| text().content(index.map(|index| format!("row:{index}"))),
).height(600.0).row_height(24.0).overscan(4))?;
```

Key Bond 只读取当前窗口。窗口范围不变的像素滚动不重新取 key，也不丢失原先的数据依赖；key 不变的内容更新由行自身绑定处理，key 重排复用保留行。固定行高下，100 万/1 亿条没有全量列表索引或快照；小数像素可能多覆盖一行，600px/24px + 双侧 4 行 overscan 的上限是 34 行。

内层表面使用保守的 1,000,000px 物理范围，逻辑位置与 origin 分离；`.position(...)` 支持程序跳转，`.on_scroll(...)` 返回逻辑位置。重定位使用 ScrollTo，接收预期 native 位置后不会再次重定位。R2 交付时只接定高模型；后续变高测量、prepend 和真实 DOM 证据见下节，焦点 pinning 与键盘/无障碍导航仍待完成。

新增证据：

- `crates/ui/tests/widgets.rs`：公开 counter 一事件一次 Patch、八组件/内容校验、Each 重排状态、空/多根节点、分支 500 次回收、过期输入、查询取消、外部 Scope clone 与失败工厂清理。
- `crates/ui/tests/list_widget.rs`：1 亿条惰性源，200 个同 tick scroll 输入只读取最终窗口的 33 个 key，并一次提交；同窗口后续像素不取 key/不提交 Patch；数据重排仍正确订阅并复用实例。
- 同上测试：10 亿条、240 亿逻辑像素的中间/末尾跳转与 1px 增量；远跳后语义节点/槽位不超过 70，事件处理器为 2，任务数与当前行数一致；全局来源的行 map 不积累到 app Owner。

R2 完成时活动 workspace 验证为 64 项运行测试、1 项编译失败测试、fmt 和严格 Clippy；`cargo run -p glory --example semantic_counter` 输出 4 个语义节点。`cargo check --workspace --all-targets` 在 WASM、Android aarch64 和 Linux x86_64 目标均通过。它们仍是参考解释器和编译证据；新增真实 DOM 证据见下面的 R3 记录。

后续继续补全 R3 资源与平台输入验收、R4 请求资源与 stream，再推进 Blitz/Android。同步 SSR 与 DOM 接管已有子集证据；整体目标仍未完成。

构建统一使用 `D:/BuildCache/glory-semantic-ui`，每次新构建配置先核对 cargo metadata 的 target_directory。后续记录实际命令和结果，不能用设计文件替代运行证据。

## R3 DOM 与 R4 同步快照子集

`glory-html` 共用 Shape、Part 和增量 Edit，把有限语义节点映射为标签、属性、表单 property 和独立文字槽。Button/Link 的标签更新只修改 Content 文本节点，保留用户子节点；Check 的 label/input 分 part；Group 为成对注释锚点，无包装盒子。映射不引用 core/ui/web-sys。HTML options 在门面 html feature 的扩展 trait 中连接 UI；语义标题与 HTML tag 冲突有错误，已挂载后的标签结构变化要求重挂载。

`glory-web` 在 wasm32 下实现真实 DOM 消费器和 RAF/async Host。原生监听器仅排队，不同步执行业务；帧开始先读取去重后的滚动/视口尺寸，再由 App 处理输入并更新 DOM。只对 Resize 订阅目标安装 ResizeObserver，不观察每一行。注册在其他组件节点上的处理器也归注册 Scope 所有，释放 Scope 会退订；已加入 100 次订阅/销毁和处理器覆盖回归。卸载会断开 observer、撤销 RAF、移除监听器与节点；基线 CSS 是文档级单例，允许多个 Runtime 共用。

表单使用 typed value/checked/disabled property，避免空字符串和 false 丢失。当前值与目标相等时不调用 value setter，保留选区；组合输入期间暂缓冲突的程序写入，组合结束后应用新 revision，旧编辑被 App 丢弃。元素移动优先用 moveBefore，回退 insertBefore 时恢复焦点和选区。几何/滚动/选区请求经过提交屏障，返回真实值或明确错误；断开连接的范围不能伪造可测量几何。

首次 R3 阶段的 `glory-ssr::render` 与 `render_document` 提供同步 HTML：确定性 key、文档元信息、转义、textarea 首换行、布尔属性、暂停节点跳过。几何查询明确不支持且保留提交水位，空 commit 不会把水位归零。当时未接 hydration；后续接管证据见末节。请求资源快照和流式输出仍未实现。SSR 与 Web 可以同时启用，不再复用互斥的 Node alias。

### 实际验收与复现

```powershell
./scripts/check-semantic-ui.ps1 -Web
./scripts/build-semantic-web.ps1 -Serve
```

第一条已通过：71 项运行测试、1 项 !Send 编译失败测试、活动 workspace 的 all-features 测试与严格 Clippy、全 workspace WASM 严格 Clippy，以及独立浏览器例子的格式/Clippy/构建。脚本固定 D 盘缓存并检查 metadata；wasm-bindgen library/CLI 版本为 0.2.123。

第二条在 127.0.0.1:8788 提供验收页。`examples/semantic-web/acceptance.js` 是接收 Playwright page 的 async 函数，可由 Playwright 的 run-code 工具加载执行；构建脚本本身不运行浏览器测试。本次已在 Windows Chromium 155 执行该函数，未出现应用错误：

- 点击按钮子图标只激活一次；标签 Patch 保留图标。
- 输入空值、false 勾选更新；无关更新保持光标/选区；Enter commit。
- 合成 composition 事件与程序写入竞争，旧 revision 不覆盖新值。此项验证浏览器协议，未操作操作系统 IME。
- Each 重排保留输入对象、草稿、焦点与选区；强制禁用 moveBefore 后回退仍通过。
- Group 暂停/恢复保留完整范围及原输入对象；分支 20 次切换正确回收。
- 嵌套和并列 Runtime 隔离输入与图状态，基线 CSS 只有一份。
- ResizeObserver 验证容器高度 600→240→600px 时，初始窗口从 29 行缩至 14 行再恢复；浏览器确实采用 overflow-anchor:none。
- Focus action 后异步 Selection/Bounds 返回真实选区和非零几何。
- 1 亿条惰性行、24px 行高、600px 高容器（测试 CSS 边框使实际 clientHeight 为 598），80 次远跳和最后一行；物理 scrollHeight 不超过 1,000,001px。
- 上述远跳阶段最多 33 行，整页（含表单、Group、并列 Runtime）带 `data-gly-node` 的元素最多 84；主 App NodeId 槽位最多 86、图节点最多 137、处理器维持 7。
- 同一帧 200 次原生 scroll 只读取最终窗口的 34 个 key；之后窗口内一次像素滚动不读 key。逻辑偏移跟随实际 fractional scrollTop，不因总条数放大；缩放/DPR 引起的浏览器舍入单独计量。
- 卸载后节点与根声明清空，挂载→跳转→立即卸载重复三次无应用异常。

以上为首次 R3 定高列表的 DOM 证据，没有采集 layout/paint 计时、堆快照、FPS，也没有运行其他浏览器/原生窗口/手机。后续变高与 prepend 证据见下节；焦点 pinning、键盘与无障碍、资源边界、hydrate/stream、Blitz/Android 和旧代码清理仍未完成。

## 大列表：变高测量与稳定 key 锚点

公开组件新增 `.measure(estimate)`、`.measure_limit(capacity)`、`.locate(key_to_index)`、`.revision(version)` 与 `.inspect(callback)`。定高仍用 `.row_height(...)`；默认 24px。变高估值至少 1px，自动高度包装行也设 1px 下限，避免空行产生无限的“可见”条数。行实际高度来自 Resize 事件，不由核心执行 CSS 布局。

`Heights::sparse` 用统一估值加稀疏 Fenwick 差值，初始不分配逐行数组；单次测量、prefix 和偏移定位的期望成本 O(log n)。缓存保留稳定 key、位置和高度，默认最多 1024 条元数据，至少保护当前 render window。超额时淘汰窗口外旧测量并恢复估值，同时围绕同一锚点补偿 prefix 差值。空间为 O(max(cache_limit, window) * log n)，不会随访问过的总行数积累；每个 Fenwick 节点还维护贡献数，最后一个贡献移除后删除节点，不留下浮点残差历史。原密集 `Heights::variable` 仍供已知全量高度的场景使用，不作为巨大数据默认实现。

这是一项明确取舍：未知及被淘汰的行高度是估值，整表总高度也会随测量修正。如果应用要求所有离屏行的精确绝对像素位置，需要业务自己的完整高度索引；不能同时保证“不保留全表高度”和“所有未观测高度精确”。UI 实例、任务、绑定继续只属于可见窗口，与高度元数据缓存分离。

SSR 不执行浏览器测量。变高阶段首次交付时只输出估值位置；后续 R4 已接正常文档流预览与接管锚点，见末节。预览只包含初始窗口，不能将其视为全表静态输出或服务端计算了实际几何。

测量和数据编辑先恢复锚点、计算窗口，再在同一个 structural controller 中按 key 协调行。这样大 prepend 不会先按旧位置销毁本应保留的行。`.locate` 必须是 O(1)/O(log n) 的数据索引；返回位置会与 key_at 验证，错误返回 InvalidLookup。没有 locate 时只在已有窗口内寻找保留 key，不执行隐式全表扫描。键来源不是响应式闭包时用 revision 显式通知编辑。

Resize 契约现在区分 border-box `size` 与 client `viewport`。Web 观察行 border-box，优先采用 observer 的小数布局尺寸（不受 CSS transform 放大）；只对同时消费 Scroll/Resize 的容器附加 content-box 观察，捕获外框未变但滚动条改变 viewport 的情况。物理表面使用 overflow:clip：单行超过表面上限或 overscan 溢出不能撑大根 scrollHeight，也不新增内部滚动容器。

验证：

- `sparse_heights.rs`：6000 次随机小数高度/重置与密集参考模型逐项比 prefix、total、window，再验证 splice；10 亿行初始 0 槽位、20,000 次远点测量/还原后无历史节点，预留容量有界。
- `list_widget.rs`：1 亿行、199 次远跳，合批测量仅一次提交，前方行高度变化保持 anchor/NodeId；测量缓存、实际高度索引预留容量、UI 节点与处理器有界；过期事件忽略、非法估值/重复 key/错误 locate 为诊断，错误恢复不触发旧行越界 panic。
- 同上：无全量数据向量的索引源，prepend 1000 条保持同一个可见 key/NodeId 和行内偏移；锚点删除选择后继，缩至零不读取旧窗口下标。
- Windows Chromium 155 运行 `adaptive-acceptance.js`：1 亿行，80 次远跳最多挂载 28 行、保留 64 条高度、164 个实际稀疏槽位、614 个预留条目，UI NodeId 槽位最多 105，响应式 arena 槽位最多 183。HashMap 顺序可能影响具体峰值，不将单次数字作为 API 保证；验收使用容量上限断言。
- 同一浏览器运行：前方行增高 30.375px、prepend 1000 条保持可见锚点及节点身份，位置差约 0.031px、验收容差 0.1px；删除锚点落到后继。scale(1.25) 不放大行高索引；2,000,000.375px 单行仍保持物理 scrollHeight 约 100 万像素上限。
- 滚动条改变时 border box 不变、clientHeight 从 598 降到 568，容器观察正确更新 viewport；回收后根无残留节点。
- 原定高/表单 12 组浏览器验收继续通过，图节点峰值从上一阶段的 137 变为 138；其他窗口与节点上限未增加。

最新 `./scripts/check-semantic-ui.ps1 -Web` 通过 77 项运行测试、1 项编译失败测试、fmt、host/WASM 严格 Clippy、例子构建；Android/Linux all-features/all-targets 编译检查也通过。浏览器结果保存于 D 盘构建目录，代码与测试步骤在 `examples/semantic-web`。本节仍不声称 FPS、堆分析、设备 UI 或焦点/键盘策略已完成。

## R4：版本化快照与 DOM 接管（2026-10-22）

已实现 `ssr::{render_with,snapshot,snapshot_with}` 与可选 `web::{hydrate,hydrate_with}`。快照片段以格式版本注释包围，Group 与 Content 文本槽有明确边界，空文本在解析器没有创建 Text 时补建空槽；没有额外可见包装元素。HTML mapper 统一规范化 CR/CRLF/NUL，使字符串序列化与 DOM 创建一致。hydration key 由 mapper 的创建序列产生，不使用服务端 RuntimeId/浏览器物理路径。

调用方将 `Snapshot.html` 原样放入独占挂载元素，并在文档中包含当前 `glory::html::BASE_CSS`；客户端用同一初始数据和组件工厂接管。普通 HTML Snapshot **只含 HTML**，不自动传输 Cage 值、闭包或异步资源。后续已加入可选命名资源的显式序列化/恢复，见末节；它不是自动序列化任意应用状态。

接管分为：在 staged mapper 中运行初始化；只读校验版本、标签/namespace、key/part、文本、属性、子节点与范围；提交现有节点映射；安装当前订阅并处理预输入；测量稳定后启用窗口定位、提交动作。初始化 Create/Patch/Place 不重放到现有 DOM。Version/Mismatch 错误包含诊断并保留原子树，修正后可以原页重试；已接管根拒绝重复接管。提交后的业务启动错误或测量不收敛是 Ui/Unstable 错误，按应用启动失败释放已接管资源，不承诺事务回滚。

Input 的 value 和 Check 的 checked 实际 property 若不同于初始状态，作为正常语义事件交给应用；不会先写回初值。比较时考虑浏览器 input type 的值规范化。已有焦点/选区保留；用户加载前已聚焦时，构造阶段的 Focus 不覆盖它；没有预先聚焦时仍执行初始 Focus。应用的事件处理器之后若主动修订业务值，仍遵循既有程序写入/revision 规则。这里验证的是浏览器属性/事件协议，未验证操作系统 IME，也尚未提供 WASM 前的 composition bootstrap。

虚拟列表由可移植 VirtualPart 元数据标记 surface/row。SSR 在 BASE_CSS 下采用 flow-root 正常流预览，包含初始窗口，避免未知高度按估值绝对定位而重叠；浏览器首先测量现有行并通过 Anchor 事件恢复可见行及行内像素，再稳定窗口并设置 data-gly-live。测量和业务更新期间仍暂缓物理动作，切换后才提交，避免 flow 表面先把 scrollTop 截断。列表内部 padding 固定为 0，外部间距放在包装组件；边框与 client viewport 仍分开处理。

Scope 现在可查询后端 capabilities 和语义 active 状态。SSR 不伪造物理 Scroll；内部列表保留逻辑窗口但不发送不支持的动作。暂停列表也不记录未提交动作的 expected 位置，恢复后可以实际定位。新增回归验证“新测量之前估值 24px，接管后同一行内 150.5px 偏移仍能保留”，避免在更新真实高度前把偏移截成旧估值。

### 接管验收

`examples/semantic-web/src/hydration.rs` 是 server/wasm 共用的纯 Rust 工厂；`ssr_fixture` 原生二进制生成 `/ssr.html`，不是在浏览器中调用序列化函数伪造 SSR。`./scripts/build-semantic-web.ps1 -Serve` 会同时构建 WASM 和生成该页面。`hydrate-acceptance.js` 运行结果（Windows Chromium 155）：

- 服务端 1 亿条源只输出初始 14 行，实际行之间无重叠，预览 scrollHeight 445px；不是百万像素空表面。
- 接管前输入文本、textarea、取消勾选、输入 keyed 行草稿并选中输入框字符 2..6；接管后业务状态与实际输入一致，焦点/选区保持，原静态元素全部复用，静态区域无新增元素，目标 input value setter 调用为 0。
- 预先滚动到约 150px 的同一行 key 4 在切换后仍可见，纵向差约 0.046px（验收容差 0.1px），窗口为 16 行；随后滚动表面约 100 万像素。初始化测量只遍历当前呈现节点，未读取全表。
- 接管后按钮图标激活一次，空文本槽能更新、Group 分支能销毁、Each 重排保留输入节点/草稿，远距离跳转后无重复 key/part，卸载清空根。
- 版本错误、属性差异、文本边界缺失及浏览器修正的非法 p 结构均拒绝接管并保留原 DOM。错误修复后原页重试通过；重复接管拒绝；无预先焦点时初始 Focus 仍执行。

本轮共享浏览器会话的变高长回归曾超时。已在独立 context 中重跑同一 80 次远跳并通过，脚本现在自建/关闭独立 context、置前测试页，并为变高帧等待和全程增加超时；没有把超时算成成功，也没有用定时 sleep 代替帧。原定高/表单、变高、接管三组脚本均已通过；变高峰值仍为 28 行/64 条高度缓存，定高保持 33 行/84 个整页映射元素。每个列表新增一个 Anchor 订阅，固定列表处理器为 3，属于常量开销。

最新验证为 80 项运行测试、1 项 !Send 编译失败测试，host/WASM fmt/严格 Clippy、hydrate 关闭/开启构建，以及 Android/Linux 的 all-features/all-targets 编译。`glory-web/hydrate` 依赖图不包含 glory-ssr、HTTP、窗口或 GPU；服务端快照不依赖 web。完整 R4 仍未完成：资源值快照、异步边界、真正流式 shell、背压、断开取消和 CSP/资源策略集成继续保留。

## R3/R4：Owned 异步资源与显式种子（2026-10-22）

新增 `glory_core::{Resource,ResourceState,ResourceSeed}`，门面和 prelude 重导出。`Owner::resource(source,load)` 以响应式 source 产生的 key 驱动请求；相同 key 不重复请求，动态来源依赖会重绑。请求在 Effect 阶段排入当前 Host，loader 的调用和 future 轮询不阻塞组件构造；loader 中偶然读取的信号不成为 driver 的依赖。没有 Host 或执行器拒绝任务时为 `Unavailable(AccessError)`；业务 `Err(E)` 为 `Failed`，两者不混用。

```rust,ignore
let key = scope.cage(42_u64);
let user = scope.owner().resource(
    move || key.get(),
    |id| async move { fetch_user(id).await },
);
text().content(user.map(|state| match state {
    ResourceState::Ready { value, .. } => value.name.clone(),
    ResourceState::Failed { error, .. } => error.to_string(),
    _ => "Loading…".into(),
})).show_in(scope)?;
```

`Resource` 是本地 Copy handle，随 Owner 失效；每份资源为两个 Cage 和一个 Effect，不缓存访问过的所有 key。状态为 Idle/Pending/Ready/Failed/Unavailable。结果存于 `Rc<T>/Rc<E>`，读取不要求业务结果可 Clone 或可比较；进入新 Pending 时释放旧终值，应用可自行保留它的 Rc。`.refresh()` 合批为一次新请求，`.cancel()` 立即进入 Idle，同 key 保持停止直到 refresh，后续 key 改变仍自动加载。

刷新/取消立即替换请求身份并撤销旧任务的提交权限；key 改变在 driver 执行时取消前一任务。身份采用不复用的 Rc ticket，旧任务不能因整数回绕再次有效。取消宿主 future、替换结果以及销毁任意用户值都在图、值及控制器借用之外执行，允许析构读取图或触发后续刷新。内部 OwnerHandle 仅保存代际身份，不延长 Owner/Runtime 生命周期。网络等外部副作用的终止仍由 loader 的取消 guard/实际适配器负责；本次验证了 future 的释放，没有把释放 Future 等同于远端 HTTP 请求已经停止。

`Owner::resource_seeded(..., Option<ResourceSeed { key, result }>)` 接受调用方明确解码的终值。匹配初始 key 时直接 Ready/Failed，不启动重复加载；key 不匹配或显式刷新则正常请求，首次 flush 之前 refresh 也不会被种子抑制。该 API 仍不是资源 wire payload、请求级注册表或自动 SSR 恢复；HTML Snapshot 不会偷偷序列化任意 Cage/Rc/闭包。

验证范围：

- `core/tests/resources.rs` 八项回归：延迟完成/参数切换/旧结果、在 loader 轮询内部刷新、取消与合批、非 Clone 终值的匹配种子、首刷之前 refresh、业务失败与 Host 拒绝及重试、动态 key 依赖、可重入析构、1000 次 Owner 交替取消已轮询/未轮询任务。最后一项始终至多 3 个图槽和 1 个任务槽。
- `examples/semantic-web/resource-acceptance.js` 在 Windows Chromium 155 运行公开 API 与真实 Web async/RAF 宿主。匹配种子没有请求；切换 key 后旧 key 已无可完成 job；新结果唤醒 RAF 并更新标签，无手工 flush；业务失败、refresh 与 cancel 均通过。
- 同一页面的 1 亿行源，为每个挂载行创建真正等待中的 future；80 次远跳最多 15 行、15 个请求、136 个图槽和 34 个语义节点。总共启动 1137 个 future，其中 2 个完成、1135 个取消；卸载后 job/任务/图/DOM 全为 0，无浏览器错误。测试 I/O 可控，不包含真实 HTTP。

本轮活动 workspace 通过 88 项运行测试、1 项 !Send 编译失败测试；fmt、host/WASM 严格 Clippy 与独立浏览器例子构建通过，Android/Linux 的 all-features/all-targets 编译检查通过。原定高、变高、接管脚本已在同一新构建上复跑通过；四组浏览器原始结果保存于 `D:/BuildCache/glory-semantic-ui/resource-acceptance-result-2026-10-22.json`。资源跨请求隔离与序列化、真正 streaming 的异步边界/背压/断开取消、列表焦点/键盘及 Blitz/Android 设备仍未交付，整体目标保持进行中。

## R4：请求级命名资源快照（2026-10-22）

新增可选 `glory-resource`，由门面 `resources` feature 连接，独立于 HTML、HTTP 和浏览器。core 中的 Resource 继续不依赖 serde；HTML mapper 继续只依赖 render/style。`cargo tree` 已验证默认门面、core 与 HTML mapper 不引入 serde/resource。资源上下文每个请求/App 单独创建，clone 只共享该上下文；第一次绑定后固定 Runtime，意外跨 Runtime 使用为 WrongRuntime，避免把请求缓存当全局单例。

`Resources::bind(owner, name, schema, source, load)` 在原有 Resource 上增加显式注册。业务 key 与结果需支持 serde，T/E 不需要 Clone。name 是稳定的应用资源名（例如 `user/profile` 或 `row/{key}`），schema 是应用维护的格式版本（例如 `profile/1`）；它们都不依赖 NodeId、RuntimeId 或 DOM 路径。重复 live 名称、格式不匹配、JSON 类型错误均有独立诊断。多个消费者共享同一个 Copy Resource，而不是重复 bind 同名资源。

`Snapshot::seed(name,schema,&key,Result<&T,&E>)` 显式编码预取请求数据，`Resources::restore(snapshot)` 建立当前请求上下文；组件 bind 时解码种子并交给 core 的 key 匹配规则。未使用种子可在完整接管成功后 `discard_unused()`；读取浏览器 payload 不会删除它。一次失败的接管会销毁临时 Runtime 和注册项，重试从原 payload 创建新 Resources，不复用已消费的上下文。

`Resources::snapshot()` 只导出当前仍挂载且终态 Ready/Failed 的命名资源，按名称确定顺序；Idle/Pending/Unavailable 不伪装成可恢复终值，未注册资源/普通 Cage/闭包不自动导出。Owner 清理移除注册项，框架不保留所有滚动访问过的资源名。用户 Serialize/Deserialize 在图、响应值和注册表借用之外运行。导出应在 App flush 后完成，序列化回调应保持纯粹；它不是跨线程的事务快照。

wire envelope 独立版本为 1：version、records，每条包含 name/schema/key/result；result 区分 ready/failed 的值。解析拒绝未知字段、重复名称与未知版本。`Limits` 默认 8MiB/4096 记录，同时限制还活跃的命名注册项数；调用方可显式调整。字节限制是完整 JSON/数据块输入输出大小限制，不是任意业务 Serialize 内部临时分配的硬上限。

SSR 的 `snapshot_resources_with(&resources,factory)` 在 App 存活时取得 HTML 与资源终值，再释放请求 Owner；避免 App 已 drop 后导出空注册表。它是同步接口，调用方先提供已预取种子，不阻塞等 future。返回 `.view` 和 `.data`；`.view.html` 放入独占 mount，`resource_script(id,&data,nonce)` 放在 mount 外面。后者产生 inert application/json 数据块，可带 nonce；共用 HTML 工具对 `< > & U+2028 U+2029` 做 JSON Unicode 转义，而不是在 script raw text 中使用 HTML entity。额外转义膨胀后的数据块也受字节限制。Web 的 `read_resources(element,limits)` 验证标签/type/版本并读取 payload，不改 DOM、不消费数据块。

实际证据：

- `resource/tests/snapshots.rs` 七项：非 Clone 终值往返、同名两个请求互不泄漏、跨 Runtime 误用、schema/类型失败无图分配且保留种子、业务错误恢复、Idle/宿主错误不导出、2000 次变换行名的 Owner 释放、格式/大小/数量限制及确定顺序。
- `ssr/tests/resources.rs` 三项：两个真实 server render 分别导出自身资源且渲染结束清理上下文；混合大小写 `</script>`、注释、分隔符/NUL/Unicode 原值往返；转义后的 HTML payload 仍检查上限。
- Windows Chromium 155 执行 `resource-snapshot-acceptance.js`：原生 `ssr_fixture` 生成 `/resource-ssr.html`，使用同一 Rust 组件工厂与外置 module bootstrap，在 CSP 限制下初始化 WASM。成功值与业务失败均恢复，首次 loader 次数为 0、任务为 0，原元素身份全部保留；刷新一次才实际加载，key 变化加载新值。
- payload 包含 `</script><script>window.payloadInjected=1</script>` 等字符串，页面只产生预期的两个 script 元素，注入标记未执行，无 CSP/应用 console error。版本、block 属性、重复名称、超 8MiB、schema、类型、DOM 结构七种错误均保留原页面，修复后原页重试、无重复首次加载。
- 既有资源滚动脚本新增真实命名注册：1 亿行、80 次远跳，live 注册数始终等于当前挂载行数（最多 15），卸载后为 0。请求/节点/图槽上限仍为 15/34/136，原有 1137 个 future 的生命周期结果不变。

本轮 `check-semantic-ui.ps1 -Web` 通过 98 项运行测试、1 项 !Send 编译失败测试、fmt 和 host/WASM 严格 Clippy；Android/Linux all-features/all-targets 编译通过。定高、变高、普通接管、行资源、资源快照五组真实浏览器脚本均通过。证据保存于 D 盘 `resource-snapshot-acceptance-result-2026-10-22.json`。以上没有完成真正 streaming：异步边界、shell 先到、背压、请求断开取消及 patch/CSP 集成仍待实施；列表焦点/键盘、Blitz、Android 设备、R0/R7 和旧架构清理也仍在完整交付范围内。

## R4：拉取式服务端帧与 HTTP 取消（2026-10-22）

新增可选门面 `stream` → ssr/resources，`glory-ssr/stream` 在 native 目标公开 `stream_with(resources,config,|scope,resources| widget)`。构造阶段同步完成初始组件与 pending shell，Host 只入队 future，第一次 `next_frame()` 返回 shell 时 loader 还没有轮询。body 同时实现 `futures_core::Stream<Item=Result<Frame,Error>>`；返回 Shell/Patch/End，非收集所有结果后的 Vec iterator。浏览器 WASM 不公开这个 native 执行器；web/ssr/stream feature 联合编译仍通过。

输出复用同一个 HTML mapper，后续提交降为有限 create/patch/place/suspend/dispose edits，以 HTML key 定位，不传 RuntimeId/NodeId，也不重新序列化/parse 整棵树。现有 `when(resource.map(...),...)` 可作为渐进内容分支，fallback 被 owned 分支替换；普通 prop 绑定也可直接产生增量。没有引入 UI DSL、源码流解析器或第二份业务状态机。

请求自带 local Executor：所有业务 future、图和 UI 留在请求线程，只有 task id/ready 标记/Waker 通过 Arc/Mutex 接收外部 I/O 唤醒。ready 队列去重并按入队顺序公平轮询，已取消 id 从 live/ready 移除，旧 waker 不能重新激活任务。`Host::wake` 只标脏并唤醒消费者，不同步执行 App。用户 future 与取消析构不在任务表/图借用或队列锁内运行。

背压直接来自拉取：不调用 poll/next 就不执行后续 loader/flush；每次产生一批已提交 edits 即返回，先让消费者处理/写出，再轮询下一生产者。没有后台无限 HTML 队列。Config 默认 8MiB/frame、4096 任务、每 tick 64 次轮询；字节超限、渲染/资源导出错误会终止 body 并清理请求，单次忙循环达到预算会让出。任务容量拒绝通过既有 HostError/Resource::Unavailable 契约报告。限制针对框架帧/任务元数据，不声称限制任意用户 future 自身分配。

`Stream::cancel`/Drop 先释放 App/Owner，撤销任务提交和宿主 future，再关闭 executor、清空 edits/key map 与未使用种子。完成时先在 App 仍存活时导出终值，再生成 End，随后同样释放请求；即使 body 对象或 Resources clone 继续被调用方保留，也没有活跃节点/任务/注册项。HTTP adapter 必须把断开/超时传播为 body drop/cancel，不能只停止写 socket 而继续保留 body。

### 最终 key 与协议

HTML snapshot 格式现为 **v2**。普通 SSR 快照导出与 Web staged build 在校验前都按最终可见树 preorder 规范化 HTML key；暂停/未放置节点取得尾部唯一 key，不让隐藏历史改变可见节点的 key。回归验证创建顺序 a→b 与 b→a、额外隐藏节点的相同可见树输出完全一致。未保留 v1 兼容分支；resource wire 版本仍独立为 1，BASE_CSS 样式标识也独立。

流中 shell 保留创建顺序 key，使用 `gly:stream:1` / `gly:/stream:1` 边界，故未结束前不能被普通 hydrate 意外接管。Patch envelope 为 version/kind/sequence/ops。End 包含最终可见/隐藏 key 的 old→new rekey、资源 envelope 与 html_version；后续 DOM consumer 必须原位更新属性/注释 metadata，并把 stream 边界转换为最终 HTML snapshot 边界后才接管。当前帧协议和服务端已提供这些数据，浏览器执行这一步尚未交付；不以 key 规范化单测代替真实流接管验收。

### 证据与限制

- `ssr/src/stream/tests.rs` 六项：shell 先于 loader 轮询；延迟/逆序完成只输出当前增量；需求暂停不继续执行；cancel/Drop 与过期提交；外部线程 waker；预算耗尽不丢 dirty commit；自唤醒任务不饿死其他任务；字节错误终止并清理；1 亿行 shell 只创建当前窗口的请求/节点。另一个 canonical snapshot 回归验证完成顺序与隐藏历史。
- `examples/semantic-web/src/bin/stream_fixture.rs` 是传输验收服务，监听 loopback:8789。每条连接显式驱动本地请求，socket 写入期间不继续 poll，独立的 socket 读监控在等待 future 时发现断开并唤醒驱动器；取消 timer future 也终止其等待。它按 NDJSON 传输帧，**不是生产 HTTP server，也不是渐进 HTML 文档/bootstrap 的交付**。
- `python scripts/check-semantic-stream.py` 通过：shell 4.385ms、首个 patch 107.204ms、第二个 patch 510.978ms、End 511.228ms（本机单次观测）。消费者确实在资源完成前读到 shell；End 含两个资源终值。之后五次在等待资源时关闭真实 HTTP 连接，completed/cancelled 为 1/5，active 与 timer 均为 0。延迟是测试指定的 100ms/500ms，数字不能当渲染吞吐/FPS。原始结果在 `D:/BuildCache/glory-semantic-ui/pull-stream-http-result-2026-10-22.json`。
- 105 项运行测试、1 项 !Send 编译失败测试、host/WASM 严格 Clippy、独立例子格式/检查均通过。HTML v2 后，普通接管、资源接管、定高、变高、行请求五组 Chromium 回归通过，列表窗口/请求上限不变；Android/Linux all-features/all-targets 编译通过。stream feature 依赖图没有自动引入 web、HTTP、窗口或 GPU。

R4 继续保持未完成：需要浏览器增量帧应用与原位范围移动、CSP 下的 bootstrap/template 传输、等待期间的预输入/焦点保护、End rekey 后真正 hydration、真实渐进页面与慢消费者网络验收。现有普通快照 CSP/接管证据不能代替这些流式场景。整个目标的列表焦点/键盘、Blitz/Android、R0/R7 与旧代码/工具清理也仍保留。

## R4：渐进 HTML、CSP bootstrap 与最终接管（2026-10-22）

本阶段补齐上节保留的框架首发契约，R4 已勾选。以下证据适用于 Windows Chromium 155、loopback HTTP/1 与参考执行器，不外推到其他浏览器、操作系统输入法、生产 HTTP 框架或原生设备。整体目标仍有 R0/R3/R5/R6/R7、列表焦点/键盘及旧代码清理待完成。

### 交付接口与分层

`glory-web/stream` 提供静态 `STREAM_SCRIPT`，门面 `stream` 只向已经选择的 web 转发该 feature；纯 native stream 没有因此引入 DOM/HTTP/窗口依赖。HTTP host 把这个 asset 放到自己的 script URL，先输出 `Bootstrap::head_script()`，再按需求写每个 `Frame::to_html(&bootstrap)`。nonce 与 CSP header 由 HTTP host 配置；Document 另支持 BASE_CSS nonce、stylesheet URL 去重（保留首次顺序），有转义/去重回归。

HTML 帧使用完整 inert application/json 数据块与紧随的 parser-blocking 外部 classic script，保证数据块完整解析后才执行。bootstrap 没有 eval、Function 构造器或内联事件处理代码，也不解析 Rust/UI 源码。接收器只实现共用 mapper 的有限 HTML edits，业务仍由相同的 Rust 工厂/资源驱动；WASM 接管后直接走原有 DOM/RAF 路径，流解释器退出。

Shell 增加当前语义节点 manifest：key、shape、逻辑 parent/children、placed/suspended。可见部分复用浏览器已经解析的节点，不重放初始 Place/value setter；只在 detached fragments 建立缺失的隐藏/未放置节点，供后续 Resume 使用。元数据成本依赖当前挂载树，不包含亿级列表的全量数据。Patch 使用 DOM API 更新独立文本槽、属性/property 与范围，不用 innerHTML 重建场景。移动优先 moveBefore，detached/不支持路径用 insertBefore 并恢复焦点/选区。

wire key 与 End rekey 两端均为十进制 **字符串**，避免 JS Number 丢失 u64 身份；浏览器验收包含 `9007199254740993`。序号仍为安全整数，HTML helper 明确限制其范围。内置 HTML 传输 profile 上限为 8MiB/frame（含 JSON raw-text 转义膨胀）、100000 个接收节点与 512 层初始树；服务端 Config 仍控制原生 frame/任务/tick 上限。HTML 包装直接复用已序列化的可信 JSON，不为包裹数据而反复 parse/rebuild 整棵 manifest/资源值。

### 编辑、结束和回收

头部 bootstrap 在第一份 controller 之前记录输入/勾选与 composition。dirty control 的实际值保持不动，默认 attribute 跟随服务端以便最终校验；正在 composition 的冲突默认值延迟处理。End 若遇到组合输入，保留一个 pending End，待 compositionend 后才原位重标 key、生成资源数据块、转换为 HTML v2 接管边界并发出 `glory:stream-ready`，不能提前 hydrate。

End 之后、WASM 之前仍可能开始新 composition。预加载事件层和 DOM 宿主以 `Symbol.for('glory.preload.composition')` 交接状态；adopt 初始化 Editing 并生成相应语义事件，native InputEvent.isComposing 也可恢复漏掉的 start。回归验证接管后的程序改值在组合结束前继续缓冲，结束后按已有 revision 规则应用；这里验证合成浏览器协议，操作系统 IME 仍在 R3 未验收范围。

`web::hydrate_resources_with(root,&data,limits,factory)` 返回 `(WebApp,Resources)`：错误时保留资源脚本供新上下文重试，成功后删除脚本，避免序列化副本继续占用内存。低层 `read_resources` 保持非破坏读取供调用方自行管理。Controllers 完成、取消、失败或 root 从文档移除时，释放 node map、pending End、composition 集合、observer 与 parser-end listener；共享预编辑监听器为每个文档固定四个，使用 WeakMap，不缓存历史行。数据块与每帧外部 script 执行后移除。

协议错误使接收器 fault/release，保留已呈现的内容，不假装回滚已应用的前缀。缺少 End 的截断响应在 parser EOF 被识别为 failed，保留 shell、释放 controller、不生成可接管数据；不会把普通 EOF 当作成功。

### 逐项验收

| R4 要求 | 当前权威证据 |
|---|---|
| 共用 HTML 映射、转义、初始 property、head 去重/nonce | `ssr/tests/snapshot.rs`、`ssr/tests/resources.rs`，现有普通 DOM/SSR 六组回归 |
| 请求隔离、显式资源快照、稳定 key | `resource/tests/snapshots.rs`、资源快照浏览器回归；最终 key canonicalization 单测与实际逆序流接管 |
| 接管复用、预输入、焦点与选区 | 普通/资源/stream 三类接管实际节点身份，目标 input setter 为 0，业务草稿与勾选同步 |
| shell 先到、按需求更新 | native stream 八项测试、真实 HTTP 到达顺序、实际 HTML 页面在两个延迟资源之间已显示首个结果 |
| 背压不无界积累 | 暂停 poll 单测；真实慢 TCP reader 下 24 个结果只有 2 个执行，两次观测保持 2 |
| 断开取消与生命周期 | 五次等待中 HTTP 断开及慢读断开；HTML 页面中取消/关闭后 active/timer 均 0；root 移除与失败释放 controller |
| CSP 与 HTML progressive bootstrap | `/page` 与 `/reverse` 使用 HTTP CSP，没有 script unsafe-inline/general unsafe-eval；不含可执行注入，console errors 为 0 |
| End 后原位接管 | rekey 不替换节点，正常/逆序完成均 0 次首次请求；初始隐藏节点恢复、Group 暂停/恢复与 Each 重排 |

`examples/semantic-web/src/streaming.rs` 是 server/wasm 共用的纯 Rust 工厂，包含受控输入、勾选、Each、初始隐藏范围、暂停/恢复范围与 1 亿行列表。稳定的逻辑 Group 隔离各组件的放置范围，不依赖多个 Effect 之间的执行先后。

`stream-acceptance.js` 本轮 Windows Chromium 155 结果：

- shell 时两个异步分支均未完成，第一帧约 1.52s 显示 first，第二帧约 3.02s 显示 second；这些延迟由 fixture 指定。实际中间状态确实可见，不是等所有数据完成后再一次插入。
- Each 从 0/1/2 重排到 2/0/1，输入对象/草稿/焦点及字符 1..4 保留；正常场景强制禁用 moveBefore，验证新解释器的回退。中途暂停的 group 原输入/草稿恢复，初始没有 DOM 的 hidden group 能建立并 Resume。
- 服务端把主输入默认值更新为 server new，用户实际 draft 不被覆盖；勾选也保留。End 和 hydration 之后原 editor/row 对象不变，目标 input value setter 调用为 0，业务状态取得真实草稿/勾选，初始 loader 调用为 0；资源脚本成功删除。
- 列表接管前后锚点同为 row 1、top -12.2855px；后续 20 次远跳最多 15 行/55 个语义节点，所有 key/part 唯一，静态表单状态仍在，流 controller 为 0。不是全表索引/克隆或浏览器热路径 JSON 消息。
- 明确暂停首个 controller script，验证 composition 在 controller 数为 0 时已被头部捕获；End 到达仍组合中则不生成接管数据，结束后保留 draft/焦点/字符 3..7。End 后新开始的 composition 也能传给正式宿主，程序改值保持缓冲直到结束。
- second→first 逆序完成同样能按最终 key 接管；同一文档两个 root 使用相同 key 空间仍互相隔离。版本/序号/缺失节点/非法 tag/inline handler 属性被拒绝；超过 Number 安全范围的 key 精确，移除 root 自动释放。
- `/broken` 在 shell 后终止，收到明确的 response ended before End，状态 failed、controller 0、数据块不存在、shell 保留。实际取消 HTML 页后服务器 active/timer 均 0。

`scripts/check-semantic-stream.py` 本轮真实 TCP 结果：shell 2.704ms、patch 107.979ms/510.631ms、End 510.827ms；随后五次等待期间断开，再在小接收缓冲的连接上停止读取。24 个 256KiB 结果只执行 2 个，下一次观测仍为 2，连接关闭后 active/timer 为 0。该测试测传输顺序/背压/回收，不是生产吞吐或 FPS。

107 项运行测试、1 项 !Send 编译失败测试，host/WASM 严格 Clippy 与例子构建通过；六组浏览器脚本均通过，Android/Linux 编译检查通过。原始结果在 D 盘 `progressive-stream-browser-final-2026-10-22.json` 和 `progressive-stream-http-result-2026-10-22.json`。最终源码另通过 Web 的 hydrate-only/resources-only WASM 检查，避免组合构建掩盖可选依赖错误。生产 HTTP 集成、其他浏览器/OS IME/设备、跨引擎样式与性能分解仍按各自阶段继续，不用 R4 的绿色结果替代全目标验收。

## 大列表：有界焦点保留（2026-10-22）

公开 `list` 现在默认保留至多一条离屏焦点行。它与当前窗口共用 Cache、排序与 LIS 协调，不另建访问历史缓存；返回窗口时复用同一实例，没有重复行。失焦或 key 删除后，离屏行的 Owner、派生绑定、DOM 与任务一起释放。`ListStats::range` 仍是普通窗口，`rows` 包含额外行，新增 `pinned` 为 0/1。

### 语义事件与索引

`FocusChanged(bool)` 仍只发送给焦点来源自身；App 派生 `FocusWithin { node, focused }`，向所有订阅的逻辑祖先发送，包括没有 DOM 元素的 Group。Input 的直接处理器不会截断列表观察，内外嵌套观察也不互相抢占。收集回调后释放树借用再调用；早先回调销毁/替换的后续订阅会跳过。Web 根捕获原始焦点事件，业务仍在 RAF/pump 执行。

原队列把同一目标的连续焦点事件跨其他控件替换，会把同帧 A→B→A 的最终状态排错。FocusChanged 现在是保序屏障，ValueChanged 只在不跨这些屏障的片段内合并。

定位焦点行只遍历逻辑祖先和当前缓存。普通控制器运行读取窗口加至多一个焦点 key，不遍历全量源；即使可见 key 输出完全相等，焦点 key 的来源仍有直接订阅。数据编辑后通过 `.locate` 查新下标并校验 key，且在新位置重建依赖，避免 prepend 后继续只监听旧下标。没有 locate 时只检查旧下标和当前窗口，无法定位就释放该行，不作全表搜索。非法定位返回 `InvalidLookup` 并清理旧窗口，避免下一属性阶段访问失效下标。

### 实际验证

新增 5 项运行测试：单 pin 与窗口统一排序/回收及无副作用验证、Group/多祖先焦点通知、回调销毁订阅、1 亿行 149 次远跳时真实 Owner/Task 上限、prepend 后保留身份及只删除离屏焦点 key 的动态依赖。现有测试也继续验证输入/范围/任务与源依赖回收。固定列表只新增一个根观察订阅，基础处理器从 3 个变为 4 个，不随行数添加焦点监听。

新增 `examples/semantic-web/{src/focus.rs,focus-acceptance.js}`，与既有六组浏览器脚本均在同一最终构建上通过。测试数据源用索引公式表达 1 亿行，不分配全表数组；行具有自己的草稿、派生下标和真实 pending future。以下为 Windows Chromium 的实测结果：

| 项目 | 定高 | 变高测量 |
|---|---:|---:|
| 40 次远跳阶段最多挂载行 | 14 | 16 |
| 远跳阶段最多语义 ID 槽位 | 60 | 68 |
| 远跳阶段最多图槽位 | 112 | 126 |
| 每次远跳两帧观察区间的最多源 key 读取 | 15 | 48 |
| 已保留输入框的框架 value 写入次数 | 0 | 0 |
| 全场景创建 / 卸载后释放的行 Owner | 597 / 597 | 682 / 682 |

变高模式会因 Resize/估值修正多次稳定化，48 次是一个观测区间的累计读取，不是单轮窗口大小；每轮仍只读窗口和一个焦点 key。任务数、created−dropped 始终等于挂载行数。后续 prepend/删除后的末尾 ID/图槽位分别为定高 64/119、变高 68/126，没有把早期远跳峰值当作全场景峰值。

每次程序远跳之后另施加真实 scrollTop +1；焦点对象、草稿和字符 2..9 始终保留。合成 composition 在离屏前后使用同一个编辑对象；prepend 后下标更新；回到窗口没有第二份输入；同帧快速焦点切换后仍保留正确行。聚焦外部控件释放旧行；离屏 key 删除后原输入断开且 pinned=0；最后卸载全部行。这里验证组合事件协议，未验证实际 OS 输入法。

最终验证为 112 项运行测试与 1 项 !Send 编译失败用例，fmt、host/WASM 严格 Clippy、浏览器例子构建和 Android/Linux all-features/all-targets 编译通过。七组浏览器完整结果保存于 `D:/BuildCache/glory-semantic-ui/focus-pinning-browser-result-2026-10-22.json`。此阶段没有测量 layout/paint/FPS，也未交付逻辑行键盘导航、ARIA 行位置/总数与真实辅助技术验收；R0/R3/R5/R6/R7 和旧代码清理仍保持未完成。

## 大列表：逻辑导航与无障碍元数据（2026-10-22）

### 契约与实现

新增纯数据 `FocusMode::{Auto,Programmatic,TabStop}` 与 `ItemPosition { index,count }`，分别进入 Descriptor 和 typed Patch。Group 不接受焦点盒子/集合位置；位置需要 ListItem 角色，index 不能越过已知 count，也不能在一基输出时溢出。未知 count 为 None。HTML mapper 将其统一映射成 tabindex、aria-posinset/aria-setsize，SSR 与 Web 读取同一份规则；普通 ListItem 角色自身不再隐式设置 tabindex。可交互祖先内的显式焦点后代也受内容模型验证。

位置是逻辑集合中的下标，不能使用当前 DOM 窗口顺序代替；总数未知时输出 -1。依据：[WAI-ARIA posinset](https://www.w3.org/TR/wai-aria-1.2/#aria-posinset)、[setsize](https://www.w3.org/TR/wai-aria-1.2/#aria-setsize)。Glory 的普通虚拟 list 仍使用 list/listitem，没有引入选择状态或 listbox 角色。

公开 `List::name(...)` 设置可响应的集合名称。列表根为 TabStop，行外壳为 Programmatic。Web 只在实际焦点来源就是 virtual root/row 外壳时，把无修饰的上下箭头、Home/End、PageUp/PageDown 转成 `Navigate { from,command }`；后代 Input、Button、Link、外部 DOM 控件、修饰键和 composition 保留原行为。Tab/Shift+Tab 始终由浏览器处理。Native host 可生成同一逻辑 Navigation，不把浏览器 key code 放进 core。

Navigate 的 origin 必须等于事件目标，沿逻辑祖先找到最近处理器。请求按同一 host tick 的顺序累积，同来源连续按键以本轮逻辑游标继续计算，不丢掉中间移动；本轮只协调最终窗口。上下到前/后一行，Home/End 到首尾；Page 以视口像素距离经高度索引定位，至少前进/后退一行，超高单行也不会卡住。定高定位 O(1)，变高使用既有稀疏索引 O(log n)，不扫描跳转之间的行。

先 seek/reveal 和协调窗口，再通过提交后的 ScrollTo/Focus 作用于目标行。变高目标保留一份有界待测意图，实际 Resize 后再次确保可见；用户滚动、外部 position/data 编辑或后续焦点转移会取消旧意图，迟到测量不抢滚动。删除焦点 key 或无法定位其新位置时，在支持 focus 的宿主把焦点还给集合，空集合仍有稳定入口。

行元数据在现有 Rows::mount/update 协调器中更新，不为每行新增 Bond/Binding。count 改变但窗口 key 相等时，也会刷新存活行的总数。固定列表额外增加一个导航订阅和一个请求 revision；基础根处理器现为 5 个。配置名称再增加一个根绑定，均与数据总量无关。

### 验收证据与边界

新增 7 项运行测试覆盖 typed metadata/重置/越界、SSR 的第 1 亿行位置与未知总数、连续 35 次导航累计及直接首尾跳转、仅 count 变化时的元数据、空集合/单行/缺失能力/删除回退、超高行翻页、失焦后的迟到测量。当前共 119 项运行测试、1 项 !Send 编译失败用例通过；fmt、host/WASM 严格 Clippy、独立例子构建及 Android/Linux all-features/all-targets 编译通过。

新增 `navigation-acceptance.js` 与既有七组浏览器脚本在同一最终构建通过。新场景使用 1 亿行、240px 视口、overscan=2；每行拥有 input、文字、button、link 和一个真实 pending task。定高和变高均通过：

- 真实 Tab 进入根、箭头进入首个可见行；同帧 35 次 ArrowDown 到行 35；PageDown/PageUp 后目标行可见。
- 15 次交替 Home/End 到 0/99,999,999，最后一行输出 aria-posinset=100000000、aria-setsize=100000000；原生 :focus-visible 与 outline=auto 保持。
- 该首尾远跳循环中，定高/变高最多 11/12 行、88/94 个语义 ID 槽位；每次两帧观测区间最多 15/40 次 key 读取。变高包含测量稳定化的多轮读取，不把它说成单次索引调用。后续场景末尾为 13/14 行；槽位末尾均为 94。
- Tab 进入行编辑，Shift+Tab 返回根并能离开集合；Input/Button/Link 的导航键没有被阻止。真实输入键 Home/End/ArrowLeft 后光标分别在 0/6/5，保留原生编辑行为；修饰键和 composition 不触发列表导航。
- 离屏焦点 key 删除后焦点返回集合；Home 可继续导航，count 为 99,999,999；prepend 后保持行身份并更新逻辑下标/总数。全部场景创建/释放行 Owner 为定高 236/236、变高 256/256，任务数始终等于挂载行数。
- Chromium Accessibility.getFullAXTree 实际返回名称 Virtual entries 的 list、11/12 个非 ignored listitem，以及 focused/focusable/level 状态。该协议输出未提供位置/总数属性，当前对它们的证据止于 DOM/SSR，不推断屏幕阅读器的朗读行为。

最终八组浏览器结果保存于 `D:/BuildCache/glory-semantic-ui/navigation-browser-final-2026-10-22.json`。以上没有完成真实屏幕阅读器/OS IME/其他浏览器或原生设备验收，也不提供 layout/paint/FPS 结论。完整交付清单仍保留平台辅助技术、R0/R3/R5/R6/R7 和旧代码清理，目标继续进行。

## 类型化样式支持与异常窗口预算（2026-10-22）

### 支持范围和更新前校验

新增 style::Profile/Support/Property/Value/Css。25 个属性由 Style 无 `..` 的穷尽解构列出，新增字段会要求更新 profile；29 个有限值/单位可分别查询、开关和诊断，隐含 px 的 gap、padding、margin、font_size、line_height 也服从单位限制。ALL 路径 O(1)。数值合法性仍由 Style::validate 负责，不能以支持声明绕过 NaN/负尺寸校验。

Render::style_profile 默认 NONE，Memory 为 RECORD，Web/SSR/stream 为 HTML；后两者描述当前类型化声明的共同 HTML lowering，参考解释器只记录声明而不做布局。Session 创建时读取一次，App::style_profile 和 Scope::style_profile 查询同一快照，不在每次 Patch 时重入后端。创建前和 StyleSet 前验证，失败不改变该目标的语义模型或待提交命令；保持上次提交样式，之后合法的响应式更新可恢复。Clear/Keep 及非样式 Patch 不额外扫描。直接使用 Render 的调用方仍负责该低层入口的契约。

Css::None/Html/Toolkit 仅描述样式表家族，class token 可以是元数据；这没有实现 Portable CSS 文本语法、选择器、级联、字体和资源策略，也没有声称跨引擎像素相等。R0 仍需完整内容/能力/错误审计；按报告 v4，未提供的 Portable stylesheet 不构成 HTML 首发的额外准入条件。

审计同时修正 Style.background 的 HTML 映射：它的值为 Color，应输出 background-color，而不是会重置背景图等设置的 background 简写。SSR 回归检查十六进制输出，浏览器同时保留应用 linear-gradient 背景图和 rgb(1, 2, 3) 颜色。

### 行预算与失败恢复

List::row_limit 默认 4096，覆盖窗口和 overscan；至多一条焦点 pin 可额外保留。0 是 InvalidSize。初始按实际 position 计算未分配窗口，在读取 source key、行工厂、Root/Cage 创建前检查；靠近尾部的小窗口仍可合法通过较小预算。极小行高、极大视口/overscan 不再隐式导致大批节点创建。

动态控制器在测量清理前和导航修正后检查。超限可能先校验旧的待处理测量和一个焦点 key，但不会读取或创建请求扩大的窗口。RowLimit/Allocation 在释放测量借用后清空行、key/start/range 和待导航目标，销毁 Owner 并取消任务；必要时把焦点还给支持 Focus 的根。之后合法视口/数据修正可恢复。Cache 对元素数溢出及不可能的 Vec/HashSet 容量通过 checked_add/try_reserve 返回 Allocation，在源读取和 Host 变更前失败；不声称能恢复所有分配失败或限制任意用户工厂的内部资源。

| 错误 | 保证与边界 |
|---|---|
| NodeGone/ForeignRuntime | Patch 先验证目标身份，再检查样式；旧身份不会因样式错误被掩盖 |
| InvalidProps | 无效数值在目标模型/命令变更前失败 |
| UnsupportedStyle | 返回 profile/version/property/可选 value，目标保持原值，无静默降级 |
| RowLimit | 不创建超限请求窗口；动态旧行/任务释放，合法输入可恢复 |
| Allocation | Cache 不可能容量在读取 key/修改行前拒绝；不等于全局 OOM 防护 |

以上是单次目标操作和窗口协调的保证，不能外推成整个应用已执行批次前缀的全局回滚。普通 Each 仍完整呈现源；测量缓存、行预算与用户行子树成本也不是同一个限制。

### 实际证据

新增 9 项运行测试：Cache 不可能容量；初始超大窗口不访问源或创建组件、0 预算、尾部实际位置；动态释放/取消/恢复；属性集合穷尽；有限值/单位；隐含 px；不支持创建；无效更新保留已提交值/可恢复及身份错误优先级；背景颜色映射。最终为 **128 项运行测试、1 项 !Send 编译失败用例**；fmt、host/WASM 严格 Clippy、独立 Web 构建通过，Android/Linux all-features/all-targets 编译通过。

新增第九组 budget-acceptance.js，与既有八组在同一最终构建通过。1 亿条索引源、240px 视口，编辑行 1 聚焦后把根实际高度设为 100 万 px，再恢复：

| 指标 | 定高 | 变高 |
|---|---|---|
| 正常行/任务 | 11/11 | 12/12 |
| 超限 requested/limit | 35717/4096 | 35718/4096 |
| 超限后实际 DOM 行/任务/语义节点 | 0/0/4 | 0/0/4 |
| 超限新增行工厂 | 0 | 0 |
| source key 累计增量 | 1 | 13（原有测量及焦点校验） |
| 最大查询下标：前/失败后 | 10/10 | 11/11 |
| 恢复行/任务 | 11/11 | 12/12 |
| 全部场景创建/释放 | 33/33 | 36/36 |

错误时 ListStats::inspect 仍保存上次成功窗口的 rows=11/12；表格的 0 是实际 DOM 和 runtime tasks，不把缓存指标误当存活行。RowLimit 在范围重置/后续帧可诊断两次，脚本明确归为预期错误；其他错误为空。恢复后 End 继续到逻辑第 1 亿行，原始结果保存于 `D:/BuildCache/glory-semantic-ui/contract-profile-budget-browser-2026-10-22-r2.json`。

这证明异常输入下资源数量有界及恢复正确，未测 FPS/layout/paint。完整目标仍保留 R0 Portable stylesheet/能力审计、R3 外部资源/OS IME/平台辅助技术、R5 Blitz、R6 Android、R7 性能与字段依赖，以及旧源码/工具清理。

## R0：查询屏障响应校验（2026-10-22）

审计发现 App::request 已强制 flush，但 pump 过去忽略 Input::Response.applied，且没有检查 QueryValue 是否与原查询匹配。现已保存每条待响应查询的 Query 与接受水位；Session 同时记录实际 Renderer receipt，不使用内部参考树的计数替代它。

成功的非空 apply 必须严格递增 Receipt::applied，空 apply 保持。后端返回未递增/倒退水位时，App 返回 InvalidReceipt 并进入 Faulted：命令已可能执行，不能假装能够安全重试或回滚。发起查询先提交，延迟响应则必须精确匹配当时的接受水位；之后的新提交不能让旧响应误报，也不能允许适配器读取更新后的树再冒充原屏障结果。

水位不符通过 QueryWatermark { expected,applied } 完成该 waiter。成功结果还必须匹配 Query 种类，Bounds 的坐标/尺寸需有限且尺寸非负，Scroll 偏移需有限，Selection 要求 start≤end，否则为 InvalidResponse。Scroll 允许负偏移以兼容原生 RTL/弹性滚动观测；程序 ScrollTo 继续使用非负逻辑坐标的既有契约。Selection 明确定为 UTF-16 code unit 偏移，所有原生适配器负责从自身 byte/scalar/grapheme 计数转换；不在接收时用后续新文本的长度校验旧屏障快照。

取出 waiter、删除反向 token 索引并释放内部借用后才调用业务回调。无效结果也只完成一次，后续合法查询/响应式更新可以继续；重复/未知 token 直接忽略。销毁目标先以 NodeGone 完成，迟到响应不会送给复用同一 slot 的新 generation。这里的校验不声称能从恶意或错误引擎中证明像素内容，只验证端口可检查的不变量。

新增 4 项回归覆盖旧/未来/精确水位、后续提交之后的正确旧响应、重复 token、错误结果类型、NaN/Infinity/负尺寸/反向选区、合法负 scroll、回调写 Cage 后稳定化、错误 receipt 的终止、销毁与代际复用。最新 **132 项运行测试、1 项 !Send 编译失败用例**，fmt、host/WASM 严格 Clippy、Web 例子构建及 Android/Linux all-features/all-targets 编译通过。

九组真实浏览器验收在同一最终构建通过。基础 acceptance 新增 `a😀z` 的 UTF-16 选区 1..3：请求后、响应投递前提交新值 after，仍返回原屏障的 1..3，新输入保持 after，无 InvalidReceipt/QueryWatermark。列表、焦点、预算、资源、SSR 接管和渐进流均通过；结果为 `D:/BuildCache/glory-semantic-ui/query-barrier-browser-2026-10-22.json`。本轮基础滚动场景 graph 槽峰值 140（新增测试编辑会产生新的历史峰值），仍在原有界断言内；不把不同测试路径的单次峰值当性能退化。

R0 仍需 Portable stylesheet 校验与完整内容/能力/错误矩阵审计；R3/R5/R6/R7 与旧代码清理保持未完成。上述屏障验证不能替代显示完成、OS IME、辅助技术或原生设备验收。

## R5：真实 Blitz 无窗口消费者（2026-10-22）

### 交付与依赖

新增活动包 glory-blitz，固定 blitz-dom/blitz-traits 为 0.3.0-beta.4。当前选择的依赖图使用 Stylo 0.22、Taffy 0.15、Parley/Fontique 0.12；没有窗口、绘制或 Dioxus 依赖，也没有把这些类型泄露进 core/ui/render/style。源码和发布元数据在 D 盘检查；上游模块职责可参照 [Blitz 架构](https://github.com/DioxusLabs/blitz#architecture)。门面原生 `blitz` feature 单独启用；实查默认 facade 不含引擎/serde，HTML+SSR 不含 Blitz/Stylo/Taffy/Parley/WGPU。

Render→共享 HtmlMapper→Dom Sink→Blitz 程序化文档变更。更新不输出整棵 HTML 再 parse，也不为 Blitz 另设布局树。Renderer 的 clone 指向同一文档；set_css 替换一个 author style 文本，set_viewport 输入物理尺寸/DPI，inspect 只读引擎，stats 提供 live ranges/engine nodes/interests/layouts/property writes。原生例子 `cargo run -p glory --example semantic_blitz --features blitz` 实际输出 3 个语义节点与 320×38 logical-pixel panel，更新复用原组件。

Linux 系统字体采用 Fontique 的 fontconfig-dlopen：第一次从 Windows cross-check 遇到 pkg-config 目标库探测失败，已通过明确的 Linux/FreeBSD 依赖 feature 改成运行时加载 Fontconfig；没有伪造目标 pkg-config 或关掉系统字体换取通过。该目标运行时仍须具备 Fontconfig/字体，当前只做了编译检查，不据此声称 Linux 字体或窗口已验收。

### 物理范围与控件初始状态

普通节点可映射多个 Main/Control/Content part；Group 使用注释首尾和一个离线 park，不引入布局盒子。Place 整体移动物理范围，Suspend 保留其对象，Dispose 同时回收逻辑后代里的另行暂停范围和 park；Blitz 自身的代际 NodeId 加上 Glory 的 NodeId 映射避免复用旧身份。

受控 value/checked/disabled 通过引擎 mutator 进入表单状态。离线构造的后代首次入文档时应用 pending 属性，后续普通重排不重放受控值；离线 Patch 在恢复后保持新值。textarea 的编辑器在 layout 阶段才生成，且初值读取子文本；适配层为其维护一个私有文本种子，使首次布局前的初值/变更和后续编辑器值都正确。它不是语义子 Widget，也不清空标签或业务后代。

### 几何、观察与明确未支持项

measure/scroll 为 true，focus/input 为 false。Query 在真实布局后读取结果并保留接受水位，未变化文档重复 Bounds 复用上次 layout。Resize 使用未变换的 border-box 和扣除 border/scrollbar 的 client viewport；Scroll 使用实际引擎偏移。只遍历活跃观察兴趣，事件排队后由 App 处理，不在 mutator/文档借用中重入业务。Bounds 对 Group 无物理盒子的情况返回 Unsupported，Selection/Focus 也明确不支持。

beta.4 的 get_client_bounding_rect 路径扣除了被查询节点自己的 scroll，导致滚动根的 y 变为 -999760 而其内容在 216；这是加入尾行实际可见性断言后发现的上游边界。现仅补回该节点 own scroll，保留祖先/viewport 滚动影响，回归另验证容器 border-box 不动、长内容移动 90px。任意 transform/writing mode、像素与其他引擎一致性仍需独立验收。

没有安装网络/文件资源 provider，没有 paint/GPU/shell/主循环/OS IME/原生焦点/AccessKit 桥。image/外部字体/CSS 资源载入、失效与取消仍需后续实现。可注册业务激活处理器不代表已有原生输入驱动。无窗口模式显式 App::pump；未来 shell 仍要接主循环 Host，而不是添加同步 block_on。

### 实际验证

新增 9 项 Windows 原生文档测试：标签文字 Patch 与 icon 子节点、真实 flex/CSS 替换与查询屏障、Group 移动/暂停/恢复及离线后代释放、keyed 输入身份/无重复 value 写入、离线构造的多行/checkbox 属性与改值、viewport/DPI/去重 Resize、系统字体换行、容器与内容滚动坐标、一亿行实际引擎布局。

最后一项使用公开 List、240px 视口、overscan=2、25 次远跳。定高为 24px；变高实际为 24/28/32px，估值 24px、测量缓存 64，断言引擎测量确实改变逻辑总高：

| 指标 | 定高 | 变高 |
|---|---|---|
| 挂载行峰值 | 14 | 14 |
| live engine nodes 峰值 | 66 | 66 |
| Glory 语义 ID 槽峰值 | 44 | 47 |
| 响应式图槽峰值 | 54 | 57 |
| 全场景累计 source key 读取 | 365 | 874 |
| 行高元数据 | 不启用 | measured≤64，预留 capacity<1000 |

测试另外检查第 1 亿行文本及实际 border-box 位于滚动根视口；实际引擎 ScrollTo +1 后，逻辑偏移增量为 1，而不是按 24 亿逻辑像素缩放；清空后 ranges/interests/tasks/edges 为 0，引擎 live nodes 回到空文档基线，仅保留应用主动持有的一个 position Cage。以上没有计时绘制或测 FPS，也没有运行 OS 窗口/硬件滚轮。

最终 **141 项运行测试、1 项 !Send 编译失败用例**，fmt、host/WASM 严格 Clippy、Web 例子构建，以及十包 workspace 在 Android aarch64/Linux x86_64 上的 all-features/all-targets 编译通过。编译器为 Windows MSVC rustc 1.98.0；这不是 MSRV 1.94 的单独证明。证据保存在 D:/BuildCache/glory-semantic-ui/{blitz-full-check,blitz-list-final,blitz-android-check,blitz-linux-check}.txt。本轮 Web 解释器未改，完成了重建，浏览器运行证据沿用上一轮九组，未把构建当成再次执行浏览器验收。

R5 保持未完成：paint/shell、原生输入/焦点/编辑/无障碍/资源服务与 Windows/Linux 实际窗口准入仍需推进；R6 Android lifecycle/Surface/IME/设备、R0/R3/R7 和旧架构清理也仍在完整目标内。

本轮测试汇总、适用范围、原始指标和实现文件 SHA-256 另存于 `D:/BuildCache/glory-semantic-ui/blitz-document-evidence-2026-10-22.json`，供后续 shell/paint 验收区分文档与平台证据。

## R5：可选绘制、CPU 光栅与 Linux 实跑（2026-10-22）

### 分层与缓存契约

新增 glory-blitz/paint（blitz-paint =0.3.0-beta.4、AnyRender =0.14.0），向任意 PaintScene 记录一帧；raster 再启用匹配的 anyrender_vello_cpu =0.18.0。门面转发 blitz-paint/blitz-raster，默认不启用；绘制没有反向进入 core/ui/style，也没有创建 GPU、窗口、事件循环或阻塞执行器。版本选择依据本地发布源码及 [AnyRender 兼容矩阵](https://github.com/DioxusLabs/anyrender#version-compatibility)。

Renderer::paint 先重置 scene，解决已提交文档的布局，然后记录完整帧。它不会替应用调用 App::flush；未提交 Cage 改值仍显示上一提交，提交后才更新像素。FrameInfo 区分 applied 与 visual revision，包含物理尺寸、有效 DPI×zoom 和文档身份。几何查询先解决 layout 时也不能消耗视觉失效；回归先 query 再 paint 仍得到新颜色。

同一 RuntimeId 可拥有两个不同消费者，不能只以 RuntimeId/revision/尺寸命中像素缓存。新增 document 身份，回归用同一 Runtime、相同 mutation/visual 水位和尺寸的蓝/红文档验证隔离；Renderer 的 clone 仍共享缓存身份。FrameInfo 仅说明命令记录/对应状态，Raster 成功才说明产生了 CPU 像素，均不代表 OS 显示完成。

Raster::draw 对未变更帧跳过布局、编码和光栅化；无可见差异的提交仍更新 applied 元信息。变更帧重用一块 buffer，重置绘制命令并清除旧像素；文档/尺寸改变时重建 backend/buffer，零尺寸与 clear 释放后端、缓冲及其 glyph/image 缓存。默认 backbuffer 限制 64MiB，可显式配置。固定版本 Vello CPU 内部把尺寸转成 u16，因此在进入后端前检查维度，再检查 width×height×4 和 try_reserve；错误保留上次成功画面。这个预算不覆盖任意引擎/业务/后端内部的全部分配，也不是通用 OOM 恢复。

当前像素格式为预乘 sRGB RGBA8；半透明回归验证 RGB 与 alpha 的关系。raster 原生例子转换成 PNG 需要的直通 alpha，输出初始和末尾两张实际图，使用有限 Rust builder、独立 CSS、受控表单和 1 亿行 measured list。它通过程序状态改值展示两帧，没有把 OS 点击/输入当作已支持能力。

### 双平台实际证据

新增 7 项绘制用例：完整 scene 不累积；文字/背景像素与无残影卸载；未提交/未变化帧；DPI/CSS/geometry query 失效；半透明格式；同 Runtime 文档隔离；u16/字节预算、零尺寸/恢复；固定/变高一亿行像素。多个断言归在同一集成用例内，不以断言数量计测试数。与既有 9 项文档用例均在 Windows MSVC 和 Ubuntu WSL 真实运行。

大列表像素场景使用 160×120px、overscan=2、30 次远跳。固定行高 24px；变高为 24/28/32px、24px 估值及 64 条高度缓存。每帧检查一条完全可见行的实际几何位置及对应背景像素；定高还检查预期逻辑 key 的颜色，不能仅凭节点字符串宣称已经正确绘制。

| 指标 | 定高 | 变高 |
|---|---|---|
| 挂载行峰值 | 9 | 10 |
| live engine nodes 峰值 | 46 | 50 |
| 当前 scene 命令峰值 | 19 | 17 |
| 始终复用的 backbuffer | 76800 字节 | 76800 字节 |
| Source key 累计读取 | 277 | 约 800；确切单次值见证据 JSON/日志 |

变高窗口在非整行位置可能多覆盖一条部分行，因此上限为 ceil(120/24)+1+4=10。命令数也受裁剪、行高和文字分布影响，不能把 17/19 解释成模式之间的通用效率排名。31 帧使用同一 buffer 指针；清空后像素全透明、ranges=0，clear 后 buffer capacity=0。没有随着 1 亿条源或 30 次访问历史保留组件/帧。

Windows 与 Linux 均导出了 720×480 的实际 UI：`blitz-paint.png`/`blitz-paint-end.png`、`blitz-paint-linux.png`/`blitz-paint-linux-end.png`，位于 D:/BuildCache/glory-semantic-ui。已逐图检查，末尾包含 Entry 99999999、更新后的 Count: 3、实际文字、表单和复选框；两平台使用系统字体，排版细节可不同，不声称逐像素一致。

### Linux 存储与复现

本次 Ubuntu WSL 为 rustc 1.98.1、具备 Fontconfig runtime 与系统字体。最初在 D: 的 9p 挂载解包遇到只读文件 mtime 操作失败，后续 Cargo 元数据 I/O 也受影响。最终使用 D:/BuildCache/glory-semantic-ui/linux-build-cache.ext4 的 8GiB 文件承载 ext4 缓存；/dev/loop0 的 backing file 已核对回指该 D: 文件。Cargo home/target/tmp/XDG/Python cache 均位于挂载后 /mnt/d/BuildCache/glory-semantic-ui/linux-fs 内，cargo metadata 验证真实 target。未把编译缓存移进 Ubuntu 的根虚拟磁盘，也未使用 Docker。

新增 scripts/check-semantic-linux.sh：验证 D: mount 或 loop 的 D: backing，检查实际 target，再运行 fmt、全 workspace/all-features 测试、严格 Clippy、document-only/paint-only 可选组合，--paint 导出真实 PNG。它不自动格式化磁盘或改变系统挂载。此处可复现入口为：

```sh
GLORY_BUILD_ROOT=/mnt/d/BuildCache/glory-semantic-ui/linux-fs \
GLORY_ARTIFACT_DIR=/mnt/d/BuildCache/glory-semantic-ui \
sh scripts/check-semantic-linux.sh --paint
```

Windows 的 check-semantic-ui.ps1 -Web 与 Linux 脚本均通过；当前是 **148 项运行测试、1 项 !Send 编译失败用例**。Windows 另验证 default facade、HTML/SSR 不含绘图引擎，paint-only 不含 CPU raster/WGPU；Android/Linux cross-check 继续只代表编译。Web 解释器未修改，本轮有 WASM 检查和重建，未重复宣称执行此前九组浏览器脚本。

本轮原始记录为 blitz-paint-full-check.txt、blitz-paint-tests.txt、blitz-linux-final-check.txt 和平台 cross-check 日志；精确指标/源码与 PNG 哈希见 blitz-paint-evidence-2026-10-22.json，均位于 D 盘任务构建目录。

尚未做 dirty rectangle/逐节点 paint cache；变更帧的 CPU 路径重画完整缓冲，成本还取决于物理视口面积，没有测 FPS 或布局/绘制时间分解。shell、真实 OS 窗口、输入/焦点/IME、无障碍、外部资源与 Android 设备仍未完成，R5/R6/R7 保持未勾选。R0/R3 与旧架构/工具清理也继续保留，整个目标没有缩减到无窗口或 CPU 绘制。

## R5 桌面窗口宿主与共享执行器（2026-10-22）

新增 `glory-blitz/shell`、门面 `blitz-shell` 与纯 Rust 的 `examples/window.rs`。`shell::{WindowApp, Shell, WindowAttributes}` 使用 blitz-shell 0.3.0-beta.4 / winit 0.31.0-beta.3，CPU 窗口由 Vello CPU 0.18 和 softbuffer_window_renderer 0.9 驱动；不依赖离屏 Raster API，不引入 GPU。只在 Windows/Linux/macOS 目标选入桌面依赖；macOS 尚无运行证据，Android 继续单独实现。

`WindowApp` 独立拥有 App、Renderer 和本地执行器。原生回调只排入 activation，App::pump 在引擎借用结束后执行业务。Scroll/Resize 观测与虚拟窗口提交先于 paint。`core::executor::Executor` 从 SSR host 提取供两个真实消费者共用；SSR 的 pull/backpressure 仍由其 stream 层控制，未改为主动推送。

调度与资源边界：

- spawn 不同步 poll；只轮询 ready task，每文档每轮最多 64 次。重复 task/dirty 唤醒整轮合并；同一文档的重复 Poll 延到下一次 native loop，避免自唤醒 future 扩大宿主消息队列。
- 外部 worker 只拿线程安全 waker/ID，UI future 与 Cage 保持本地。取消、关闭在借用外销毁 future，执行器关闭后拒绝重新 spawn，退休 waker 不再通知。
- Surface suspend/零尺寸释放 CPU/表面缓存，保留 App/document；恢复后实际呈现。关闭走宿主 CloseRequested 路径，App/行任务先于执行器清理，renderer/surface 先于 window 释放。
- CPU 先拒绝超 u16 维度及 64MiB RGBA 预算，任意 u32 尺寸不会先乘法溢出。该预算不限制整个引擎和系统窗口的分配。窗口为不透明软件输出，变化帧仍整帧重画。
- `CpuRenderer::frames` 只在绘制闭包实际执行且 softbuffer present 正常返回时增加；获取 buffer 失败的跳过不计帧，也不作为 compositor/vsync 完成凭证。
- Shell::request_size 处理 Some(size) 同步成功但不再发 resize 事件的路径；None 保留异步事件路径。测试同时等待原生 surface 尺寸与扣除 safe area 的文档 viewport，不能把“请求已接受”当作几何已同步。
- Renderer::advance_time 使用单调秒数；shell 独占窗口时钟。查询使用同一时间，不回退到 0。实际 CSS keyframes 回归验证 10→60→110px，拒绝逆时/NaN；静态时间推进不增加视觉 revision。

默认测试新增共享执行器 3 项、WindowApp/CPU guard 4 项及文档动画时钟 1 项。涵盖 10000 次跨线程重复唤醒的合并、连续自唤醒的每轮通知上限、取消/关闭期间重入析构、静态空轮询不重画，以及引擎输入回调不调用业务。

大列表轮询专项注入原生 PointerMove/Wheel 到同一 WindowApp：1 亿条定高/变高列表，先定位 2400 万逻辑像素；1px wheel 产生逻辑 1px，随后 200 次输入在处理器内不读取 key/建行，最终偏移为 +200px，新增 key 读取少于 100，任务仍有界，卸载全部释放。这是引擎输入路径的合成事件，不是物理滚轮设备验收。

另有显式 ignored 的真实窗口测试，已分别在 Windows 与 Ubuntu WSLg 的 X11/Wayland 执行。每次打开两个独立窗口，一亿行定高与 24/28/32px 变高，180px 列表视口、overscan=2、20 次远跳；验证 activation、surface 暂停/恢复、零尺寸恢复、实际 320×240→360×280 尺寸变化、无手工 poll 的 worker 唤醒及关闭回收。单窗口峰值 14 个 Runtime 任务、64 个引擎节点；单模式累计 key 读取约 600，具体单次结果随几何稳定化记录在日志。关闭后所有行 future Drop，业务 Cage 已失效。没有随 1 亿源总量或访问历史积累组件/任务。

复现入口：

```powershell
./scripts/check-semantic-ui.ps1 -Web -Window
cargo run -p glory-blitz --features shell --example window
```

```sh
# 任务环境中 X11 缺失库解包在 D:，通过进程环境加载；未向系统安装。
LD_LIBRARY_PATH=/mnt/d/BuildCache/glory-semantic-ui/linux-fs/runtime/usr/lib/x86_64-linux-gnu \
GLORY_BUILD_ROOT=/mnt/d/BuildCache/glory-semantic-ui/linux-fs \
GLORY_ARTIFACT_DIR=/mnt/d/BuildCache/glory-semantic-ui \
sh scripts/check-semantic-linux.sh --window
```

Linux verifier 分开以 GLORY_WINDOW_BACKEND=x11/wayland 运行同一窗口测试，不能以默认后端通过代替另一后端。Ubuntu noble 的 libxkbcommon-x11-0 1.6.0-1build1 与 libxcb-xkb1 1.15-1ubuntu2 从发行版仓库下载并解包到上述 D: runtime 目录，依赖经 ldd 核对。Windows 与 Linux 的 Cargo home/target/tmp/cache 继续全部在 D:；原 8GiB ext4 构建镜像因窗口调试产物耗尽，核对 backing file 后在线扩容到 16GiB，无 C: target/cache 回退。

依赖隔离已检查：默认门面、HTML/SSR 不含窗口引擎；CPU shell 不含 WGPU/Metal/Vulkan 绘制依赖；Android all-features 实际图不含 desktop winit/blitz-shell/softbuffer。Android 的构建通过不代表已经提供移动窗口入口。WASM 严格检查及 fixture 重建已有证据；本次未修改 Web 解释器，不重复声称跑过旧九组浏览器脚本。

本轮仍未完成原生编辑/value/composition/selection、focus/列表逻辑导航、实际 OS IME/辅助技术、外部资源 provider、Android Activity/Surface/设备、R7 时间分解与字段/style 优化、R0/R3 审计及旧架构清理。focus/input capabilities 保持 false，activation 子集不等于完整输入支持；R5/R6/R7 和完整目标保持未完成。

最终验证：Windows 与 Ubuntu WSL 均通过活动 workspace 全 features 测试（156 项运行测试、1 项 !Send 编译失败用例）及严格 Clippy。默认套件显式忽略需要显示器的窗口用例；Windows 另实跑 1 次、Linux 以 X11/Wayland 分别实跑 2 次，日志 passed 合计分别为 158/159。Windows 另通过 WASM 检查/fixture 重建，Android all-features/all-targets 编译及依赖隔离复核通过。

共享执行器后也重跑实际 HTTP/TCP 传输验收：本次 shell 约 2.8ms 到达，两个资源 patch 约 110/513ms，End 约 514ms；暂停读端后 produced 稳定为 2，断开后 active/timers 均为 0。时间仅为这次 loopback 运行值，不作为生产性能指标。专用 fixture 已在测试后停止。

原始记录位于 D:/BuildCache/glory-semantic-ui：blitz-shell-full-check.txt、blitz-shell-linux-final.txt、blitz-shell-android-check.txt、blitz-shell-deps-*.txt、blitz-shell-stream-acceptance.json；源码/锁文件/验证脚本哈希和确切单次窗口计数见 blitz-shell-evidence-2026-10-22.json。

## R5 焦点、受控编辑与原生组合协议（2026-10-22）

本节更新上一阶段仍未接通的焦点/选区/编辑子项；旧 shell 数字保留为历史场景。R5 整体仍未勾选，真实 OS IME、辅助技术、资源 provider 和 Android 宿主没有因此完成。

WindowApp 现在安装原生焦点与编辑驱动，按实例报告 focus=true/input=true；独立无窗口 Renderer 仍为 false，并拒绝 Selection。原生事件处理器只排入语义事件，不在引擎借用中执行业务。焦点 A→B→A 按顺序发送，Control/Main 的物理映射分开，逻辑 Group 仍可观察 FocusWithin。焦点可位于裁剪范围外以保留编辑行，但 disabled、display:none 与计算后的 visibility:hidden 不可获得焦点；允许 visible 子节点覆盖继承的 hidden。

键盘与生命周期：

- Tab/ShiftTab 只遍历已挂载的物理树，支持无初始焦点、程序焦点、跳过 disabled/隐藏/-1；不查询列表数据源。Ctrl/Alt 等组合不触发 beta.4 的无条件 Tab 默认动作。
- 仅虚拟 root/row 的直接按键产生 Navigate；后代 Input/Button/Link 保留各自按键，修改键不触发行导航。Home/End/Page 沿用共同高度索引及提交后焦点/滚动动作。
- 单行 Enter 排入 Commit，多行 Enter 由文字编辑器处理。Button Enter、Button/Check 的 Space 激活已接通；Space 在释放时生效，重复 keydown 不重复激活，切换焦点取消待激活状态。
- Selection 将 Parley 的 UTF-8 byte 位置转换为 UTF-16 code units，包含 emoji 和 preedit。查询仍是提交屏障，不表示窗口已显示。
- 实际发现并修复暂停后的焦点 pin 未释放：App 原先忽略所有 inactive 事件，现允许 live 但 Suspend 的 Scope 接收终止 FocusChanged(false)/Composition(false)。普通输入仍忽略，Dispose/过期代次不再回调。UI 与 native 回归覆盖此区别。

受控编辑在 `crates/blitz/src/editing.rs`，不复制业务响应式运行时：

1. 共用 mapper 的 Revision 被保留为 requested/accepted 水位。相同 value 回写只确认版本，不触碰原生 editor、光标或 preedit；不同值在组合期间暂存。结束时旧输入携带旧水位，App 拒绝它覆盖更新的程序值。
2. value attribute 的增量更新不再绕过 property 路径，避免在组合期间提前重写 editor；checked 也只有一条受控路径。beta.4 的 checked setter 解析 bool，更新时使用其实际表示，覆盖 false→true→false。
3. 非空 Preedit 只开始一次 Composition，并输出 composing=true 的 ValueChanged。空 Preedit 立即结束/取消并恢复被替换的文字与选区；其后的 Commit 可跨 OS 事件到达。若期间程序替换了基础值，则丢弃旧提交，重复空 Preedit 不能解除该保护。
4. 直接 Commit、Disabled、失焦/隐藏/禁用/暂停分别完成或清理组合状态。DeleteSurrounding 使用公开 driver 删除周围 UTF-8 字节，保留 preedit；无效字符边界不切割 Unicode scalar，不 panic。原生组合中键盘文本不会再走一遍引擎默认插入。
5. 单行 paste/IME 去掉 CR/LF，多行与程序 textarea 值统一 CRLF/CR 为 LF；原生粘贴清理同步转换 anchor/focus，避免合法粘贴导致无效 Input props 或光标跳动。普通多行 LF 不触发额外 setter。
6. 启用上游 complex-scripts 分词数据；中文/Japanese 的模型缺失诊断消失。系统字体、文字整形与 OS IME 候选窗仍是不同层。

新增 8 项原生编辑回归，另有 4 项原生焦点/导航测试。覆盖受控 echo、Enter、多行、CJK/emoji、选区替换取消、周围删除、无效字节偏移、程序写入交错、重复空 Preedit、过期输入、换行粘贴、label/control 复选事件及 Space 激活。它们是实际 Blitz/Parley 引擎上的合成协议，不是操作 OS 输入法的自动化。

大列表使用同一公开 List：定高与 24/28/32px 变高的一亿行源，180px 视口、overscan=2。行编辑现在绑定行 Owner 的 Cage，并将事件回写。组合输入中连续 30 次远跳，原生编辑对象、草稿与选区保持，窗口外最多一个焦点 pin；indexed prepend 保留同一对象，Suspend 清理组合/焦点，blur 或删除后回收。存活行 ≤15、图槽 <256、源 key 累计读取 <2500 的断言通过，没有按数据总量或访问历史保留编辑器。

真实窗口 fixture 也改为受控编辑：在固定/变高的两个窗口中开始 preedit、远跳保留，验证 props 与 editor 同步，再提交并失焦回收；同时检查尾行/首行导航后的实际 Bounds、Tab 与 UTF-16 选区。Windows、WSLg X11/Wayland 分别通过，关闭后所有行任务释放。此次单次计数：

| 宿主 | 单窗口任务峰值 | 引擎节点峰值 | 单模式累计 key 读取峰值 |
|---|---:|---:|---:|
| Windows | 14 | 77 | 676 |
| Ubuntu WSLg X11 | 14 | 77 | 674 |
| Ubuntu WSLg Wayland | 14 | 77 | 672 |

77 与旧阶段的 64 不作性能排名：新 fixture 每行多了编辑控件。以上仍是合成输入 + 实际窗口/CPU present；没有物理键盘/鼠标、OS IME、剪贴板系统集成、屏幕阅读器、移动设备、FPS 或阶段耗时结论。

最终验证：Windows 与 Ubuntu WSL 均通过 169 项运行测试、1 项 !Send 编译失败用例及 workspace all-features/all-targets 严格 Clippy；Linux 另验证无窗口 default/paint 的 Clippy。显式窗口测试 Windows 1 项、Linux X11/Wayland 各 1 项，日志 passed 合计 171/172。Windows WASM 严格检查和 fixture 重建通过；Android all-features/all-targets 编译通过，仍无移动宿主入口。default/HTML+SSR 不包含 Blitz/窗口，CPU shell 不包含 GPU 绘图实现，Android 实际依赖图不包含桌面 shell/winit/softbuffer。

UI 生命周期变化后，重新执行 Chromium 155 的 acceptance.js、focus-acceptance.js、navigation-acceptance.js，三组 errors 均为空。基础列表 80 次远跳最多 33 行/84 DOM 节点/140 图槽，每 host tick 最多 34 次 key 读取；焦点 fixture 定高/变高最多 14/16 行，程序 setter 为零，所有行最终 created=dropped。浏览器原生按键/选区与 AX role/name/focus 有证据，OS IME 与实际辅助技术继续独立验收。测试 HTTP fixture 已停止。

复现入口仍为 `./scripts/check-semantic-ui.ps1 -Web -Window` 与 `scripts/check-semantic-linux.sh --window`；原生编辑单测可定向运行 `cargo test -p glory-blitz --features shell --lib editing_tests`。Cargo home/target/tmp 均在 D:，Linux 使用已核对 D: backing file 的 ext4 loop。记录位于 D:/BuildCache/glory-semantic-ui：blitz-editing-final-{windows,linux}.txt、blitz-editing-full-check.txt（含 WASM）、blitz-editing-android-check.txt、blitz-editing-deps-*.txt、blitz-editing-browser-2026-10-22.json；源码/锁文件/验证脚本哈希与范围见 blitz-editing-evidence-2026-10-22.json。

后续仍需 R0 契约/能力/内容审计，R3/R5 外部资源策略与真实平台输入/辅助技术，R6 Activity/Surface/真机，R7 时间分解、字段依赖/style 优化及旧架构清理。完整目标保持进行中。

## R5 资源内核与图片滚动历史回收（2026-10-22）

本轮先完成资源协议/预算/生命周期底座，没有将标准 HTTP/文件/打包资产 loader 计为完成。最初核对发现：固定 Blitz 版本将所有解码过的图片按 URL 永久保留，仅限制 Glory 行节点或请求并发不能限制滚动历史内存。上游公开 API 也没有回收该缓存的接口。

因此在 `vendor/` 保存固定 `0.3.0-beta.4` 来源的 dom/traits/paint/shell 四包，原提交为 `92b11f1bd8ab7975411328b0406a95a09445b4e7`。它们以 glory-blitz-* 本地 package 身份保持类型一致，Rust 依赖别名仍为 blitz_dom 等；用户应用无需全局 Cargo patch。四包不进入十包活动 workspace。除资源补丁与 manifest 外，原源码保持；paint 仅注释一个 SVG 未启用时的未使用 variant，shell 源码没有改动。原 archive/file SHA-256、许可证与可审查差异见 vendor/blitz-source.json、vendor/blitz.patch、vendor/README.md。没有新增资源实现的 unsafe。

`scripts/check-vendored-blitz.py` 校验 103 个文件，意外更改原源码或未记录的补丁会失败；刷新已声明补丁时从 D: 的原 archive 源提取差异并检查新增 unsafe。Windows/Linux verifier 已接入来源验证和额外 69 项引擎单测。根与 semantic-web 的 Cargo.lock 改为保留，构建/检查使用 --locked，避免新依赖在后续机器静默漂移。分发/发布本地 fork 仍是后续 release 工作，此处没有 crates.io 发布声明。

实现边界：

- `glory_blitz::Config` 为 Renderer/WindowApp 提供显式 base URL、ResourceLimits 和 provider。默认 `glory://app/` 不授权网络/文件访问，也不安装实际 I/O。自定义 provider 使用 `glory_blitz::assets` 导出的接口；它们属于 adapter，不进入 core/UI。
- decoded-image 复用缓存同时限制条目和 payload 字节，命中更新 LRU；默认 64 项/16MiB。pending image 默认 128 个 URL、每 URL 最多 4096 waiter；新消费者超限计入 rejected。配置不支持缩小到现有不可回收注册/等待资源以下。
- 单张图片在解码和 RGBA 扩展前检查尺寸及输出字节，默认每维 4096px、RGBA 16MiB。接通 PNG/JPEG/WebP codec；不启用 SVG，也不以猜测的 RGBA 大小缓存其不透明树。
- 外部字体按成功注册次数和原始数据量限制，默认每文档 64 次/32MiB。坏字体不消耗成功配额；quota 拒绝与解析失败分别计数。该统计不包括系统字体/UA 字体。
- `prune_resource_interests` 只遍历有上限的 pending 表，校验活跃节点代次及当前图片/背景/mask URL。最后一个消费者离开后调用 cancel；Dispose/清空后没有历史 waiter，Suspend 保留已拥有的内容并取消未完成图片。
- 重新接入的背景可能有完全相同的 computed style，因此 Resume 显式重访取消的 layer；旧 request-id 不能填入新同 URL 的 waiter。改变或清除 src 释放节点的旧图像引用，失败会完成 critical stylesheet 并解除阻塞。Doc Drop 调用 provider.close。
- 每次引擎消息摄取最多 64 条，后续轮次继续；没有业务 Cage 写入也会因资源完成推进视觉版本。稳定的 coalesced waker relay 支持窗口出现前开始的请求唤醒后来宿主，Close 后的晚到回调不解码/唤醒。
- Raster 在“像素未变化”判断之前摄取资源，paint 在资源/布局处理后返回 FrameInfo；已绘制 placeholder 的帧可因图片到达而更新，再次无变化时仍跳过光栅化。core 的 staged UI 仍须显式 flush/pump。

9 项新增集成回归使用真实引擎 handler 注入字节，包括实际 PNG 解码、8×8 几何及 CPU 像素、过期同 URL 响应、节点代次、清除 src、waiter/admission/解码预算、无效 URL、CSS import/font/background、坏字体后重试、critical CSS 失败解锁、暂停恢复、关闭清理及 worker 线程完成的单次宿主唤醒。测试 provider 只是一份可控制的请求队列，不是 HTTP/文件客户端。

大列表场景是每行不同图片 URL 的一亿条惰性源，定高/24–32px 变高分别运行，180px 视口、overscan=2、80 次远跳。每隔一窗完成一批 8×8 PNG，另一批被滚动丢弃。断言解码复用缓存 ≤8 项/1024 payload 字节、pending URL/waiter 各 ≤16、transport 队列瞬时峰值 ≤32、语义节点 <64、图槽 <160、累计 key 读取 <4000；cache eviction 和取消均超过 100 次。清空后队列与 waiter 为零，最后 Renderer/Doc 释放时关闭 provider。

上述预算是缓存/等待表与特定场景计数，不是进程 RSS 或所有活跃图片的总内存。可见元素直接拥有的 Blob、系统字体、CPU 转换缓存及 surface/backbuffer 是不同所有权；当前 Vello CPU 另有自己的转换缓存策略。没有 FPS、阶段耗时、网络吞吐或真实 I/O 中止结论。

最终验证：Windows 与 Ubuntu WSL 均通过 178 项框架运行测试、1 项 !Send 编译失败用例、额外 69 项引擎单测及严格 Clippy；Linux 另通过 default/paint 组合。显式窗口测试仍覆盖 Windows 一次、WSLg X11/Wayland 各一次，日志 passed 合计 249/250。它们沿用受控编辑 fixture，峰值 14 个任务/77 引擎节点，累计 key 本次分别为 676/669/670；不是本轮图片注入测试的节点计数，也未新增真实窗口网络请求声明。

Windows WASM 严格检查与 fixture 重建通过；本轮没有改变 Web/UI 解释器，不重复声称重跑浏览器脚本。Android all-features/all-targets 编译通过。实际依赖图确认 default 与 HTML+SSR 不包含原版或本地 fork 的 Blitz/image/window，CPU shell 不引入 GPU 绘图实现，Android 不引入桌面 winit/shell/softbuffer。

Linux 首轮链接阶段出现 Bus error，当时 D: backing 的 32GiB ext4 分区剩余约 1.7GiB。确认进程终止与实际挂载后，只清理任务 target/debug/incremental，释放后约 7.2GiB；重跑设置 CARGO_INCREMENTAL=0 后通过。记录相关条件，不把一次链接失败断言为源码问题或已证明的唯一磁盘故障原因；没有回退到 C: target/cache。

原始记录：D:/BuildCache/glory-semantic-ui/blitz-assets-final-{windows,linux}.txt、blitz-assets-kernel-tests.txt、blitz-assets-upstream-tests.txt、blitz-assets-android-check.txt、blitz-assets-deps-*.txt。失败首轮保留在 blitz-assets-full-linux.txt，最终源码/锁文件/来源与验证脚本的 SHA-256 见 blitz-assets-kernel-evidence-2026-10-22.json。

下一步继续标准 asset loader 的 URL/redirect/路径策略、encoded-body/并发/队列/期限预算和真实 I/O 取消，再接宿主资源包及 Android。重定向后的 CSS 相对地址、异步图片对变高锚点的影响需要专门验收。R0、R3/R5、R6、R7、平台辅助技术及旧架构清理保持未完成，整体目标继续推进。

## R5 桌面 HTTP 与内置资产包（2026-10-22）

本阶段更新上一节的 transport 子项，新增可选 `glory-blitz/assets`（门面 `blitz-assets`），没有增加空 crate，也未让 core/UI/默认门面依赖 HTTP。`assets::{Loader, LoadConfig, LoadLimits, LoadStats}` 属于真实渲染适配器服务；应用数据的 Cage/Resource 不与图片/CSS 传输混合。Loader 每文档独立，不能复用于第二个 WindowApp 或关闭后重开。可运行示例为 `crates/blitz/examples/assets.rs`，需要 `assets,shell`，默认只读显式内置 CSS 包，不发网络请求。

策略和生命周期如下：

- HTTP(S) origin 必须显式授权，包含 scheme/host/effective port；默认空列表。每一跳验证 URL/凭据/origin/长度，拒绝跳入 `asset:` 或 HTTPS 降级。只接收空 body/header 的 GET；不启用自动重试/重定向、cookie、环境 proxy、referer、压缩解码。HTTP/1 client 使用显式 Ring provider 和系统证书/主机名校验，不修改进程级 crypto 默认值。
- 内置包由 `/theme.css` 等 encoded path 与不可变 Bytes 组成，位于 `asset://app/`。不是文件挂载，不解码路径后读磁盘；路径、项数、路径加 payload 字节和单体大小在构造时验证。
- 默认最多 4 个活跃任务、64 个排队 job、每 body 8MiB encoded payload、包括排队/所有跳的 15s 期限、4 个跳转和 4096 URL 字节；包预算独立为 256 项/32MiB。配置有硬性上限检查。一个专用 current-thread Tokio runtime 做 HTTP 与引擎 handler 处理，UI/业务状态不跨线程。
- Cancel/close 撤销排队工作并丢弃进行中的 HTTP future/socket；真实 peer 已观察到连接断开。已经开始执行的 handler 不可强行中止，其过期结果继续受文档 request ID 隔离。完成 task 及时从 JoinSet 回收；空闲只等待 notify。close 不在 UI 线程 join，并禁止后续接收。
- 系统 DNS 按每个允许主机 single-flight，只保留一次成功或失败及最多 16 地址；最多 64 origin。取消不能反复制造 system resolver 队列。OS getaddrinfo 本身不可中断，可能在 close 后短时存在；它不持有 App/Runtime/window，运行时不等待它阻塞关闭。要刷新 DNS 需新建 Loader；origin 授权也不是另一个私有地址信任策略。

真实传输揭示并修复了两个引擎边界：图片最终 URL 不能用于匹配原始 pending/cache key，现改为在有界 pending 表按 request ID 找原请求；否则 302 后会一直 pending。外部 StylesheetHandler 与 nested import 则必须用最终 URL 解析相对资源，现已分别修正。实际 HTTP 用例验证 `/theme.css → /theme/main.css → nested.css → /fonts/end.css` 下的 background/font 地址，图片 302 后的成功缓存复用与坏图片失败清理；另有 linked stylesheet 的独立基址回归。来源和本地 patch 哈希仍由 vendor verifier 检查。

新增 7 项真实传输集成用例及 1 项 linked stylesheet 回归（资源内核累计 10 项）：内置包/默认拒绝、输入与包预算、队列满载、活动与排队期限、AbortSignal、100 次连接取消/重建与关闭、逐跳拒绝、Content-Length 和 chunked body 超限、非 identity encoding、格式有效但不可信的 TLS 证书、真实 CSS/import/font/background/image，以及大列表。这里的 TLS 是负向 loopback 证据，不是所有平台/公共 CA/CDN 已验收。

真实 HTTP 大列表对定高与 24/28/32px 变高各运行一亿行、180px 视口、overscan=2、40 次远跳、每行独立 URL。每隔一窗等待真实 PNG 完成，其余可被取消。断言为 active ≤2、queued ≤32、复用缓存 ≤8 项/1024 payload 字节、pending/waiter 各 ≤16、语义节点 <64、图槽 <160、累计 key <4000。最后一次 Windows 定向记录为：

| 模式 | key 读取 | active / queued 峰值 | 完成 / 取消 | eviction | 清空后缓存 payload |
|---|---:|---:|---:|---:|---:|
| 定高 | 491 | 2 / 21 | 310 / 181 | 247 | 888B |
| 变高 | 1179 | 2 / 22 | 278 / 215 | 209 | 888B |

清空后 transport active/queued、引擎 pending/waiter 均为 0；Doc close 后 Loader stopped。完成与取消的分配会随线程时序改变，测量修正次数也可能不同；表中是单次记录，不是稳定吞吐或跨模式效率排名。图片行使用显式 CSS 高度，因此没有完成未知固有图片高度改变锚点的验收。

尚未完成：文件挂载/Android AssetManager、端到端 decoded completion 背压、未知图片高度锚点、真实 OS IME/辅助技术、Android Activity/Surface/设备、阶段耗时与 RSS/FPS。transport job/encoded-body 预算不覆盖已经发布到引擎队列的 decoded results；不能据此宣称全进程内存上界。Loader 当前 target-gated 到 Windows/Linux/macOS，Android/WASM 不引入其依赖，macOS 本阶段没有编译/实跑证据。R5 和完整目标继续保持未完成。

最终验证：Windows 与 Ubuntu WSL 均通过 186 项框架运行测试、1 项 !Send 编译失败用例、69 项引擎单测及严格 Clippy；Linux default/paint 组合也通过。显式实窗继续是受控编辑 fixture（没有改成 HTTP/资产窗口验收），Windows/X11/Wayland 各一次，峰值均为 14 tasks/77 引擎节点，本次 key 为 674/675/674；passed 合计 257/258。窗口包资产 example 已编译，未单独运行；7 项 transport 用例在两平台使用真实本地 HTTP/TLS。

WASM 严格检查与 fixture 重建通过，没有在本轮重跑浏览器脚本。Android all-features/all-targets 编译通过；依赖图确认默认门面与 HTML+SSR 没有引擎/HTTP/window，普通 shell 没有自动引入 HTTP/TLS/GPU，Android 没有桌面 Loader/shell 依赖。构建/缓存/tmp 都在 D:。Linux 核对现有 loop0 及 backing file 后，将 D:/BuildCache/glory-semantic-ui/linux-build-cache.ext4 从 32GiB 在线扩到 64GiB，消除本轮约 1.6GiB 剩余空间的风险；没有使用 C: 回退。

复现仍使用主 Windows/Linux verifier；定向入口为 `cargo test --locked -p glory-blitz --all-features --test loader --test resources -- --nocapture`。原始日志为 D:/BuildCache/glory-semantic-ui/blitz-loader-{tests,clippy,final-windows,final-linux,android-check}.txt 与 blitz-loader-deps-*.txt；确切源码、锁文件、来源 patch、脚本哈希和单次计数保存在 blitz-loader-evidence-2026-10-22.json。其窗口/传输/编译范围分开记录，不把其中任何一项算作 R5/R6 整体完成。

## R5 请求租约与资源完成背压（2026-10-22）

上一节保留的“已发布 decoded results 没有背压”子项现已补齐：资源内核新增共享 RequestPool/lease，不再把 HTTP 结束视为资源处理结束。每文档的全部资源 handler（图片、外部/导入 CSS、字体与引擎 HTML 资源）都先登记有界请求身份；默认 `requests=256`、`completion_bytes=32MiB`、`encoded_bytes=8MiB`。请求数涵盖排队、解码、已经发到结果队列但宿主尚未消费的阶段。ID 在文档内单调递增，到耗尽时拒绝新登记，不回绕。`ResourceStats` 新增 requests、completions、completion_bytes、decode_waiters，和图片复用缓存、pending URL/waiter 分开观察。

`NetHandler::prepare(url, bytes) -> NetReady` 在开始同步解析前异步取得容量；标准 Loader 将它放进原有 deadline/cancel 路径，期间仍持有固定 worker 槽位和受 transport 预算约束的 encoded body，不持有 CSS/资源锁。期限涵盖排队/HTTP/prepare 等待，不强行中断已经运行的同步解析。注入式同步 provider 若不 await prepare，内核仍校验并在容量不足时显式完成失败。CSS import 的请求槽位不足时返回 refused rule，字体跳过 fetch，linked critical CSS 不登记无法完成的 request。失败使用已有请求槽，拒绝本身不发送另一条错误事件，避免把超限转为无界错误 channel。

结果携带 lease，直到宿主完成消费、丢弃过期响应或关闭 receiver 才归还。图片离屏取消会标记 lease 过期并停止传输；若结果已经进入队列，其计费保留到实际结果释放，不能先退额度再继续堆旧图片。`ResourceLoadResponse` 明确把 payload 放在 lease 字段之前，利用 Rust 字段析构顺序先 Drop 数据、再归还预算/唤醒生产者；弱引用测试直接检查这个实际顺序。响应被应用后，活跃节点/复用缓存成为另一份所有权，不再属于结果队列计费。

提高预算会唤醒等待解码的任务，低于当前 lease/计费的缩减被拒绝。取消的 prepare future 清理其 waker；Waker 唤醒、替换和析构均在池锁外。Doc 关闭先停止登记、唤醒等待者并丢弃 receiver/队列，再关闭 provider；迟到的 handler 不解析、不发布。资源完成通知合并到宿主消费轮次，单轮仍最多处理 64 条结果。旧的异步/代次/取消/输入实窗回归继续保留。

计费边界：raster 是 RGBA payload，font 是 bytes+alias，HTML 是保留的 UTF-8；CSS/SVG 使用源字节计费，其 opaque AST 以请求数和源体上限约束，不谎称测出了 AST 大小。Glory 当前不启用 SVG/WOFF。URL/错误诊断各最多保留 4096/512 个 UTF-8 字节，过大的 String capacity 同样会被替换释放。decoder scratch、请求元数据、系统/活跃字体、缓存、绘图转换与 surface 不属于完成队列字节计费，仍不能推导全进程 RSS 上限。

新增 2 项真实 HTTP 压力用例、1 项 critical-CSS 准入回归及 7 项引擎内部回归：

- 宿主不 pump/poll，6 个有效请求、8×8 PNG、256B completion 预算、2 workers。断言只完成一个图片（256B），另两个任务等待解码，额外两个图片在 fetch 前被拒绝。清空行并取消后，旧队列仍持有 1 lease/256B；实际消费过期结果后归零且不进入 cache。恢复后 4 张图片正常加载；再次暂停后直接关闭文档也结束所有 transport 工作。
- 2000 个 import 与 400 次图片失败，宿主暂停，只持有 4 请求/4 完成消息/0 payload 字节，Loader 只收到 4 次拒绝，池记录 2396 次准入拒绝。恢复处理和一次新失败后 requests/completions/bytes/waiters 全为 0；超限没有制造额外错误流。
- linked stylesheet 第一个请求占满唯一槽时，第二个不 fetch、不遗留 critical ID；第一个失败后解除 paint 阻塞。内部回归覆盖过期结果仍计费、Payload-before-wake、锁外 waker 重入/析构、关闭/扩容/缩减、取消 future、UTF-8/容量诊断，以及请求身份耗尽不回绕。

一亿行真实 HTTP 定高/24–32px 变高列表继续使用 180px 视口、overscan=2、40 次远跳与独立图片 URL；新增请求租约 ≤32、completion 计费 ≤8192B 的断言，原有节点/图槽/key/缓存/pending/transport 预算继续生效。清空必须等待 transport 和请求租约均归零，不能只等 HTTP worker 空闲。最终一次 Windows 定向记录 key 为 491/1167、active/queued 峰值均为 2/21，结果/lease/计费/waiter 最后全部为零；完成/取消与测量修正仍随线程时序变化，不是吞吐或 FPS 排名。

最终 Windows/Linux 均通过 189 项框架运行测试、1 项 !Send 编译失败用例、76 项引擎单测及严格 Clippy；Linux 另验证 default/paint。实窗继续是原受控编辑 fixture，Windows/X11/Wayland 各一次，14 tasks/77 引擎节点，本次 key 677/673/669；passed 合计 267/268。窗口 fixture 没有改成 HTTP 压力场景。WASM 严格检查和 fixture 重建、Android all-features/all-targets 编译及依赖隔离通过；本轮未重跑浏览器脚本，也不新增 OS IME/辅助技术/Android 设备验收声明。

实现主要在 vendor/blitz-dom/src/resource_queue.rs、net/resources/document/mutator/layout/iframe 与 traits/net，外层只转发 prepare 并纳入 Loader 的等待/期限/取消。来源核对现在为 4 个 fork package/104 文件，iframe 变更与新增模块已显式登记，patch 没有增加新 unsafe。没有新增依赖，核心/默认门面和 HTML+SSR 仍不含引擎/HTTP，Android 不含桌面 shell/Loader。构建/cache/tmp 继续全在 D:。

原始记录为 D:/BuildCache/glory-semantic-ui/blitz-pressure-{tests,engine-tests,clippy,final-windows,final-linux,android-check}.txt 与 deps 日志；最终源码/锁文件/patch/脚本哈希及确切单次范围见 blitz-pressure-evidence-2026-10-22.json。首轮 loader fixture 在 shutdown 时解析半截 HTTP header 的旧假设被真实取消时序触发，已修复为关闭/不完整 header 直接结束；首轮失败保留在 blitz-pressure-baseline-tests.txt。

下一步继续未知图片固有高度的 measured-list 锚点、文件/Android 资产、真实平台输入/无障碍、R0/R7 以及旧架构清理。R5 与完整目标不因完成队列背压而标为完成。

## R5 固有 PNG 高度、持续导航与滚动接受位置（2026-10-22）

本阶段完成上一节的固有 PNG 高度子项，没有固定图片 CSS 高度，也没有把其他格式、字体到达、OS 输入或移动设备算作同一项完成。两个发现改变了共享契约：

- 原生 End 目标已经测量占位 24px 后，导航意图被提前删除；图片到达变 96px，原 y=156 导致底端 252px，超出 180px 视口。`ui::list` 现为变高列表持续保留至多一个 reveal 意图，修正为 y=84、底端 180px；超出视口的行起点对齐。真实用户移动、新程序位置/数据或焦点移走即取消；定高导航仍立即结束。
- Chromium 实际接受的 scrollTop 曾比请求偏约 0.28125 CSS px，旧的 0.25px 判断把程序赋值误认为用户滚动。没有扩大容忍吞掉小输入，而是在 render port 增加 ScrollRequest/ScrollAck：最后一次 requested/accepted 与当前 x/y 分开。Web 在 ScrollTo action 屏障立即读回接受值、安排观察；Blitz 也读回并报告。确认保留在后续观察中，所以原生移动抢在首个确认之前到达也可区分。元数据按活跃节点有界，销毁释放；HTML Snapshot/wire 版本不因运行时事件扩展而变更。

控制器保留当前定位的量化偏差，新导航或外部 seek 重设，避免连续 Home/End 累积误差。`.position(cage).on_scroll(...)` 的相同受控回写只确认报告值，不重新 seek/取消 reveal；仅逻辑值变化才报告，空闲不回写。专门 UI 回归覆盖接受坐标 -0.625px 与确认同帧的 -1px 原生移动，真实逻辑 -1px 被保留；随后 Resize 不恢复已经取消的意图。既有导航 fixture 另验证反复首尾切换。

实现位置：`crates/render/src/lib.rs`、`crates/ui/src/list/{widget,measure}.rs`、`crates/web/src/browser.rs`、`crates/web/src/browser/dom.rs`、`crates/blitz/src/document.rs`。不向 core 增加引擎类型，不在 adapter 复制列表算法；没有新增依赖，也没有改动本轮 vendor 源码。

新增验收及实际范围：

| 场景 | 实际行为与上界 | 单次观察 |
|---|---|---|
| Blitz 注入 PNG，一亿行/180px/overscan=2 | 上方 24→96px 保留同一 key 的 -7px；行内图片变 128px、源替换成 32px、迟到旧请求、40 远跳、测量 eviction 后返回；行≤16、测量≤64、height capacity<2000、nodes<64、图槽<256、key<4000 | key=1090，末窗 12 行、64 测量、height capacity=31，复用图片 3 项/8296B |
| Headless WindowApp 导航 | 已测占位 End 后收到 96px；受控位置回写；原生/程序移动取消；宽度变化及 384px 超高目标起点对齐 | 目标 y=156→84、底端=180；不创建实际 OS 窗口 |
| 真实 loopback HTTP | 门控上方和最后一行 PNG；实际解码、事件、测量与逻辑修正；结束 transport/lease 清零 | 相同 -7px 锚点与 84–180px End 行，key=57 |
| Chromium intrinsic fixture | 实际 PNG decode/ResizeObserver；增长/缩小、受控回写、滚动/seek 取消、40 远跳、返回和卸载；行≤17、测量≤64、height capacity<2000、nodes<70、图槽<300、key<5000 | 峰值 11 行/28 语义槽/79 图槽；末测量64、height capacity644、key2109、errors=[] |

浏览器版本 Chrome 155.0.8059.84，独立 context 请求 deviceScaleFactor=1，实际报告 DPR≈0.99999998，几何断言容忍 1 CSS px。PNG 通过 route 拦截完成，不宣称此脚本证明真实网络取消；真实 HTTP 由原生用例覆盖。key/报告次数和中间测量受调度影响，以上单次值不作固定性能指标，更不能作跨宿主速度、FPS 或 RSS 结论。

新增 `examples/semantic-web/src/intrinsic.rs` 与 `intrinsic-acceptance.js`，WASM fixture 的 unmount 统一释放该 App。最终重跑全部十组浏览器脚本：acceptance、adaptive、focus、navigation、budget、resource、resource-snapshot、hydrate、intrinsic、stream，均通过；同时重跑 Python stream transport 检查。End 的原生修复前失败保留于 blitz-intrinsic-navigation-before.txt。

最终 Windows/Linux 全量 gate 各通过 193 项框架运行测试、1 项 !Send 编译失败用例、76 项引擎单测及严格 Clippy；Linux 另检查 default/paint。实窗仍是既有受控编辑/生命周期 fixture：Windows 一项、WSLg X11/Wayland 各一项，峰值 14 tasks/77 引擎节点，本次 key 分别 673/669/676；日志 passed 合计 271/272。不要把这三次窗口运行算成 HTTP/PNG 交互窗口验收。末次只追加超高行断言及注释后，Windows 严格 Clippy/定向测试、Linux 两项资源和一项真实 HTTP 定向测试继续通过。

WASM workspace 与 fixture 的 all-targets 严格 Clippy、WASM 重建、Android workspace all-features/all-targets check 均通过；Android 仍只有编译证据，没有 Activity/Surface/设备。全部 Cargo home/target/tmp 在 D:，Linux metadata 指向 `/mnt/d/BuildCache/glory-semantic-ui/linux-fs/target`，loop0 backing 继续是 D: 的 ext4 image。固定来源核对仍为四包/104 文件，未改变其 patch/依赖。

复现：主 verifier `./scripts/check-semantic-ui.ps1 -Web -Window`、`scripts/check-semantic-linux.sh --window`；定向运行 `cargo test --locked -p glory-blitz --all-features --test resources intrinsic -- --nocapture` 和 `cargo test --locked -p glory-blitz --all-features --test loader delayed_http_intrinsic -- --nocapture`。浏览器另执行十个 acceptance 函数，build 本身不执行它们。

最终记录位于 D:/BuildCache/glory-semantic-ui：intrinsic-final-{windows,linux}.txt、intrinsic-linux-targeted.txt、intrinsic-{wasm-check,wasm-fixture-check,android-check,web-build,stream-transport}.txt、intrinsic-browser-final-2026-10-22.json；源码/锁文件/脚本及日志哈希见 intrinsic-evidence-2026-10-22.json。完整目标保持进行中：R0 内容/能力审计、R3/R5 平台输入/辅助技术及文件服务、R6 Android、R7 时间分解/字段与缓存优化、旧源码/工具清理仍待完成。

## R5 显式只读文件挂载与阻塞读取上界（2026-10-22）

本阶段完成桌面文件来源，继续放在 glory-blitz/assets 内，不新增薄文件 crate，不把 filesystem/HTTP 引入 core/UI/默认门面。LoadConfig::mounts 将至多 32 个绝对目录映射为命名 asset:// 主机，app 保留给内置包；空 mounts 不授权磁盘。名称为至多 32 字节的 ASCII 小写字母/数字/连字符标签，根在构造时必须是现存目录。仍拒绝任意 file://、未知资产主机/凭据/端口和 HTTP 重定向进入资产命名空间。

文件访问使用固定 cap-std/cap-fs-ext 4.0.3 与 percent-encoding。构造时打开目录 capability，后续路径相对句柄解析；没有 canonicalize 判断后按环境路径重开的窗口。路径段按 UTF-8 解码一次，拒绝编码分隔符、NUL/控制字符、驱动器/ADS 和常见 Windows 设备名/尾点空格；query/fragment 不参与磁盘路径，但保留请求/缓存身份。root-relative 符号链接必须留在授予的目录内；实际打开后的 handle 再验证 regular file，目录、socket/FIFO 不读。挂载树中的 hardlink 是已授予文件，这不是进程级沙箱或可变文件的原子快照。

读取复用原有 encoded body、transport 队列/worker/期限以及 prepare/完成租约，不设另一个历史文件内容缓存。默认 body 8MiB；读前 metadata 上限与逐块上限都检查，文件增长不能越过预算，分配容量不超过 body limit。chunk 至多 64KiB，chunk 之间检查取消/期限。文件任务在 spawn_blocking 前取得信号量许可，并让实际闭包持有它；取消 future 不提前归还还在执行的文件槽。底层 OS 文件系统调用可能不可中断；迟到结果不发布，不能声称强制终止了内核读。

LoadStats 新增 file_active/file_peak_active，与 transport active/stopped 分开。停止异步 runtime 不代表已取消的 OS 调用已经返回。关闭停止许可准入、唤醒等待者并释放空闲根句柄；正在读取的闭包只保留自己的根克隆直到返回。Loader 仍用于统计时，关闭也不保留所有挂载根的 Windows 锁。相关实现为 crates/blitz/src/loader/files.rs 与 loader.rs；没有改变 vendor 代码或语义 render 协议。

新增实际验收：

- Windows 5 项文件集成测试：授权/配置/UTF-8 单次解码、空文件与失败、精确 256B 与 257B 超限、根句柄/关闭、junction 越界、嵌套 CSS 与 PNG 的 32×48px 实际几何、一亿行定高和变高列表。Unix 同组多 1 项：200 次请求伴随链接并发反复指向内部/外部，仅返回内部内容或拒绝；FIFO/socket 均拒绝。
- Linux 改名并替换根目录路径后，原句柄仍读取原内容；Windows 当前 cap-std 根 handle 阻止 rename，观测到 sharing violation 32。关闭后，即使 Loader 仍保留用于统计，两个平台均能再改名。Windows 测试用实际 junction，不依赖 symlink privilege；Linux 用实际 symlink，没有把一方替代为另一方的平台证据。
- 一项共享 blocking 路径的门控单测：第一个闭包尚未返回时取消其 async task，再创建/取消 200 个等待者，没有新闭包进入；peak=1。关闭令另一个等待者返回 closed，释放门控后 file_active=0。它验证真实生产路径的许可寿命，不是故障磁盘/网络盘测试。
- 定高与 24/28/32px 变高文件图片列表各为一亿行、180px 视口、overscan=2、40 次远跳。每个逻辑行使用独立查询 URL 指向同一个真实 PNG，避免测试自身生成一亿文件。断言 file_active/peak≤2、transport active≤2/queued≤32、reuse cache≤8 项/1024B（含 URL）、pending/waiter≤16、请求≤32/completion charge≤8192B、语义节点<64、图槽<160、key 读取<4000。离屏取消与 cache eviction 都发生；清空后请求/完成/pending/transport/file_active 均归零。完整 gate 验证最终代码的这些上界，不作单次线程时序的吞吐排名。

初版测试暴露了两个测试假设错误：Windows 根 handle 本身禁止 rename；同一 PNG 被背景和 image 复用时只有 3 次 Loader 完成，不应要求重复第 4 次读取。已按实际平台行为和几何/共享缓存断言修正，失败原始记录保留在 file-tests.txt / file-linux-tests.txt；没有为满足假设放宽根权限或强制重复加载。

最终验证：Windows 199 项框架运行测试、1 项 !Send 编译失败用例、76 项引擎测试，passed 合计 276；Linux 因 Unix 链接/特殊文件用例为 200+1+76，合计 277。工作区 all-features/all-targets 严格 Clippy 均通过，Linux 另通过无窗口 default/paint Clippy。本阶段不重跑实窗或浏览器脚本，未修改它们的交互实现；不能把这些新增文件测试当作 OS 窗口/输入法/辅助技术验收。

Windows 主 verifier 的 -Web 通过 WASM 工作区/fixture 严格检查并重建可运行样例；Android all-features/all-targets check 通过。依赖树确认默认门面和 HTML+SSR 不包含引擎/HTTP/cap-std，Android 不包含桌面 Loader/文件 capability/shell。macOS 没有本轮编译或运行证据。根 Cargo.lock 新增 11 个 capability 相关解析包，均为 opt-in desktop assets 依赖；四个 engine fork/104 文件的来源核对继续通过。

assets 示例可接收绝对目录参数，显式从该目录加载 theme.css；无参数仍只读内置包。复现入口：cargo test --locked -p glory-blitz --all-features --test files -- --nocapture；许可用例为 cargo test --locked -p glory-blitz --all-features --lib loader::files。完整检查仍用 ./scripts/check-semantic-ui.ps1 -Web 和 scripts/check-semantic-linux.sh。本轮 Cargo home/target/tmp、fixture 文件及 Linux ext4 backing 均在 D:。

日志与证据：D:/BuildCache/glory-semantic-ui/file-final-{windows,linux}.txt、file-{tests-fixed,cancel-tests,linux-tests-fixed,android-check}.txt、file-deps-*.txt，最终哈希与范围记录为 file-assets-evidence-2026-10-22.json。Android AssetManager/Activity/Surface、实际 OS IME/辅助技术、R0 审计、R7 阶段耗时与缓存/字段优化、旧工具/源码清理仍待完成，整体目标保持进行中。

## R7 固定聚合阶段观测与重复基线（2026-10-22）

本阶段提供实际计时/工作量和后续优化依据，不将整个 R7 标为完成。新增 render::profile 的 Profile/Phase/Sample/Snapshot/Span：18 个固定聚合槽位，无节点/事件/帧历史。默认 None 不读取诊断时钟；原有 RuntimeStats 计数仍正常工作。计时覆盖 Flush、Validate、实际 Derive/Structure/Bind/Effect、future poll、mount_with 构建、List Window/Reconcile、语义 Patch、非空 Commit、Observe/Action、Blitz 资源消费/合并 Layout/场景 Paint/变化帧 Draw。

App::set_profile 同时更新 Runtime 和支持的 adapter，返回的旧/未使用 renderer handle 在借用结束后释放；Runtime 替换也在图借用之外 Drop。Profile 克隆共享聚合值，默认按 app/实验创建；不使用全局 profiler。native 工厂使用 Instant；WebApp::enable_profile 使用自身文档的 Performance，缺失则 Unsupported，不在 WASM 执行 std Instant。自定义时钟须轻量/单调、不重入 UI/图。clock callback 执行时不借用 samples；反向时间增加 invalid_clock，时间/次数饱和累加。reset 的 epoch 不回绕，旧 span 不污染新窗口。

所有时间为包含耗时，彼此可重叠；max 是单次阶段调用最大值，不能当成整帧 p95/p99。Validate units=旧依赖数量，Reconcile units=成功协调的 RowHost key 读取（通常是缓存，不是原始数据源访问），Commit units=语义 mutation 数，其余默认一次调用为1。原始 source key 读取继续单独统计；错误协调的 units 为0，attempt 次数仍保留。Task 只计 future 的 poll，不把等待 I/O 的时间计作计算。

Blitz Layout 包含 Glory Dom::resolve 调用内的 style/box/text 等；Paint 只计场景记录；Draw 含变化帧的 buffer/CPU 和嵌套布局/paint，不是纯光栅耗时。直接引擎调用、upstream 桌面 paint/present 路径、引擎内部更细分阶段和业务事件 handler 尚未全部计时。浏览器内部 style/layout/paint 也没有伪造计时，相关槽位保持0。启停/reset 不应生成 Patch、改变视觉版本或唤醒持续重画。

新增五项回归：两项 collector 测试验证嵌套包含时间、reset 撤销 live span、时钟倒退与 unwind；一项 runtime 测试验证 source 1→3、相等门控 parity 仍验证2次/求值1次但消费 Binding=0，关闭后 source 3→4 仍更新且诊断时钟读数不再增加；一项一亿行 List 测试验证同窗读取0、100次远跳有界及关闭/卸载；一项实际 CPU 像素测试验证共享 Layout/Paint/Draw、空闲五轮全为0且视觉版本不变、恢复更新与关闭观测。

新浏览器 profile-acceptance.js 使用真实 Performance：40 次远跳和状态写入后各已接通阶段有计数、无反向时钟；关闭后更新仍执行而聚合值冻结，reset 清零、unmount 清空。最终重新执行原有十组脚本，合计十一组 errors=[]；同时重跑 Python stream transport。浏览器用例不作为性能基准。新 Native profile example 则是真实文档/CPU pixels，setup/cold/warm 分开；UI semantic_frames bench 是实际 Runtime/UI/List + 参考解释器，两者均 release、三轮重复。具体值见列表基线末节。

主要实测：UI 一亿行 1000 次同窗更新读取0 key，远跳33000 key，语义/图槽峰值与百万行相同。Windows 关闭/开启观测的每轮平均耗时三轮中位数为同窗0.955/1.169µs、远跳333.675/348.184µs。原生160×120、100次远跳，一亿行定高667.929µs/次、变高1872.059µs/次，分别100/300次真实 Draw。变高样例在观察迭代之间绘制，下一步应对照不绘制的稳定化与宿主调度，不能把逻辑更新数算成呈现帧数或直接外推生产窗口。没有 FPS、RSS 或逐帧分位数声明。

最终 Windows 204 框架运行测试、1项 !Send 编译失败、76引擎及1项实窗，passed 合计282；Linux 多一项 Unix 文件用例及两个实窗，为205+1+76+2=284。工作区 all-features/all-targets 严格 Clippy、Linux default/paint 通过。原实窗 fixture 的任务/引擎节点峰值仍为14/77；本次累计 key Windows=670，WSLg X11/Wayland=677/680。这不是开启计时器的 OS 窗口性能基准；窗口回归验证默认关闭观测时的既有行为。

WASM workspace/fixture 严格检查与重建、Android all-features/all-targets check 均通过。首次全检查发现新 CPU bench 未隔离 WASM 的 native 工厂，已明确 cfg 原生入口；WASM 本身的计时由实际浏览器验收，而非空基准入口证明。首次 Linux 父 shell 在 verifier 子脚本后单独启动 bench，没有继承其 Cargo 环境，离线索引解析即失败；确认终止、在父 shell 显式设置同一 D: 路径并核对 metadata 后重跑，没有回退到 C: 编译。原始失败日志保留。

没有新增依赖或修改 vendor；四包104文件来源检查继续通过。根与 fixture 锁文件保持不变，构建/缓存/tmp/临时服务数据在 D:；最终基线在本任务构建与浏览器脚本结束后按 Windows UI→Windows CPU→WSL UI 串行执行，未宣称宿主无其他负载。本轮启动的8788/8789服务均按实际PID/命令核对后停止。

复现：cargo bench --locked -p glory-ui --bench semantic_frames；cargo run --locked -p glory-blitz --example profile --features raster --release。完整验证仍为 Windows verifier -Web -Window 与 Linux verifier --window；浏览器另执行十一组函数。最终日志为 D:/BuildCache/glory-semantic-ui/profile-final-{windows,linux}-r2.txt、profile-android-check.txt、profile-browser-2026-10-22.json、profile-benchmark-summary.json，源码/锁文件/二进制与数据哈希见 profile-evidence-2026-10-22.json。

下一步先对照原生测量稳定化与 Draw 次数，再补 style/text/box/纯光栅/呈现等待和事件 handler 计时，依据收益实施字段依赖、style/paint cache。R0 内容/能力审计、平台输入/辅助技术、Android 与旧架构清理也继续保留在完整范围中。

## R7 绘制前的有预算观察与实际收益（2026-10-22）

上一节提出的原生对照已落实为公共 `App::settle(limit)` 和只读 `Render::needs_observe()`。每轮调用原有 pump，保持输入、查询与更新顺序；最多 1–32 轮（默认8），返回 passes/handled/pending。耗尽只交回宿主，允许显示当前状态并安排下一轮，不无限循环或把尺寸反馈当成运行时故障。WindowApp 自动使用8轮，本地任务仍另限64次 poll；观察期间不继续 poll tasks，稳定后不为再次确认 geometry 额外唤醒重画。

Blitz 以文档级待观察标记、未观察的视觉 revision、dirty layout、输入和已完成消息决定继续；未完成资源请求、动画时间不算可取得本地进展。新 Resize/Scroll interests 也需观察；ScrollTo 即使坐标不变，仍欠一次 ScrollAck。查询可能先 resolve，所以只检查 dirty 会漏观察；清除隐藏控件焦点则会在 resolve 后再次 dirty，必须继续一轮。异步消息在返回后仍可到达，settle 不构成原子显示事务。Memory 对已排队 Unsupported 查询响应启用有限重观察；Web 保持异步，不同步等待 RAF/ResizeObserver。

本轮新增6项框架回归：Memory 五个串联查询在2轮预算后按序让出并恢复；Blitz 持续尺寸反馈让出/恢复、查询已 resolve 但仍欠观察及 no-op ScrollAck、等待 I/O 不空转而完成结果可消费、一亿行定高/变高 CPU 最终 geometry/pixels 与单次 Draw；另有 WindowApp 8轮观察预算与任务预算分离。CPU 像素场景为160×120、120px视口、overscan=2、60次带7px余量的远跳，逐次查实际行 Bounds 并比对该行颜色；断言行≤12、测量≤64、高度索引capacity<4096、节点<64、图槽<256、key<5000，空闲5轮无 Draw、卸载归零。初次定向记录定高/变高最多2/3轮，最终全量 gate 继续通过8轮内完成的同一测试。

既有 wheel、focus、controlled editing/IME 协议用例的 helper 已收紧为一次宿主 poll 就不再有本地待观察工作。实窗 fixture 只在 waker 已安装且实际 poll 的轮次断言稳定；ResumeReady 前未激活的 poll 按上游生命周期为 no-op，暂停轮也不作该断言。持续尺寸反馈的独立测试仍要求预算耗尽并让出，不把真实应用一律假设为有限收敛。

收紧测试发现并修复固定引擎的一处崩溃：state-only snapshot 未消费时换 stylesheet，Stylo stylesheet invalidation 读取空 attrs。现在仅在 stylesheet 变化时升级这些待消费快照，保留原 state 和已有的旧属性快照；普通状态变更仍走轻量路径。直接引擎回归对 checkbox 状态和新增 class/:checked 规则同批 resolve，验证最终样式及快照回收。vendor 的 stylo.rs 显式加入补丁记录，document.rs 增加回归，patch 与来源哈希已刷新；没有新增 unsafe 或依赖。

同一 release 二进制交替执行 progressive（原 pump/draw）和 settled（settle/draw），百万/一亿行、定高24px/变高24–32px、160×120、overscan=2，每组10次预热后100次测量、三轮重复。最终一亿行变高平均更新耗时的三轮中位数为1932.650→1279.984µs，约减少34%；必要 Layout 保留300次，Paint/Draw 从300→100，源 key 均3100。百万行同方向，定高基本持平；活跃节点/图槽/引擎节点与源总量无关。Draw 包含范围改变，部分 Layout 移入 Observe，不能把 Draw 耗时降低全部解释为纯光栅提速；完整数字与限制见列表基线末节。

最终 Windows gate 为210项框架运行、1项 !Send 编译失败、77项引擎与1项实窗，passed合计289。Ubuntu WSL 多一项 Unix 文件测试及两个实窗，为211+1+77+2=291。workspace all-features/all-targets 严格 Clippy、Linux default/paint 全通过；实窗仍是受控编辑/生命周期 fixture，任务/引擎节点峰值14/77，本次累计 key Windows=670、X11=670、Wayland=669。不是发布模式或开启观测的窗口性能测试，也不是 OS IME/辅助技术验收。

十一组浏览器函数与 Python stream transport 全通过；profile-acceptance 新增40次更新中的 settle 单轮断言，结果 browserSinglePass=true，原有启停/reset/cleanup 继续通过。Windows gate 同时完成 WASM workspace/fixture 严格检查和重建，Android workspace all-features/all-targets check 通过。Android 仍只证明编译，未提供移动宿主。所有构建/缓存/tmp 在D:，Linux继续核对 loop0 的D: backing；根与fixture锁文件未变。

本轮初次隐藏样式测试的崩溃、过早在 ResumeReady 前断言实窗已观察等失败日志均保留；最终源码重新执行完整 Windows/Linux gate 后通过。release 最终对照在本任务构建、浏览器验收和 owned 8788/8789服务结束后串行运行，未宣称整台机器无其他负载。没有执行 Linux 耗时对照；其单次绘制/最终像素行为由 Linux 全量测试验证，不能借用 Windows 数值作为 Linux 时间。

复现：主 Windows verifier -Web -Window、Linux verifier --window；CPU 对照为 `cargo run --locked -p glory-blitz --example profile --features raster --release`。定向测试为 `cargo test --locked -p glory-blitz --all-features --test settle -- --nocapture`、`cargo test --locked -p glory-ui --test settle`。浏览器仍需单独执行十一组函数。原始记录位于D:/BuildCache/glory-semantic-ui：settle-final-{windows,linux}-r3.txt、settle-android-check.txt、settle-browser-2026-10-22.json、settle-stream-transport.txt、settle-benchmark-windows-final.jsonl、settle-benchmark-summary.json；源码、vendor、锁、二进制及日志哈希见 settle-evidence-2026-10-22.json。

R7继续保留 style/text/box/纯光栅/呈现等待与事件 handler 的观测、字段依赖和缓存收益验证；R0审计、实际 OS IME/辅助技术、Android宿主与旧代码清理也仍在完整目标内。

## R0 基础目标契约与 HTML 内容扩展（2026-10-22）

新增[组件契约审计](semantic-ui-contract.md)，将八组件内容、11类事件、查询/动作、宿主 capability、错误与剩余项集中记录。不是将 R0 勾选完成：本轮只落实基础目标、订阅和一处 HTML 内容漏洞。

render 层新增 Query/Action/EventKind::validate_target，UI 与参考/SSR/Web/Blitz 复用。Group 不再被 Web 的 DOM Range 偷偷转换成聚合 bounds；各后端一致把 Group 的物理查询/动作和直接 Resize/Scroll/Anchor/FocusChanged 目标诊断为 Unsupported，但逻辑 Activate/FocusWithin/Navigate 订阅保留。编辑事件仅针对 Input，checked事件仅针对Check，Selection先要求Input再看宿主类型/状态。SSR合法订阅仍可记录，不因没有输入驱动而拒绝整棵应用。

UI在替换或登记handler前验证，错误订阅不分配handler/listener；原有合法handler仍能执行，后续挂载/清理不受影响。动作先校验身份，再参数/目标，再capability，foreign/retired不被“不支持”遮盖，NaN坐标不进入队列。参考与SSR也补上直接action/listen的身份与目标检查；有效查询的Unsupported继续排队，保持请求水位与异步回调语义。

HTML mapper在Place/Resume变更之前补查button上下文：TextKind::Inline虽可显式映射为p，也不能通过先放进Group再移动/恢复而进入button。回归验证操作失败后parent、suspended、HTML和sink操作数不变，恢复到合法位置仍可用。低层批次仍允许前缀应用，并非新增整个batch回滚；当前UI已进入物理提交后的失败仍按fault处理。普通列表移动只走祖先路径，只有button上下文遍历被移动的已挂载子树，不访问总数据源或历史行。

新增7项框架测试：同一八目标/11类interest/24查询矩阵由Memory、SSR、真实Blitz文档各执行一次；两项UI验证错误订阅不覆盖handler、无fault和动作错误优先级；一项HTML验证Group移动/恢复的p覆盖；一项门面SSR验证直接和Group包裹的p失败、span成功。共用代码作为可选glory-render/conformance模块，SSR/Blitz仅通过dev-dependency开启，无新workspace包或第三方依赖，也不使用跨package文件包含。生产default及HTML+SSR的正常依赖图验证不含conformance或Blitz/window。

矩阵初次假定空inline span一定有引擎box，真实Blitz返回Unsupported。校正的是测试前提：矩阵现在显式建立display:block/64×24px盒子，再要求实际bounds/scroll；不将空inline或display:none可测作为统一保证。失败原日志保留。此后完整Windows/Linux运行相同测试通过，不能据此隐去空inline的后端差异。

新增target-acceptance.js及独立Rust fixture，实际浏览器中八语义节点仅生成七个元素，无Group wrapper；24个查询接受后仍未同步回调，随后按序完成，Group几何为Unsupported，waiter归零。错误注册后真实按钮可激活、文本可填入emoji，卸载无残留。该矩阵和原有十一组浏览器函数全通过，共十二组；Python stream transport也通过。既有百万/一亿行滚动、焦点/编辑、固有图片、资源、SSR/hydrate/stream预算继续通过，不因契约审计改成小列表替代场景。

最终Windows为217项框架运行、1项!Send编译失败、77引擎及1实窗，passed合计296；Ubuntu WSL为218+1+77+2=298。workspace all-features/all-targets严格Clippy、Linux default/paint、WASM workspace/fixture严格检查与重建均通过；Android同样all-features/all-targets编译通过。实际窗口仍为既有编辑/生命周期fixture，任务/引擎节点峰值14/77，本次key Windows=678、X11/Wayland=677/677，不是窗口性能或实际OS IME验收。

首次gate的UI匹配分支Clippy及独立WASM fixture格式检查失败均修正后重跑；格式使用fixture自身manifest，不假设workspace fmt覆盖外部应用。没有改vendor或锁文件，四包104文件来源继续验证；Cargo home/target/tmp、Linux loop0 backing和临时服务在D:。owned8788/8789服务按PID/命令核对后停止。本轮未重跑release耗时基准，不用上一轮数字声称此次契约检查没有任何时间开销；现有预算与源读取回归仍是结构性性能证据。

完整日志位于D:/BuildCache/glory-semantic-ui：target-contract-final-windows-r3.txt、target-contract-final-linux-r2.txt、target-contract-android-check.txt、target-contract-browser-2026-10-22.json、target-contract-stream-transport.txt、target-contract-deps-*.txt；源码、fixture二进制、锁、来源及日志SHA-256见target-contract-evidence-2026-10-22.json。定向入口见组件契约页，完整检查仍使用主Windows/Linux verifier；浏览器十二组另执行。

下一步R0继续事件payload/来源身份、Web焦点结果回读、结构变更预检、全部props和无障碍映射审计。R3/R5真实平台输入/辅助技术、R6 Android宿主/设备、R7细分观测与字段/缓存收益，以及旧代码/工具清理继续保留在完整目标中。

## R0 事件几何、来源与同批输入恢复（2026-10-22）

上一节的事件入口子项继续推进：render新增Event::validate_payload、Memory::validate_event及共用is_terminal_loss，错误数值/结构用InvalidEvent诊断。Scroll与accepted偏移允许负值；size/viewport要求局部x=y=0、有限非负尺寸，允许零和小数；ScrollAck的requested必须符合现有非负动作范围，accepted仍保留真实偏移。Anchor的within有限非负，单行ValueChanged/Commit拒绝CR/LF；多行值不因此禁用换行。旧revision仍按已有规则在App丢弃，未来版本/回退与跨后端规范化不在本批宣称完成。

Anchor/FocusWithin的嵌入来源检查runtime、generation、存活、逻辑祖先与非Group物理表示。Anchor来自严格后代，FocusWithin可来自目标自身或后代；来源已销毁/非终止状态不活跃则作为迟到消息忽略，不把复用slot当成旧行。活着但无关的来源报InvalidEvent，foreign报ForeignRuntime。Navigate始终要求from等于主目标。该检查只走已挂载祖先链，无数据源、远跳中间行或访问历史扫描；不证明OS真实发生过该输入，也不在无布局层猜测Anchor像素是否低于实际行高。

参考队列在相关清理/drain时重验来源，移动或删除后不送达陈旧观察；暂停保留存活节点的FocusChanged(false)/Composition(false)/FocusWithin(false)，Dispose仍丢弃。App派生FocusWithin时记录模型stats；前面的直接/祖先callback改变模型后才重验，避免直接focus handler删除来源仍让后续祖先拿到失效节点。正常路径收集一次祖先链，未新增每帧/每行缓存。

本批也修复App::pump的早退：之前route错误会丢掉已drain批次后面的输入和query response，等待者可能永久悬空。现在保留第一项route错误，继续按序处理该批输入，完成query回调与有效写入，再flush。flush错误优先，否则返回第一项route错误。单独dispatch保留直接返回自身错误的行为；没有mutation全批回滚或业务panic捕获。实际fake-host用例把坏Scroll、有效编辑、坏Commit和有效查询响应放入同批，确认编辑/程序revision生效、waiter归零及下一轮恢复。

新增6项框架测试：render的数值/单行值、嵌入来源/slot复用、队列移动与终止清理共3项；UI的同批恢复与直接焦点callback删除来源共2项；一亿行列表输入专项1项。后者180px视口、24px估值、overscan=2、row_limit=20、measure_limit=64，错误坐标/无关来源不增加key读取、节点、滚动动作或pin，正常测量/30次远跳后行≤13、测量≤64、索引capacity<4096、节点<64、图槽<256、累计key<5000，卸载归零。既有实际Web/Blitz滚动与像素用例继续保留，没有用此内存测试替换它们。

首轮导航回归发现“错误from已经retired”被来源通用检查提前忽略，已把Navigate的精确匹配放在该路径之前，保留原错误断言通过。新专项最初沿用120px fixture的12行假定；诊断显示180px在非整行偏移可相交9行，加双侧4行overscan为13，修正测试预算解释，未放宽row_limit或改变实现。原始失败日志保留。

新增event-acceptance.js在真实浏览器拒绝13种错误消息，坏消息callback=0、有效负scroll callback=1；真实slot复用后旧来源仍被忽略。接着24个查询正常完成、按钮和emoji编辑继续工作、卸载无DOM残留。它使用公开dispatch检查入口，实际drain批次恢复由独立fake-host测试证明，二者范围不同。原有十二组浏览器函数全部重新通过，共十三组；Python stream transport也通过。

最终Windows为223项框架运行、1项!Send编译失败、77引擎和1实窗，passed合计302；Ubuntu WSL为224+1+77+2=304。workspace严格Clippy、Linux default/paint、WASM workspace/fixture严格检查和重建全部通过；Android workspace all-features/all-targets编译通过。实际窗口仍是既有编辑/生命周期fixture，峰值14tasks/77引擎节点，本次key Windows=673、X11/Wayland=669/674。没有新增OS输入/辅助技术/Android设备或release耗时结论。

未新增依赖、改变锁文件或vendor，四包104文件来源检查继续通过。Cargo home/target/tmp、Linux loop0 backing及临时服务仍在D:，owned8788/8789按实际PID/命令核对后停止。复现为render/ui的events测试、ui list_widget的invalid_geometry用例，完整Windows verifier -Web -Window及Linux --window；浏览器十三组另执行。

原始记录在D:/BuildCache/glory-semantic-ui：event-contract-final-{windows,linux}.txt、event-contract-targeted.txt、event-contract-huge-list-r2.txt、event-contract-android-check.txt、event-contract-browser-2026-10-22.json、event-contract-stream-transport.txt；来源、源码、锁、二进制及日志哈希为event-contract-evidence-2026-10-22.json。R0继续输入版本/规范化、Focus结果回读、通用结构预检与其余props/无障碍映射；平台、Android、R7剩余观测/缓存及旧架构清理也仍在完整目标中。

## R0 输入版本、规范值与实际接管（2026-10-22）

本批补上上一节的文本输入版本与规范化。新的render::normalize_text在多行CRLF/CR→LF、单行主机输入移除换行、NUL→U+FFFD；已规范文本借用，无额外分配。程序单行声明仍先拒绝CR/LF，不能把错误声明偷偷过滤。Descriptor初始化/patch与Memory存储规范Input值，Web/HTML/Blitz复用该助手；业务Cage保留原始值，不因渲染被写回。规范等价输入会作为相等显示值确认版本，保留caret/preedit。

程序Set/Clear版本单调，相同版本只接受规范相同值，Keep忽略revision。InputRevision携带current/incoming，拒绝回退和同版本冲突；自动版本耗尽为InputRevisionExhausted，不panic/回绕。原builder的私有计数器会落后于显式Scope.patch，现在绑定从live节点读取下一版本，显式100后绑定101、有效输入后102，混用仍有序。失败写入不改变节点或消费版本，重挂新NodeId可建立新寿命。

Commit也改为{ value, revision }，不保留无版本tuple；Enter由Web/WindowApp捕获当时编辑器的程序屏障。ValueChanged与Commit小于当前版本都忽略，大于当前版本在callback前报错，等于才可消费。终止组合/焦点清理不因旧程序值丢失。SSR接管使用新客户端NodeId/版本，不把服务器计数器跨runtime复制。

浏览器原生探针确认type=email/url会裁剪两端空白，其余文本类型保留。因此Email/Url改为type=text + inputmode=email/url，Number维持decimal提示；七种purpose都保持字符串和选区接口，目的校验归业务。Password的原生呈现另见下方已确认缺口，不能由HTML类型表推断。

Web Entry读值修复非规范主机文本并保留UTF-16选区；平常规范值不设value。hydration验证阶段只计算规范观察，不修改当前DOM；全形状通过后才修复NUL并送入canonical事件。后续节点验证失败时，之前的原始dirty value/selection保持不变；修正后可重新接管同一元素。原生normalize也用于IME/paste与字节选区换算，避免NUL替换改变UTF-8长度后游标失效。

新增10项框架测试：render4项覆盖借用/规范存储、单调与Keep/Clear、拒绝后恢复、未来输入/规范事件；UI3项覆盖混合显式/绑定版本、business原值与规范model区别、耗尽/重挂恢复；WindowApp2项覆盖多行NUL/CRLF等价echo的选区和显式版本、实际IME协议及Email/Url空白保留；SSR1项覆盖purpose映射和首行CRLF textarea。既有receipt伪造测试将准备写入改为合法revision2，仍验证原不变/回退receipt故障；旧multi-CR事件断言更新为拒绝非规范生产者，LF事件仍接受。

新增input_form/input_client与input-acceptance.js。CSR验证七purpose字面值及八控件选区，100→101→102、旧/未来编辑和Commit不回调、正常Enter、NUL修复不跳光标、错误单行声明后恢复、合成composition期间CRLF/NUL等价回写保留选区。原生ssr_fixture生成inputs.html供真实HTTP读取；预输入含NUL/CRLF，先制造最后控件结构错误证明接管失败未规范或贴metadata，重试保留同一元素/焦点/双向选区并把dirty值写入客户端业务state。

最终十四组浏览器函数全部errors=[]，Python stream transport通过。Windows为233项框架运行、1项!Send编译失败、77引擎与1实窗，passed312；Ubuntu WSL为234+1+77+2=314。workspace all-features/all-targets严格Clippy、Linux default/paint、WASM workspace/fixture严格检查与重建通过，Android同范围编译通过。实窗仍为既有一亿行编辑/生命周期fixture，峰值14tasks/77引擎节点，累计key Windows=672，X11/Wayland=676/675。既有滚动/资源预算继续通过，无新增FPS/RSS或release时间结论。

额外原生CPU探针揭示尚未修复的Password呈现：320×60、固定monospace18px和相同input样式，Text("Wiim123!")与Password同值的像素完全相同（0差异通道），而两个同长度Password字符串图像不同；Text正向控制也确实改变像素。证明当前固定引擎实际绘制了明文，不是只缺一个API声明。探针位于D:/BuildCache/glory-semantic-ui/password-probe，使用本地真实Renderer/Raster，结果为input-contract-password-probe.txt。Native Password遮罩或明确拒绝列为下一项高优先级修复；不将HTML七purpose测试当成此能力交付，也不将R0/R5勾选完成。

没有新增第三方依赖或修改workspace/fixture锁与vendor，四包104文件来源检查继续通过。新增诊断crate只在D: BuildCache，不增加workspace成员。早期Commit变更的编译错误、fixture类型/导入及严格unused import失败日志均保留，修正后完整Windows gate重跑；Linux完整gate也通过。Cargo home/target/tmp、Linux backing、临时服务与探针全在D:，owned8788/8789结束后按PID/命令核对停止。

复现为render/ui的input测试、Blitz lib的canonical_multiline/email_and_url测试、SSR input_purposes用例；完整Windows -Web -Window、Linux --window，浏览器十四组另执行。最终日志：D:/BuildCache/glory-semantic-ui/input-contract-final-windows-r2.txt、input-contract-final-linux.txt、input-contract-android-check.txt、input-contract-browser-2026-10-22.json、input-contract-stream-transport.txt；源码、锁、来源、fixture/探针二进制与结果哈希见input-contract-evidence-2026-10-22.json。

下一步优先修复已证实的Native Password缺口，并继续后端能力预检、focus回读和其余props/无障碍映射。实际OS IME/辅助技术、Android宿主、R7剩余观测/缓存及旧架构清理保持在完整目标中。

## R0 描述符预检与原生 Password 能力边界（2026-10-22）

新增 Render::preflight(node, patch)。UI 创建在身份分配前调用；patch 使用包含未提交修改的当前语义描述符，在模型更新与命令入队前调用。检查只读本次配置，不允许查询宿主树、执行布局/I/O 或回调业务。默认后端无额外限制，实际 adapter 声明自己的约束；不是引入全树 diff 或新的事务协议。

HTML mapper 共用 text_tag/view_tag 与 preflight：冲突 Text 映射在创建前拒绝；TextKind、View Role、Input Multiline 变更若改变 tag，Scope.patch 立即返回需要重挂的 InvalidProps。相同 tag 的变化仍允许，例如 Inline 显式 p 与 Paragraph 互换、div 的 List/Status 互换。局部检查不创建完整 Shape、不克隆 Input/Text 内容或样式，也不遍历业务列表。直接 Mapper.apply 原有 shape/difference 检查继续执行。

固定 Blitz 的密码遮罩尚未实现。本批消除静默明文接受：新增 Password 与 Purpose 切换为 Password 都返回 Unsupported("Blitz password masking")。UI preflight 和直接 Renderer.apply 经 Mapper.apply_checked 共用拒绝规则，拒绝前不建立语义/引擎节点，不把全部 input capability 关闭。SSR/Web 保留 HTML Password。后续需要完整的原始值/遮罩索引投影与命中、caret、选区、滚动和 IME 几何，不能只在 paint 中换字形后宣称支持。

同时修复直接批次错误后的查询水位：Mapper.receipt 暴露最后成功前缀；SSR/Web/Blitz 即使后续操作被拒绝，也同步该水位。测试以 Create/Place 成功、后续配置失败为例，查询保持 applied=2，下一次合法更新得到 3。仍不承诺整个 batch 回滚，宿主执行错误后的 App fault 规则保持；本批没有完成跨树 Place/Resume 的通用 UI 预检。

新增八项运行测试：SSR 四项覆盖结构拒绝后的模型/队列、未提交描述符、同 tag 合法更新、错误选项不分配身份、既有绑定继续更新及前缀水位；Blitz 三项覆盖 Password 创建/更新、直接 apply 无法绕过、身份未消费、前缀查询/恢复及无布局/属性写入；WindowApp 一项验证拒绝 Password 期间保留焦点、UTF-16 选区与正在进行的组合输入，随后仍可 commit 与键入。浏览器 target-acceptance 扩展四项拒绝及同 tag 更新，检查同一 DOM 输入、活动焦点、双向选区与后续编辑。描述符拒绝与密码控件实现完成分别记录。

最终 Windows passed=320（241 框架运行 + 1 编译失败用例 + 77 固定引擎 + 1 实窗），Ubuntu WSL passed=322（242 + 1 + 77 + X11/Wayland 两次实窗）。严格 workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture 严格检查和重建通过；Android 同范围仅编译通过。十四组浏览器全部 errors=[]，Python stream transport 通过。原有一亿行/变高/资源回收回归保持：浏览器主列表峰值 33 行、84 节点、每 tick 34 次 key 读取；实际窗口峰值仍 14 tasks/77 引擎节点，累计 key Windows=677、X11=673、Wayland=671。未新增 release/FPS/RSS 结论。

源码、workspace 成员、依赖锁与 vendor 来源约束保持；没有新增第三方依赖或本地引擎修改。构建、缓存、临时服务和结果仍在 D:；初次测试误把 engine_id 的 Result 当 Option，修正后七项 targeted 测试及最终完整 gates 通过，失败日志保留。报告更新 v13，继续区分语义层、布局拥有者、实际渲染技术与 OS 宿主；不会用 native 总 feature 或空平台包模糊支持范围。

复现：glory-ssr/glory-blitz 的 preflight 集成测试、Blitz shell 的 rejected_password_configuration 用例；完整 Windows -Web -Window、Linux --window 和十四组浏览器。日志位于 D:/BuildCache/glory-semantic-ui/preflight-final-{windows,linux}.txt、preflight-targeted.txt、preflight-android-check.txt、preflight-browser-2026-10-22.json、preflight-stream-transport.txt，源码/锁/来源/二进制和日志哈希为 preflight-evidence-2026-10-22.json。

R0/R5 不因明确拒绝而勾选完成：原生密码遮罩、focus 结果回读、其余 props/无障碍与跨树预检仍需推进；真实 OS IME/辅助技术、Android Activity/Surface/资源/设备、R7 细分观测及收益驱动缓存、旧架构/工具清理仍在完整目标内。

## R0 实际焦点回读与请求拒绝恢复（2026-10-22）

上一批描述符预检已完成；本批继续实际焦点契约。先运行真实 WindowApp 回归，确认旧路径中隐藏控件的正常焦点拒绝会令 App Faulted，并丢掉同批后续动作，失败记录为 focus-readback-baseline.txt。Web 的 HTMLElement.focus 正常返回也不能证明实际聚焦，不能用这两个不同结果推断业务焦点。

新增 Query::Focus(NodeId) / QueryValue::Focus(bool)，复用身份/目标校验、异步响应、类型验证与接受时水位。Web 读 control 实际 Document/ShadowRoot.activeElement；Check 比较内部 checkbox；Blitz 在解析当前样式/布局后同步并比较 control。Group、SSR/Memory 与无交互 Blitz 明确 Unsupported。Focus 不等于 FocusWithin、OS 激活、IME 或显示完成；动作不推进 applied，同一水位的不同查询仍各自保存接受时的焦点值。

Action::Focus 规定为不主动滚动的请求，不能只凭动作被处理就认为目标持有焦点。Scope 入队前仍检查身份、Group、disabled 和 capability；入队后发生布局/启用变化，adapter 的 Unsupported 焦点拒绝作为未达成请求，不再 fault App 或丢弃后续动作。真正 Backend 错误与其他动作错误仍 fault，有独立回归。这里没有把拒绝转换成伪造 FocusChanged，也没有提前给不可聚焦目标创建焦点 pin；需要结果时显式查询。

新增五项运行测试：WindowApp 四项覆盖正常拒绝后后续动作/编辑、确切 control 与祖先差别、接受后转移焦点不改写旧响应、CSS 改变后的同步、同帧禁用与恢复，以及交互宿主的共用契约；UI 一项验证 Unsupported Focus 继续但真实后端/其他动作错误仍 fault。既有无效查询响应测试扩为10种载荷，验证 Focus bool 与错误类型。共用 conformance 为八组件32种查询，Focus/Selection 按宿主声明能力验证正向值；Memory/SSR/Blitz 文档和 WindowApp 均运行。

新增第十五组浏览器 focus-result-acceptance.js：八目标焦点矩阵、异步快照、普通 text/image 的 focus 请求未达成、programmatic View、Check 内控件、preventScroll、display:none/visibility:hidden/inert/opacity:0、visibility 子级覆盖、禁用与连接恢复、Shadow DOM 实际根以及清理。原有 target/event 矩阵同步为32查询；其他十三组业务/大列表/资源/接管/流式验收继续通过。Shadow 测试只证明本次焦点回读，不宣称全部 Shadow DOM/SSR/接管行为完成。

既有一亿行原生定高/变高列表的30次远跳另逐次查询离屏编辑器焦点，查询前后 key/source 读取数不变；保留同一物理编辑器、UTF-16 选区、一个 pin 与任务/节点预算。最终 Windows passed=325（246框架运行 + 1编译失败用例 + 77固定引擎 + 1实窗），Ubuntu WSL passed=327（247 + 1 + 77 + X11/Wayland两次实窗）。实际窗口峰值14 tasks/77引擎节点，累计 key Windows=672、X11=677、Wayland=672。没有新增 release/FPS/RSS 结论。

严格 workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture 严格检查和重建、Android 同范围编译通过。十五组浏览器全部 errors=[]；Python stream transport 通过。补充交互 conformance 后完整 Windows/Linux gates 重跑，最终浏览器测试的 WASM 与 stream 二进制哈希未变。未添加第三方依赖、修改锁、workspace成员或四包104文件 vendor 来源；所有构建与证据在 D:。验收服务按 PID/命令验证后停止。

复现：Blitz shell 的 focus_tests、UI 的 query_barriers、各端口 targets，以及完整 Windows -Web -Window、Linux --window 和十五组浏览器。最终日志为 D:/BuildCache/glory-semantic-ui/focus-readback-final-{windows,linux}-r2.txt、focus-readback-native-final-targeted.txt、focus-readback-ui-targeted.txt、focus-readback-android-check.txt、focus-readback-browser-2026-10-22.json、focus-readback-stream-transport.txt；来源、源码/锁、日志和二进制哈希为 focus-readback-evidence-2026-10-22.json。架构报告更新 v14。

基础焦点回读项已有上述证据，R0/R5 与完整目标仍未完成：Native Password 继续明确拒绝直至遮罩/编辑索引与几何整体实现；全部 props/角色/名称/无障碍、跨树预检、Portable stylesheet、真实 OS IME/辅助技术、Android 宿主与设备、R7 细分观测及收益驱动优化、旧架构/工具清理仍继续推进。

## R0 原生 Password 的显示、编辑与几何投影（2026-10-22）

本批实现上一节仍明确拒绝的原生 Password。原始编辑器继续拥有值、UTF-8 选区和组合输入；独立显示编辑器按 ICU 扩展字素簇边界生成每簇一个圆点，并保存双向索引映射。绘制、命中、caret、选区、水平滚动和 IME 区域都使用圆点布局，再将编辑位置映回原文。显示编辑器不克隆原文缓冲或原文字形布局；配置更新只复制样式等参数，generation 未变时复用投影，单独选区更新复用圆点布局。它只属于存活输入控件，没有按访问历史缓存列表行。

方向键、Home/End、拖选与点击使用显示坐标；删除覆盖完整字素簇，组合字符、ZWJ emoji 等不会被按圆点的 UTF-8 字节偏移误删。Password 的单词移动/双击按整字段处理；Copy/Cut 不暴露原文，Paste 仍可编辑。预编辑只给对应圆点加组合标记，隐藏 caret 的组合状态也保留。引擎新增回归同时发现并修复原有 IME Commit 未清理 compose 而重复保留预编辑文本的问题。Text/Password 在同一控件切换时保留原始值、身份、焦点和选区，移除此前的临时 Unsupported 拒绝；其他 HTML 结构预检与成功前缀水位不变。

宿主接口新增 Normal/Password 输入目的；winit 声明对应能力、初始目的与 cursor area，目的/区域相同时不重复提交，未发送 surrounding text。布局后仅对当前焦点控件修正滚动与 IME 区域，没有扫描整个输入树或列表。实际系统输入法是否遵守目的提示、辅助技术如何处理密码以及 Android 宿主仍需专门验收，本批的 ShellProvider 捕获与合成 IME 协议不替代这些证据。

新增六项引擎测试与四项框架测试。CPU 像素比较覆盖 Latin/CJK/组合字符/家庭 emoji 的八字素簇密码与八圆点文本相同，明文正向控制及一/八圆点差异成立；同控件切换和受控更新保留身份。原来揭示明文缺口的独立探针源码不变，当前输出 password_equals_plaintext=false、equal_length_passwords_equal=true，与同值明文相差 1239 个通道。实际指针事件、UTF-16 选区、替换、双击、预编辑区域、DPI=2、水平滚动及缩短值后归零均有回归。引擎还覆盖 Apple 编辑命令与剪贴板边界。

一亿行离屏输入用例扩为定高/变高 × 普通/密码四组，每组30次远跳，保留同一物理编辑器、caret、preedit 和最多一个焦点 pin；Focus 查询不增加源/key 读取。实际窗口 fixture 的变高列表改用 Password，定高列表保留普通输入。最终 Windows passed=335（250框架运行 + 1编译失败用例 + 83固定引擎 + 1实窗）；Ubuntu WSL passed=337（251 + 1 + 83 + X11/Wayland两次实窗）。窗口峰值14 tasks/77引擎节点，累计 key Windows=671、X11/Wayland=673/673。没有新增 release 耗时、FPS 或 RSS 结论。

严格 workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture 严格检查及重建通过。十五组浏览器全部 errors=[]，Python stream transport 通过；Android 同范围仅编译通过。最终完整检查在移除显示编辑器原文克隆后重新执行。早期类型、IME 重复提交、区域断言及局部编辑错误的失败日志保留，均在最终完整检查前修复。

四个固定引擎包仍使用原锁定上游版本，显式维护14个引擎源码路径，新增 password.rs 后来源记录共105文件；补丁和来源校验通过，无新增 unsafe、第三方依赖、workspace 成员或锁文件变动。构建/缓存/探针/临时服务与证据在 D:，owned8788/8789按 PID/命令核对后停止。报告更新 v15，契约和后端 README 同步支持边界。

复现：引擎 node::password 测试、Blitz 的 password 像素集成测试、editing_tests 中的 password 指针/IME/purpose 用例、focus_tests 的四组大列表，以及完整 Windows -Web -Window、Linux --window 和十五组浏览器。最终日志位于 D:/BuildCache/glory-semantic-ui/password-final-{windows,linux}-r3.txt、password-probe-final-r3.txt、password-android-check.txt、password-browser-2026-10-22.json、password-stream-transport.txt；来源、源码/锁、日志与二进制哈希见 password-evidence-2026-10-22.json。

原生 Password 的上述显示/编辑投影已交付；完整目标继续推进：R0 其余 props/角色/名称/无障碍映射、跨树预检与 Portable stylesheet，真实 OS IME/辅助技术、Android Activity/Surface/资源/设备，R7 细分观测与收益驱动优化，以及旧架构/工具清理。

## R0 创建与跨树放置预检、流式 SSR 契约补齐（2026-10-22）

本批先用真实 SSR App 复现按钮内 Inline text 显式映射 p 被 UI 接受，返回已分配 NodeId，直到提交才由 HTML mapper 拒绝的问题。通用语义错误也曾先创建节点，再因父节点是 leaf、button 内容非法或父节点已销毁而失败。新增 Render::preflight_place，借用 App 当前 Memory，Subtree::New 借用无身份描述符，Existing 指向存活逻辑范围。先执行共享 child/placement 校验，再检查 adapter 的内容规则；创建在分配身份前，移动/恢复在修改层级、暂停状态或输出队列前完成。

预检看到尚未提交的祖先、Group 和描述符修改，不能读 adapter 的已提交树、执行布局/I/O 或业务回调。HTML 共用只读助手供同步 SSR、Web、Blitz 和直接 Mapper.apply 使用，现有按钮段落限制与 Group Place/Resume 保持一致。拒绝不消耗身份或改变模型/队列，旧父节点 slot 复用也不能将存活 scope 的孩子误挂到新节点。后续合法创建、绑定更新、移动/恢复可以继续。多根 widget 的多项操作及直接 apply 批次仍不承诺原子回滚，宿主实际失败保留 fault 契约。

这里不创建整树快照、不克隆文本/样式、不访问业务源或保留访问历史。普通列表移动沿祖先链；受限制的交互/button 目的地才检查被移动的存活子树，包含暂停后代。共享与 adapter 校验可能各走祖先链，尚无新增 release 耗时收益结论。现有一亿行固定/变高、普通/密码输入、焦点 pin、远跳与清理回归继续执行。

适配器总检查发现渐进 SSR 的私有 Renderer 没有自动继承同步 SSR 的能力预检，也未在直接批次的后续项拒绝后同步成功前缀水位。分别运行失败回归后修复：接入两项预检，拒绝后保留 Mapper 的成功水位；事件注册、动作和查询目标也复用共享校验，八组件/32查询 conformance 单独覆盖此私有端口。它仍没有物理焦点/输入能力。不能用同步 SSR 的测试代替流式 Renderer 的验收。

新增八项框架运行测试：UI 两项创建/旧父节点与一项 staged Place/Resume 拒绝恢复，SSR 一项嵌套按钮创建，Blitz 一项无引擎工作/身份消费的创建，以及渐进 SSR 的预检、前缀水位与目标契约三项。HTML 原有 Group 移动/恢复测试扩展为直接检查只读助手、receipt 和 sink 均不变。浏览器 target-acceptance 的拒绝矩阵从4扩为6，保留活动输入的同一元素、双向选区、组合状态及后续编辑。

补齐私有流式端口后重新执行完整 checks。最终 Windows passed=343（258框架运行 + 1编译失败用例 + 83固定引擎 + 1实窗），Ubuntu WSL passed=345（259 + 1 + 83 + X11/Wayland两次实窗）。窗口峰值14 tasks/77引擎节点，累计 key Windows=675、X11=666、Wayland=673。浏览器十五组全部 errors=[]，主列表仍最多33行/84节点、每 tick 34次key读取；变高列表最多28行、64条测量记录。严格 workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture 严格检查和重建通过；Android 同范围仅编译通过。十五组浏览器所用 WASM 在最后重建后哈希不变；受私有 SSR 修改影响的 stream fixture 重新编译后，流式浏览器和 Python transport 另外重跑通过。

没有新增依赖、workspace成员、unsafe或修改锁文件/四包105文件引擎来源。所有构建/缓存/临时服务/结果在 D:，两轮验收服务分别按 PID/命令验证后停止。初期测试的 Widget 导入、raw create 清理假定、stream 测试访问 API 与临时 scope 寿命问题均修正，原始失败日志保留；最终完整检查之后只有文档/证据整理。没有新增 FPS/RSS 或 release 性能结论。

复现：glory-ui 的 preflight 与 placement_tests、SSR/Blitz 的 preflight 集成测试、HTML content、SSR stream 单元与 conformance；完整 Windows -Web -Window、Linux --window 和十五组浏览器。最终日志位于 D:/BuildCache/glory-semantic-ui/tree-preflight-final-{windows,linux}-r2.txt、tree-preflight-android-check-r2.txt、tree-preflight-stream-targeted-r2.txt、tree-preflight-browser-final-2026-10-22.json、tree-preflight-stream-transport-r2.txt；源码、锁、来源、日志和二进制哈希见 tree-preflight-evidence-2026-10-22.json。报告更新 v16，契约与 AGENTS 同步。

已补上当前父子规则的预检入口和具体缺口；R0 全部 props/角色/名称/无障碍与扩展行为仍需审计。Portable stylesheet、真实 OS IME/辅助技术、Android 宿主与设备、R7 细分观测/收益驱动优化以及旧架构/工具清理继续保留在完整目标中。

## R0 有限角色与名称、清除回退和实际 AX 验收（2026-10-22）

本批检查发现所有有盒子 builder 都能使用结构 Role，低层也允许 Input 等被覆盖为 List；普通 Text 与被隐藏的装饰 Image 同样接受 name。先运行失败回归，再将有限词表约束放入 render：结构角色只给 View，builder 也仅 View 暴露 role。增加 Role::Group，Descriptor::effective_role 给有 name/无显式角色的 View 返回 Group；无名 View 保持 generic，显式角色优先。HTML mapper 复用它，不改布局标签或物理身份。

Plain Inline/Paragraph Text 通过 content 表达文字，只有 Heading 接受作者 name。装饰性 alternative=None 的 Image 拒绝 name 和显式 focus，Focus 动作在入队前返回 Unsupported；有说明文字的 Image 仍可命名/显式聚焦。Name Clear 移除作者配置，恢复可见标题、控件 label/content 或图片 alternative 的宿主命名算法；Input 可无作者名称。Check 的 name 属于内部 checkbox，不挂在外层 label。Keep 与 Clear 不再含混，隐式 group 角色随 name 清除，显式 Role::Group 则保留。具体元数据矩阵及标准依据见契约页与报告 §6.14，未承诺空作者名称强制覆盖宿主名称算法。

新增九项框架运行测试：render 三项覆盖六类非 View 的八种角色拒绝/原子恢复、Text/Image 命名依赖、effective role 与装饰焦点；HTML 三项覆盖默认映射、真实 typed edit 的 Keep/Set/Clear 与稳定 key、控件命名不替换值或内容；UI 一项证明错误创建/覆盖不消费身份、队列不变且既有 Cage 绑定继续更新；SSR 一项验证实际输出与清除；WindowApp 一项验证 name 更新/Keep/Clear 保留值、选区、preedit 与物理 control，并用实际引擎键盘事件验证 programmatic/默认 Tab 恢复。原有 SSR 结构拒绝后的恢复用例改用 Text.content 更新普通文本，避免依赖已禁止的无效命名配置。

新增 semantics_form/semantics_client 与第十六组 semantics-acceptance.js。相同纯 Rust form 在 CSR 与原生生成的 semantics.html 中运行；SSR 预输入后接管保留同一元素/焦点/双向选区。Chromium 实际 AX 树分别记录16个初始/更新角色名称及15个清除后的节点，覆盖分组、结构区域、标题、控件、图片、链接、List/ListItem。清除后恢复可见内容名称，显式 group 仍保留；Keep 的 MutationObserver 记录为零，name/role/focus/item 更新的 value setter 计数为零。真实 Tab 验证 programmatic 输入被跳过及 Clear 后默认行为恢复；每种入口拒绝15项错误后继续命名和键入，最后卸载归零。浏览器原焦点用例相应验证装饰图动作被提前拒绝，实际 Focus 查询仍返回 false。

最终 Windows passed=352（267框架运行 + 1编译失败用例 + 83固定引擎 + 1实窗），Ubuntu WSL passed=354（268 + 1 + 83 + X11/Wayland两次实窗）。严格 workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture 严格检查和重建通过；Android 同范围仅编译通过。浏览器十六组 errors=[]，Python stream transport 通过。亿级列表保持：主浏览器最多33行/84节点、每 tick 34次key读取；实窗峰值14 tasks/77引擎节点，累计 key Windows=675、X11=672、Wayland=669。新规则只读取当前描述符的枚举/可选名称，不增加每行缓存或数据源扫描；没有新增 release 耗时、FPS/RSS 结论。

没有新增依赖、workspace成员、unsafe或改变锁/四包105文件引擎来源。所有构建/缓存/fixture/证据在 D:，临时8788/8789按 PID/命令核对后停止。最初三项行为失败记录保留；最终完整检查后仅补 Rust 文档注释和文档，浏览器用例追加 AX 结果导出后单独重跑通过。Browser AX/引擎属性和输入协议不证明系统屏幕阅读器播报、OS IME 或 Android 实机支持。

复现：render/html/ui/ssr 的 semantics 集成测试、Blitz editing_tests 的 semantic_metadata、完整 Windows -Web -Window、Linux --window，以及十六组浏览器。最终日志为 D:/BuildCache/glory-semantic-ui/semantics-final-{windows,linux}.txt、semantics-android-check.txt、semantics-native.txt、semantics-browser-2026-10-22.json、semantics-stream-transport.txt；源码/锁、来源、日志、AX 数据和二进制哈希见 semantics-evidence-2026-10-22.json。报告更新 v17，契约、README 与 AGENTS 同步。

有限角色/name/focus/item 的当前矩阵已有上述证据；其余属性/样式与扩展行为仍按 R0 审计。真实 OS IME/辅助技术桥与播报、Android Activity/Surface/资源/设备、Portable stylesheet、R7 细分观测和收益驱动优化、旧架构/工具清理继续保留在完整目标内。

## R7 存活字段更新与 HTML 单字段 lowering（2026-10-22）

本批先测量而后修改。旧路径对单项 Patch 克隆完整 Descriptor；HTML 再构造前后完整 Shape，Input 的元数据更新还会重复规范化/版本比较未变的大文本。D:缓存目录内独立诊断包固定源码/锁，测量32字节/1 MiB、Input/Heading、Memory App/SSR App/直接Mapper、Name Set/Keep，20次预热、500次计时、三次重复共72样本。旧实现每样本500次都会更换原载荷缓冲；1 MiB Input Name Set中位数约4.082ms、10.952ms、8.571ms，证明成本随无关内容增长。

render新增Memory::prepare_patch/PreparedPatch，针对已规范化、验证的存活节点，仅交换本次字段并保存逆向值，扫描被触及载荷并执行跨字段与祖先内容检查；错误或guard Drop/展开恢复原字段，stats/水位不变。commit接受一项修改；Memory::apply复用相同语义。未触及文本、class缓冲、style和options不因元数据更新克隆/重扫；任意原始Descriptor的patch/patches继续完整构建期校验，未对外暴露可绕过初始化约束的可变节点。

HTML字段lowerer读取已验证的新描述符与旧字段，生成相应edits，不再生成两份完整Shape；creation/snapshot/hydration仍用full shaping。有限purpose/role映射由相同助手共享。Value新版本即使内容相同仍保留Revision和property屏障，Keep忽略版本；CSS/user-select、布尔复位、内容规范化、Check内部control、隐式命名group和结构重挂规则保持。sink仍在接受后执行，整体批次/宿主错误继续按成功前缀与fault契约处理，没有增加全批事务。

新增六项运行测试：render四项覆盖1 MiB value与500个class的缓冲保持、guard Drop/展开/commit水位、错误ancestor与revision恢复、全部Patch种类对完整Descriptor验证；HTML两项覆盖1,232组原完整Shape差分oracle与大载荷指针保持。oracle430接受/802拒绝：接受的typed edits及顺序相同，拒绝不改模型、sink、stats或receipt；规范化、Keep/Set/Clear、十八类字段、样式带符号零与ARIA依赖均覆盖。旧difference仅保留在cfg(test)，生产patch不再调用。最初测试误用Style字段为builder方法的编译失败日志保留，修正后全部检查通过。

同一独立探针复测：所有72样本的500次载荷更换均降为0。1 MiB Input Name Set中位数约0.318µs（Memory App）、0.402µs（SSR App）、0.210µs（直接Mapper）。亚微秒数据接近计时/调用开销下限，且不含布局/paint/present；不据此宣称FPS或通用提速倍数。属性编辑计数只有直接Mapper路径有效，其余路径只观测模型指针，不以未采集的零值证明宿主行为；真实DOM的value写入与编辑状态另由十六组浏览器覆盖。实际修改的字段仍承担必要的规范化/拥有数据复制。

另对原semantic_frames基准在修改前、后及复测各跑24组。无profile远跳中位数µs/帧：百万367.38→383.40（复测359.58）；一亿363.11→394.51（复测367.32）。首轮较慢数据保留，复测与基线重叠，尚无稳定远跳耗时收益/归因结论。全部远跳33000次key/1000帧、68节点/111图槽；同窗口0次key、62节点/102图槽，百万到一亿预算不增加。包含阶段不能相加，参考解释器基准不替代实际浏览器/原生绘制验收；详见列表基线新增节。

最终Windows passed=358（273框架运行 + 1编译失败用例 + 83固定引擎 + 1实窗），Ubuntu WSL passed=360（274 + 1 + 83 + X11/Wayland两次实窗）。严格workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture严格检查和重建通过；Android同范围仅编译通过。十六组浏览器全部errors=[]，Python stream transport通过。主浏览器仍最多33行/84节点、每tick34次key读取；实窗峰值14tasks/77引擎节点，累计key Windows=673、X11=675、Wayland=673。

项目依赖、workspace成员、锁与四包105文件引擎来源均未改动，无新增unsafe。诊断包只在D:缓存且前后自身源码/锁固定，未成为workspace成员。所有构建/缓存、临时服务、基准与结果在D:，8788/8789按PID/命令核对后停止。报告更新v18，契约/列表基线/AGENTS同步。

复现：render的field_patch集成测试、HTML lib的patch_tests，完整Windows -Web -Window、Linux --window和十六组浏览器；workspace release基准为cargo bench --locked -p glory-ui --bench semantic_frames。最终日志位于D:/BuildCache/glory-semantic-ui/field-patch-final-{windows,linux}.txt、field-patch-android-check.txt、field-patch-browser-2026-10-22.json、field-patch-stream-transport.txt；全部微基准、原/后/复测及汇总为field-patch-{baseline,after}.txt、field-patch-list-{baseline,after,recheck}.txt、field-patch-performance-2026-10-22.json；来源/源码、环境、二进制与数据哈希见field-patch-evidence-2026-10-22.json。

本批优化已选中字段的验证与消费，未完成Cage内部字段级依赖、引擎style/文字/box/光栅细分或新的paint缓存。R0实际引擎属性/样式审计、Portable stylesheet、真实OS IME/辅助技术、Android宿主与设备、R7剩余优化与旧架构/工具清理继续保留在完整目标内。

## R7 重复失效传播去重与读取后重开（2026-10-22）

本批重新核对源读取，确认Cage::map已借用源T、可选相等门控也已有效；未重复发明借用/lens API。实际缺口在State::invalidate：每次源写入都创建visited并遍历全部后代，队列去重不能避免图遍历。新增失败回归先证实64菱形分支、1000写入产生257000次marked，而需要257次；原始失败记录保留。修改前另保留完整runtime.rs及独立release探针可执行文件，再测前后，未将诊断包加入workspace。

每个活节点新增已传播标记；存活源每次publish仍增加version/source_writes，已转发的前沿不再重复展开，内部也不另建visited集合。开始求值重开当前及旧依赖前沿，登记依赖/完成求值/展开失败重开刷新的路径。这样不把Check/Dirty当作剪枝充分条件，批次中途get之后的新写入、晚订阅、Effect计算中回写与异常重试仍可达。标记随节点代次初始化/移除，不缓存所有访问过的行；Source写入闭包、可选相等门控与执行队列语义保持。marked只代表实际传播工作，不能代替source_writes。

新增六项核心运行测试：非Clone的1 MiB模型与菱形前沿257000→257、批次中间读取/动态依赖重开、未观察写入后的新订阅与300轮owner/slot复用、共享源菱形的局部读取、Effect回写/异常重试，以及1000组确定性混合批次对普通函数求值。已有源写入panic、循环诊断、动态直接依赖、任务/清理和10k owner回收继续通过，完整runtime集成为31项。没有引入字段级Store、替换Cage/Bond/Lotus或要求所有值可比较。

独立release诊断在前后使用相同源码/锁，各24组：1/1000次源写入、32/256菱形分支或百万/一亿行窗口，每组20逻辑批次、三次重复，构造/初始flush在计时外。图与窗口前后已采集的source_writes、derived/binding runs、节点/边/key等语义计数逐样本相同，只有marked与耗时改变。256分支1000写入中位数µs/批次34777.83→88.78，20批marked20500000→20500；一亿行1000写入219.54→126.52，marked20558→578；一亿行单写入118.98→101.75，marked均578。窗口每20批均280次key、最多30节点/54图槽。数据仅证明这个Runtime/UI参考后端工作负载，不含layout/paint/present、FPS或RSS；全部样本及v18不稳定远跳结果继续保留。

浏览器profile-acceptance新增dirty_burst：一亿行连续1000次位置写入，记录source_writes=1002（含内部发布）、marked=71、只读最终33个key、逻辑offset=189865944，最终33行/86节点。最初将另一fixture状态的84节点上限复用在当前初始88节点的页面，导致断言失败；诊断确认偏移/key正确后，改按本fixture最多34行及一行两个节点相对预算检查，保留原失败/诊断。随后停用/重置profile及卸载继续通过。

最终Windows passed=364（279框架运行 + 1编译失败用例 + 83固定引擎 + 1实窗），Ubuntu WSL passed=366（280 + 1 + 83 + X11/Wayland两次实窗）。严格workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture严格检查与重建通过；Android同范围仅编译通过。浏览器十六组errors=[]、Python stream transport通过。实窗峰值14tasks/77引擎节点，累计key Windows=679、X11/Wayland=678/678。其他浏览器列表、资源、输入、接管和已实现AX行为回归保持。

项目依赖、workspace成员、锁、四包105文件vendor均未改动，无新增unsafe。所有构建/缓存、诊断源码/原及新可执行文件、临时服务与证据在D:；8788/8789按PID/命令核对后停止。报告更新v19，契约/列表基线/AGENTS同步；没有增加平台IME/辅助技术或Android设备完成声明。

复现：glory-core的runtime测试（六个新增传播/读取/生命周期用例）、完整Windows -Web -Window、Linux --window和十六组浏览器。最终日志为D:/BuildCache/glory-semantic-ui/dirty-final-{windows,linux}.txt、dirty-android-check.txt、dirty-runtime-targeted.txt、dirty-browser-2026-10-22.json、dirty-stream-transport.txt；原/后源码与二进制分别是dirty-runtime-{before,after}.rs、dirty-probe-{baseline,after}.exe，基准dirty-{baseline,after}.txt及汇总dirty-performance-2026-10-22.json。来源/源码、环境、日志/数据和二进制哈希见dirty-evidence-2026-10-22.json。

完整目标继续：R0实际引擎属性/样式及扩展审计、Portable stylesheet、真实OS IME/辅助技术桥和播报、Android宿主/资源/设备、R7字段级响应依赖与细分/收益驱动缓存、旧架构/工具清理。此次合并的是失效传播，不是这些剩余能力的完成替代。

## R0 Blitz 原生 inline profile 的实际滚动校验（2026-10-22）

本轮继续R0实际能力审计。原生Renderer过去将Profile::HTML改名为blitz，所有有限值都被接受。先用独立D:诊断包通过同一App/Renderer、真实布局和异步Bounds/Scroll查询确认：sticky的首项在90px滚动后y=0→-90；自动锚定场景，上方块100→200px，锚项y=-10→90而scrollTop保持110。引擎解析了CSS，但未执行这两种滚动行为。修复前五项测试中四项失败，既有relative几何对照通过；原始布局日志和失败记录保留。

现公开glory_blitz::STYLE_PROFILE（blitz v2，Css::Html），仅从原范围排除Sticky、AnchorAuto；AnchorNone继续接受，以保留Glory虚拟列表的手动锚点。App缓存的支持声明、无宿主查询的preflight和直接apply使用同一常量。Create/Style Set先数值/支持检查，direct Patch先验证节点；Keep/Clear及无关字段不扫描旧样式或大文本。direct apply按操作验证，拒绝不抹掉已接受的前缀，收据/异步查询水位和后续恢复保持。未新增样式解析器、布局树、行历史缓存或依赖。

七项回归包括profile全部property/value查询、拒绝前identity/queue/layout保持、staged patch/input值和物理身份、direct前缀/创建拒绝/查询恢复、实际relative坐标，以及五种overflow双轴滚动和交互WindowApp的焦点/值更新。Visible/Clip不可程序滚动，Hidden/Auto/Scroll实际offset和子项坐标变化一致。WindowApp测试没有创建系统窗口；下述单独实窗gate才证明窗口链路。测试初版固定值更新revision=1与已有初值版本冲突，已改为读取当前revision+1，保留失败日志；未放宽输入版本约束。

最终Windows passed=371（286框架运行 + 1编译失败用例 + 83固定引擎 + 1实窗），Ubuntu WSL passed=373（287 + 1 + 83 + X11/Wayland两次实窗）。严格workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture严格检查与重建通过；Android同范围仅编译通过。三类实窗峰值14tasks/77引擎节点，累计key为669/675/679，包含原有一亿行定高/变高/导航/编辑/生命周期场景。数据不证明吞吐或FPS改善。

本轮仅改原生生产代码；HTML mapper、浏览器/SSR/stream、响应运行时与列表代码未变。Web检查执行严格编译/重建；上轮十六组浏览器记录和stream transport按未变源码及WASM二进制哈希引用，本轮没有重新执行这些浏览器脚本或传输服务。原始CSS探针修复后再次执行，几何相同、profile版本由1变2，明确证明本次交付是能力拒绝，不是宣称已经实现sticky/自动锚定。原始set_css继续是引擎author CSS，未新增Portable stylesheet能力。

构建/缓存/诊断及证据全部在D:。本轮未启动HTTP或流服务；检查/探针进程均已终止。依赖、workspace/锁及四包105文件vendor未变，无新增unsafe。架构报告v20、契约、列表基线与README/AGENTS同步。R0全部属性/上下文/扩展、Portable文本、真实OS IME/辅助技术、Android宿主/资源/设备、R7字段依赖/性能细分/缓存及旧代码清理仍继续。

复现：cargo test --locked -p glory-blitz --all-features --test style_profile；完整scripts/check-semantic-ui.ps1 -Web -Window、Linux scripts/check-semantic-linux.sh --window；Android --workspace --all-features --all-targets --target aarch64-linux-android。D:/BuildCache/glory-semantic-ui/native-style-final-{windows,linux}.txt、native-style-android.txt、native-style-targeted-final.txt记录最终结果；native-style-probe/{Cargo.toml,Cargo.lock,src/main.rs}、native-style-probe-{before,after}.txt和native-style-document-before.rs保留诊断及原始实现。完整源码、日志、二进制和未变依赖/vendor校验见native-style-evidence-2026-10-22.json。

## R0 最大尺寸的独立类型、样式表覆盖与实际布局（2026-10-22）

延续实际样式审计，旧max_width/max_height使用Option<Length>，将Length::Auto视为合法并输出max-width:auto/max-height:auto。独立诊断在Glory validate通过的情况下测得：声明200×100px的盒子仍受class的80×40px上限约束。Chromium也拒绝这两个auto声明，inline max为空、computed max为80px；原样式域/formatter与诊断源码保留。该问题依据CSS Sizing最大尺寸语法修正，报告§8.9给出规范链接。

现在使用短名Limit::{None,Px,Percent}；两项max字段为Option<Limit>，Style新增成对min/max builder，Glory prelude导出Limit。Length::Auto不能再传入max，compile-fail用例固定边界。Option::None表示没有inline声明，Some(Limit::None)显式取消class上限；数值仍要求有限/非负，min大于max合法并交由引擎执行min优先。HTML直接生成正确none/px/%，清除Style后恢复class规则；不引入兼容别名、额外布局树、DSL、wire字段或serde依赖。HTML/RECORD profile升v2，Blitz升v3；CSS/BASE_CSS与snapshot格式形状未变，Blitz继续拒绝Sticky/AnchorAuto。

两个新增Style单元测试覆盖两个轴、合法/非法数值、None/Px/Percent与profile property/value拒绝。HTML原差分矩阵增加六种Limit组合，最终1316项（469接受、847拒绝）；拒绝保持模型、输出、stats与水位。新增实际Blitz布局用例验证200×100取消上限、60×30像素、400×300包含块中25%/20%=100×60、min140×70覆盖max60×30、Style Clear回到80×40及非法值之后恢复，物理身份不变。独立探针改为Limit::None后输出none，实测200×100；前后源及记录保留，不用于性能对比。

第十七套size-acceptance使用同一Rust form生成CSR与原生SSR快照。两种入口均完成七步尺寸变化、Keep零style写入、每种入口六项非法值零style写入、恢复与卸载。SSR接管保留预输入/输入与盒子身份；所有尺寸更新后焦点、值与双向UTF-16选区保持，再次键入正常。百分比场景有确定的父宽高；测试不宣称覆盖所有不确定尺寸/内在布局/字体和CSS组合。

最终Windows passed=376（290框架与正向doc-test + 2编译失败用例 + 83固定引擎 + 1实窗），Ubuntu WSL passed=378（291 + 2 + 83 + X11/Wayland两次实窗）。严格workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture严格检查与重建、Android同范围编译通过。十七套浏览器errors=[]与Python流传输本轮重新执行通过；流fixture也已重新链接当前源码。实窗峰值14tasks/77引擎节点，累计key673/672/676，原亿级窗口预算继续通过；无吞吐/FPS/RSS改进声明。

首次Windows gate的框架/实窗检查通过，但新WASM fixture误用了UI的Result别名而编译失败；已改为本模块JsValue结果别名，最终完整gate重新通过，保留初次日志。未放宽任何类型、版本或布局断言。Rust生产修改在最终gate前固定，之后只补文档/证据。项目依赖、workspace/锁和四包105文件vendor未变，无新增unsafe；所有构建/缓存/诊断/日志在D:。测试服务按PID/命令验证后停止，8788/8789无监听；执行会话已回收。

复现：cargo test --locked -p glory-style -p glory-html；cargo test --locked -p glory-blitz --test limits --test style_profile；完整Windows -Web -Window、Linux --window及Android --workspace --all-features --all-targets --target aarch64-linux-android；运行size-acceptance.js与既有十六套浏览器。D:/BuildCache/glory-semantic-ui/limit-final-{windows,linux}.txt、limit-android.txt、limit-oracle.txt、limit-browser-2026-10-22.json、limit-stream-transport.txt记录最终结果。limit-style-before.rs/limit-html-before.rs、limit-probe-before.rs、limit-probe/{Cargo.toml,Cargo.lock,src/main.rs}、limit-{baseline,after}.txt和limit-browser-diagnostic.json保留实际差异；完整来源/源码/日志/二进制哈希见limit-evidence-2026-10-22.json。

报告更新v21。R0其余样式/上下文/后端扩展、Portable文本验证、真实OS IME/辅助技术、Android宿主/资源/设备、R7字段依赖与性能细分/收益驱动缓存、旧架构/工具清理仍按完整目标推进；这一尺寸域修复不将整体重构勾为完成。

## R5 可选 AccessKit 语义树、动作桥与 Windows UIA 客户端（2026-10-22）

本轮推进原生平台边界，新增glory-blitz/accesskit和门面blitz-accesskit，接入AccessKit 0.25.0/accesskit_xplat 0.2.0；同一包内按accessibility/accesskit_platform模块组织，不新建native/OS占位crate，不改core/render/UI或HTML/SSR协议。上游接线尚未处理ActionRequested，且物理节点树不包含完整Glory集合语义，故使用Glory的已提交语义树、实际控件和引擎几何构建平台树，没有开启那条上游窗口接线。

平台对象在首次显示窗口前创建，保留调用者可见性意图。外部线程只投递WindowId+RuntimeId请求，主循环先poll/提交，再验证当前目标并处理Focus/Click/字符串SetValue。Click沿用Activate与Check一次CheckedChanged；值更新沿用规范化、组合结束和当前revision事件，业务回调由UI循环消费，借用adapter时不重入。失效、隐藏、禁用或不支持请求不破坏后续应用。关闭先释放adapter，平台对象也持有窗口引用直至自身释放；Surface暂停不因此Dispose语义树。

树采用每文档单调、不回绕的可访问ID，与物理slot解耦；仅保留存活Main/标签映射及上一份树，删除/暂停/隐藏时清理，停用客户端释放快照但不重置计数。Group透明、装饰图像省略，角色/name/Heading/Status/Alert、实际input值和check状态导出；Password只输出保护角色，不读入原始value。Button/Link标签与逻辑子内容交给正常后代命名路径。根transform只做一次DPI/安全区转换，控件Bounds使用既有自身scroll补偿。

客户端未激活时不建树，revision/窗口配置不变时跳过；正常更新重建当前活树并只提交不同节点和焦点。首次初始同步与几何失效有独立状态，移动/改尺寸不强制完整树。几何成本仍取决于活跃内容和祖先深度，不把缓存条目界限称为总RSS上界。AccessKit索引为零基，已知一致总数在最近List容器；未知/不一致不伪造total。Windows UIA的一基i32位置边界另有回归，超出范围省略，避免固定SDK在i32::MAX加1溢出；不是把逻辑行号截断为最大值。

六项通用回归覆盖有限角色/名称/密码/总数、真实consumer增量解析、Actual控件值、动作排队后才调用业务、双轴几何/DPI、过期/隐藏/暂停/禁用/销毁/重新激活ID，以及亿级窗口。定高/变高各40次远跳，缓存映射与consumer节点均少于100，导出前后key数相同；另30次编辑器焦点pin远跳保留原ID、逻辑0位置与焦点，删除该行后旧SetValue拒绝。Windows另有整数边界测试。初版fixture错误使用tuple作为Widget、误以为外部Mounted归App.clear所有，以及删除数据后locate仍返回索引，均已修正测试夹具；未放宽Widget/Owner/lookup约束，失败日志保留。

独立Windows UI Automation验收由隐藏控制台的.NET客户端进程驱动两个真实窗口，通过系统接口发现控件并调用Invoke/Toggle/Focus/SetValue。结果：A窗Count 0→1、B窗仍0；checked变On、emoji值写入成功、系统焦点正确；禁用Invoke被拒绝；Password.IsPassword=true且读不到原值；Editor bounds非零；100,000,000行源在系统树中仅7个ListItem。随后关闭两窗，所有业务Cage失效、语义节点归零，helper已经wait/join。这不是通过合成DomEvent或本地快照推断系统接口成功。

最终Windows passed=385（297框架/doc-test + 2编译失败用例 + 83引擎 + 普通shell/AccessKit实窗/UIA三项验收），Ubuntu WSL passed=386（297 + 2 + 83 + X11/Wayland各普通/AccessKit共四项实窗）。严格workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture严格检查与重建通过；Android只编译通过。默认门面normal/build依赖图不含AccessKit/winit/Stylo。浏览器源码与重建WASM哈希和v21已执行的十七套验收完全一致，本轮未重跑浏览器/stream服务，不把原结果描述成新执行。

Linux新增依赖后首次链接发生Bus error，当时D盘loop卷仅余443MB。确认该次进程终止后，先用cargo metadata、realpath和loop backing核对target确在D盘本任务目录，再只cargo clean -p glory-blitz，释放2451个/约41GiB旧产物，卷恢复40GiB可用；保留源码/样本/日志，重新完整gate通过。首次跨壳cache同步变量转义失败也保留，改用绝对路径后同步；未改动测试断言或转移到C盘。后续首次同步/几何失效分离、UIA整数边界修正后再次完整验证。

新增可选SDK/Unix桥依赖（首次锁解析新增58包）；workspace成员和其余包边界不变，四包105文件vendor及补丁/来源哈希未变，无新增unsafe。构建、包cache、临时文件、UIA报告和诊断在D盘；未启动HTTP/stream服务，UIA子进程与执行会话均已终止。Native默认无窗口document仍不创建平台对象。项目README、blitz README、契约、AGENTS和架构报告v22同步。

复现：cargo test --locked -p glory-blitz --features accesskit --lib accessibility::tests；Windows独立real_accesskit_windows_ui_automation需--ignored --nocapture --test-threads=1；完整scripts/check-semantic-ui.ps1 -Web -Window和Linux scripts/check-semantic-linux.sh --window已包含普通/AccessKit实窗与Windows UIA。直接应用可用cargo run -p glory-blitz --example window --features accesskit，门面选blitz-accesskit。D:/BuildCache/glory-semantic-ui/accesskit-final-{windows,linux}.txt、accesskit-android.txt、accesskit-position-boundary.txt、accesskit-default-tree.txt、accesskit-linux-{link-failure,clean}.txt及唯一accesskit-uia-*.json记录结果；完整源码、依赖/vendor、环境、日志和二进制哈希为accesskit-evidence-2026-10-22.json。

继续保留完整目标：Unix AT-SPI客户端消费、实际屏幕阅读器/Live播报、完整文本行/选区与更多滚动动作，OS IME、Android宿主/资源/设备；R0样式/上下文/扩展、Portable文本、R7字段依赖/性能细分/收益驱动缓存，以及旧工具/源码清理仍未完成。Linux adapter创建/窗口成功不代表AT-SPI或读屏器通过，Windows UIA成功也不代表全部辅助技术已验收。

## R5 虚拟行的系统 ScrollIntoView 与共享逻辑定位（2026-10-22）

上一轮完成AccessKit基础桥，本轮补足真实辅助技术显示离屏焦点行的链路。render增加短名Navigation::Reveal；ui::list按事件原点所在的已挂载行解析稳定key/index，复用原布局与ScrollSpace，不向原生适配器泄漏业务源。Reveal不发送Focus、只要求scroll能力，Root/空列表无目标；不回退首行，也不继承同批Last/Next的临时游标。已有普通导航仍要求focus+scroll。只有实际解析到新目标才重置分段bias/确认状态，避免无目标请求破坏已有精度校正。

Blitz可选桥只在有有效Root/Navigate的虚拟Row wrapper声明ScrollIntoView，接受None或可选ScrollHint；其他data拒绝。外部请求照旧只排队，业务与逻辑seek由主循环消费。导出前处理合并后的存活目标，以当前盒子和边框/client大小对原生祖先执行双轴nearest滚动，最后显示文档视口；引擎保持overflow规则。scroll导致observation失效时也安排下一次poll。退休、隐藏和停用重新验证/清理，原NodeId代次与AXID不复用；pending只存当前导出目标，不是访问历史。未新增另一套虚拟列表、CSS parser或平台crate。

新增三项UI测试覆盖定高/变高100,000,000行、9千万行远跳后的嵌套Input身份/草稿、无Focus动作、少于50额外key/少于100节点，scroll-only能力、Root零读取/零动作，以及同批Last/Reveal按实际原点而非游标显示。两个新增AccessKit测试经真实consumer检查定高/变高编辑器焦点/值和可见几何、回调零源读取、非法data与退休拒绝、32次重复请求只留一个pending，以及border/双轴overflow:hidden祖先和文档viewport。焦点pin、稀疏测量、原生scroll确认及数据定位约束未放宽。

独立.NET Windows UIA客户端增加ScrollItemPattern验收：编辑row 0为retained+emoji；准备位置变化、将焦点交回编辑器，由同一FocusChanged tick触发远跳。实际平台树窗口到第89,999,998行附近，原行在视口外且仍聚焦；随后通过系统接口请求显示，原RuntimeId、系统焦点、实际草稿与可见几何均保持。平台树初始7行，远跳9行加1个pin，返回后7行；关闭两窗后业务Cage失效、语义节点归零，辅助进程wait/join完成。

诊断保留：最初测试混用了consumer FullNodeId与本地AXID、反向遍历取错行、误写Style border字段/set_css返回值，已按实际API修正。真实UIA初测还揭示旧Wait-For局部taskValue遮蔽外部ValuePattern，异步首轮未完成时之后轮询会错误读取bool，已改为独立变量。UIA动作日志进一步确认Invoke在Click前发送Focus；直接Invoke远跳按钮会合理地结束编辑并回收该行，因此fixture明确改为准备seek、再回焦点触发，不屏蔽系统焦点或允许失焦行永久保留。临时动作/焦点追踪已移除生产源，失败报告/日志保留；主控件聚焦在等待期间重申以处理双窗口初次激活的异步时序。

最终Windows passed=390（302框架/doc-test + 2编译失败 + 83固定引擎 + 普通/AccessKit实窗/UIA三项），Ubuntu WSL passed=391（302 + 2 + 83 + X11/Wayland各普通/AccessKit共四项）。严格workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture严格检查与重建、Android同范围编译通过；Android仍无宿主/设备验收。原有实窗峰值14tasks/77引擎节点，累计key为Windows672/673、Linux673/667/673/679。17套浏览器本轮重新执行errors=[]，Python流传输的提前shell、结束、取消和慢读背压通过；本轮不沿用旧WASM结果冒充重跑。

依赖、workspace、全部manifest/lock与四包105文件vendor/补丁来源未变，无新增unsafe。生产Rust在最终gate开始前固定；之后仅补文档/证据。所有build/cache/临时服务、失败日志/源与结果保存在D:；8788/8789按PID/命令核对后停止，最终无监听。报告v23、契约、列表基线、README/AGENTS同步。新增显示功能不等于FPS/RSS提升，也不证明任意未物化行的系统检索、百分比滚动、文本接口或读屏器播报。

复现：cargo test --locked -p glory-ui --test list_widget；cargo test --locked -p glory-blitz --features accesskit --lib accessibility::tests；完整Windows scripts/check-semantic-ui.ps1 -Web -Window、Linux scripts/check-semantic-linux.sh --window；Android --workspace --all-features --all-targets --target aarch64-linux-android；17套examples/semantic-web/*acceptance.js及scripts/check-semantic-stream.py。D:/BuildCache/glory-semantic-ui/reveal-final-{windows,linux}.txt、reveal-android.txt、reveal-browser-2026-10-22.json、reveal-stream-transport.txt及accesskit-uia-1792674543841367.json记录最终结果，reveal-{render,navigation,widget,accessibility}-before.rs和reveal-uia-r*.txt保留前实现与诊断；完整哈希为reveal-evidence-2026-10-22.json。

完整目标继续保留：R0全部字段/样式/上下文与扩展审计，Portable文本验证，真实OS IME、Unix AT-SPI客户端、读屏器/Live播报、完整文本行/选区和更多滚动/项目检索，Android Activity/Surface/资源/设备，R7字段依赖/性能细分/收益驱动缓存，以及旧工具/源码清理。不能把本次已挂载行显示链路作为整个R5或整体目标完成。

## R0 确定尺寸 flex 矩阵与小数因子的 gap 修复（2026-10-22）

本轮返回P0样式行为审计。先用独立D:探针创建46组实际布局，并在隔离Chromium文档中使用相同typed HTML声明比较：六种justify、四种align、零/小数/整grow/shrink、两轴与五种overflow自动最小尺寸。四组0.25因子存在约5px差异，其余42组在有限误差内一致。横轴300px容器、两个50px子项与10px gap，本应各97.5px却为92.5px；250px子项按shrink=0.25应197.5px却为192.5px，纵轴有相同问题。两项原生回归修复前均失败；探针原可执行文件、前锁和原输出保留。

问题在Taffy 0.15.0：initial_free_space已经减去固定gap，小于1的未冻结因子之和分支又减一次。修复只移除grow/shrink分支中的第二次扣除并解释规范依据。没有在Glory添加几何补偿、第二套布局、拒绝小数或扩宽profile；HTML/RECORD v2、Blitz v3及合法数值域保持。修复后同一独立探针46组全部与Chromium相符，最大差约0.02451 CSS px；只四组输出改变，保留完整前后数据，不构造性能提速结论。

为保持Dom/Paint/Stylo连接的Rust类型身份一致，新增vendor/taffy与vendor/stylo_taffy，包名glory-taffy/glory-stylo-taffy；四个Blitz包仍beta.4。Taffy来源commit eb6450694f33e2b7413e40677f230ff63f0fc194，连接包保持Blitz原commit；原crate archive SHA及每文件原/后哈希均记录。未在Glory声明全局patch，也不要求应用复制patch配置。连接包只改manifest边和补充许可证；补充的上游license/rustfmt按精确commit URL与独立source hash验证。当前六包183文件，单独测试锁另行跟踪/校验，活动workspace仍十包。默认门面normal/build图无Blitz/Taffy/Stylo/AccessKit。

两个Taffy直接回归覆盖两/三子项、零/不等/冻结/小于1/等于及大于1的因子和、0/10/25px gap与margin；原有169项加新2项，共171通过。render/conformance增加可选FlexCase数据，Native和Web使用相同46组描述符及规范预期局部矩形。两个Blitz测试分别构造全部46场景、在同一物理节点上连续修改36种非文字样式，检查实际坐标/尺寸与身份；相关CSS仅在验收范围生效，没有把数据矩阵放到生产滚动路径。

第18套浏览器flex-acceptance以同一Rust组件生成CSR与SSR：各36次样式变更全部保持输入/父子盒身份、预输入emoji、焦点和双向UTF-16选区，随后实际键入和卸载；十种Text/overflow场景另行构建。两入口最大误差约0.02451px，所有delta有限，errors=[]。首版自定义fixture没有保留child Scope，导致两子项在build结束后释放；最初脚本未断言子项数而将空集合的-Infinity误差序列化为null。现明确retain Scope、断言恰好两个子项且误差有限，旧诊断不计为通过证据；修正后完整Windows gate和18套浏览器重新通过。

首次对新vendor文件执行cargo fmt时继承了Blitz的100列配置，来源检查及时发现非局部格式变化；已从验证过的原archive还原未改源码，补入Taffy精确上游120列配置，最终183文件检查通过。独立Taffy有dev dependencies，不能在被排除的依赖上用-p运行；门禁改用--manifest-path vendor/taffy/Cargo.toml，并跟踪/校验其测试锁。WSL cache首次同步遇到尚未下载的可选parse包，已按锁fetch到D:并校验21包archive/source与原子同步稀疏索引，未改用C:或去掉offline要求。失败输出、原源码与来源恢复脚本保留。

最终Windows passed=563（304框架/doc-test + 2编译失败 + 83 Blitz + 171 Taffy + 普通/AccessKit实窗/UIA三项），Ubuntu WSL passed=564（304 + 2 + 83 + 171 + X11/Wayland各普通/AccessKit四项）。严格workspace all-features/all-targets Clippy、Linux default/paint、WASM workspace/fixture严格检查及重建通过；Android同范围编译通过。实窗峰值14tasks/77引擎节点，累计key为Windows674/674、Linux673/673/676/676；Windows UIA的离屏编辑行找回与关闭回收继续通过。18套浏览器errors=[]、新链接的stream fixture传输/取消/慢读背压通过，亿级窗口、资源、焦点与测量预算保持。

构建/cache/临时文件、前后探针二进制和全部日志在D:，无新增unsafe；原库已有unsafe依靠未改文件原哈希保留。仅新增布局两包及相应本地依赖边，余下第三方生产依赖版本保持；测试锁与acceptance fixture的conformance opt-in独立记录。HTTP/stream helper按PID/命令验证后关闭，8788/8789无监听；fixture Scope修正后重新链接stream binary。报告v24、组件契约、列表基线和README/AGENTS同步。

复现：cargo test --locked -p glory-blitz --test flex_layout；cargo test --locked --manifest-path vendor/taffy/Cargo.toml --lib；完整Windows -Web -Window、Linux --window、Android --workspace --all-features --all-targets --target aarch64-linux-android；18套examples/semantic-web/*acceptance.js及scripts/check-semantic-stream.py。D:/BuildCache/glory-semantic-ui/flex-final-{windows,linux}.txt、flex-android.txt、flex-browser-2026-10-22.json、flex-stream-transport.txt记录最终验收；layout-probe-native-{before,after}.jsonl、layout-probe-comparison-{before,after}.json、flex-probe-{before,after}.exe与flex-before-tests.txt保留原/后结果和失败；完整源码、锁、来源和二进制哈希为flex-evidence-2026-10-22.json。

这次覆盖确定尺寸的有限flex行为，不代表R0全部属性/上下文、Grid/内在尺寸/字体、Portable stylesheet或跨引擎完整CSS已经完成；也不替代真实OS IME、AT-SPI/读屏、Android宿主/设备、R7字段依赖/性能细分/缓存，以及旧架构/工具清理。整体目标继续进行。

## R7 固定内联字段依赖与巨量列表回收（2026-10-22）

§12.4 的收益准入由独立 release 探针验证：非Clone的1 MiB模型、32/256字段、整模型观察者和字段相等门控，每次20逻辑批次。256字段无关单写仍有5,120次派生求值/10,260次失效访问；消费门控正确，但没有省掉求值。因此新增core内部fields模块，`Cage::field/try_field(name,get,get_mut)`返回现有Cage，不引入第三方Signal、宏/DSL或新crate。

字段仍借用根模型，保存投影链与独立的依赖身份。路径按根/父/名称共享，类型和位置一致；每次调用另有Owner别名。同路径跨行/跨Owner写入通知所有存活别名；字段修改同时通知根和祖先，兄弟不求值，根修改保守通知全部存活字段。无默认值相等比较；可继续派生Bond并选择with_partial_eq。根模型失效清理所有别名，最后别名/子路径离开回收路径；投影与用户捕获在图借用外drop，不为整个源或访问历史建索引。

访问器必须纯并选择固定内联子对象，检查名称/类型/位置、越界、不同兄弟重叠、读写位置不一致、隐藏响应读写及深度128；try接口返回InvalidField等已有类型错误。纯派生仍不能写；业务写入panic保留已发生的版本发布，借用错误不执行业务闭包。根RefCell仍统一借用。结构体/元组/固定数组适用；String/Vec整体可选，但其堆上元素、Map条目、Deref和动态枚举路径尚未实现。不能以本批替代集合key/删除/重排/位置失效契约。

新增10项核心测试覆盖嵌套祖先/别名通知、菱形与批次中间读、6,000轮Owner/slot回收、写入中释放别名、根跨Owner失效、非法路径和可变投影、panic恢复/部分发布、纯读写限制、捕获drop重入、移动位置/深度128及1,000轮混合字段/根/晚订阅。新增一亿行组件测试连续300次远跳，字段路径/槽位保持2、图/Owner槽位均不超过180；无关字段不读key、不执行行mapper、不提交修改，prefix只更新挂载行，最后卸载回收所有行路径。1 MiB载荷指针不变。

新增公共Rust字段组件、SSR快照测试及第19套浏览器用例。CSR/SSR接管均保留预输入；1,000次背景字段写入只求值背景文本一次，title/editor mapper保持，input.value setter零次、原身份/焦点/UTF-16反向选区保持。整模型fallback求值三个字段但相同编辑值不重写控件；程序draft和真实输入事件继续正确往返。SSR确认转义和请求结束后的模型/字段失效。

性能收益按场景记录：256字段20次单写无关派生5,120→0，失效访问10,260→20；选中字段访问为40，消费通知与原门控相同。批量1,000写入并不保证提速，32字段场景字段版本/路径开销高于原粗粒度批处理。初版复测还发现普通Cage批量写入受到字段分支开销影响，已拆回独立原写入guard，将投影/别名处理移至fields::revise，并消除重复节点查询；原始慢样本保留，最终原/后可执行文件交替复测。没有将微秒数据解释成FPS、RSS或平台呈现收益。

完整门槛Windows575/Linux576项通过，分别执行`check-semantic-ui.ps1 -Web -Window`与`check-semantic-linux.sh --window`，含严格all-features/all-targets、Blitz83、Taffy171、普通/AccessKit窗口、Windows真实UIA；Android仍只做all-features/all-targets编译。独立fixture的fields_form SSR测试1项另执行，不计入workspace总数。快路径修正后重新构建WASM和stream，19套浏览器/流传输重新通过；实窗仍不超过14tasks/77引擎节点，HTTP/stream helper按PID/命令验证关闭，8788/8789无监听。

诊断不隐藏：初次fixture opaque返回捕获Scope lifetime，改为Rust2024 use<>；CSR/接管错误类型不同，分别map_err后统一；while-let严格lint已修正。第一轮浏览器stream与其他套件并行时waitForFunction超时，隔离重跑通过，保留错误结果，未确认根因；最终快路径版stream单独执行。初版probe在after粗路径也创建两个writer别名，不能衡量无字段路径，已改为只在field模式创建；初版数据保留，不混入最终粗路径对照。

构建、缓存、临时文件、独立probe及全部证据在D:；依赖/锁及十包workspace、六包183文件vendor保持，没有新增unsafe。最终源码/二进制/日志与剩余范围见D:/BuildCache/glory-semantic-ui/field-evidence-2026-10-22.json；统计和复现见列表基线。整体目标继续：R0样式/上下文/扩展与Portable文本，OS IME、AT-SPI/读屏/完整文本选区，Android宿主/资源/设备，R7集合路径、引擎细分与收益驱动缓存，以及旧架构/工具清理。

## R7 Vec/Map 集合路径与缺失恢复（2026-10-22）

按上一批剩余范围增加`Cage<Vec<T>>::item/try_item(index)`与`Cage<BTreeMap<K,V>>::entry/try_entry(key)`、`Cage<HashMap<K,V,S>>::entry/try_entry(key)`。依旧返回Cage，不新增Store引擎、UI DSL、crate或依赖。item是位置，entry是完整业务key；每次访问重新按索引/key从当前根借用元素，不保存堆元素地址。Vec扩容/重排、HashMap rehash不会留下旧引用。K/V无需Clone；BTreeMap额外要求K:Hash以索引活跃路径，HashMap保留其S。

新错误MissingField区分不存在与Disposed。缺失可先注册item/entry和已知类型的嵌套集合路径，try读取仍订阅；失败try_revise不执行业务闭包、不改版本/stat。根/集合revise删除后原句柄仍属于组件，重插相同key恢复到新值；业务新实体使用含代次的key。首次内联field仍需要父元素存在以验证offset，已有field在缺失/重插后恢复。根/集合整体修改保守通知所有存活路径；entry/field写入只通知根、祖先/别名/后代。没有全集合diff、并发可变借用或自动插入/删除。

路径段区分Field/Item/Entry，禁止集合与整体inline投影作为重叠兄弟，要求通过共同父字段细分。所有构造都在根可变借用时返回Borrowed，避免普通/字段写入已捕获的通知集合漏掉新别名。动态路径只检查兄弟种类与本段身份，不扫描全部兄弟；inline字段继续验证存储重叠。按父路径建立数字哈希桶，完整类型化Eq解决碰撞，桶持Weak，canonical路径持key；最后路径离开删除key/空桶，根释放使其他Owner别名也失效。没有访问历史索引或删除值缓存。Hash/Eq在图和注册表借用外执行；访问源的比较/hasher在图借用外执行；key方法必须纯，key析构在借用之外执行。Root payload仍不由注册表强持有。

新增11项核心测试：Vec扩容/重排/删除/重插、嵌套field和Missing订阅恢复、单项通知范围、非Clone key/value、真实哈希碰撞/跨Owner/rehash、6,000轮缺失嵌套路径回收、inline/集合重叠拒绝、写入中注册拒绝、Hash/Eq/Drop图重入、panic恢复/失败注册无元数据、key Hash期间释放源、1,000组混合批次对普通求值，以及1,024/131,072实际HashMap的单项更新key操作<=20/只运行一个绑定。原10项fields与31项runtime继续通过。

新增公开List回归：100,000,000逻辑行、业务自管的单值稀疏缓存、300次远跳。临时writer与行共享key，单项name修改仅计算该行、不读列表key，1 MiB行载荷指针保持；字段槽<=21、图/Owner槽<=180、key读取<7,000。卸载后fields/entry_keys/entry_buckets均0；此场景不代表已加载一亿业务值，实际大Map另由上述源大小测试和release探针覆盖。

新增Rust entries_form、SSR测试与第20套浏览器验收。CSR/SSR接管预输入后，另一key的字段更新、HashMap reserve/rehash、Vec位置swap、删除key7/拒绝缺失写入/同key重插均保持key9输入身份、草稿、焦点、UTF-16反向选区，value setter零次；key7控件原身份保留并按缺失禁用/清空，重插恢复。单字段只运行其mapper，根reserve保守重验四个字段但相等门控保护DOM。SSR确认转义与请求结束后句柄失效。

同一release二进制交替比较最佳粗路径map+with_partial_eq与rows.field→entry→map+with_partial_eq，观察者语义和消费次数相同；源1,024/131,072，观察行32/256，修改无关background/观察count，每20批次各写1或1,000，三次进程重复、每单元9样本。131,072源/256观察行的无关单写将key比较75,380→0、派生5,120→0、marked10,260→20；选中单写比较75,480→200、派生5,120→20、marked10,260→60，消费均20/40、所有写入仍发布版本。代价是同一配置图节点514→774（显式别名和保留的相等派生），并且32观察行每批1,000写入明显更慢；原样本和时间范围全部保留。此为Windows核心诊断，不含布局/paint/present、RSS或FPS；不能据此声称所有场景更快或占更少内存。高频路径管理的收益驱动优化继续保留。

最终Windows588/Linux589项通过，含严格all-features/all-targets、Blitz83、Taffy171、普通/AccessKit实窗及真实UIA。另执行fixture SSR全部2项，20套Chromium与stream transport本轮重新通过，stream隔离运行；原生峰值仍14tasks/77引擎节点。Android all-features/all-targets通过只证明编译。未改wire/HTML/引擎/布局或第三方依赖、锁、十包workspace/六包183文件vendor，无新增unsafe。HTTP/stream进程按PID及命令验证后关闭，8788/8789无监听。

保留诊断：新List测试局部labels遮蔽函数，已改名；common注册检查的collapsible_if严格lint已修复。新增浏览器断言在执行前修正为所属frame内查询，避免CSR页面中同名SSR静态控件被误认；没有因此移除身份/编辑状态断言。完整日志、原始288个性能样本、源码/二进制哈希与来源在D:/BuildCache/glory-semantic-ui/entry-evidence-2026-10-22.json；列表基线含时间表。

报告v26、README/AGENTS/契约同步。完整目标继续：R0实际样式/上下文/扩展及Portable文本，OS IME、AT-SPI/读屏/完整文本选区，Android宿主/资源/设备；R7 Deref/自定义容器/动态枚举、专门结构增删发布、批量路径开销、引擎细分和收益驱动缓存，以及旧架构/工具清理。

## R7 通知目标命中与缓冲复用（2026-10-23）

上一批明确记录的批量退化继续处理。分配探针确认旧fields::write_target每次创建nodes和pending Vec，深路径还会增长nodes容量。现在Fields按根模型懒存一个Box缓存槽；同路径使用已有代际身份，失配清空并复用nodes/pending容量，绝不按每个写过的key保存一份目标。没有源数据/元素地址/订阅者或版本缓存，也不改变普通无字段Cage快路径。

投影/借用先通过，当前版本逐一预检后才取走或修改缓冲。命中也检查版本；失配用深度<=128的无分配验证遍历，再构造当前目标。业务代码执行时guard独占缓冲，根中留空槽作为归还许可；别名/路径新增或释放、根释放删除对应槽，guard不可复活已撤销的槽。同根可变借用阻止新路径和嵌套写入，因此许可没有重建ABA，不需要缓存epoch。其他根独立。每次实际写入仍推进全部相关源版本、读取当前订阅者和传播前沿；panic仍发布已发生的部分写入。Root已关闭则沿用既有不再发布的规则。缓存及缓冲只持内部身份，在图内回收不调用用户代码；用户值/捕获仍在图借用外析构。

增加RuntimeStats::field_target_builds/hits/visits/caches/keys，前3项是验证成功的准备工作量，后2项是活跃根槽/身份（含guard借用期间）。准备与最终发布并非同一时刻，根在业务闭包中关闭时也不应强行令其等于source_writes。成员变化直接释放容量；没有历史窗口、字段矩阵或Scene/paint缓存。

新增5项fields集成与3项内部测试：20批各1,000写只构建一次、19,999命中；中间读取、无关字段、晚订阅与Effect自写、部分panic、跨Owner别名/子路径改变、同根切换/跨根隔离、写入中释放当前Owner、错误投影/借用/缺失不改变缓存和统计、3,000位置/代际slot复用、版本耗尽先拒绝业务代码、guard借走后槽撤销不被放回、1,000次交替目标保持两个缓冲地址与当前拓扑容量边界。当前core lib7、fields15、entries11、runtime31均在全量门槛内。

新增公开List组合回归：一亿逻辑行，单值业务稀疏缓存，300次远跳保留原输入焦点pin；每次两项写入分别重建/命中一次，只更新原输入且不读列表key，目标槽/身份最多1/3，路径及graph/Owner槽有界。writer结束撤销缓存，失焦销毁pin，clear后路径/key/目标全0，业务数据仍归业务持有。它不宣称加载了一亿值或证明真实OS焦点/输入法；实际DOM编辑状态另由浏览器覆盖。

两阶段诊断保留：最初Rc<Box<[Key]>>目标快照虽然减少同字段写入分配，却将32观察项、交替两字段、20批各1,000写的分配40,180→80,181，故未采用。改成独占缓冲后183；最后将较大的槽Box化以缩小BTreeMap节点，最终184（多一个首次Box分配），累计分配字节823,520→23,856。同根宽→窄目标只保留当前成员拓扑需要的容量，成员改变即释放。首次稀疏background样本也由100次/8,640字节降至64次/8,176字节，避免最初inline槽的8,888字节固定成本。原型源码、二进制和通过的门槛仅作诊断保留，不当作最终版证据。

最终System转发分配探针覆盖entry/field/mixed三族1,080样本，每单元9次；业务值、回调/失效/派生、key操作、图节点与边保持相同，所有组分配调用及累计分配字节均未增加。实际BTreeMap源131,072、观察256项、20批各1,000次选中写，分配60,200→205，累计字节2,104,320→24,720；未观察字段40,060→64。诊断allocator只在D盘独立probe内用有注释的unsafe转发System，框架生产无新增unsafe；累计字节不代表存活内存或RSS。

另用v26原未插桩二进制与完全相同入口源码的新版二进制交替测576样本，避免把计数器开销误算为应用收益。131,072源/256观察项中，20批各1,000写的字段路径background约1,730.1→1,026.1µs，选中项3,031.4→1,427.8µs；单写分别8.8→8.6、19.3→17.7µs。仍有5组中位数较慢，全部范围保留。32观察项高频选中写的当前字段路径1,436.9µs仍慢于同版粗路径600.1µs，不宣称字段总是胜出；投影、查找、版本维护成本继续存在。以上为Windows核心诊断，无布局/paint/present/FPS结论。

最终Windows597/Linux598项通过，含严格all-features/all-targets、Blitz83/Taffy171、普通/AccessKit窗口与真实UIA；额外SSR fixture2项、重建后20套Chromium和stream transport通过，stream单独执行。字段browser真实核对1,000背景写的1重建/999命中和零input.value setter；集合browser核对两个根独立槽、Root目标复用、缺失写不改变有效缓存，输入身份/草稿/焦点/UTF-16选区继续保持。原生峰值14tasks/77引擎节点，Android仍只all-features/all-targets编译。helper按PID/命令验证关闭，8788/8789无监听。

依赖、全部Cargo manifests/locks、十包workspace、六包183文件vendor、renderer wire及引擎保持，无新增生产unsafe。全部构建/缓存/临时/前后源码与二进制在D:。混合写入旧版基线使用D盘最小三包源码快照，生产.rs逐项对照v26证据；未改当前源码来切换基线。未插桩入口先将备份CRLF恢复原LF后核对原始源码哈希，二进制也与v26记录匹配。日志/数据/来源/454源码哈希归档为D:/BuildCache/glory-semantic-ui/cache-evidence-2026-10-23.json；原/后、失败原型、全部慢组和缓冲未Box时样本均保留。

报告v27和契约/README/AGENTS同步。完整目标仍未完成：R0实际样式/上下文/扩展与Portable文本，OS IME、AT-SPI/读屏/文本选区，Android宿主/资源/设备，R7 Deref/自定义/动态路径、专门结构增删、其余引擎细分与style/paint收益驱动缓存，以及旧架构/工具清理。


## R7 窗口顺序与按需 LIS（2026-10-23）

继续按报告处理大列表滚动的实际开销。旧Cache虽只访问窗口，但每次协调都Clone旧窗口key建位置HashMap，再分配位置向量及LIS状态。现在每个活跃Entry保留一个usize窗口order；下一窗口在验证key时读取其旧位置，无逆序时不运行LIS，有实际重排时仍计算最少移动子序列。源index独立，pin参与同一次协调；order写回合并进现有mount/update循环，不另做每行查表。没有访问历史、全源索引、后台任务、新crate/依赖、协议或公共API变化。

重复key/无效pin检查仍早于行副作用；完全相同range/revision/pin仍不读key。字段版本与通知优化不受影响。新增两个回归：六个key的720个排列，混合成员增删、空窗口和错误恢复，对独立二次复杂度最少移动oracle逐项比对物理顺序/身份/生命周期；以及可观察Clone的key在一亿逻辑范围远跳时不再克隆离屏旧key。最终glory-ui测试81项（含windowed_list12项）通过，原有焦点pin、空组件、Owner/任务与300次远跳组合测试继续执行。

先保存原虚拟列表bench可执行文件，再用完全相同的benchmark和release opt-level=3/LTO配置构建新版，交替运行三次。20,000次固定33行窗口：百万/一亿行连续协调中位40.016/40.248→21.915/21.324ms；远跳42.295/43.577→35.066/35.170ms。源key读取、创建/销毁与峰值行完全一致，一亿源仍为惰性索引，不是加载了一亿业务对象。

D盘独立同源probe用cfg引用改前/改后cache；usize/长字符串key、33/256/4096窗口、九场景，每格9样本。分配版972样本，未插桩计时另972样本，共1,944。所有业务轨迹、读取和生命周期一致；54配置的分配次数及累计字节中位数均未增加。33行2,000次连续协调：数字key20,001→4,001次分配、7,589,216→3,300,240累计请求字节；长key420,001→338,001次、40,348,288→28,665,312字节。统计包括同一Host工作，长key创建也分配；每行增加的order和临时旧位置意味着这些数字不能推导RSS。

保留代价：最终未插桩4096行数字key、100次轮转中位62.4806→66.1972ms，约慢6%，原始范围有重叠；另一较慢组为100次no-op的0.8→0.9µs。插桩版还有5个时间较慢组，均保留。第一版额外逐行HashMap查找写回order，4096轮转退化约20%；合并遍历后重新做全部最终测量。初版源码/二进制/样本在reconcile-initial，不能混入最终结果。分配汇总最初假定各进程计数恒定，发现部分组不同后改为记录中位数和完整范围；行为轨迹仍要求完全一致，没有筛掉慢样本。诊断allocator复用D盘System转发模块，unsafe不进入生产。

最终完整check-semantic-ui.ps1 -Web -Window通过599项，Linux已验证D盘loop后check-semantic-linux.sh --window通过600项，含Blitz83、Taffy171、严格all-features/all-targets、X11/Wayland普通/AccessKit窗口和Windows真实UIA。原生峰值仍14任务/77引擎节点；WindowsUIA保留7个初始列表项，远跳约9千万行仍保留一个编辑pin并能Reveal返回。另执行独立fixture lib的两个SSR测试、20套Chromium及stream传输背压/断开清理，全部通过。最终生产Rust源码在这些验证间未再修改；报告/契约/列表基线及AGENTS同步。Android本批未重做设备或宿主验收，原先编译证据不等于移动实现完成。

诊断保留：首次Windows全量链接并发内存不足，降低CARGO_BUILD_JOBS=2后通过；下一次UIA在initial controls的Editor.SetFocus超时，未进入列表操作，报告accesskit-uia-1792690156902437.json保留。其他窗口测试结束后完整隔离重跑通过，真实UIA成功报告为accesskit-uia-1792690535536134.json；不据此断言初次聚焦失败的根因。Linux首命令相对路径及非登录PATH缺cargo的失败分别保留，使用脚本绝对路径和已验证cargo路径后通过。首次SSR bin命令执行0测试，另以--lib执行实际2项，不把空测试算通过覆盖。浏览器结果一次性传给Windows命令超过长度上限后，保留内存结果并分块写档，无需重跑或裁剪测试结果。

全部构建、缓存、临时、诊断源/二进制和原始证据在D:。HTTP/stream helper按PID及完整命令验证后停止，8788/8789无监听。最终汇总reconcile-performance-summary.json，完整证据reconcile-evidence-2026-10-23.json；列表基线记录范围、复现和全部较慢组。

整体目标继续保留：R0实际引擎样式/上下文/扩展审计与Portable stylesheet文本，实际OS IME、AT-SPI/读屏/完整文本选区，Android Activity/Surface/资源/IME/设备，R7 Deref/自定义容器/动态路径与专用结构增删发布、引擎细分及收益驱动style/paint缓存、旧源码/工具/入口清理。本批窗口协调优化不替代这些交付物。
## R0 受限 Sheet、共享 HTML CSS 与状态映射（2026-10-23）

继续实施报告§8的样式输入边界。glory-style的可选css依赖固定cssparser0.38.0，增加不可变Sheet及Rule/Selector/Declaration视图，不引入UI DSL、VDOM或新的运行时。默认facade和SSR依赖图已实际检查，仍无parser/serde/DOM/native engine；开启CSS的WASM fixture也没有Blitz/Stylo/Taffy。root lock复用既有解析器版本并增加style依赖边；fixture lock新增可选图的解析记录，不能据此断言native引擎实际进入浏览器，实际tree输出分别保留。

首批文本覆盖现有25类属性、overflow shorthand、class组合、子代/后代、hover/focus/enabled/disabled/checked；规则/声明顺序、重复声明和specificity保持。未知属性/selector/value/unit、at-rule（包括上游默认忽略的初始charset）、important、函数/变量等返回错误，绝不返回部分成功Sheet。数字域复用Style::validate。默认1MiB、4,096规则、8,192选择器、65,536项、32,768声明，单选择器128项硬限制；诊断为一基行和UTF-8字节列。六项集成覆盖全部类型化属性、顺序/转义/状态/层级、28类以上拒绝输入、profile拒绝原声明、全部预算及长selector边界，另有Sheet文档用例。

glory-html/css提供共同sheet编译：每个compound用零specificity的Main guard排除内部part；Group透明。Check的class在label，禁用/启用/选中通过Main语义属性映射，原生property仍在input；focus仅将Check内部control的focus-within算作自身焦点，普通View聚焦后代不匹配自身focus。已提交props决定checked/enabled状态，宿主决定hover/focus。class中的原始<按CSS hex转义，可用于style raw-text而不做HTML实体逃逸。HTML格式升至3，full-shape/delta、SSR/WASM、stream End/markers和Python传输准入同步；没有旧格式别名或转换器。

SSR Document的可选sheets在组件build前检查并按顺序输出，沿用nonce；Blitz set_sheet先完成全部目标profile检查，再替换author CSS并失效，失败不改原文档/visual revision。Style Profile的inline版本独立于HTML格式；本批没有扩大Sticky或AnchorAuto的原生承诺。解析/编译只在资产准备或显式替换发生，普通滚动和业务绑定继续生成原有属性/结构Patch，CSS cascade/layout仍由引擎处理。

新增原生验收发现并修复两处实际引擎问题。块/flex变成被祖先inline上下文吸收的内容后，旧layout_children/inline-root/匿名盒子未退休，使内部checkbox同时出现在旧/新容器；现在只清理过期构造状态并保留控件，直接引擎回归连续切换200次且匿名节点有界。原先FocusWithin匹配固定false；现维护当前元素祖先状态，切换/清空/移除时快照并失效相关路径，跨父节点移动仅懒捕获当前焦点路径并在mutator边界收口。同父重排和挂载新行不分配该路径，不建历史索引。直接回归覆盖移动、保持焦点、清空、删除与状态回收；Glory原生focus用例验证Check内部焦点、切换到Input和祖先View不误匹配。来源校验仍六包183文件，直接引擎85项，补丁/哈希与vendor说明同步。

新增第21套浏览器用例与同一纯Rust CSS form。CSR和SSR接管实际核对class/child specificity、Group透明、继承、hover/focus、预输入/预选中、disabled/checked及反向UTF-16选区、身份和卸载。Native实际几何还验证同一状态规则、unsupported Sticky原子拒绝和替换CSS后不重挂载。HTML lowerer另验证顺序/guard/转义/目标拒绝，SSR验证多Sheet与nonce。

最终完整Windows check-semantic-ui.ps1 -Web -Window通过613项，Linux已验证D盘loop后check-semantic-linux.sh --window通过614项，包括严格all-features/all-targets、Taffy171、Blitz85、X11/Wayland普通/AccessKit窗口及Windows真实UIA。原生仍峰值14任务/77引擎节点，巨大列表的窗口/焦点pin/测量/源读取回归继续通过。fixture lib两个SSR测试另执行；最终21套Chromium与stream传输顺序/背压/断开清理均通过，End marker实际为gly:v:3。控制/协议测试不替代OS IME、屏幕阅读器或Android设备准入；本批没有新的FPS/RSS结论。

诊断保留：初版按旧cssparser API编写，按固定0.38的单lifetime Parser、拥有错误及带SourceLocation的三元组修正。引擎直接复现和集成复现的layout_parent断言、focus样式20px而非60px的失败日志保留。最初直接引擎测试把没有UA尺寸的checkbox假定为正宽，修为显式14px后继续检查真实布局；其源问题的冲突父节点复现先于尺寸断言。聚焦清理测试修正为检查内部focus_node_id，因为公开getter无控件时回退到body；没有改该既有行为。两平台首轮新格式测试仍写死v2，已改为v3；classic stream结束marker及Python html_version断言同步，旧失败日志保留。新browser严格整数相等遇到缩放下约0.01px量化，改用既有flex矩阵相同0.03px有限容差，同时保留实测Rect；10/20px语义变化不能被容差掩盖。

所有构建/缓存/临时/日志/JSON在D:。两个fixture helper按PID与完整命令验证后停止，8788/8789无监听。完整来源、源码和产物哈希归档为D:/BuildCache/glory-semantic-ui/css-evidence-2026-10-23.json；本批没有新增生产unsafe。报告v30、README、AGENTS、契约与vendor说明同步。

该批结束时仍保留原§8.2更宽的font-family/weight/wrap、border/radius/opacity、主题token、其他语法及非HTML resolver；后续文字/装饰进展见下节。R0其余上下文/扩展/实际引擎审计，OS IME、AT-SPI/读屏/完整文本选区，Android宿主/资源/设备，R7 Deref/动态容器/专用结构增删、引擎细分与收益驱动缓存、旧架构/工具清理继续保留。

## R0 字体、空白/换行与装饰属性（2026-10-23）

继续实施报告§8.12。新增Family有序回退列表、绝对分数字重、WhiteSpace四模式、独立边框宽度/颜色/None或Solid样式、统一px圆角与整体透明度。typed Style与Sheet共用33类属性、42类有限值；Support改为u64，并逐属性/值测试独立拒绝，包括32位边界之外的字段。HTML/RECORD profile升到3、Blitz升到4；HTML映射仍是格式3，无新增crate、依赖、feature或布局树。

命名字体与generic区分，Named恒序列化为CSS字符串，转义引号/反斜杠/控制符/原始<，避免改动声明或style raw-text边界。测试以包含注入片段、换行、反斜杠、中文和emoji的名称往返同一Sheet解析器，并保证只产生原声明。空列表/空名/NUL、非法数字、未支持generic/keyword/shorthand/多角/百分比圆角等报错。边框各项独立，撤销inline恢复级联；不因默认值填充而覆盖其他声明。

真实原生几何首测捕获nowrap→normal仍为20px单行的错误。固定引擎的损伤判定只比较white-space-collapse，没有比较text-wrap-mode；重用Parley数据保留旧模式。现在将真实wrap-mode变化加入文本重收集判定，不全局失效。直接引擎回归40次切换继承模式；adapter覆盖四模式与显式换行、替换Sheet及控件身份。CPU像素测试证明边框/圆角及包含子节点的0.5整体透明度，撤销边框恢复内容位置、卸载清空像素。

新增一亿行真实文字变高回归：源为惰性索引，初始20px，锚点上一行换行变60px，节点身份和屏幕位置不变，逻辑offset补偿高度差。Windows独立原生回归25次远跳共705次取key，最终14活跃行、64条测量，height_capacity为31；不扫描一亿条源或持有访问历史。第22套浏览器用例覆盖同一文字列表和远跳边界；已有CSS用例扩展CSR/SSR接管、分数字重/字体/装饰、typed覆盖/撤销、输入草稿/反向UTF-16选区/焦点及身份。

本批没有改变core与Cache/Heights实现，不重复CPU对照，也不声明FPS、RSS或全CSS提速。报告v31、契约、README、AGENTS与vendor说明同步；完整目标仍保留主题、更广文字/视觉语法、非HTML resolver及实际引擎/上下文/扩展审计，OS IME/AT-SPI/读屏/完整文本选区，Android宿主/资源/设备，R7 Deref/动态容器/结构增删/引擎细分与收益驱动缓存、旧源码/工具/入口退出。

最终完整Windows check-semantic-ui.ps1 -Web -Window通过621项，Linux已核对D盘loop后的check-semantic-linux.sh --window通过622项，包含直接引擎86、Taffy171、严格all-features/all-targets与默认/paint组合、X11/Wayland普通/AccessKit窗口及Windows真实UIA。WASM workspace/fixture严格检查和实际重建均通过，fixture lib另有两个SSR测试。22套浏览器及stream顺序/背压/断开清理均通过。原有原生窗口仍峰值14任务/77引擎节点；实际UIA保留7个初始列表项及约9千万行处的编辑pin、Reveal返回、草稿和焦点，最终报告accesskit-uia-1792697229116314.json。

浏览器文字列表峰值39活跃行、64条测量、120个语义slots、210个graph slots；上一行约20.006→80.025px时锚点约-7.005px保持不变。其文字包含行号，视口600px、overscan每侧4，因此预算按ceil(600/20)+2×4+2=40计算；最初误沿用24px旧fixture的35行预算导致测试失败，修正为独立窗口公式，未改实现或数据规模。装饰首测把已使用border宽度写死为2px，实际缩放下为1.96078px；现在同时精确核对CSSOM声明2px并按一个物理像素界定used值，其他确定几何继续0.03px容差。原始失败和实测值均保留。

诊断保留：新增直接引擎测试最初没有安装块级UA规则，p为inline而final_layout高度0；补全测试的默认块级环境后40次切换通过。原生真实nowrap→normal单行失败独立保留，不与测试环境问题混淆。UIA前两次完整运行及一次独立运行在初始Editor.SetFocus超时；额外尝试对Window根节点SetFocus报不可聚焦，该前置动作已撤回。增加的仅是失败时诊断（不记录其他应用标题、文本或控件名），成功路径/断言未变；之后独立及最终完整重跑通过，不能据此认定之前超时的根因或宣称修复了平台聚焦。所有失败与成功报告均归档。

构建缓存、临时、日志和原始证据全在D:。空间偏低时，目录级旧增量缓存清理被自动审批拒绝而未执行；随后使用Cargo官方单包clean回收15.5GiB开发缓存，未删源码或历史证据，再完成最终完整检查。fixture helpers按PID/完整命令验证后停止，8788/8789无监听。完整源码、报告、产物及失败记录哈希为D:/BuildCache/glory-semantic-ui/typography-evidence-2026-10-23.json；本批无新生产unsafe，也无Android宿主/设备或真实OS IME/读屏完成声明。

## 第三方依赖：迁移到组织 Git fork（2026-10-23）

按用户明确要求，删除仓库内的第三方源码目录及旧来源校验器，停止本地副本维护。
创建真正的GitHub forks glory-rs/blitz与glory-rs/taffy，并将已有修复提交推送到
chris/glory-engine与chris/glory-layout。Glory固定Blitz完整提交
d58b604a7a20a84e607d45ae6ce6c15a7b9d3dce；Blitz统一固定Taffy提交
dd7914fba2e6e0b5d8a2c8854d764a2ade817efd。上游基线、完整提交及测试参数由
docs/engine-forks.json记录；维护方式见engine-forks.md。AGENTS明确禁止vendor。

恢复六个上游package名字及正常仓库/workspace结构，Glory自己的适配器仍叫
glory-blitz。只改组织fork的Git来源；无全局Cargo patch、子模块、移动branch
依赖或应用端配置要求。两个消费锁文件同步更新；原四个Blitz包、Stylo连接和
Taffy只解析到各自的一份Git类型身份，活动workspace仍十包。Web/SSR的实际
normal/build依赖树不含Blitz/Taffy/Stylo/AccessKit/Dioxus。

迁移前校验通过六包183文件的历史来源记录；146个源码/资源文件逐字节迁入
上游Git checkout，并再次对最终Git提交的blob核对SHA-256，全部一致。上游
许可证/历史保留。Cargo规范化manifest改回上游workspace manifest；Linux/
FreeBSD独立库测试增加test-only Fontique fontconfig-dlopen依赖，保持与适配器
相同的运行时加载方式。没有新生产Rust实现或unsafe。集合CPU探针的103个
源码/manifest哈希也与迁移前相同，普通字段热路径回退的优化与重测仍待完成。

新的scripts/check-engine-forks.py核对Cargo metadata、两份消费锁、完整来源、
重复身份及十包边界。--test在D:构建目录维护干净的正常Git checkout，以fork
自己的固定锁运行测试，不修改Cargo源码缓存，也不作为Glory的path依赖。
Windows/Linux gate均改用该入口。Blitz库86项、Taffy库185项通过；Taffy的
上游manifest通过self dev-dependency开启parse，所以比旧规范化manifest的
171项多出14项已有解析测试，不能理解成新增14项修复。

最终固定版本：Windows -Web -Window共647项通过，Linux --window共648项通过，
均含fork库测试、workspace单测/文档测试及真实窗口。两端严格Clippy通过；
Windows另有真实UIA、WASM workspace/fixture严格检查及fixture重建。Android
workspace/all-features/all-targets cargo check通过，仍只证明编译。原生大列表
继续保持峰值14个任务/77个引擎节点；本轮不新增CPU/FPS/RSS提速声明。
本轮未重新执行浏览器22套脚本；先前集合功能的浏览器证据仍独立保留。

诊断保留：首轮Windows链接出现PDB LIMIT错误，相关包经Cargo官方clean后完整
重跑成功；首轮Linux独立测试缺Fontconfig开发配置，单独环境变量尝试与
Fontique条件分支不一致也失败，已撤回该尝试，最终由fork的test-only依赖修正。
一次offline完整metadata因未下载的非本机依赖停止；锁的唯一差异核对后再做
锁定库测试及最终两端门禁。全部失败/父提交/最终日志保留。旧源码来源及补丁
作为迁移前历史证据移到D:构建证据目录，已不在仓库或依赖图中。

复现入口为scripts/check-semantic-ui.ps1 -Web -Window及
scripts/check-semantic-linux.sh --window；原始日志forks-final-{windows,linux}.txt、
forks-android.txt、逐blob核对与源码/锁/文档哈希归档到
D:/BuildCache/glory-semantic-ui/forks-evidence-2026-10-23.json。
构建/cache/tmp继续在D:，8788/8789无监听。完整改进目标保持进行中：集合热路径
优化、主题/更广CSS、非HTML resolver、真实OS IME/AT-SPI/读屏、Android宿主及
设备验收、其他R7投影和旧架构退出均没有因本次依赖迁移而宣布完成。

## 集合结构编辑与普通字段热路径（2026-10-23）

新增core的collections模块，公开普通Rust Cage方法：BTreeMap/HashMap的
insert/remove，以及Vec的push/pop/insert/remove/truncate/clear，均有try_形式。
只选择实际受影响的活跃逻辑路径，包含集合/祖先及选择子树；复杂revise仍保守。
不要求业务key/value Clone，不构造写入临时item/entry，不扫描源或无关兄弟。
Vec中间编辑依然执行标准库的数据移动；没有把框架窗口化当成数据操作O(1)。

无操作和访问/索引失败不发布、不唤醒或改变准备统计；版本在数据编辑前预检。
用户Hash/Eq/Ord/hasher不在图/注册表借用中调用，正常完成时重复/移除key的析构
延后到模型借用与发布之后。panic后的部分Vec修改仍发布实际状态，符合原revise
规则。HashMap保留自己的hasher与扩容行为，插入覆盖保留原存储key。

首版把选择器合入普通Target，旧字段高频路径的CPU对照出现回退。现将集合
准备独立为write_collection_target；普通参数及guard仅带类别/字段身份。一个
可复用数字选择器Box留在每根缓存槽；切回普通路径可保留，类别检查阻止旧集合
目标误命中。它只保存parent与内部entry身份/位置范围，与槽一起随成员或Owner
失效回收。根仍只有一个通知缓冲，没有逐key历史/多目标矩阵或用户析构。

8项新增集合测试覆盖区间/缺失/祖先/旁支、无操作与错误、非Clone数据和1MiB
payload地址、完整key碰撞及用户比较/析构回调、1,000步嵌套集合与普通数据oracle、
1,000次无观察key修改及Owner回收、实际1,024/131,072源与HashMap扩容，以及
析构panic后的真实状态/恢复。另有私有版本耗尽测试和3条新增接口文档例子。
这些不把alloc或用户panic伪装成AccessError。

一亿逻辑行的既有稀疏缓存测试改用显式remove/insert，连续300次远跳；每次
局部加载恰好求值一行，不重读窗口key。源仅持当前加载项；活跃路径、焦点pin、
临时writer及payload/Owner回收继续检查，不分配一亿个真实业务对象。真实浏览器
用例新增CSR/SSR接管的key删除/重插、另一key不求值、空删除完全无操作、Vec尾部
不计算位置0及前部插删恢复；焦点/反向选区/编辑器身份与无value覆盖继续校验。

性能分三套，均Windows CPU与Rust/runtime/std操作，不计源构造，不测分配/RSS/
布局/绘制/FPS。新基准在仓库core/benches/collections.rs，bench opt-level=3、
单codegen unit、LTO=true；真实源1,024/131,072、观察32/256、20批、burst1/128，
三进程各三轮，64组1,152样本。粗粒度版本可在一次revise组合两次业务操作，
精细API每次调用单独准备发布，最终Binding相同，不人为拆旧版本来偏置结果。

| 源131,072/观察256，20批单次替换或成对尾部编辑 | revise中位数 | 精细中位数 | 求值/最终Binding |
|---|---:|---:|---|
| BTreeMap替换 | 1.9963ms | 0.0188ms | 派生5,120→20，Binding均40 |
| HashMap替换 | 1.8859ms | 0.0204ms | 派生5,120→20，Binding均40 |
| Vec尾部push/pop | 2.0109ms | 0.2026ms | 派生5,120→0，Binding均20；精细发布次数为两倍 |

不能只列快组：64组中5组Vec前部精细编辑较慢，最慢是源1,024/观察256/burst128，
3.5716→5.3939ms（比值1.5102）；另4组约1.018–1.272。大量位置同时改变且可
合并为一次操作时，整体revise仍有优势。旧首版14个慢组及全部原始样本保留，
不把不同运行环境的绝对耗时当成首版→优化版的直接速度比。

既有API探针使用完全重建并核对100个源哈希的v31及同一输入程序，独立release
opt3/LTO=thin（不与上表绝对时间混算）。原20批/32组576样本中，优化后仍有
13组中位数慢于v31，整体中位比0.99466；最慢短组比1.06793，其原/后范围
1.2294–1.4061ms与1.2551–1.3936ms均保留。没有删除这些不利观测。

为检查此前高频字段回退，另建完全相同的2,000批程序，每样本200万次字段写入，
覆盖全部8个高频字段组，v31/首版/优化版旋转进程顺序，三进程各三轮，共216
样本。首版的103个源码/manifest全部核对；三个版本的探针行为与工作输出一致。
优化后各组中位数为首版的0.9336–0.9891；对v31为0.9626–1.0035，7组略快、
1组约慢0.35%。这支持本次表示调整，不证明所有旧API工作负载不存在回退。

输入、二进制、全部原始行与范围分别保存在collections-hot-*、collections-long-*
及collections-initial目录；集合首版代码/结果未覆盖。临时还原首版首次漏复制
新benchmark文件，哈希核对时停止；补齐后103项全部匹配才构建探针。新生产
改动仅fields.rs及collections.rs，无第三方源码修改，无新增unsafe。

最终门禁：Windows -Web -Window为647项通过；Linux主门禁644项加X11/Wayland
普通/AccessKit窗口4项，共648项。两端严格Clippy通过，WASM workspace/fixture
严格检查及新产物重建通过；独立fixture --lib两项请求级释放测试通过。此前误对
ssr_fixture bin运行test只得到0项，不能算验收；随后明确改跑含fields_form与
entries_form测试的lib，两项实际执行成功。日志两者均保留。

22套真实浏览器全部通过，无pageerror；entries用例的CSR/SSR逐阶段工作计数与
首版一致，最终runs=[5,4,2,4]、source_writes=11、targetBuilds=8/hits=3、两根
targetCaches/5个身份，positions=4。删除key7不计算key9，重复空删除不改任何
统计；草稿/焦点/反向选区/身份与value写入屏障保持。流式Shell/Patch/Patch/End、
断开取消及慢消费者背压另行通过，未与浏览器流测试并发消费相同服务器统计。

原生六次大列表窗口运行峰值均14任务/77引擎节点，key读取分别为Windows
677/667、X11/Wayland的666/676/668/676；真实UIA的远行显示、唯一focus pin、
编辑器身份与Owner释放继续通过。Android全workspace/all-features/all-targets
仍另做编译检查，不能作为Activity/Surface/设备完成证据。

全部build/cache/tmp、失败/初版/最终数据保留在D:。Web launcher及实际Python
worker、stream helper按父子PID与完整命令核对后停止，8788/8789无监听。来源、
最终源码/锁/报告、性能原始行和产物哈希由
D:/BuildCache/glory-semantic-ui/collections-hot-evidence-2026-10-23.json归档。
本批完成集合编辑的这一功能与表示优化；Deref/自定义或动态投影、别名及事件
Owner审计、集合事务、R7引擎细分、主题/更广CSS及非HTML resolver、真实平台
输入/辅助技术、Android宿主/设备和旧架构退出继续保留，完整目标不勾选完成。

## 注册 Owner 上下文与重复路径（2026-10-23）

按报告v33/§12.14继续审计，先保留失败回归：同Owner重选五条路径一次，节点
6→11而canonical字段仍5；事件重复选择也增节点。单独复用别名后，跨Scope
监听的字段仍在注册Scope.dispose后alive，确认归属错误。Effect/Structure也
继承flush调用方或source Owner，短期Effect留下已访问key。对应失败日志保留。

最终生产只改core fields/entries/runtime及ui app。canonical路径按Owner代际
key索引唯一别名，重复选择不重建通知目标，其他Owner独立退休。访问/类型/
layout/pure检查保持。Projection只在新路径构造；驻留/查找共用首候选及碰撞
候选快照，单候选无候选Vec分配；未用captures/key在图/注册表借用外析构。

Runtime::enter(instance, callback)先检查本Runtime的存活代际身份，不持图借用、
不保持Scope存活、不自动flush。UI在已有live/callback检查中读Subscription.owner，
按注册Scope执行，允许它监听别的Scope节点；没有额外UI Scope引用或逐事件
Owner快照。实际observer回调也进入节点Owner，与flush调用者无关。依赖验证/
旧值析构退休observer后不执行captured callback。Pure约束及task poll/commit
边界保持。不同map/不同key仍是Owner持有的新资源，不推断Copy句柄死点或
Effect每轮GC；临时跨key修改使用无路径注册的集合接口或短Owner。

新增13项回归：3项别名/独立Owner/未用capture析构，5项Runtime身份、嵌套、
Effect/结构归属、1,000个缺失key回收及依赖值退休，4项事件/跨Scope/panic/自我
卸载，1项一亿逻辑行。最后一项300次远跳、每行5次事件，普通行资源卸载失效，
唯一focus pin保留至blur：4,513次key读取，观测峰值62个响应式节点/62个节点槽/
95个Owner槽。不是引擎节点或RSS，也没有实际构造一亿项业务数据。

新增公开Rust builders构造的owner_form/client及owner-acceptance。CSR/SSR
接管均保持原按钮：1,000次点击节点5→7、field=1、两句柄alive；隐藏后节点5、
field/target=0、句柄失效，旧DOM点击无效。100次卸载重挂载后count=1100，节点槽7、
Owner槽13保持，1MiB payload地址不变。第三个SSR fixture lib测试验证请求释放。

CPU对照还原并核对103个v32 core/style/render输入及全部UI生产输入，同一独立
程序，release opt3/LTO=thin，交替进程顺序，三进程各三轮，10组180样本。
源构造不计时；没有allocator/RSS/renderer/FPS测量：

| 负载 | v32中位数 | 最终中位数 |
|---|---:|---:|
| field重复选择/写入2,000次 | 9.9218ms | 0.5917ms |
| item重复选择/写入2,000次 | 10.1656ms | 0.5685ms |
| entry重复选择/写入2,000次 | 10.2298ms | 0.6242ms |
| 100,000个直接空scroll | 13.8096ms | 14.7198ms |
| 同上并写根 | 15.5356ms | 17.1755ms |
| 100批×1,000个queued空scroll | 13.1932ms | 16.4905ms |
| 同上并写根 | 14.8755ms | 18.6024ms |

6组重复选择更快：2,000次节点2,004→5、目标身份2,001→2、重建2,000→1、命中
0→1,999、Binding均2,000。4组事件微基准仍慢约6.6–25.1%，保留新增Owner校验/
上下文的成本，不将原错误生命周期路径当成同等能力的更快方案，也不外推
真实滚动帧。已保存字段另测8组144个加长样本（每样本200万写入），最终/旧
中位比0.9424–0.9942；输出/工作相同，只合并一个重复别名节点（774→773）。

WeakScope包装handler、复制Owner进路由快照、TLS Cell等原型和全部较慢样本
独立保留；Cell未取得可靠CPU收益，已还原现有嵌套Vec guard。最终使用已存
注册身份和live检查，不按较快一组覆盖较慢观测。

最终Windows -Web -Window 660项、Linux主门禁657加X11/Wayland窗口4项共661项
通过（含fork测试）。两端严格Clippy、WASM workspace/fixture严格检查与新产物
重建、SSR fixture lib实际3项、23套浏览器全部通过且无pageerror；流式
Shell/Patch/Patch/End、慢消费者/断开取消另行通过。Android单独编译检查仍只
证明编译。构建/cache/tmp都在D:，第三方仍为固定组织fork，没有vendor。

诊断保留：测试侧用Rc<Runtime>替代不存在的Runtime.clone；fixture修正click
名称、两类launch错误及type-complexity；探针补齐Render::request后才运行。
上下文不匹配的大patch均未写文件。Web launcher随worker自动退出，在检查后
停止父进程时遇到已退出竞态；最终父/子/stream PID及8788/8789均核对清理。

完整源码、锁、文档、最终/原型样本与二进制哈希归档为
D:/BuildCache/glory-semantic-ui/owner-alias-evidence-2026-10-23.json。
cleanup/应用级query策略、动态投影/事务、引擎阶段、主题/更宽CSS与非HTML
resolver、真实OS IME/辅助技术、Android宿主/设备及旧架构退出继续推进，完整
目标保持进行中，不以本节替代剩余生命周期或平台审计。

## 清理回调与Scope查询（2026-10-23）

两条改动前失败见证保存在owned-query-before-cleanup.txt和
owned-query-before-reply.txt：旧清理回调从全局模型创建field时落到卸载
调用者Owner；旧App查询回复创建的field/map随pump调用者Owner提前失效。
修改前runtime.rs/app.rs及93个v33生产输入经上一批归档SHA核验后独立保存。
对照只复制Glory自身源码，第三方仍为固定组织Git fork，没有vendor。

on_cleanup按Owner分组，在全部身份失效后untracked进入登记的退休Owner，
保持子先于父、同Owner逆序、无图借用。临时读写其他活状态仍允许，隐式
路径/派生构造不能逃入调用者/源Owner，显式进入其他活Owner仍可构造。
Task取消也untracked进入任务Owner：直接取消时Owner可活，清理时已退休。
不突破Pure；panic展开恢复上下文，不保证剩余回调或重试。任意用户值Drop
的上下文未由本批统一，不能据此宣称完整析构审计完成。

新增Scope::request，App::request显式代理应用根。回复归请求者、untracked，
不归目标/pump调用者；屏障复用原flush，在屏障后重验请求者再发送。注册表
只保存InstanceId及closure，不增加Scope持有。双索引为Owner→token和target→
token；等待项在请求者退出时直接删除，捕获在借用外析构。目标退出标记
NodeGone，只向活请求者交付；取消队列仅保存token，所以请求者后来退出也
能立即释放已取消项。全部终止路径移除空桶，销毁优先于同批晚到成功回复，
水位/typed结果验证保持，后端协议不变。

新增12项回归：core 4项覆盖退休构造拒绝、清理read不订阅外部Effect、活/退休
任务取消归属及嵌套Runtime/panic恢复；UI query 7项覆盖App/Scope归属、2,000个
等待项立即释放、已取消项回收、同批竞态、屏障卸载及panic恢复；列表1项为
一亿逻辑行、300次远跳、每行5个跨Scope查询。1,500个捕获在行卸载时析构，
随后排空真实Memory回复不执行回调，外部目标保持；4,212次key读取、Owner槽
峰值75、固定1MiB共享payload不留历史引用。没有实际分配一亿项数据。

共享Rust query_form/client、queries.html和query-acceptance.js验证CSR/SSR
接管保留原按钮。每种模式先成功1次、取消100次、再成功1次：sent=102、
called=2、captured dropped=102，query/Owner/target索引归零。活回复构造2个
响应式节点，隐藏回到5个，节点槽7/Owner槽15保持；外部input身份、内容/选区
及源payload地址不变。最后unmount清空；新增SSR lib测试验证无host查询和
模型释放。它不是实际OS IME/屏幕阅读器或实际引擎帧时间证据。

CPU对照沿用现有semantic_frames全部程序：同package、release opt3/LTO=thin，
3组交替进程、每进程3轮，8组144个run样本。计1,000次runtime/UI/Memory更新
累计时间，含100次预热、源构造不计稳态；无allocator/RSS/引擎Layout/Paint。

| 逻辑行数/更新/profile | v33中位数ms | 最终中位数ms | 最终/旧 |
|---|---:|---:|---:|
| 1M/远跳/off | 281.1655 | 286.7582 | 1.0199 |
| 1M/远跳/on | 281.8336 | 289.7055 | 1.0279 |
| 1M/同窗口/off | 0.6999 | 0.7146 | 1.0210 |
| 1M/同窗口/on | 0.9104 | 0.9003 | 0.9889 |
| 100M/远跳/off | 267.6608 | 273.1888 | 1.0207 |
| 100M/远跳/on | 272.5257 | 297.1318 | 1.0903 |
| 100M/同窗口/off | 0.6885 | 0.6915 | 1.0044 |
| 100M/同窗口/on | 0.9008 | 0.9229 | 1.0245 |

7组变慢、1组变快，全部样本保留，不以小差异宣称显著性或CPU收益。远跳
每组均33,000次key读取、最多68个UI节点/111个响应式槽；同窗口零key读取、
62/102。所有输出及profile calls/units相同。普通列表程序不能证明旧版具备
Scope查询回收能力，新增能力另由生命周期及浏览器验证。

最终Windows -Web -Window 672项、Linux 669加X11/Wayland窗口4项共673项，含
fork测试、两端严格Clippy及WASM workspace/fixture严格检查；WASM/SSR产物
实际重建，SSR fixture lib 4项，24套浏览器无pageerror。重建stream_fixture
另通过Shell/Patch/Patch/End、慢消费者及断开取消。Android完整workspace/
all-features/all-targets仍只证明aarch64编译。两个helper核验exe/命令行后
退出，记录PID与8788/8789均已清理；构建/cache/tmp都在D:。

诊断：正式旧版回复失败见证前修正测试临时Wire借用；初次编译修正cancel
补丁位置，新列表fixture补FnMut内部Rc副本及多root定位。临时Linux窗口
脚本CRLF在测试前失败，LF修复后实际跑完4项，原错误单独保留。上下文不匹配
的文档patch原子失败后重做，没有隐藏修改或改第三方源码。

源码、锁、对照输入、全部原始样本/二进制及门禁哈希见
D:/BuildCache/glory-semantic-ui/owned-query-evidence-2026-10-23.json。
完整目标继续：任意值析构上下文、动态投影/事务、引擎阶段和缓存、主题/更广
CSS与非HTML resolver、真实OS IME/辅助技术、Android宿主/设备、旧架构退出。

## 调度队列与大量待更新绑定（2026-10-23）

旧State::remove_node无条件retain三个VecDeque，每移除一个source/field/
derived/observer都扫描全部待执行身份。大页面一批绑定未执行时，窗口化列表
只卸载少量行也会重复扫描无关工作。改为马上从活集合取消观察者，在FIFO只
保留有限的完整代次Key；达到阈值后按arena身份统一压缩，不保活closure/值。
普通source/derived从未入队，直接跳过集合查找；单一数值计数器让无废弃ID
时不计算压缩条件。没有增加逐节点队列链接、逐行缓存或第三方依赖。

取消和取出活观察者后、进入回调前，物理entries≤2*queued+64；queued=0则
entries=0。取出废弃Key同时减计数，空队列以Copy Key直接清长度。阈值压缩
只检验完整代次，不访问用户值，保持Structure/Binding/Effect优先级及各自
活FIFO。作用域值和捕获仍在原清理点、无图借用下析构。allocated capacity
保留各阶段峰值，不保证clear后退还全部内存；线性压缩是摊销优化，不是
固定时限/全局flush预算。RuntimeStats增加entries/capacity及压缩次数/检查
身份数，queued仍为活成员，均不能当RSS/FPS指标。

新增core/tests/queues.rs并在autotests=false的manifest显式注册5项：小退休不
扫描4,096个活绑定且捕获即时析构、旧代槽复用不执行旧回调且阶段/FIFO正确、
5,000次三阶段行退休保持空间界限/顺序、drain期间回调重入仍见界限，以及
Effect运行时自我重新排队再退休不复活。UI新增一亿逻辑行+16,384待绑定用例：
远跳先退出旧行，随后按原序执行全部活绑定，15次新key读取、0次压缩扫描，
最终两类队列长度为0。原有依赖/equality/panic/runaway/Owner回归全部保持。

准备93个经v34归档SHA核验的自身生产输入及独立同package程序，不复制第三方。
release opt3/LTO=thin；3对交替顺序进程、每进程3轮，每格每侧9样本。首次
简单压缩候选及其较慢空队列结果、源码/二进制单独保留；最终加入废弃计数
快路径和source/derived跳过查找后再跑正式样本，未覆盖候选观测。

6组108样本的Owner探针，计1,024次创建/退休，背景绑定构造不计时，活回调
另行drain计时；输出、完成数、节点/槽相同，已退休观察者从未执行。

| 待绑定/行资源 | 旧中位ms | 最终中位ms | 最终/旧 |
|---|---:|---:|---:|
| 0/source | 0.3256 | 0.2858 | 0.8778 |
| 0/source+三阶段observer | 0.6409 | 0.6246 | 0.9746 |
| 4,096/source | 2.0147 | 0.3532 | 0.1753 |
| 4,096/source+三阶段observer | 7.3040 | 0.7892 | 0.1081 |
| 65,536/source | 29.7357 | 1.9471 | 0.0655 |
| 65,536/source+三阶段observer | 113.9511 | 2.5553 | 0.0224 |

另外记录的drain中位比：4,096 observer组1.0625、65,536 source组1.0256，
这两组变慢；其他两组变快，零背景两组近时钟分辨率的100/200ns不作意义推断。

原semantic_frames完整程序8组144个run样本（1,000次实际UI/Memory更新累计，
100次预热），所有输出和phase calls/units一致，4组更快、4组更慢：

| 逻辑行数/更新/profile | 旧中位ms | 最终中位ms | 最终/旧 |
|---|---:|---:|---:|
| 1M/远跳/off | 299.6188 | 292.2005 | 0.9752 |
| 1M/远跳/on | 304.2002 | 284.5947 | 0.9356 |
| 1M/同窗口/off | 0.7296 | 0.7099 | 0.9730 |
| 1M/同窗口/on | 0.9021 | 0.9247 | 1.0251 |
| 100M/远跳/off | 270.9825 | 272.5122 | 1.0056 |
| 100M/远跳/on | 286.6192 | 294.1143 | 1.0262 |
| 100M/同窗口/off | 0.7080 | 0.6963 | 0.9835 |
| 100M/同窗口/on | 0.9176 | 0.9318 | 1.0155 |

实际App/List/Memory另做3组54样本：固定一亿逻辑行，4次预热+16次计时远跳。
每次先注册当批背景绑定，注册/初始构造在计时外；计时包括position修改、
真实控制流/行卸载、活绑定drain和内存提交，随后才退出背景Scope。

| 当批待绑定 | 旧累计中位ms | 最终累计中位ms | 最终/旧 |
|---|---:|---:|---:|
| 0 | 1.9563 | 2.0407 | 1.0431 |
| 4,096 | 9.6278 | 8.2500 | 0.8569 |
| 65,536 | 116.1537 | 98.5692 | 0.8486 |

此处前后均224次key读取、UI节点峰值30，响应式槽分别54/4,150/65,590，完成
背景回调0/81,920/1,310,720次（含预热）。这证明有大量活队列时的实际框架
切片收益；无背景慢4.3%也保留，不把局部收益外推为所有滚动改善。306个正式
样本及候选观测全部保留，没有allocator/RSS/真实引擎Layout/Paint/FPS测量，
也没实际分配一亿项业务数据，不能与上一批不同时间段的绝对数拼接推断收益。

本轮检测到外部移除rustfmt.toml（原max_width=150），保留该删除后按默认100
格式化活动源码/独立web fixture，未恢复或另设替代配置。88个对照生产Rust
文件（除修改的runtime）经相同rustfmt stdin、skip_children、edition2024
归一化后字节相同；raw TokenStream比较会因新增尾逗号误报，因此保留初始
诊断并改用完整格式归一化。其他adapter的格式输出随完整门禁验证，不宣称有
未保存前像的逐字差分证明。初次新测试因autotests=false未注册，补manifest
后才执行；初始候选、错误日志及旧输入均未替换。

最终Windows -Web -Window 678项、Linux 675+X11/Wayland窗口4项共679项通过，
含fork测试、两端严格Clippy与WASM workspace/fixture严格检查；新WASM/SSR
已重建，SSR fixture lib 4项，24套浏览器无pageerror，重建stream_fixture另
通过Shell/Patch/Patch/End、慢消费者/断开取消。Android仍是完整aarch64编译
检查。两个测试helper核验实际exe/参数后关闭，记录PID和8788/8789均清理。

证据及完整源码/样本/二进制哈希：
D:/BuildCache/glory-semantic-ui/queue-retire-evidence-2026-10-23.json。
目标仍进行中，保留任意值析构上下文、动态投影/事务、引擎阶段与缓存、主题/
更广CSS和非HTML resolver、实际OS IME/辅助技术、Android宿主/设备及旧架构
退出。队列优化没有改变它们的完成定义。

## v36：固定聚合的原生引擎细分计时（2026-10-23）

本批补 R7 的观测基础，不以计时器代替实际布局优化。Blitz fork 固定到
bd2375bc010300fc734e73ef245208d4d242d0a3，保留包名、仓库结构、许可证；
四个直接 Git 依赖、主/示例锁及 engine-forks.json 一起升级。Taffy 未改。
独立 fork 测试流程新增 extra_tests 的 parallel-construct 配置。

引擎 collector 默认 None；开启后固定 13 槽，以 Arc/Mutex 支持并行文本
构建。时间来自引擎 web_time::Instant；不执行用户回调，不持有节点/文本/帧。
父文档子文档循环为包含耗时，子文档 collector 需显式挂载，不能把循环次数
当子文档数量。reset 在锁内原子切换非回绕 epoch，旧 span 不污染新周期。

Glory Profile 从18扩为31槽，原序号不变；新增 Recording 捕获本地epoch，
只接受形状有效且代次仍活的外部聚合，导入不读取其时钟。Dom::resolve 用
RAII 在同步工作/并行 join 后导入快照，展开时也收集已完成的阶段；Glory
的 Rc collector、Runtime 和业务对象不进入 worker。更换/移除 collector
不标脏；不同文档即使共享 RuntimeId 也隔离。计量覆盖与包含关系见契约。

新增两项 render 记录导入测试、两项 native 文档/缓存隔离测试，并增强原有
native shared-profile 测试与浏览器 profile 验收。Fork 新增五项测试覆盖
reset/展开、4000次并行样本、epoch耗尽、空文档与真实 Parley 文本变化。
真实字体高度必须大于零，不能用没有字体的空布局假装通过。

原生 profile 样例新增 --quick/--no-profile，默认仍每格3次重复、10次预热、
100次计时更新。新增 scripts/check-semantic-profile.py 交替开关并保留每次
stdout/stderr、二进制SHA及工作量；参数输出目录必须为新的 D: BuildCache
目录。开启计时不能改变 key 读取、节点峰值或像素缓冲预算。脚本不编译，
构建与实际 target 检查由前置命令负责。

本轮重新创建 D: Cargo 缓存。此前 D: 构建/原始证据不在当前环境，不能重新
验证历史前像；v35等段落为历史记录，不将其数值与本轮拼接。Linux最初使用
经验证的D:直接挂载，依赖展开卡在9p文件访问；明确终止该次gate，保留初始
日志，再创建 D:/BuildCache/glory-semantic-ui/linux-build-cache.ext4 并挂载
到 linux-cache，脚本验证 loop 的实际 D: backing 后重新执行。

本轮首组 release 对照使用仓库现有 opt-level=z、LTO、codegen-units=1，
4对交替×每次24格，共192个run/19,200次计时更新；另计各格冷启动与预热。
百万/一亿逻辑行均按需生成内容，并没有分配一亿项业务数据。定高每100次
更新读900个key，峰值20语义节点/39响应槽/37引擎节点；测量行读3100个key，
峰值24/45/43。所有开关对照的这些工作量与缓冲大小相同。

| 逻辑行数/行高/绘制 | off中位ms/更新 | on中位ms/更新 | on/off |
|---|---:|---:|---:|
| 1M/定高/逐轮 | 0.9777 | 0.9384 | 0.9599 |
| 1M/定高/稳定后 | 0.9816 | 1.0353 | 1.0546 |
| 1M/测量/逐轮 | 2.6335 | 2.5600 | 0.9721 |
| 1M/测量/稳定后 | 1.6122 | 1.6638 | 1.0320 |
| 100M/定高/逐轮 | 0.8624 | 0.8505 | 0.9862 |
| 100M/定高/稳定后 | 0.9506 | 0.8119 | 0.8541 |
| 100M/测量/逐轮 | 3.1418 | 2.6354 | 0.8388 |
| 100M/测量/稳定后 | 2.1864 | 1.9909 | 0.9106 |

每格每开关12个样本；保留全部较慢组。本次机器上仍有并行构建负载，on比
off更快不代表计时使程序更快；该对照不能确定隔离环境下的诊断开销，更不是
修改前后比较。不能把这些离屏CPU值称为FPS、RSS或所有滚动的保证。

100M/测量/稳定后开启计时的包含中位数ms/更新：Layout 0.7732，Style
0.3640，PostConstruct 0.2217（内含TextShape 0.1842），BoxLayout 0.1352
（内含TextBreak 0.0309、TextAlign 0.0014），Paint 0.2608，Draw 0.4453。
各阶段分别取中位，不能相加，也不能将Draw解释成纯光栅；稳定阶段的多轮
布局有些发生于Draw之前。下批优先审计变高列表的多轮样式/布局失效与文本
重复构建，保持资源/焦点/CSS失效正确；不先做跨访问行的全局文本历史缓存。

原始对照：D:/BuildCache/glory-semantic-ui/engine-profile-on-off/evidence.json。

因首轮存在明确的本任务并行编译干扰，在所有门禁结束后补同参数第二组，
未替换首组：仍192个run/19,200次更新，全部工作量一致；--quick的开/关
也分别实际运行通过。两组共384个正式run，不将快速自检混入统计。

| 门禁后复测 | off中位ms/更新 | on中位ms/更新 | on/off |
|---|---:|---:|---:|
| 1M/定高/逐轮 | 0.6537 | 0.6678 | 1.0216 |
| 1M/定高/稳定后 | 0.6590 | 0.6535 | 0.9917 |
| 1M/测量/逐轮 | 1.8698 | 1.8994 | 1.0158 |
| 1M/测量/稳定后 | 1.2274 | 1.2519 | 1.0200 |
| 100M/定高/逐轮 | 0.6521 | 0.6710 | 1.0291 |
| 100M/定高/稳定后 | 0.6473 | 0.6611 | 1.0213 |
| 100M/测量/逐轮 | 1.8601 | 1.9215 | 1.0330 |
| 100M/测量/稳定后 | 1.2338 | 1.2507 | 1.0138 |

七格开启较慢，一格略快；当前程序的中位耗时变化约-0.8%至+3.3%，不能
外推所有应用诊断开销或声称计时器带来提速。本任务编译已结束，不保证整机
没有其他进程负载。第二组100M/测量/稳定后on包含中位ms：Layout .4426，
Style .2005，PostConstruct .1269（TextShape .1080），BoxLayout .0828
（TextBreak .0175、TextAlign .0009），Paint .1641，Draw .3488。

实际100次稳定后更新：定高Layout/Style各100次、TextShape900次、断行/
对齐900次；测量行分别300/300、2000及4000次，Paint/Draw仍各100次。
计数说明测量反馈保留多轮布局、减少的是中间绘制；不能先假定三轮都冗余。
下一步应定位每轮对应的行集合/尺寸依赖，仅消除可证明重复的失效或文本
构建，继续保留所需反馈、焦点和资源更新。第二组原始证据为
D:/BuildCache/glory-semantic-ui/engine-profile-on-off-post-gates/evidence.json。

最终门禁：Windows -Web -Window 692项，Linux基础689项加X11/Wayland各
shell/accesskit共4项，即693项；这些总数包含185项Taffy、91项Blitz与5项
parallel-construct profile测试。两端严格Clippy通过，WASM workspace/fixture
严格检查与实际重建通过。Chromium 155的24套验收均无pageerror；profile
验收读到31阶段，原生内部阶段为零。SSR fixture lib 4项、重建stream_fixture
的Shell/Patch/Patch/End、慢消费者及断开取消通过。Android全workspace/
all-features/all-targets的aarch64检查通过，仍只证明编译。

Linux初次窗口因libxkbcommon-x11缺失失败；Ubuntu两个deb仅解包到D:运行时
缓存，未全局安装。期间WSL退出卸载了loop映像，导致一次解包落到下层9p；
保留失败日志，改为同一root脚本挂载/解包并runuser执行检查。X11随后通过，
Wayland初次NoCompositor，改用WSLg实际绝对socket路径后完整四项均通过。
初始gate、解包和Wayland错误都保留，最终通过不覆盖初始诊断。
Windows UIA实际.NET客户端结果为ok，包含两窗口隔离、操作、密码保护及
大列表reveal；不外推实际屏幕阅读器播报或Unix AT-SPI客户端验收。

测试HTTP/stream helper分别为PID69224/34984；核验真实exe与命令行再关闭，
8788/8789监听及两个进程均清理，记录见engine-profile-helpers-stopped.json。
完整输入哈希、最终门禁/诊断日志及样本索引汇总在
D:/BuildCache/glory-semantic-ui/engine-profile-evidence-2026-10-23.json。
目标仍未全部完成：任意值析构上下文、动态Deref/自定义路径及事务、收益
驱动引擎缓存、主题/更广CSS与非HTML resolver、真实OS IME/辅助技术、Android
宿主设备及旧架构退出仍按原准入推进。

## v37：变高行反馈与重复失效（2026-10-23）

先核对v36的287项源码/锁/文档输入并保存十包旧工作树、原release二进制。
查明旧profile样例在inline Text上设置height，因此实际约18px，并没有得到
24/28/32px变高分布。保留旧样本为adaptive API历史测量，不能继续借它声称
真实变高性能。本批统一改为View给出真实高度、内含Text，查询每个初始mounted
row的实际Bounds并断言24/28/32px；修正版程序在旧/新源码中逐字相同。

追踪首个远跳：旧实现先布局新行，ScrollTo后再次完整resolve，测量回调修正
位置/锚点后又布局，再因ScrollTo重复完整resolve。该切片4→2，不代表所有
帧都有四次或三次；新行、字体/图片、宽度与锚点反馈必须按需要保留。

Glory的唯一生产改动在blitz adapter：拆开布局失效与像素失效。瞬时ScrollTo
前仍resolve，写offset后引擎已经更新geometry generation；改变时推进visual
revision、保留observation与ack，但不自动再标布局脏。立即refresh_hover，
命中改变则回退完整失效，继续响应hover CSS尺寸变化。新测覆盖纯滚动盒/文本
复用、变化坐标/像素版本/ack、空动作和后续CSS更新，以及滚动下新hover目标
需要布局的情况。资源/动画/viewport/普通Patch及所有提交/查询屏障保持原路径。

Blitz fork固定b40cdfc2c9b5a000759275b673e4ed8aadd68f78。inline style的Set/
Clear保留snapshot、父/祖先与selector/inherited restyle，仅不再预置ALL_DAMAGE，
改用计算样式差异。自定义Widget继续保守完整失效，其他属性不变。新4项fork
测试含offset/height不重塑、等价声明不塑形/断行、Clear回退、16种连续变化
相对强制完整重建几何一致及属性选择器关联文本；95项库测试通过，parallel
配置另执行原5项计时测试与4项样式测试。custom-widget配置编译也通过。

曾用:has属性规则探测，旧/新都未匹配该规则；保留两份失败日志，不将该
既有raw CSS范围扩充成已支持。正式关联选择器测试使用已工作的相邻节点规则。
有限Sheet本来就不接受:has，不扩大其能力声明。

改正行高后，64项测量缓存出现同一二进制不同窗口的情况：同批pending HashMap
次序赋予不同年龄，影响后续跨窗口保留集合。初次96个quick run拒绝配对，
全部原始日志保留。正式对照使用measure_limit=1（有效下限为当前窗口行数），
只保留当前窗口测量，前后相同；普通64缓存仍由既有原生/浏览器回归覆盖，
没有为压低耗时引入全源扫描或所有访问行的历史缓存。此配置的数字不能直接
外推较大缓存的速度。

还发现共享target产物的滚动计数不符合保存的旧源码，故共享target的初期
计时全部作废，不作为结论。最终用feedback-before-target/feedback-after-target
两套D: target，复制外部依赖构建缓存后，官方cargo clean仅清理十个自身包；
分别检查metadata，fresh重编译各自本地库/样例。compiler-artifact保存实际
manifest_path/executable，确认before来自旧树、after来自当前树且fresh=false。
除同一份基准修正外，旧树的286项其他输入仍与保存前像相同；不修改Cargo
管理的Git源码。两端同Rust工具链、opt-level=z、LTO、codegen-units=1。

新增compare-semantic-profile.py保留每个stdout/stderr及二进制SHA，交替旧/新、
profile开/关，严格按pair/重复/场景比对每次更新最终RGBA序列的FNV校验、挂载
节点/图槽/引擎节点和像素预算。图像校验在计时外；实际行高查询在冷阶段。
每格每版本每开关12样本，每样本10预热+100计时更新；正式远跳与程序化物理
滚动各384个run，共768个run/76,800次更新。另有quick诊断，不混入统计。
后者先定位至逻辑源中部，再每步按当前物理offset增加17px，包含实际ScrollTo
和测量校正。远跳预热后物理位置始终在分段中心，计时段无ScrollTo，不能用
它冒充普通物理滚动。下表均profile关闭的观测中位ms/更新。

| 逻辑行/行高/远跳绘制 | 旧 | 新 | 新/旧 |
|---|---:|---:|---:|
| 1M/定高/逐轮 | .7406 | .7399 | .9991 |
| 1M/定高/稳定后 | .7371 | .7349 | .9970 |
| 1M/变高/逐轮 | 1.2625 | 1.2527 | .9922 |
| 1M/变高/稳定后 | .9977 | .9748 | .9770 |
| 100M/定高/逐轮 | .7278 | .7335 | 1.0079 |
| 100M/定高/稳定后 | .7315 | .7426 | 1.0153 |
| 100M/变高/逐轮 | 1.2715 | 1.2633 | .9936 |
| 100M/变高/稳定后 | 1.0118 | .9963 | .9847 |

| 逻辑行/行高/17px物理滚动绘制 | 旧 | 新 | 新/旧 |
|---|---:|---:|---:|
| 1M/定高/逐轮 | .5787 | .5760 | .9954 |
| 1M/定高/稳定后 | .3943 | .3789 | .9608 |
| 1M/变高/逐轮 | .7581 | .7416 | .9782 |
| 1M/变高/稳定后 | .4577 | .4537 | .9912 |
| 100M/定高/逐轮 | .6191 | .6165 | .9957 |
| 100M/定高/稳定后 | .4115 | .4062 | .9871 |
| 100M/变高/逐轮 | .7965 | .7891 | .9907 |
| 100M/变高/稳定后 | .4896 | .4819 | .9843 |

所有正式样本最终图像序列校验与节点/像素预算一致。每100次远跳定高key900、
变高2900；物理滚动定高736、变高840，前后相同。远跳最大29语义节点、
39/42响应槽、46引擎节点；物理滚动最大32/42/50，百万与一亿规模不增长。
没有分配一亿项业务模型，不把这些槽数当RSS。

一亿变高物理滚动每100次：Action均150，Layout/Style 274→124（少54.7%）；
TextShape/TextBreak均60，稳定后Paint/Draw均100。移除的是150次滚动后的
重复pass，必要新行/测量反馈保留。开启profile的包含中位累计ms：Layout
14.4356→13.6392、Style9.5457→9.3879、Construct .5328→.3085、BoxLayout
2.7531→2.4586、TextShape .7032→.7002、Draw24.9565→25.0088；阶段嵌套，
不能相加或把Draw当纯光栅，Draw稍慢也保留。空pass有缓存命中，计数降幅
不等于实际CPU/帧率降幅。新行文本在View隔离下原本已复用得当，该场景的
塑形次数没有减少；不能把fork单项测试的收益套到所有列表。

远跳定高Layout均100、变高均200，必要两轮没有被删。一亿定高远跳两格慢
.8%/1.5%，完整保留。此批只支持特定离屏CPU工作负载的观测差异，尤其1–2%
小差异不承诺统计显著或通用提升；未测真实wheel普遍收益、OS呈现/FPS或RSS。

正式样本：
D:/BuildCache/glory-semantic-ui/feedback-isolated-seek-formal/evidence.json
D:/BuildCache/glory-semantic-ui/feedback-isolated-scroll-formal/evidence.json

最终Windows -Web -Window 702项、Linux --window 703项通过（含185 Taffy、
95 Blitz、parallel profile5/style4及实际窗口）；严格Clippy、WASM workspace/
fixture与实际重建通过，最终benchmark参数/断言更新后另跑example严格Clippy
和全workspace格式检查。24套Chromium验收无pageerror，SSR fixture lib4项、
重建stream_fixture及Shell/Patch/Patch/End、慢消费者/断开取消通过。Android
全workspace/all-features/all-targets的aarch64检查通过，仍仅编译证据。

Linux首次链接Bus error时D: ext4 cache已99%使用、只余324MiB；保留日志，
验证loop backing确属D:后扩32→64GiB，再完整门禁通过。此前benchmark query
捕获借用、progressive额外绘制断言、64缓存配对及共享target身份诊断都保留，
不能把它们混入正式样本。两个fixture helper PID66332/19516核验exe/参数后
停止，进程与8788/8789监听均释放。主仓库未提交；第三方修改已在组织fork
提交并推送，未创建PR。完整源码、锁、fork、编译产物与原始日志哈希汇总：
D:/BuildCache/glory-semantic-ui/feedback-evidence-2026-10-23.json。

后续优先级：P1让同批测量的年龄顺序可复现（避免HashMap遍历赋龄），在默认
64/1024预算重新建立可配对工况；P1继续定位没有语义/视觉变化时的观测几何
读取，先加计数与事件/查询回归再跳过；P2按实际样式失效分类收益决定是否
增加有界文档缓存。没有依据删除必要测量反馈，也不为所有访问行缓存文本。
完整架构目标的OS IME/辅助技术、Android宿主设备、主题/更广样式与非HTML
resolver、动态投影/事务及旧架构退出仍未完成。

## v38：测量缓存的稳定淘汰与64/1024独立构建复验（2026-10-23）

本节接替v37末尾的P1淘汰顺序任务。Measurements的pending现在为FIFO Vec加
key→slot索引，同key在本批首次位置合并最终index/height，apply按顺序赋龄；
rebuild也按age应用高度。clear/drain后复用有界批缓冲，无排序、业务源扫描或
访问历史。五项单元测试加入最终门禁，覆盖16个随机HashMap实例及真实小数
prefix的bit一致性。原始v37回归探针返回101，修复版返回0；这是保留的预期
负向证明，不属于最终测试失败。

正式前/后的共同部分包括该确定性修复与完全相同的profile.rs。前为保存的
v36库/锁，只有measure.rs和profile.rs两文件更改；其余原始输入哈希保持。
后为当前v37原生优化加v38修复。前fork bd2375bc010300fc734e73ef245208d4d242d0a3，
后b40cdfc2c9b5a000759275b673e4ed8aadd68f78；Taffy和六包来源维护方式未变。
不能称这些数字为v38相对随机v37的纯FIFO速度差异。

源在measurement-before-src，两个独立target为measurement-before-target和
measurement-after-target。复制依赖构建缓存后，各target用cargo clean --profile
release清理十个自身包，再独立cargo build --locked -p glory-blitz --features
raster --example profile --release --message-format=json-render-diagnostics；
metadata、编译日志和compiler-artifact的fresh=false/manifest/executable已核验。
共同benchmark和measurement修复字节一致；只使用最后构建的两份二进制。

实际View盒为24/28/32px，初始查询逐行断言；每次更新后再断言全部mounted行。
缓存分别6/104次预填充迭代达到64/1024，再额外移动一个窗口触发淘汰。预填充
位于最后八分之一区域，计时区range必须小于该区域前1000行，避免1024大缓存
提前测过计时行。预填充、最终RGBA滚动校验及逐行几何/滚动状态校验均在计时
外。64/1024的估算历史可以不同；前后同一预算及各自重复、profile开关、绘制
管线的物理/逻辑位置、range/total、行盒序列和RGBA校验一致。

每个预算×运动一组384 run，共1536 run/153600计时更新。每格profile状态、
管线及变高标志12个样本，每样本100更新。下表全为profile关闭，单位ms/更新；
“变化”直接按中位数之比计算，正数是变慢，所有32格均保留。

| 缓存 | 场景 | 逻辑行数 | 行高 | 绘制 | 前 ms/更新 | 后 ms/更新 | 后/前变化 | Layout/100次 |
|---:|---|---:|---|---|---:|---:|---:|---|
| 64 | 远跳 | 百万 | 24 | 逐轮 | 0.7349 | 0.7367 | +0.24% | 100→100 |
| 64 | 远跳 | 百万 | 24 | 稳定后 | 0.7425 | 0.7393 | -0.44% | 100→100 |
| 64 | 远跳 | 百万 | 24/28/32 | 逐轮 | 1.2814 | 1.2779 | -0.27% | 200→200 |
| 64 | 远跳 | 百万 | 24/28/32 | 稳定后 | 1.0022 | 0.9942 | -0.80% | 200→200 |
| 64 | 远跳 | 一亿 | 24 | 逐轮 | 0.7334 | 0.7332 | -0.03% | 100→100 |
| 64 | 远跳 | 一亿 | 24 | 稳定后 | 0.7373 | 0.7310 | -0.86% | 100→100 |
| 64 | 远跳 | 一亿 | 24/28/32 | 逐轮 | 1.2783 | 1.2806 | +0.18% | 200→200 |
| 64 | 远跳 | 一亿 | 24/28/32 | 稳定后 | 1.0006 | 0.9998 | -0.08% | 200→200 |
| 64 | 17px滚动 | 百万 | 24 | 逐轮 | 0.5801 | 0.5753 | -0.82% | 174→74 |
| 64 | 17px滚动 | 百万 | 24 | 稳定后 | 0.3897 | 0.3836 | -1.58% | 174→74 |
| 64 | 17px滚动 | 百万 | 24/28/32 | 逐轮 | 0.8187 | 0.8002 | -2.25% | 296→141 |
| 64 | 17px滚动 | 百万 | 24/28/32 | 稳定后 | 0.4773 | 0.4688 | -1.79% | 296→141 |
| 64 | 17px滚动 | 一亿 | 24 | 逐轮 | 0.6130 | 0.6120 | -0.17% | 174→74 |
| 64 | 17px滚动 | 一亿 | 24 | 稳定后 | 0.4099 | 0.4023 | -1.86% | 174→74 |
| 64 | 17px滚动 | 一亿 | 24/28/32 | 逐轮 | 0.8618 | 0.8581 | -0.43% | 296→141 |
| 64 | 17px滚动 | 一亿 | 24/28/32 | 稳定后 | 0.5053 | 0.5004 | -0.96% | 296→141 |
| 1024 | 远跳 | 百万 | 24 | 逐轮 | 0.7373 | 0.7329 | -0.60% | 100→100 |
| 1024 | 远跳 | 百万 | 24 | 稳定后 | 0.7375 | 0.7331 | -0.61% | 100→100 |
| 1024 | 远跳 | 百万 | 24/28/32 | 逐轮 | 1.2635 | 1.2431 | -1.61% | 200→200 |
| 1024 | 远跳 | 百万 | 24/28/32 | 稳定后 | 0.9869 | 0.9671 | -2.01% | 200→200 |
| 1024 | 远跳 | 一亿 | 24 | 逐轮 | 0.7341 | 0.7343 | +0.03% | 100→100 |
| 1024 | 远跳 | 一亿 | 24 | 稳定后 | 0.7335 | 0.7245 | -1.23% | 100→100 |
| 1024 | 远跳 | 一亿 | 24/28/32 | 逐轮 | 1.2619 | 1.2404 | -1.71% | 200→200 |
| 1024 | 远跳 | 一亿 | 24/28/32 | 稳定后 | 0.9924 | 0.9771 | -1.55% | 200→200 |
| 1024 | 17px滚动 | 百万 | 24 | 逐轮 | 0.5712 | 0.5709 | -0.06% | 174→74 |
| 1024 | 17px滚动 | 百万 | 24 | 稳定后 | 0.3832 | 0.3850 | +0.46% | 174→74 |
| 1024 | 17px滚动 | 百万 | 24/28/32 | 逐轮 | 0.8173 | 0.8049 | -1.52% | 296→141 |
| 1024 | 17px滚动 | 百万 | 24/28/32 | 稳定后 | 0.4858 | 0.4780 | -1.61% | 296→141 |
| 1024 | 17px滚动 | 一亿 | 24 | 逐轮 | 0.6181 | 0.6150 | -0.50% | 174→74 |
| 1024 | 17px滚动 | 一亿 | 24 | 稳定后 | 0.4101 | 0.4064 | -0.90% | 174→74 |
| 1024 | 17px滚动 | 一亿 | 24/28/32 | 逐轮 | 0.8586 | 0.8521 | -0.76% | 296→141 |
| 1024 | 17px滚动 | 一亿 | 24/28/32 | 稳定后 | 0.5089 | 0.4992 | -1.90% | 296→141 |

物理滚动定高Layout174→74；变高296→141，前后ScrollTo155不变、TextShape/
Break61不变，稳定后Paint/Draw各100。两种预算、一亿变高稳定后、profile开启
的每100次累计inclusive阶段时间如下；各阶段相互包含，禁止相加：

| 缓存 | 阶段 | 前 ms/100次 | 后 ms/100次 |
|---:|---|---:|---:|
| 64 | layout | 16.2735 | 15.2347 |
| 64 | style | 10.8590 | 10.6435 |
| 64 | construct | 0.5877 | 0.3160 |
| 64 | box_layout | 3.0505 | 2.7012 |
| 64 | text_shape | 0.7297 | 0.7362 |
| 64 | draw | 25.1053 | 25.1929 |
| 1024 | layout | 16.0846 | 15.4495 |
| 1024 | style | 10.7235 | 10.9470 |
| 1024 | construct | 0.5730 | 0.3211 |
| 1024 | box_layout | 3.0368 | 2.7256 |
| 1024 | text_shape | 0.7191 | 0.7216 |
| 1024 | draw | 25.0860 | 25.0778 |

64的Draw累计25.1053→25.1930、TextShape .7297→.7362；1024的Style
10.7235→10.9470也略慢，原样保留。布局次数下降52.4%不代表帧时间同幅下降。
远跳定高Layout100、变高200均不变，必要测量反馈保留。百万→一亿物理滚动均
峰值32语义节点/42 graph slots/50 engine nodes；前后同预算key读取相同：
定高滚动736、变高861，远跳定高900、变高64缓存2867/1024缓存2581。
1024变高远跳峰值32/42/50，64变高远跳29/42/46；仅当前窗口及稀疏高度缓存，
没有实分配一亿业务项。没有据此计算RSS/FPS，也没有真实wheel性能结论。

最终Windows完整-Web -Window707项，Linux完整--window708项通过，含五项新
测量测试、185 Taffy/95 Blitz及profile5/style4、真实Windows/X11/Wayland窗口
和Windows UIA（accesskit-uia-1792732297945997.json）。严格全workspace/all-
features/all-targets Clippy、WASM/fixture构建通过；最终benchmark区域调整后
另跑Windows/WASM/Linux严格全target Clippy及格式检查。SSR fixture4项、24套
Chromium验收、stream Shell/Patch/Patch/End和慢消费/断开取消通过。Android
all-targets/all-features检查通过，仍仅编译证据。HTTP PID39088/stream25440
核验所属exe与命令行后停止，进程退出、8788/8789监听释放。

初轮Windows Clippy指出测试模块位于实现前，已移到文件末尾后完整门禁通过，
不使用allow屏蔽。早期预填充与计时远跳重叠的快速样本、所有quick样本都不
进入正式耗时表。未改第三方源码、依赖rev或引入vendor；所有target、临时与
证据在D:，Linux确认ext4 loop backing实际在D:。主仓库未提交/未创建PR。

复验示例（独立二进制已保留，out必须使用新目录）：

```powershell
python scripts/compare-semantic-profile.py `
  --before D:/BuildCache/glory-semantic-ui/measurement-before-target/release/examples/profile.exe `
  --after D:/BuildCache/glory-semantic-ui/measurement-after-target/release/examples/profile.exe `
  --out D:/BuildCache/glory-semantic-ui/measurement-64-scroll-rerun `
  --pairs 4 --measure-limit 64 --scroll
```

1024改measure-limit和out；不加--scroll运行远跳。正式数据分别在
measurement-{64,1024}-{seek,scroll}-formal/evidence.json。完整源码/锁/构建来源、
二进制/原始日志/验证哈希索引：
D:/BuildCache/glory-semantic-ui/measurement-evidence-2026-10-23.json。

本批完成确定性和两预算复验；仍需P1量化重复geometry观察并减少无变化读取。
观察341次、Structure/Window216次仍真实存在，不能凭布局次数少就视为解决。
后续先用固定聚合计数分清宿主几何读取与业务依赖，再建立不丢scroll ack、
query、hover、资源/viewport变化的失效条件。OS IME/辅助技术、Android宿主、
主题和其他架构任务仍按原范围未完成。


### 2026-10-24：显式静态构造计划

已实现普通 Rust builders 的 `plan(...).part(...)`、`Scope::instantiate` 和弱 Scope 的实例绑定；没有 DSL 或模板 VM。render 的 CreatePlan 作为离线完整创建单元，HTML 编译静态 Shape，web 复用文档内有界原型并深克隆一次、挂载根一次。每个语义节点身份保留；静态后代由实例 Scope 管理，其他 HTML 适配器默认展开有限操作，stream v3 不变。Group 目前正确回退到逐节点路径。

同库独立构建、两轮 864 个正式样本：千行创建 Script 55.27→28.36 ms，千行替换 83.79→46.38 ms，万行创建 646.39→362.41 ms。四版数据/身份/万行几何和六检查点画面一致；当前工作树 Windows/Linux 各 466 项测试及最终 26 个浏览器套件通过，真实行高与容差保留。体积成本、拆分对照、源码边界及完整证据见 [静态构造计划报告](static-construction-plan-2026-10-24.md)。


## Sapid P1-04–06：输入组合与内容尺寸（2026-10-24）

Sapid 增加独立 Textarea、PasswordInput 和 InputGroup/Addon/Section，继续使用
既有 Input、Field 和版本/Owner 协议。Textarea 行数换算为显式 line-height、padding、
border 对应的尺寸；自动高度通过 style::FieldSizing::{Fixed,Content}，不注入组件 DOM
或隐藏测量输入。HTML/RECORD style profile 升至 v4；CSS Sheet 校验每项原始声明，
HTML full-shape/字段 Patch 差分 oracle 覆盖新增声明与重置。stream 使用相同样式串，
不改变当前 HTML snapshot/stream wire 版本。

Blitz style profile 升至 v5，明确拒绝 FieldSizing 两值，包含 App 创建/预检/更新、
独立 direct apply、成功前缀/查询水位和 shell 拒绝后编辑恢复。固定行数沿用普通尺寸；
本次不宣称原生自动高度。密码切换只修改 Purpose，选区/编辑器与已有 native 密码投影
约定一致；未增加 OS IME 或屏幕阅读器语音的验收声明。

验证入口：Sapid scripts/check.ps1 -Web -Native、tests/inputs.rs、
examples/web/input-acceptance.js；Glory style/tests/sheet.rs、html/tests/sheet.rs、
html/src/patch_tests.rs、blitz/tests/style_profile.rs。具体本次执行结果与适配器边界
记录在同级 Sapid docs/widget-inventory.json。


### 2026-10-24：分页活节点存储与 DOM 元数据归并

保留 staged / committed 两份 Memory 和全部节点身份，改用带完整 runtime / generation 校验的分页 NodeMap；DOM parent 并入 Entry，移除重复 token 索引和字符串副本，静态计划批量登记。五份全新独立构建、四轮 2,160 样本，千行创建 / 替换 / 万行创建 / 清空的 Script 中位数降低约 14% / 20% / 25% / 31%；小操作未全面改善。

实测候选 Windows / Linux 各 470 测试、27 浏览器套件、真实行高 64/1024 缓存检查通过，全部数据 / 存续身份 / 万行几何 / 画面一致。另行冻结同期功能修改做合并验证，493 项 Windows 测试及 native/WASM 严格 Clippy 通过；这批代码不混入性能收益。完整拆分对照、剩余开销、体积/空间代价和证据边界见 [节点存储与 DOM 元数据报告](node-storage-dom-metadata-2026-10-24.md)。

### 2026-10-24：双份 Memory 共享声明

显式 Plan 的静态元数据从每节点 Descriptor 拷贝中分离；LiveDescriptor 保存独立 Props 和按字段分离的覆盖值，通过借用 DescriptorView 读取。普通 Create 保持独占。prepare_patch 回滚同时恢复值与原来的共享状态，staged / committed 独立校验、完整节点身份和成功前缀/查询水位不变。无 DSL、模板 VM 或依赖更改。

六份独立 release 构建、最终 1,296 个样本：千行创建 / 替换 / 万行创建 / 清空 Script 为 19.59 / 30.43 / 234.37 / 15.45 ms，同批改善约 16% / 17% / 13% / 12%。Windows/Linux 各 522 项工作区测试及 27 浏览器套件通过；64/1024 真实行高、锚点、对象身份和画面检查保持原容差。同期功能另行合并验证，不计入性能收益。原生 size_of 记录、WASM 体积代价、消融异常与完整证据见 [声明共享报告](shared-declarations-2026-10-24.md)。


### 2026-10-24：counter 示例迁移到共享语义组件

`examples/counter` 已替换旧 `web::widgets`、DOM 事件和 BrowserHolder。共享库
中的 Counter 包含状态、语义事件及类型化样式；`src/web.rs` 只处理 WASM/DOM
挂载及 WebApp 生命周期，`src/main.rs` 只处理 Blitz 桌面窗口/事件循环。
移除旧 Salvo 资源服务器、Trunk 配置和 web-csr/web-ssr features；独立 workspace
默认不选宿主，显式选择 web 或 desktop。保留 signed i32 数值行为，输入草稿
允许暂态负号，Enter/按钮规范化文本，边界加减饱和。Owner Effect 在宿主 tick
后通过 set_input_value 确认最终值，覆盖输入与 Enter/Clear 同批回到原值的情况。

新增 D: 构建脚本、README、锁文件与独立 counter CI；旧 CLI counter/hot-reload
启动配置退出，Playwright counter 项目改用当前 Web 入口。引擎来源检查包含新锁。
本地共享组件3项测试、desktop feature 下3项测试、native/WASM all-features/
all-targets counter Clippy、两端构建与 Chromium counter 套件通过。WASM 依赖
glory-web 当前仍有 validate_children 的既有 dead_code warning。Windows 实窗
通过按钮加减、实际按键输入 -42、Enter 和 Clear；未据此宣称 OS IME、屏幕
阅读器、Linux/macOS 设备或 Android 宿主完成。运行命令见示例 README；构建
产物及浏览器验收输出均在 D:/BuildCache/glory-semantic-ui。

### 2026-10-24：NativeSelect 有限原生选择协议与 HTML v4/classic v2

新增真实 Props::Select、不可变有界 options/optgroups、独立选择 key/revision 补丁与事件；UI setter 从 live revision 分配，Memory guard 只交换 selection，HTML full-shape differential oracle 覆盖默认/Keep/Clear/相等写入。业务 value 与 key 分离，空 single 用内部禁用隐藏占位项，readonly 和 FormControl.value 对 Select 明确拒绝。Web 构造原生子节点并读写真实 selectedness；采用先验证全部子树再回放 dirty 状态。Blitz preflight/direct 明确拒绝并保留成功前缀/query 水位。

这是有意破坏的格式升级：HTML FORMAT_VERSION=4，classic stream start/patch/end envelope=2、边界 stream:2、End 回交 v4；资源 envelope 维持1。新增 typed options/selectedKeys wire，旧版先拒绝、不兼容。现有手写 choices/menu/hints、Glory semantic-web/snapshot/transport 的当前格式常量同步迁移。Graphic 的 svg/path 与两个显式 camelCase 属性共同进入当前有限格式。具体 API/预算/证据见 Sapid docs/native-select-implementation.md；浏览器最终结果在该文档记录。

### 2026-10-24：Popover 有限锚点定位与 HTML v6

本批引入 FORMAT_VERSION=6；此前 v4/v5 记录保留为历史。AnchorPlacement 用显式物理
side、align、gap、viewport_padding、flip、shift 取代四个旧 Anchor 枚举，不保留别名。
纯几何 resolver 由 Web 宿主复用，并保留嵌套浮层的父位移拓扑计算。六项 HTML 属性
与 prepared-field lowering/full-shape oracle 一致。Classic envelope 保持2，Start/End
都严格携带 HTML6，旧 Start 在控制器注册及 DOM 写入前失败。
Memory 通过活 name index 与祖先判断锚点可用性；缺失关系允许晚绑定，失效后的
AnchorLost 请求在直达 App/参考队列中重新校验。Web 抑制失效锚点的实际层，受控
拒绝不保留悬空可交互浮层。Sapid trigger/close slots 保持真实 Button；不扩大
native 能力声明。十三组 Chromium148 实际几何/AX/adoption/stream 验收见
Sapid docs/popover.md；跨 App modal 协调和原生引擎浮层仍继续实施。

### 2026-10-24：当前 HTML9、Tree 与物理目标滚动

当前 FORMAT_VERSION=10，classic envelope=2。Tree/TreeItem 的有限方向、typeahead、
multiple 与层级/兄弟位置独立校验和 lowering 进入相同共享格式；所有手写 classic
Start/End、End marker 与旧版拒绝探针统一迁移，不接受 version8 别名。v8 的四个
Viewport 字段及 v7 Translation 继续保留；现有 full-shape differential oracle 不删。

新增 Action::ScrollIntoView：仅 current physical identity，先校验 scroll capability，
不移动 focus、不查业务数据、不替代虚拟 Row 的逻辑 reveal。Web/native 都在实际
top-layer 边界停止祖先滚动。UI focused1 和 native实际1覆盖 identity、能力、晚到
hidden/inert、拒绝后继续、真实祖先滚动及背景视口/编辑状态保持；原有 AX 回归
32 passed、1 interactive Windows test ignored。日志见 D:/BuildCache/sapid/reveal-*。


### File physical ranges and format 10

The following Environment extension advances the current HTML format to **11**;
the classic envelope remains **2**. View-only `Environment` stores bounded
`LanguageTag` and finite `Direction`; HTML emits independent `lang`/`dir`
attributes, and the complete shape oracle checks Set/Keep/Clear. Native AX reads
the nearest current semantic ancestor. Native actual tests change flex direction
while retaining the editor identity, UTF-16 selection and active preedit. Classic
Start/End and all browser marker/probe consumers use 11, with 10 explicitly
refused before host mutation. Style profile revisions are unchanged by this wire
upgrade. Sapid providers pass immutable explicit context values to one Owner-bound
factory; no global application context or implicit recoloring registry is added.

File controls have a finite fourth `Picker` part. Their `Control` span is the
physical range root (`display:contents`); the semantic `Main` button and hidden
file input are siblings. `Shape.control = Some(Main)` preserves genuine button
focus and geometry. `Shape.root_part()` validates one root, unique parts and
parent-before-child order. Serialization, cloned plans, DOM creation/adoption,
native ranges and classic Start/Create/End use that root, rather than assuming
that semantic Main is the whole physical range. Native File construction remains
explicitly unsupported until an actual system file driver exists. This shape
checkpoint does not claim a completed Web file picker/read component.

### Salvo HTTP SSR adapter（2026-10-26）

`crates/salvo-adapter` 以 `glory-salvo` 重新加入活动 workspace，直接消费当前
`glory-ssr` / `glory-ui`。旧 ServerHolder、GloryConfig、HTML widget API 和伪流式
collect 路径全部移除；`crates/axum-adapter` 与 `crates/actix-adapter` 的旧包、源码、
测试删除。其他未迁移应用示例继续作为提取参考，不计为当前支持。

普通 `render_response` / `render_response_with` / `document_response` 返回带 UTF-8
HTML content type 的 Salvo 响应；错误保留 typed Result。`html_response` 可包装显式
拼装的 hydration/resource snapshot。Document::opening_html 复用同步文档的样式验证、
转义、nonce 和 stylesheet 去重，避免流式宿主另写文档格式。

可选 stream 的 StreamRenderer 跨路由共享有界 admission，每个活跃请求在一个本地
线程/current-thread Tokio runtime 内构造独立 App/Resources 和 !Send future。
网络端只接收 owned HTML/error。初始化只生成并序列化 Shell，不轮询 loader；HTTP
body 每次需求通过单一 outstanding request/reply 拉取下一帧，无预生成 frame backlog。
body drop 同时覆盖等待需求和 pending resource I/O；取消在本地清理 Owner/future 后
归还 admission。同步业务代码的取消仍需其返回/yield，不声称抢占阻塞闭包。
初始化错误在 HTTP commit 前返回；后续错误/worker panic 终止 body，不发送成功 End。
HTTP 响应设置 no-store / X-Accel-Buffering:no，代理/压缩的实际 flush 仍由应用配置。

示例 `cargo run -p glory-salvo --example ssr --features stream` 提供 `/plain`、`/stream`
和匹配的 `/stream.js`，在 CSP 下先展示 shell，再分别更新 350ms/1200ms 的两个资源。
使用当前 Bootstrap preload + Frame::to_html，End 携带 rekey/资源种子；示例不加载
WASM，不将渐进 DOM 输出验收当作新增 WASM hydration 验收。

验证：适配器 21 项测试通过（含普通 SSR、CSS、资源种子、独立 Owner、Shell 提前返回、
逐次 demand、乱序完成、初始化取消、初始化/后续错误和 panic、准入回收，以及真实
Salvo chunked TCP、pending I/O 断开取消、暂停 TCP 读取后的背压）。glory-ssr 的
all-features 测试通过。真实 Chromium 执行 examples/ssr-acceptance.js，先读到两个
pending，约 361ms 只更新 fast、约 1208ms 更新 slow 后 End，两个资源种子完整，
controller active=0，无 CSP/页面错误。

## 2026-10-26：浏览器批次元数据与退休身份

`Plan::semantic_nodes` 公开声明中带 Semantics 的构造索引，复用既有不可变事实；
不保存实例身份或业务访问历史。Web 在 Mapper 完成后只提取一次当前已提交的
元数据变更，layer / sheet / collapse / translation 共用借用视图。Clear 保留
None 项，重复更新按最终已提交值合并，退休世代不进入视图；拒绝后缀不能提供
未提交的配置。结构、可见性、Style 和存活成员的独立检查继续保留。

`Memory::dispose` 与普通 Dispose 共用同一个退休实现，可同时返回完整退休身份，
供 Mapper 移除 HTML key 和下游宿主状态，避免先行重扫子树。挂起后代、遍历顺序、
名称、FocusScope、世代水位、事件清理和 receipt 语义不变；两份 Memory 和全部
节点身份仍保留。实现、约束及前后测量见
[批量路径质量优化报告](bulk-lifecycle-quality-2026-10-26.md)。

## 2026-10-26 跨平台功能差距和移动宿主

按用户指定与本地 Dioxus 工作树比较功能，并优先补跨平台交付；完整矩阵见
[功能差距及补齐报告](dioxus-feature-gap-analysis-2026-10-26.md)。SSR 框架集成
仍只支持 Salvo。项目内 CLI 新增显式 target、诊断与跨目标运行拒绝，以及含
资产/哈希清单的桌面目录包；真实 Counter release 与复制后的资产定位均验证。

Blitz shell 新增显式 AndroidApp 和外部事件循环入口，并开放 UIKit 编译路径。
独立 mobile 示例复用 Counter，提供两种 Android ABI 的实际链接/签名 APK。
Android 16 x86_64 的全新 D: AVD 已验证绘制、触摸、Gboard 数字键、后台恢复、
旋转和同进程 Activity 重建。Winit Destroy/循环独占许可修复维护于真正组织
fork，和 Blitz/锁文件/来源记录一起固定；没有 vendor 或新运行时 unsafe。
唯一导出属性例外经用户明确授权，限制在示例 android_main。

工作区全特性测试和 native 严格 Clippy、三个 fork 的独立测试、CLI 19 项测试和
复制后运行、Windows 普通/AccessKit 真窗口及独立 UIA 客户端均通过。
Android 适配器严格 Clippy 为 --no-deps，已有 core const thread-local 的 Android
全图诊断另行保留，不宣称全图严格检查通过。脚本新增 -Android 和可选的
-AndroidSerial gate，构建与运行分别报告。

用户确认暂无 Mac。iOS 入口、plist 与模拟器包脚本仅通过语法/元数据检查，未在
Apple SDK 编译或运行。Android arm64 真机、CJK 组合输入、移动资产/无障碍、
iOS 签名/生命周期/输入及桌面安装器仍待补齐；不据此关闭完整平台目标。

## 2026-10-26：移动端嵌入式资产

新增可选 glory-blitz/package 和 facade blitz-package。不可变 asset://app 字节、
队列/期限、取消和 NetHandler::prepare 解码准入复用原有 Loader；HTTP/TLS、DNS
及挂载文件移到桌面后端，assets 包含 package。包专用构建拒绝 origins/mounts，
生产依赖图没有 reqwest/Rustls/cap-std；默认/HTML/SSR/WASM 不获取加载器。

mobile Counter 用 include_bytes 嵌入 CSS 和 PNG，每个文档（含重建 Activity）
各有独立 Loader。包专用 6 项测试、桌面 Loader 9 项回归、工作区 all-features
测试和 native/WASM 严格 Clippy 通过。规范脚本新增包专用测试和生产依赖图 gate。
Android 16 x86_64 实际运行验证 CSS/PNG 像素、Gboard/触摸/编辑、后台/旋转及
同进程 Activity 重建；旧文档 completed=2、closed/stopped=true，队列/活动为零。
证据在 D:/BuildCache/glory-mobile/package-assets-acceptance-retry/result.json。
启动时 System UI 弹窗造成的首轮失败截图单独保留，稳定后完整复验通过。

这不包含 Android AssetManager/动态资产或联网权限；CJK、TalkBack、arm64 真机
和 iOS Apple SDK 构建/运行仍待验收。SSR 集成继续只提供 Salvo。

## 2026-10-26 Apple 主机与桌面开发补齐

本次环境已换为 Apple Silicon/Xcode 27.1；前节“iOS 没有 Apple-host 证据”为前轮边界。
macOS 真窗口/调整尺寸/关闭清理、Counter release `.app`、外部 AX 客户端读取与
Click/SetValue/真实键盘已验收。iOS 27 模拟器实际通过 CPU/CSS/PNG、触摸、软件键、
pinyin marked text/候选/中文提交、中文与 emoji 缓冲区退格、组合取消/Clear、
同进程恢复及旋转后继续输入；arm64 设备目标已实际链接，未签名安装。

场景和 UIKit IME 改动在真实 glory-rs Winit/Blitz fork 内固定完整 SHA。新增 unsafe
仅限用户明确授权的 Winit UIKit Objective-C UITextInput 协议桥接，Glory 运行时
无新增 unsafe。上下文是焦点编辑器的有限 UTF-8 片段，密码为空，preedit 不被确认
回写覆盖；其他平台不隐式导出文本。完整文本选择手势/selection rects、VoiceOver、
彩色 emoji 绘制、键盘避让、scene 重连及生产签名/公证仍独立待验收。

CLI 支持 Mac 数据盘缓存和 `.app` 结构/签名/完整哈希；生产签名脚本显式要求凭据，
校验完毕后整体替换包。桌面/server serve 支持 watch/restart，真实 Cargo/notify
验收确认失败构建保留旧进程、修复后新 PID、Ctrl+C 回收。它不保持应用状态。
详见 [差距报告的 Apple/IME 后续记录](dioxus-feature-gap-analysis-2026-10-26.md) 与
[移动工程说明](../examples/mobile/README.md)；未把整体差距标为关闭。

随后补齐 Web release 可复现 gzip 和预览内容协商，压缩文件进入完整清单。
真实 Counter WASM 从 2,107,005 压到 570,130 字节，解压及全部清单哈希一致，
浏览器启动/按钮交互通过；CLI 25 项测试与严格 Clippy 通过。指纹、图片转换、
Brotli 和 WASM 分包仍独立待办。

## 2026-10-27：移动输入与 Unicode 程序选区

共用输入选区改为校验当前值的 UTF-16 scalar 边界；文本/密码支持 CJK/emoji
正反向选区，半代理对在入队前拒绝，Blitz shaped-cluster 内部位置明确 Unsupported。
原生选区替换/预编辑取消、revision、方向与失败后恢复有独立回归；CSR/SSR 夹具
保留真实输入对象。详见 feature-gap report 当日记录及 semantic-web Unicode 夹具。

iOS scene 宿主增加 docked keyboard safe-area 避让和独立的底部输入探针，十二项
Swift 几何测试进入 Apple gate。实测发现按下时键盘移动编辑器、抬起时错误合成
空白点击的问题；genuine Blitz fork 增加受限触摸处理和实际布局回归测试，关闭
修复可复现失败。完整场景重连、VoiceOver 和签名设备验收继续开放。

Android 保留默认 NativeActivity，显式新增 GameActivity 4.4.0 选择与 Mac 打包脚本，
不可同时启用两种 Activity。Winit 安全 Rust bridge 翻译真实 GameTextInput state，
限制缓冲大小、校验 UTF-16/UTF-8 边界并在锁外调用 Android。用户同意许可后安装
隔离 NDK/ARM 镜像；ARM64 模拟器真实 Gboard 拼音通过预编辑、取消恢复非空 Unicode
选区、候选提交、汉字/emoji 删除、物理字符不重复以及同进程 Home/旋转。记录在
`/Volumes/Data/Caches/glory/android/unicode-ime/`。模拟器页大小 4 KiB；16 KiB ELF 对齐
检查、真机/16 KiB runtime 和 TalkBack 分开保留。所有运行时新代码仍为安全 Rust。

## 2026-10-27：同进程 UIKit scene 重连

Swift 应用宿主用当前 scene token 防止重复/过期生命周期通知。断开时解除原窗口
关联并暂停 surface；后继 scene 重新挂接 Winit 持有的相同窗口，更新尺寸并恢复
仍存活的弱 native responder。无第二个 Rust loop/document，无新增 unsafe。
独立 simulator-only scene-probe 通过 UIKit 真正销毁/激活 session，纯 UIKit anchor
仅负责保持测试进程存活；普通构建没有按钮或多场景配置。iPad 实测两轮重连保持
PID/window/value，软件键继续编辑、断开期间旋转、键盘和 safe-area 恢复及包资产
像素通过。Swift 重复/过期回调与 1,000 轮测试通过。证据及失败边界见缺口报告和
移动工程 README；不扩大为多 Glory 窗口或进程死亡持久化。

## 2026-10-27：显式 Android APK 资产能力

`android-assets`/facade `blitz-android-assets` 在原 package Loader 上增加显式
AssetManager 和 APK 前缀，命名空间为 asset://android/；不依赖 shell、全局
AndroidApp 或新 unsafe。安全 NDK streaming API 与 mounted files 共用 blocking
许可/计数和有界分块读取；逐块取消/deadline、实际 body budget、prepare 和文档
关闭撤权保持。Android 网络和权限没有随此 feature 开放。

ARM64 模拟器中实际 APK probe 通过压缩 CSS、中文路径、PNG、缺失/拒绝、限额、
prepare 和 close；Counter 内存包为空时仍显示正确样式/图片。capture 脚本核对
已安装 APK 哈希、PID 日志与实际像素，证据和边界见缺口报告。全工作区 970 项
通过，独立 blocking 取消许可回归仍有效；真机、TalkBack 和移动网络继续推进。

## 2026-10-27：共用原生 HTTP(S) 与移动平台证书

`network`/facade `blitz-network` 将旧 desktop HTTP worker 供 Android/iOS 共用；
desktop assets 在 network 上再开放文件挂载，package-only 不增加 HTTP/TLS。
空 origins 不创建客户端；origin/redirect、body/deadline/cancel 和 prepare 契约
保留。Android 的非空 origins 需要显式 AndroidTls，由安全 JNI Application 对象
初始化匹配的系统 verifier AAR，拒绝 null、非 Application 或缺少能力；无 raw JNI
或新 unsafe。Java 只保留 Application。iOS 复用 Security.framework。

两端模拟器实际通过可信/自签 TLS、redirect/origin/body 编码规则、HTTP CSS/PNG
像素及 cancel/close/deadline 的真实服务端 socket 断开，未迟到 prepare/publication。
Android 初次 250ms 建连前超时的观察失败保留，三秒后复验实际连接到期关闭通过。
Cargo/Gradle 版本、哈希和可选 INTERNET manifest 由构建脚本关联；普通包独立。
全工作区 970 项、native/WASM 严格 Clippy 及独立 network/package/assets gates
通过。证据及未关闭的权限/真机/读屏/分发边界见 gap report 和移动 README。

## 2026-10-27：有界权限 future 与宿主（未全部关闭）

permissions/facade blitz-permissions 提供显式 PermissionDriver、query/request 和
Owner 可取消的本地 future；无 UI 状态跨线程，无自动权限请求。准入在外部调用前
保留，driver 重入或 poll 时关闭不能绕过预算或发布已撤权结果。八个新回归通过，
全工作区 978 项及 native/WASM 严格 Clippy 通过。

Android safe JNI host 弱持有 Activity、限定主线程批次/回执/单 prompt，销毁关闭；
iOS 应用 Swift ledger 弱 client、独立 native query/prompt 许可，不因取消提前归还。
实际 SDK 状态查询验证 Android Limited/Granted/Denied、iOS grant/revoke/reset，
原始证据保存在数据盘 permissions/。用途描述与 manifest 权限只由 opt-in 构建加入。
Swift 宿主已经编译及链接，但 Rust iOS 连接仍待应用级最小整数 FFI 声明授权；
真实系统弹窗选择因 UI 锁屏仍待验收。没有新增 unsafe 或关闭整体权限缺口。

## 2026-10-27：原生 Mixed checkbox

Glory 固定真正 Blitz fork 251e4e288b287c77c59538425b09f25824c35821，增加独立
indeterminate property、`:indeterminate` 失效和 CPU 横杠；程序 checked 更新保留
mixed，点击/标签/Space/AX 激活清除 mixed 并只发布一次 boolean change。模式更新和
Clear 保留物理身份，AX 读取实际原生状态，禁用/只读不变。未引入新 unsafe。
旧实现下两项回归失败，当前全工作区 981 项与 native/WASM 严格 Clippy 通过，固定
fork 干净检出测试通过。Android/iOS `controls-probe` 实际显示三态，截图/JSON 在
`/Volumes/Data/Caches/glory/native-checks/`；真实输入/读屏和签名设备另验收。
radio、select、placeholder、菜单、富文本、文件/剪贴板及应用导航等仍开放；整体
目标没有因一个控件补齐而关闭。详细证据和 fork Clippy 边界见缺口报告。

## 2026-10-27：原生 Radio 与当前 tree/form 分组

固定 Blitz fork eb375a029330b6179d5276a41dda5643b6077a25，Radio 通过实际 checked
property 和当前非空 name/tree/form owner 互斥；同名 checkbox/其他表单/断开树保持
独立。Click/标签/Space/AX 只给新目标一次 true，旧 peer 无 false，重复选中无值变化；
受控 true 重申可恢复被拒绝的提案。批量插入、name/type/form、移动/挂起/重连和代次
退休有真实引擎回归，不存历史组或扫描业务源。Auto Tab stop 和默认方向/Home/End
复用当前宿主树；显式 Navigate/tab 策略仍由组件控制，readonly 不陷住焦点。
全工作区 988 项、最终七项新适配器回归、native/WASM 严格 Clippy、Blitz 默认/并行
各 121 项及固定来源 gate 通过；Android/iOS 普通包
和 radio-probe 构建通过，实际初始显示截图/JSON 在 `native-checks/radio/` 数据盘目录。
框架事件和 AX 树/动作不代替真实物理输入或读屏；typeahead/select/菜单等继续开放。


## 2026-10-27：原生 placeholder 的独立布局

Glory 解除有限 Input placeholder 的原生拒绝；真正 Blitz fork 0811cfd9 提供懒惰
提示布局和内容框裁剪，覆盖单行、多行及空密码输入。Set/Keep/Clear 保留真实值、
property 屏障、编辑器身份、选区和预编辑；提示不进入密码、IME 或点击选区几何。
`:placeholder-shown` 跟随当前值/组合状态，正常 selector snapshot 仍可刷新字体和
关联节点。AccessKit placeholder 与标签/值分开，密码原值继续受保护。

全工作区 991 项测试、native/WASM 严格 Clippy、固定 fork 独立来源/测试检查通过。
两端 opt-in probe 均已安装启动，初始提示像素和已安装包哈希见
`/Volumes/Data/Caches/glory/native-input-placeholders/result.json`。普通移动包及 iOS
设备目标链接另行通过；没有以截图代替实际触摸或读屏。详细范围见
[dioxus 缺口报告](dioxus-feature-gap-analysis-2026-10-26.md)
及 [placeholder 契约](input-placeholder.md)。select、菜单、移动无障碍和应用服务等
其他缺口仍开放。
