# 批量创建、替换与销毁优化实测（2026-10-23）

本轮保持 `semantic-rows-v1` 的应用代码、数据生成器、CSS、构建参数和计时 harness
逐字节一致，只改变框架实现。两轮正反顺序对照共 432 个计时样本；创建一万行的
脚本时间中位数从 **1127.54 ms 降到 730.45 ms（减少 35.2%）**，替换一千行减少
31.8%，清空一千行减少 23.4%。一万行仍完全挂载，没有通过虚拟化、固定测量值或改变
benchmark 应用模型取得结果。

完整工作区测试通过 419 项；浏览器 24 套中通过 23 套。唯一失败是变高列表的
0.375 px 锚点偏差，未优化版本在同一浏览器复现了完全相同的几何结果，详见下文。
后续已单独[修复滚动余差](anchor-residual-fix-2026-10-23.md)，在 64/1024 测量缓存、
真实小数行高和原容差下通过全部 24 套浏览器验收；本报告保留原配对运行的历史结果。
不能把本轮验收写成全部浏览器测试通过。

## 实际耗时

每个版本、每项操作各有 24 个计时样本，表中为合并两轮后的样本中位数。
单位 ms；“减少”为正表示更快。wall 从点击到两次前台 RAF，包含帧等待；Script
是同一时间区间的 CDP `ScriptDuration` 增量，包含 WASM 执行。两者不是互斥阶段，
不能相加，也不等于显示完成或 FPS。

| 操作 | wall 前 | wall 后 | wall 减少 | Script 前 | Script 后 | Script 减少 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| create_1k | 124.40 | 89.05 | 28.4% | 95.82 | 64.11 | 33.1% |
| replace_1k | 160.00 | 113.20 | 29.2% | 129.69 | 88.48 | 31.8% |
| update_10th | 11.50 | 12.90 | -12.2% | 0.65 | 0.65 | 0.1% |
| select_row | 8.60 | 7.65 | 11.0% | 0.99 | 1.02 | -3.0% |
| swap_rows | 10.25 | 10.50 | -2.4% | 0.55 | 0.56 | -2.7% |
| remove_row | 12.15 | 8.00 | 34.2% | 0.61 | 0.61 | 0.0% |
| append_1k | 133.25 | 97.45 | 26.9% | 99.22 | 66.94 | 32.5% |
| create_10k | 1457.40 | 1014.75 | 30.4% | 1127.54 | 730.45 | 35.2% |
| clear_1k | 42.35 | 33.60 | 20.7% | 40.56 | 31.06 | 23.4% |

收益集中于批量操作。update 的 wall 变慢，而 CPU 基本相同；remove 的 wall
减少约 34%，CPU 也基本相同。这两项不能据此宣称计算变快。select/swap 的 CPU
变化只有约 0.02–0.03 ms，本轮没有建立其性能改进。

创建一万行的 Layout + RecalcStyle 中位数也从 272.32 ms 降到 226.58 ms，
但仍需要布局 80,008 个真实元素。post-bindgen WASM 从 817,225 增至 818,366 bytes
（增加 1,141 bytes）。减少 Owner/DOM 登记并不等于已测量总 RSS；本轮没有该结论。

这是本轮重新构建的配对基线。此前三框架单轮中的 Glory create_10k wall 为
2809.90 ms，本轮未优化版本为 1457.40 ms；不能拿历史数值作为本轮分母，也不能
直接把本轮结果与旧 Leptos/Dioxus 数值拼接排名。当前报告只比较 Glory 前后。

## 原因与实现

### Owner 与 Binding

旧 Element.fill 为每个带内容的元素创建一个额外的 content Scope，再为每个子
Widget 创建独立挂载 Scope。一行有行容器和三个带子内容的 cell，因此额外产生四个
Owner。它们都有 Instance、清理闭包、根节点/资源集合和 arena 登记；销毁时还会逐一
经过这些结构。一万个业务行并不等于一万个 Owner。

新 `Scope::mount_at` 直接在指定语义父节点下建立子 Widget 的独立 Scope。
`Scope::fill` 保留这个 Mounted，取消中间 content Scope。组件之间仍有独立的
Owner/Instance；内容 Mounted 放在资源集合中，不能加入容器本身的 movable roots。
后一条由回归测试约束，避免移动一行时把 cell 一起重新挂到行的兄弟位置。

| 一万行存活状态 | 优化前 | 优化后 |
| --- | ---: | ---: |
| 语义节点 | 80,010 | 80,010 |
| 行标签注册 | 10,000 | 10,000 |
| 响应式图节点 | 60,003 | 60,003 |
| Owner | 130,016 | 90,013 |
| 清空后的 Owner | 16 | 13 |

Binding 的数量没有被减少。每行仍有 item/index/label 三个 Cage、选中态 class
的一个 Bond，以及 class/label 两个 Binding；另有应用数据源和 Each 结构观察者。
这解释了为什么批量创建、清理仍然有明显的 Rust 图管理成本。更新单个标签没有改成
整行重建，也没有为了性能改变业务模型。初始响应式属性仍通过现有 Binding/Patch
路径建立依赖；本轮没有冒险把它变成组件构建时的隐式订阅。

### 销毁被拆成过多小操作

Core 会先退休整棵 Owner 子树的身份，再在图借用之外按既有顺序执行清理回调。
以前 UI 在每个叶 Scope 的清理回调里单独提交 Dispose，导致一千行产生 8,000 次
元素移除；每个小操作又经过 UI Memory、HTML Mapper Memory、DOM 登记清理和范围处理。

现在 `InstanceId::is_alive` 提供无回调的代际存活检查。`Session::dispose_owner`
沿语义祖先向上寻找**已经退休**的最高节点，遇到存活 Owner 就停止，一次 Dispose
整个退休子树。后续清理回调照常执行，但已释放的身份不重复提交。没有延迟销毁队列、
全树克隆或业务数据源扫描；处理器捕获、用户清理及值析构仍在借用之外执行。

一千行清空/替换的 `removeChild` 从 **8,000 降到 1,000**。每行的业务清理和
响应式节点退休仍须执行，因此整体 clear 的 Script 仅减少 23.4%，并非八倍加速。
子树可能在第一个内部 UI 清理回调时整体消失；用户回调顺序保持原样，但不能依赖
已经退休 Owner 的展示节点在后续回调中继续存在。

### DOM 元数据、注释与 fragment

HTML 经过字符串解析后，需要 key/part 属性和文本注释来恢复语义身份。CSR 创建时
已经持有 typed Part → Node 引用，重复写这些解析地址没有必要。新浏览器创建路径：

- 保留 `data-gly-node` 的 Runtime/slot/generation 路由身份、`data-gly-kind`、语义状态
  和无障碍属性；省去新建 CSR 元素的 `data-gly-key` / `data-gly-part`。
- checkbox 的 control 判断改用 typed control Element 身份比较。
- 新建文本直接保留 Text Node 引用，不再附加 `gly:t` 注释。Group 的两个范围注释
  继续保留，它们负责没有布局盒子的逻辑范围。
- `data-gly-live` 只写到 VirtualPart 对应节点，用于虚拟列表 SSR 预览到实时布局的
  切换；普通元素不再重复写这个运行标记。语义 enabled/checked 等状态没有删除。
- 普通元素直接创建为脱离文档的主节点；DocumentFragment 只用于 Group 或实际
  Suspend 停放，避免每个节点预先创建一个 fragment。
- 脱离文档的新建/恢复范围不读取 activeElement、选择区及 moveBefore 方法。
  已连接范围的移动继续保护焦点、IME 和选择区；单节点范围直接返回其引用。

SSR serializer、classic stream v3 和 hydration 输入验证的格式保持原样。
已采用的服务器 DOM 保留 key/part/文本标记；后续 CSR 节点通过存活的 typed 引用
工作，不依赖从最终 DOM 重新解析一次 wire snapshot。有限 CSS 主节点 guard 及状态
属性也未改变。完整 CSR/SSR 输入和流式交接套件实测通过。

| 一万行的实际 DOM/调用 | 优化前 | 优化后 |
| --- | ---: | ---: |
| 元素 | 80,008 | 80,008 |
| Text Node | 30,006 | 30,006 |
| 注释（含初始 Group） | 30,010 | 4 |
| 属性（整棵 benchmark DOM） | 560,061 | 320,037 |
| 创建行时 setAttribute | 560,000 | 320,000 |
| 创建行时 createDocumentFragment | 80,000 | 0 |
| 创建行时 activeElement 读取 | 160,000 | 0 |
| 创建行时 appendChild | 140,000 | 30,000 |
| 创建行时 insertBefore | 80,000 | 80,000 |

调用统计来自单独的 DOM 包装诊断。正式计时不安装这些包装，也不运行 CPU Profiler。
原始 CPU profiles 保留了 WASM 函数名，可看到字符串 decode、属性写入、分配/rehash，
以及销毁路径的 Scope 清理、State::dispose_owner、Memory 操作；包装与采样本身有
开销，不能用它们的耗时相加或据此给各项优化分配精确贡献百分比。

### 重复位置确认

Each 已按顺序创建新行，协调器仍会确认行应处于哪个兄弟之前。旧
`Session::place` 的无变化判断每次都扫描 siblings，N 个新行形成平方级确认。
新 Memory 给每个存活节点保留一个逻辑 next sibling 身份，移动/删除时维护相邻边；
无变化判断只读取这条边。另将 unlink 的整段 retain 改成定点删除，尾节点直接定位。

这是每棵语义模型每个存活节点额外一条边的空间换时间，没有访问历史缓存。
兄弟 Vec 仍保留顺序；真正的中间插入/删除仍会移动 Vec 元素，不能声称所有树操作
都变成 O(1)。本轮也没有给 DOM 增加模板克隆或绕过适配器预检。

## 数据、身份与画面验证

- 计时前后使用相同 Rust 数据生成器及种子。432 个正式样本按操作/样本序号比对
  最终行数、完整数据 SHA-256；相应结果跨版本、跨轮一致。
- 原有独立 JS 数据 oracle 验证创建、更新、选择、交换、删除、追加、替换和清空。
  交换保留原 DOM 对象；替换断开全部旧行，连续清空回到各版本的初始存活统计。
- 补充验证在更新、选择、交换、删除和追加后，存活行、所有后代元素、Text Node
  及 `data-gly-node` 都保持原身份；被删行和替换后的旧后代均离开文档。
  独立 Runtime 的分配槽号不作为跨版本相等条件，代际身份不能复活。
- 六个视口截图检查点（创建、替换、追加、一万行顶部/中部、清空）PNG SHA-256
  完全一致。额外比对全部一万行的业务 key、x/y/width/height 数组 SHA-256，
  每行有真实非零布局盒子。没有把几个可见行的截图称作一万行全部像素的截图。
- 回归测试覆盖逻辑邻接的跨父移动、Suspend/Resume、无效操作、销毁和身份复用，
  以及内容组件独立退休、根范围和存活父节点边界。

## 验证结果与已知问题

工作区 `cargo test --locked --workspace --all-features`：419 通过、0 失败，另有
两个 OS 测试默认 ignored。随后实际运行 Windows 实窗及 UIA 两项，均通过。
Windows/WASM 工作区 all-features/all-targets Clippy、最终 web 默认/全部 feature
的 WASM Clippy、独立 release benchmark 构建均通过。SSR fixture 的四项 Rust
测试、流式到达顺序/取消/慢读取背压验收通过。未在本轮重新运行 Linux 实窗或 Android
设备验收；另已执行两份真实 glory-rs 引擎 fork 的固定提交校验和 fork-owned 测试。
浏览器性能数值也不代表原生渲染速度。

最终 24 个浏览器套件中，23 个通过，唯一失败 `adaptive-acceptance.js`：

| 观测 | 未优化版本 | 优化版本 |
| --- | ---: | ---: |
| 增长前锚点 top | -15.625 | -15.625 |
| 增长后锚点 top | -15.250 | -15.250 |
| 增长前 scrollTop | 4828 | 4828 |
| 增长后 scrollTop | 4858 | 4858 |

请求增长 30.375 px，当前 Chrome 的物理滚动只增加 30 px，留下 0.375 px，超过
该测试的 0.1 px 容差。未优化快照重建 fixture 后复现了相同失败，测试阈值没有放宽。
本轮没有改动列表测量/滚动算法；需要另行处理宿主滚动量化后的亚像素残差，不能用
本次批量 DOM 的一致性证明该旧问题已解决。

## 后续优先级

1. **P1：合并每行内部数据 Owner 与挂载 Owner。** 本轮已取消空的 content Scope，
   仍有 Each 的 data Scope 和 Mounted Scope。先明确工厂构建、失败回滚与临时
   item/index Cage 的共同寿命，再取消这一层；保留用户组件独立的退休边界。
2. **P1：响应式初值与订阅建立分开处理。** 分析 class/label 两项 Binding 的初次
   求值、Patch 和描述符处理；若合入 Create，必须保留即时依赖追踪、纯计算
   约束、Input revision/预编辑和预检原子性。不能仅靠未追踪读取省掉 Binding。
3. **P1：修复滚动量化残差。** 独立于本轮优化，按请求/实际 scroll ack 之差保留
   有界残差，并继续使用真实行高、64/1024 测量缓存验证。
4. **P2：减少有限 Part/编辑状态的小分配和重复树处理。** 优先测量 Entry/Shape 的
   小 BTreeMap、非输入节点 Editing 存储、Descriptor 创建/验证与双 Memory 的
   分配成本。保持 typed preflight、prefix receipt、SSR/stream 和节点身份合同。
   不以删除必要的语义/无障碍字段换取分数。

## 证据与复现

根目录：`D:/BuildCache/glory-bulk-optimization/`。

- 总索引：`evidence-index.json`，校验冻结源码、运行 driver、构建产物和 117 份证据
  的哈希，并单独记录通过项及基线已有的失败。
- 最终正式计时：`accepted-paired/evidence.json`，顺序 before→release，再
  release→before；每项每轮 3 次预热、12 次计时，共 108 次预热、432 次计时。
- 校验后的汇总：`summary.json`；数据、全部行几何及后代身份补充证据：
  `integrity-accepted/evidence.json`；最终浏览器回归：`browser-final/evidence.json`；
  原版锚点复现：`baseline-adaptive/evidence.json`。
- `before/source/inputs.json`（232 个输入）和 `release/source/inputs.json`
  （234 个输入）记录源文件 SHA-256。除六个框架实现文件及两份新增测试外，原有
  输入相同，特别是 benchmark Rust/HTML/lock、data/CSS/harness 全部相同。
- 两套 Cargo target 完全独立，metadata 核实均位于 D:；两个应用 compiler-artifact
  均为 `fresh=false`。CLI/library wasm-bindgen 0.2.129，Rust 1.98/MSVC，release
  opt-level=3、LTO、one codegen unit，无 wasm-opt。两个 WASM 和构建记录有哈希。
- Chrome 155.0.8059.84，Playwright 1.60.0，独立 headless 进程，1280×900、scale 1。
  本机 i7-14700KF/28 逻辑处理器；正式运行前没有 Cargo/rustc 进程，记录负载 2%。
  这仍是单台共享工作站实测，不是统计显著性或硬件通用保证。
- 早期 `paired`、`final-paired`、`after*` 和 `optimized` 目录是开发迭代证据，
  不参与上述最终表格。正式计时期间没有本任务的构建、验收或包装 profiler 并行。

在已有快照和构建上，可由单独终端启动静态服务：

```powershell
python -m http.server 8791 --bind 127.0.0.1 --directory D:/BuildCache/glory-bulk-optimization
```

另一个终端设置 D: 临时目录后运行（输出目录必须不存在）：

```powershell
$env:TEMP='D:/BuildCache/glory-bulk-optimization/validation/tmp'
$env:TMP=$env:TEMP
$env:NODE_COMPILE_CACHE='D:/BuildCache/glory-bulk-optimization/node-cache'
node benchmarks/bulk-compare.cjs --variants before,release --out D:/BuildCache/glory-bulk-optimization/new-paired
node benchmarks/bulk-compare.cjs --integrity --variants before,release --out D:/BuildCache/glory-bulk-optimization/new-integrity
python benchmarks/summarize-bulk.py --evidence D:/BuildCache/glory-bulk-optimization/accepted-paired/evidence.json --out D:/BuildCache/glory-bulk-optimization/rechecked-summary.json
```

汇总器检查原始样本、工作负载哈希、冻结源码、独立 target、fresh 构建和实际执行
driver 的哈希。已验收运行的 driver 在 `accepted-paired/driver-source/`，后续加入
的 `--integrity` 代码不属于原始计时 driver。当前 runner 会在新运行开始时自动
保存 driver、acceptance、browser-run、harness 和补充身份验证源码。
