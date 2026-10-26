# Role 更新的字段依赖与前后测量（2026-10-26）

`Patch::Role` 原先为同一个 Semantics 分别生成旧角色和新角色的完整属性
`BTreeMap`，再逐项差分。这会重复克隆 id、关联键、语言、范围文本、layer
配置等没有发生变化的内容。现在完整映射与 Role 更新共用一个私有、固定五项
属性投影；Role 更新只比较该投影，不构造完整语义属性表。

这是字段依赖和规则归属的改进，没有新增 feature、裁剪功能、改变公共 API，
也没有合并不同类型的集合或绕过现有校验。`Patch::Semantics` 仍使用完整映射；
创建、快照、SSR 和 hydration 仍生成完整 Shape。

## 依赖及覆盖顺序

| 属性 | 依赖与原有覆盖规则 |
|---|---|
| `role` | effective Role 的名称；依次允许 combobox、option、tab、menu item kind 覆盖 |
| `aria-level` | Heading 等级；tree item level 优先 |
| `aria-multiselectable` | tree navigation multiple；Grid / Listbox 分别使用自己的 multiple，包含默认 false |
| `aria-modal` | 只有 Dialog / AlertDialog 且存在 layer 时取 layer.modal |
| `aria-orientation` | separator axis；slider axis 覆盖；TabList / RadioGroup / Toolbar / Menubar 的 navigation axis 最后覆盖 |

投影保持属性名字典序。差分先输出新值存在的变更，再输出删除，两组各自保持
字典序，与完整 Shape 差分一致。常量通过 `Cow::Borrowed` 借用，等级才生成
整数字符串；五项都只能生成规范 ASCII，不需要再次规范化用户文本。

effective Role 相同时可以直接返回空 edits，例如显式 Group 改为 None，但名字
仍使 View 的默认角色为 Group。此判断只跳过 HTML edits：原有准备、校验、
提交和 applied 水位照常执行，显式角色仍正确保存。没有缓存或保留访问历史。

## 正确性

- 保留现有所有字段的完整 Shape 差分 oracle，增加全部 Role、Heading 0–7、
  None / Keep / Clear 的转换矩阵；包括普通 Create 和共享 Plan、长名字/关联、
  两种导航方向、Grid / Listbox multiple、Dialog / AlertDialog 和 intrinsic 控件拒绝。
  3,990 次成功、3,578 次拒绝均与完整差分一致，包含 edits 的精确顺序。
  拒绝后 descriptor、模型统计、receipt、sink 均保持不变；成功空更新仍推进水位。
- 另以测试专用的旧顺序属性写入器作独立 oracle，验证 41 个角色输入与 1,024 组
  元数据组合，以及每组的 None 元数据情况，共 83,968 次投影比对。
  特意包含构建时会拒绝的重叠元数据来检查覆盖优先级，避免完整 Shape 和局部
  更新复用同一投影后掩盖共同错误。真实 Mapper 矩阵仍经过完整语义校验。
- `glory-html` 默认 / all-features 测试及严格 Clippy 通过；Sapid 默认 / 全 feature
  测试与 Clippy、共享 style/core/render/ui/html/ssr 全 feature 测试与 Clippy 通过。
  新 benchmark 的 native / wasm Clippy 通过。
- Chrome 实际 CSR、SSR hydration 的 semantics 与 Plan 验收通过，检查 AX 名称/角色、
  Keep 零 DOM 写入、Clear、输入值、选区、IME 和节点身份。
- 两版完整 rows 应用各自的 bindgen / Binaryen 产物全部通过浏览器契约验收。
  两版额外比较存活元素、文本、语义身份及全部 10,000 行矩形，结果一致。
  最小应用和 text 的现有体积门禁、真实浏览器 mount / cleanup 也通过。

## 性能

`benchmarks/role-patches` 测实际 Mapper.apply 的准备、校验、提交、属性输出和
计数 sink。没有 DOM、UI 回调、布局或帧调度，不能将结果换算为整页 FPS。
构造及结果序列化不计时；相同业务数据和输入序列，没有对未使用功能作裁剪。

Windows 11、i7-14700KF、Rust 1.98.0、Chrome 155.0.8059.84。
native / wasm 都使用 opt-level 3 + LTO + CU1；wasm 额外使用 bindgen 0.2.129、
Binaryen 132.0.0 `-Oz --enable-bulk-memory --enable-reference-types`。
编译和其他测试结束后运行四轮，交替前后顺序；每个案例每轮五次预热、二十个
正式样本，每样本 4,096 次 Role 更新。每格中位数来自 80 个样本。

单位 ns / patch；完整 IQR、原始样本位置及产物哈希见机器可读结果。

| 案例 | native 前 → 后 | 耗时变化 | WASM 前 → 后 | 耗时变化 |
|---|---:|---:|---:|---:|
| Group ↔ Status | 308.8 → 183.3 | −40.6% | 390.6 → 268.6 | −31.2% |
| Heading 2 ↔ 3 | 407.7 → 205.2 | −49.7% | 439.5 → 219.7 | −50.0% |
| Group ↔ Toolbar，导航方向 | 717.5 → 248.3 | −65.4% | 854.5 → 293.0 | −65.7% |
| Group ↔ Grid | 503.1 → 258.8 | −48.6% | 634.8 → 293.0 | −53.8% |
| 长关联元数据，Group ↔ Status | 1,517.9 → 226.7 | −85.1% | 1,806.6 → 268.6 | −85.1% |
| Dialog ↔ AlertDialog | 885.6 → 225.8 | −74.5% | 1,001.0 → 293.0 | −70.7% |
| 相同角色，长关联元数据 | 1,400.4 → 130.9 | −90.7% | 1,684.6 → 170.9 | −89.9% |
| 具名 View 的 Group ↔ None | 1,377.2 → 127.7 | −90.7% | 1,709.0 → 170.9 | −90.0% |

各版本 / 平台的编辑数量和有序内容校验值一致；浏览器无异常。以上为本机内核
实验，浏览器时钟有量化，未将每个 patch 当成独立统计样本，也未丢弃慢样本。

### 完整 rows 应用的额外回归对比

同一批 before / after 的最终 Binaryen WASM，再用未修改的 rows harness 做
前后 / 后前两轮。每项每轮三次预热、十二个正式样本，共 432 个正式样本。
下表为每版每项 24 个样本的 ScriptDuration 中位数，单位 ms；包含实际 DOM
工作。RAF 等待时间另存于机器可读结果，CPU 采样探针在计时结束后另行运行。

| 操作 | 前 | 后 | 耗时变化 |
|---|---:|---:|---:|
| 创建千行 | 17.897 | 18.840 | +5.3% |
| 替换千行 | 27.781 | 28.488 | +2.5% |
| 更新每十行 | 0.491 | 0.472 | −4.0% |
| 选行 | 0.628 | 0.632 | +0.6% |
| 换行 | 0.360 | 0.368 | +2.1% |
| 移除一行 | 0.333 | 0.349 | +4.5% |
| 追加千行 | 20.206 | 20.583 | +1.9% |
| 创建万行 | 223.385 | 219.186 | −1.9% |
| 清空千行 | 14.641 | 14.042 | −4.1% |

保留上述不利观测，不能宣称整页性能全面提升。该 workload 不以 Role patch
为热点，主要用于检查共享完整映射的实际表现。千行创建两轮分别为
18.148 → 18.057、17.582 → 19.317 ms，万行创建的两轮变化方向也相反；
只有两轮，尚不足以将小幅差异确定归因到本次投影实现。这里不根据体积数字
推断运行速度，也不把这个整页结果与上面的 Role 内核结果混为一谈。

## 完整应用 WASM

同一路径先后编译不可变的 before / after 输入，各用独立 target。应用仍为
`semantic-rows-v1` Glory，opt-level 3 + LTO + CU1，与之前三框架比较一致。
前基线已包含上轮 role / range / direction 的公共映射去重；此表只衡量本轮
Role 字段投影，不能与其他路径或旧轮结果拼接成 A/B。

| 指标（字节） | 前 | 后 | 变化 |
|---|---:|---:|---:|
| bindgen WASM | 1,852,963 | 1,845,014 | −7,949 |
| Binaryen −Oz WASM | 1,417,887 | 1,411,391 | −6,496（−0.46%） |
| WASM Brotli q11 | 376,131 | 376,383 | +252（+0.07%） |
| WASM gzip 9 | 512,757 | 513,260 | +503（+0.10%） |
| 全部 JS | 60,648 | 60,648 | 0，哈希完全一致 |

WASM 二进制减少，但压缩传输量略增。保留改动的依据是去掉整表重复生成、集中
定义依赖，以及实际更新开销下降，没有将压缩体积改善作为通过条件。

另一次现有最小应用门禁使用其既定 opt-level z 配置，minimal / text 分别为
894,630 / 915,505 B（Brotli 282,468 / 288,433 B）。该门禁基线早于本轮，
包含此前改动，只作为累计回归检查，不能据此推算本轮的独立收益。

## 复现与证据

[内核构建与测量说明](../benchmarks/role-patches/README.md)及
[机器可读汇总](../benchmarks/role-patches/results-2026-10-26.json)。
`benchmarks/snapshot.py` 现包含独立 Role fixture；前后库输入只变化
`html/src/semantics.rs`、`patch.rs`、测试模块及其注册，没有依赖或 feature 变化。

本机证据根目录 `D:/BuildCache/glory-role-patches-20261026`：
`inputs-before/`、`inputs-after/` 保留逐文件 SHA256；共同编译路径为 `source/`。
`before/`、`after/` 分别保存 native/WASM 内核和完整应用、Cargo metadata、
编译日志、bundle-size 报告及浏览器证据。`performance.json` 保存全部正式样本、
分位数和可执行文件/WASM 哈希。全部 target、缓存和临时文件位于 D 盘。
`rows-performance/evidence.json` 另存完整应用的 432 个计时样本、契约与截图结果；
诊断 CPU 采样与计时区间分离。

原始实验及日志不会被下一次运行覆盖；历史体积基线也没有重设。
