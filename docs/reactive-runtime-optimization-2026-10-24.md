# 核心响应图与提交优化：实施与实测

后续 HTML / DOM 优化与当前工作区回归已完成，见 [2026-10-25 专项报告](html-dom-bridge-optimization-2026-10-25.md)。本报告的响应图数据保留为独立历史批次，不与新数据混算。

测量日期：2026-10-24；报告更新：2026-10-25 / Asia/Shanghai。实施、独立计时、文档复核及最终合并回归均已完成。

本报告执行 [核心优化评估](../vibe/core-reactivity-rendering-optimization-2026-10-24.md)。保留 Cage/Bond/Vary、属性 Binding、Owner、两份独立 Memory 和完整节点身份。主要收益来自响应式传播/求值，不是批量构造的数量级突破。

## 1. 保留、选用与撤回

| 项目 | 决定 | 实际边界 |
|---|---|---|
| 提交/输入空缓冲复用 | 保留 | 每流一个 spare；单个已处理缓冲最多 1 MiB，超限释放；没有访问历史 |
| queued 成员资格 | 保留 | 节点独立 bool + live 计数，保留 propagated、代际身份、FIFO、取消和有界压缩 |
| 重复依赖登记 | 保留 | 旧边仍供退休/重入读取，只跳过已有 subscriber 插入；仍重置前沿、比较版本 |
| Empty / One / Many 依赖 | 保留 | 0/1 无树节点分配，多依赖回退原 Key 排序 BTreeMap；subscriber 集合未替换 |
| 直接属性闭包 API | 提供，默认 benchmark 不迁移 | 选行更快，但万行创建不稳定；共享昂贵计算继续用 Bond |
| 空提交快速分支 | 撤回 | 独立原生列表对照未显示改善，未留在源码 |
| 首次等值 class 注入 | 撤回 | 合成测试成立，但标准 RowPlan 没有声明 bench-row，实际首次写入必需；该实验没有命中目标负载 |
| 进一步退休重写 / 通用编码提交器 | 未实施 | 新 profile 仍主要指向 Mapper/宿主；不能把 inclusive 时间指定给某个临时集合 |

当前标准 benchmark 的 15 个应用输入与 before 字节一致。直接闭包实验只改一个应用文件，单独归类为应用迁移，未冒称内部优化。没有 DSL、模板 VM、第三方 Signal 替换、依赖更新或 vendor。

## 2. 源码与测量边界

根目录：`D:/BuildCache/glory-reactive-optimization-20261024`。

before 冻结 494 个输入。累计内部候选 buffer / queued / edges / deps / direct-base 分别为 496 / 496 / 497 / 498 / 499 个输入；direct-base 已含新 API，应用不使用。direct 是其单文件迁移版。empty / empty-direct 和 initial / initial-direct 是被撤回的实验。11 个 Glory 候选及 Leptos 0.8.19、Dioxus 0.7.9 各自独立 target，全新 release 构建；没有复制 target 或 WASM 代替编译。

主批 primary：9 版本、正反两轮、每操作每版本 24 样本，共 1,944 个；final：6 版本同规则，共 1,296 个。**两个批次不混池**。下节使用 final；消融表使用 primary。诊断/profile、验收、构建与主计时分开。本任务的构建在主采样时停止；边界记录 cargo/rustc 均为零，主机仍是共享桌面，负载并非保证恒零。

环境：Windows、i7-14700KF、Chrome 155.0.8059.84、1280×900、DPR 1、同一 CSS/种子/九项负载；浏览器统一绑核掩码 65535、AboveNormal。ScriptDuration 是 CPU 计时指标；TaskDuration、两次 RAF 的 elapsed 都单独保留，不代表显示完成或 FPS。所有原始慢样本保留，无剔除。原生第一次采样尚与验证进程重叠，完整保留在 native-pilot，随后全量重跑，未混入正式结果。

source-audit.json 校验候选源码、应用差异及 WASM 哈希；各构建 metadata 核对 target 位于 D 盘，构建 JSONL 可检查 release artifact 的 fresh 标志。合并验收另行冻结同期功能，不能用它的测试数量暗示整份合并源码都参与了性能测量。

## 3. 同批三框架结果

单位：ScriptDuration 中位数 ms。Glory after 是最终保留的 direct-base 内部实现，应用不调用闭包新 API。

| 操作 | Glory before | Glory after | Leptos 0.8.19 | Dioxus 0.7.9 |
|---|---:|---:|---:|---:|
| 创建千行 | 21.618 | 20.975 | 13.992 | 4.336 |
| 替换千行 | 32.750 | 30.986 | 20.128 | 7.683 |
| 每十行更新 | 0.590 | 0.523 | 0.352 | 1.009 |
| 选行 | 0.893 | 0.623 | 169.669 | 0.843 |
| 交换 | 0.611 | 0.616 | 0.176 | 0.930 |
| 删除一行 | 0.622 | 0.649 | 0.271 | 0.901 |
| 追加千行 | 23.041 | 22.980 | 15.965 | 5.693 |
| 创建万行 | 246.364 | 247.109 | 178.258 | 47.560 |
| 清空千行 | 15.707 | 15.317 | 16.285 | 2.672 |

Glory 四项批量操作分别变化：创建千行约 -3.0%、替换千行 -5.4%、创建万行 +0.3%、清空千行 -2.5%。选行下降约 30.2%，每十行更新下降约 11.4%；交换、删除和追加没有全面改善。

不得把这些结果解释为所有场景都变快。primary 的千行创建是 21.893→22.147 ms，反而略慢；因此批量创建的小幅收益并不稳定，万行构造仍未解决。final 中替换的 TaskDuration 是 62.801→62.663 ms，几乎不变，不能把 Script 改善直接说成端到端同幅改善。选行的 TaskDuration 也出现反向变化，完整计数见 final-summary.json。

Leptos 的选行 169.669 ms 是本九项连续流程中的历史敏感异常，primary 同样出现。保留它但不用于通常选行能力排名；本轮没有完成其根因定位，也不能把 Glory 说成常态下快数百倍。

Glory 与 Leptos 的千行创建/替换仍约 1.50× / 1.54×，与 Dioxus 仍约 4.84× / 4.03×。下一轮批量优化应以 HTML/DOM 创建与映射为主，继续只改响应式算法很难填平这部分差距。

## 4. 内部逐项消融

以下仅为 primary，单位 Script ms，不与上节减算收益。每个候选在前一项基础上累计，direct-base 与 deps 的差异只增加未使用的 API，不应当作一个新的优化收益。

| 版本 | 创建千行 | 替换千行 | 创建万行 | 清空千行 | 选行 |
|---|---:|---:|---:|---:|---:|
| before | 21.893 | 32.965 | 257.395 | 16.153 | 0.903 |
| buffer | 21.949 | 31.856 | 256.685 | 15.957 | 0.862 |
| queued | 21.474 | 32.107 | 253.614 | 15.885 | 0.796 |
| edges | 21.499 | 32.031 | 260.920 | 16.239 | 0.764 |
| deps | 21.474 | 32.107 | 256.837 | 15.907 | 0.647 |
| direct-base | 22.147 | 32.351 | 252.567 | 15.869 | 0.638 |
| direct | 22.088 | 32.384 | 255.274 | 15.768 | 0.522 |
| leptos | 13.922 | 21.784 | 182.993 | 17.229 | 173.751 |
| dioxus | 4.372 | 7.716 | 48.190 | 2.645 | 0.754 |

buffer 对替换有所帮助，但不是所有批量项都改善；queued、edges 的纯浏览器差异较小。deps 对选行更明显。独立原生实验进一步把图计算与 DOM 成本分开。

## 5. 原生少依赖与高扇出对照

使用相同 Rust 工作负载、独立 native target、正反两轮，每格 18 次。表中是 100 次源写入 + flush 的总 ms；不是单次更新时间，也不是浏览器行创建时间。包括 0/1/2/4/32 依赖、1/1,000/10,000 fan-out，验证节点/边数、队列清空和最终退休。完整创建/销毁数据在 native-summary.json。可维护版本已加入 `cargo bench -p glory-core --bench graph`；设置 GLORY_GRAPH_CENSUS 可输出活跃图统计。

| 依赖数 / 观察者数 | before | queued | edges | deps | 保留实现 |
|---|---:|---:|---:|---:|---:|
| 0/1000 | 0.002 | 0.002 | 0.002 | 0.002 | 0.003 |
| 1/1 | 0.020 | 0.018 | 0.019 | 0.012 | 0.012 |
| 1/1000 | 21.757 | 15.560 | 13.319 | 7.022 | 7.159 |
| 1/10000 | 233.954 | 160.023 | 137.115 | 70.442 | 71.475 |
| 2/1000 | 28.970 | 21.597 | 17.943 | 18.706 | 18.545 |
| 4/1000 | 42.275 | 33.358 | 27.108 | 29.514 | 28.824 |
| 32/1000 | 256.939 | 239.643 | 210.684 | 218.576 | 218.301 |

1 / 10,000 从 233.954→71.475 ms，约 -69.5%；1 / 1,000 约 -67.1%。32 / 1,000 从 256.939→218.301 ms，约 -15.0%。后者相对 edges 又慢约 3.6%，说明单依赖表示并没有让多依赖回退免费；销毁的 32 依赖格也有回退，不能只展示单依赖结果。

参考解释器的百万/一亿行窗口列表不变：同窗口每 1,000 帧读取新 key 为 0，跳转均为 33,000；最大节点 68，graph slots 111。第一正式批的同窗口约 0.609–0.612→0.647–0.659 μs/帧，绝对增加约 0.04 μs；跳转约 216–220 μs/帧，基本持平。空提交快速分支另测仍未改善，已撤回。没有扫描大列表来换取这些结果。

## 6. 数据布局、实际图分布与保留成本

scene-census 从标准 benchmark 复用 Row/Model/RowPlan/ui 构造代码，以 Memory adapter 观察，不是另造一个更有利的行模板。千行：Source 3,002、Derived 1,000、Structure 1、Binding 2,000，共 6,003 个；其 3,001 个计算/观察节点都只有一个直接依赖。只有一个源落在 65+ subscriber 桶。清空后回到 Source 2 + Structure 1，UI 节点 10。

| 布局探针 | before | after |
|---|---:|---:|
| x86_64 Core Node | 128 B | 128 B |
| x86_64 依赖字段 | 24 B | 24 B |
| wasm32 Core Node | 80 B | 96 B |
| wasm32 依赖字段 | 12 B | 24 B |

原生不增大记录，不代表 WASM 也不增大。千行 6,003 个节点的内联记录增加 96,048 B；同时避免单依赖树节点及验证克隆分配。**没有测量总 allocator 分配量或 RSS，因此不声称总体内存下降。** WASM 从 1,793,863→1,799,640 B，约 +0.32%。

提交缓冲只存空容量，不能忽略保留成本。原生标准行诊断的空闲/队列缓冲：初始 2,496 B，创建千行 641,472 B，替换千行 1,327,104 B，创建万行 1,474,560 B，清空万行 1,572,864 B，回到小批并清空后 2,236,416 B。统计排除正在处理的 batch。每流最多一个 spare，队列与 spare 可各保留一份不超过 1 MiB 的已处理存储；执行中和实际待处理的大批次另计。arena/FIFO 的数字槽位容量遵循峰值工作，不能当作活跃行或业务数据历史。

## 7. 显式直接属性 API

[Owner::compute / 图与队列](../crates/core/src/runtime.rs)、[Dependencies](../crates/core/src/dependencies.rs)、[builder](../crates/ui/src/builders.rs)、[Plan 绑定](../crates/ui/src/plan.rs)、[缓冲池](../crates/ui/src/buffer.rs)是实际实现入口。

```rust
text().content_fn(move || title.get());
view().class_fn(move || if active.get() { "active".into() } else { String::new() });
instance.class_fn(&row_part, move || {
    selected.with(|value| if *value == Some(id) { "bench-row danger" } else { "bench-row" }.to_owned())
})?;
```

提供 content_fn / label_fn / class_fn / style_fn，以及 PlanInstance::bind_fn / text_fn / label_fn / class_fn。普通 builder 初值通过 Owner::compute 在正确 Owner 的 untracked + Pure 上下文运行；完整初值校验先于语义身份分配。随后只由最终 Binding 追踪。Plan 本来已有身份，新闭包首次在 Binding 中运行；初值和后续均保持纯计算规则。受控值仍使用 live revision，不捕获旧计数。

单文件 direct 迁移每行省一个 Bond：6,003→5,003 个响应节点，语义节点和 DOM 不变。primary 选行 0.638→0.522 ms，但万行 252.567→255.274 ms。final 的 initial-direct 还叠加了未命中的等值 class 分支，不能把该版本的全部差异归给闭包。API 是显式选择；默认 benchmark 继续使用原来的 selected.map 和 instance.class。

## 8. 剩余热点与下一步优先级

profile-final 是五版本独立 unwrapped CPU 采样，每版本四项各三次；未与主计时混池。以下为三次 inclusive 平均 ms，函数类别可能嵌套，**不可相加**：

| 场景 / 类别 | before | after |
|---|---:|---:|
| 创建千行 Mapper.apply | 15.258 | 15.684 |
| 替换千行 Mapper.apply | 22.744 | 22.795 |
| 替换千行 Owner.dispose | 3.986 | 4.404 |
| 清空千行 Owner.dispose | 4.633 | 4.082 |
| 清空千行 removeChild | 4.156 | 4.787 |

退休改善并不统一，不能从单项采样把剩余成本归因于某个 HashSet、收集 Vec 或 sibling 查找。已有父表 retain、unlink/搬移与部分 Scope 合并不再重复列作待办。

下一轮按此顺序：

1. **P1：HTML/DOM 窄实验。** 重点看固定语义字段初始化、身份登记、JS/Rust 逐节点桥接；保持节点身份、独立验证、成功前缀和查询/动作/输入屏障。先测一个窄点，再决定是否建设编码传输。
2. **P1：WASM 依赖表示的体积。** 现有 80→96 B 的节点增幅要单独消融，尤其 Source 多、更新少的图。不得只凭原生布局下结论。
3. **P2：静态 class 的应用迁移。** 标准 RowPlan 当前没有 bench-row 声明。若先把固定 class 写入声明，再评价首次等值注入，必须建立同应用源码的内部 A/B 和独立迁移对照，不能用本次未命中实验声称已节省千行 Patch。
4. **P2：继续退休优化。** 需先定位到具体路径并给出计数；不移除 Owner/Binding、双 Memory 或完整身份。

## 9. 正确性、回归与未完成范围

primary 的九版本和 final 的六版本均通过独立数据 oracle、清空存活状态检查、六个逐像素一致检查点，以及全部存续 Element/Text 对象、语义 ID、10,000 个实测行矩形的补充核验。替换后旧后代断开，清空后活跃图回到基线。旧输入文件与容差未修改。

新增测试覆盖池归还的 unwind/析构重入/超限释放/借用冲突、持续小帧与突发回落、新排队不被覆盖、失败 apply 不重放；依赖容器与 BTreeMap 差分、动态依赖/晚订阅/退休/队列上界沿用完整 core 套件；闭包补充纯计算、初值校验、分支切换、等值跳过、身份与 live revision 耗尽。

最终合并快照 integrated/source 冻结 510 个工作区/benchmark 输入，追加必要 fixture/脚本与格式化后保存 validated-inputs.json。完成：

- Windows **898** 项、Linux **897** 项完整 workspace/all-features 测试；不把额外显式 ignored 验收混入此数。
- native/WASM workspace all-features/all-targets 严格 Clippy；Linux 另测 no-default-features 和 paint 组合。
- Windows 实际窗口和 UI Automation 各一项；Linux X11/Wayland × shell/accesskit 四项实际窗口验收。未宣称 OS IME、屏幕阅读器和 Unix AT-SPI 已完成。
- 独立 Web fixture 构建、六项 fixture 测试、27 组浏览器套件及真实 pull-driven stream 传输。26 组保持源码不变的套件通过，stream 修复旧测试字段后单独重跑通过；完整失败记录和通过记录分别保留，汇总见 browser-verified.json。
- 64/1024 测量缓存均使用真实行高；含 ±0.375 等小数偏移的各次锚点 top 均为 -15.125 px、同一语义 ID，原 <0.1 px 容差不变；两个缓存的最终 logical/physical geometry 和 PNG 哈希相同。
- Windows/Linux CPU raster 首尾图像已生成并检查；各平台系统字体不同，不宣称跨平台 PNG 逐像素相等。
- 实际工作区的九个核心生产文件与合并验收快照逐文件核对一致，标准 benchmark 15 个应用输入与性能 before 字节相同；工作区 core/UI 测试亦直接复跑通过。

旧冻结实现的 Windows 884 / Linux 883 项只是先前验收，不冒充最终合并数量。Config.motion、Swipe 测试错误、feature 限定和 RichEditor 原生枚举缺口独立修复/记录，未改写性能 before。

验收另外修复了两个长期陈旧的测试假设：HTML marker 不再写死 gly:v:4，而严格匹配 fixture 编译的 glory::html::FORMAT_VERSION（本轮性能 before 为 v11、合并验收为 v13）；合成 stream frame 使用真实起始帧的 version，并显式提供 options:null。版本错误 999、非法标签/属性、乱序、2^53 以上身份、焦点/IME/滚动与背压检查仍保留。未改动运行时协议版本来迁就测试。

共享回归 target 曾出现源码包含 Rich/shape_in、Clippy 却引用旧 HTML 元数据的失败；在任务专用 check-target 强制重新核验通过。基准从一开始就使用每版本独立 target，不受这次回归缓存问题影响。

所有构建/cache/临时文件在 D 盘。Linux loop cache 经 backing file 核验后从 64 扩至 96 GiB，修复空间不足导致的链接失败；字体链接环境在 D 盘提供安装版系统库的 pkg-config 元数据，未修改引擎依赖。一次快照误复制的 counter target 留在任务构建目录，明确排除源清单和验证输入；后续快照只复制所需源码/lockfile。
