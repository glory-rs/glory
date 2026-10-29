# Git fork 与发布依赖调查（2026-10-29）

结论需要区分层级：Taffy 的补丁没有新增 API，问题是已知布局误差；
Blitz 有新增接口，直接替换会阻断当前原生适配器编译；winit 的功能差异集中
在移动端，桌面端可以单独评估收敛。Web/SSR 路径不依赖这三个引擎 fork。

按用户要求，版本发布和 js-framework-benchmark 暂停。没有打 tag、创建 Release、
上传 crates 或改动 fork 的远端分支。Sapid 根库清单中的 `publish = false` 已移除；
应用/测试项目的私有发布标记保持原状。此前性能优化的工作副本保持完整。

## 核对方法

- 从 crates.io API 读取当前最高版本、稳定版本、发布时间和 SHA-256。
- 下载八个实际 `.crate`，校验 API 提供的 checksum，读取 `.cargo_vcs_info.json`，
  避免把相同版本号当成相同源码。
- 对照 `docs/engine-forks.json` 的固定提交、fork Git 历史和最新上游分支。
  只更新 D: 独立 checkout 的调查用远程引用，没有修改 Cargo 管理的源文件。
- 在 D: 独立小项目中，对发布包和 fork 运行同样的布局回归/API 编译探针。
  没有改动 Glory/Sapid 的依赖来源来绕过失败。

完整证据在 `D:/BuildCache/glory-release-review-20261029/fork-investigation/`：
`registry.json`、`sources.json`、校验后的 `.crate`、`registry-source/`、两个 probe
及其 Cargo.lock、编译/测试日志。Sapid 的包检查另存于
`D:/BuildCache/sapid/release-review-20261029/package-list.txt` 和 `package-check.txt`。

## 依赖现状

| 依赖 | 当前固定 fork | crates.io 对照 | 结论 |
|---|---|---|---|
| Taffy | `dd7914fb`，基于 0.15.0 | 0.15.1，源码 `0064bd54`，2026-10-25 发布 | API 可编译；两项 gap 回归失败，替换会接受布局误差 |
| Blitz 五包 | `ee125981`，基线 `92b11f1b` | 0.3.0-beta.4，源码仍为 `92b11f1b`，2026-10-16 发布 | 缺少适配器所用 API，不能直接替换 |
| winit 平台包 | `3b6f5a09`，基线 `d597ced7` | 0.31.0-beta.4，源码 `d597ced7`，2026-10-24 发布 | 桌面源码一致；移动端 fork 修复未包含 |

Blitz 的稳定线分别是 blitz-dom 0.2.4、traits 0.2.0、paint 0.2.1、shell 0.2.3、
stylo_taffy 0.2.0；winit 稳定线是 0.30.13。表中的 beta 是**已发布的预发布包**，
不是稳定版。本次重点验证当前所用 API 代际的可替换性，没有执行降级迁移。

## 修改规模、功能扩展与实际阻断

下面按固定 fork 相对其上游基线统计 `git diff --numstat`；它衡量补丁规模，
不是相对整个库的占比，也不是替换回官方版的精确工作量。原始分类在
`patch-statistics.json`。

| Fork | 提交 | 源码路径新增/删除 | 独立测试新增 | 锁文件新增/删除 | 主要性质 |
|---|---:|---:|---:|---:|---|
| Taffy | 1 | +8 / -2 | +91 | +1997 / -0 | 两处算法表达式修复，未增加 API |
| Blitz | 26 | +4377 / -239 | +2274 | +102 / -71 | 功能/API 扩展与内部正确性修复并存 |
| winit | 7 | +1176 / -77 | 计入源码路径 | +2586 / -0 | 移动端后端能力补齐与生命周期修复 |

“源码路径”仍包含注释和内嵌单元测试，不能把这些数字全部称为生产代码。
文档和清单另计。Taffy 的 +8 中还含测试模块接线；真正算法只改了 grow/shrink
两处表达式。winit 的总 diff 很大一部分是锁文件，因此不能用总行数判断功能规模。

具体影响如下：

| 类别 | 是否增加功能/API | 换成官方包的后果 |
|---|---|---|
| Taffy flex/gap | 没有新增 API | 同一份公开 API 回归代码可编译运行；特定小数 grow/shrink 与 gap 的布局尺寸错误，几何回归测试失败。应用不因此必然无法启动 |
| Blitz 资源管理 | 新增 ResourceLimits/Stats、请求身份、完成事件、撤销等接口 | 当前适配器直接引用，缺失时编译失败；丢弃这些接入还会失去相应预算/取消/状态语义 |
| Blitz top layer | 新增 TopLayerConfig、set_top_layer 等引擎能力 | 当前原生浮层与 Select/菜单等接入无法编译；重新实现或降低功能需要另行设计 |
| Blitz profile | 新增引擎阶段计时接口 | 当前接入无法编译；独立移除该接入会减少计时能力，但不能消除另外的阻断 |
| Blitz 隐藏删除/移动缓存、焦点等 | 大部分是内部行为修复 | 未必在普通静态页面触发；动态卸载/重排可能有残留绘制、布局/命中或编辑状态问题 |
| Blitz placeholder、三态 checkbox、radio、IME | 补齐标准控件/平台行为，包含新增实现和接口 | 依赖这些行为的原生组件会失去对应正确性，不能把所有问题都称为轻微视觉差异 |
| winit Windows/Linux | 对照的公共/桌面源码树没有变化 | 没有发现来自这些补丁的桌面功能差异；统一依赖来源后仍需替换验收 |
| winit Android/UIKit | 新增后端 IME 实现，修复 Activity 退出/重建；公共 winit-core API 未变 | 编译接口未必阻断，但特定移动端输入或重建流程可能成为功能性阻断 |

特别是 Blitz 的 ResourceLimits、图片观察和 layers 在当前非 WASM 文档适配中
直接引用，并非仅在用户打开某个菜单或触发资源加载后才动态查找。因此直接替换
发布包，原生宿主会先在编译阶段失败。该事实不表示官方 Blitz 无法构建基本应用，
而是当前 Glory 已建立在我们扩展过的引擎接口之上。
Taffy 的探针没有执行整个 Glory/Blitz 依赖图的正式替换验收；其结论是现有补丁
没有引入额外 API 障碍，仍需在统一来源后验证完整项目。

## Taffy：小数 flex 因子与 gap

fork 相对其上游基线只有一个补丁提交，修正 grow/shrink 因子和小于 1 时，
固定 gap 被重复扣除的问题。0.15.1 的实际发布源码仍有这两个扣除表达式。

把 fork 的 `flex_gap_tests.rs` 通过公开 Taffy API 原样运行，只有导入路径因
依赖别名调整。测试同时覆盖行/列、grow/shrink、三种 gap 和八组因子：

| 同一组测试 | 发布版 0.15.1 | 当前 fork |
|---|---:|---:|
| fractional grow | 失败 | 通过 |
| fractional shrink | 失败 | 通过 |

例如容器宽 300、两个 50 宽元素、grow 均为 0.25、gap 为 10：
期望每项 97.5，发布版得到 92.5。对应 shrink 用例期望 197.5，得到 192.5。
这会改变真实原生布局。测试结果分别为 registry exit 101、fork exit 0，
见 `flex-results.json`、`taffy-registry-test.txt`、`taffy-fork-test.txt`。

0.15.1 另有上游性能和布局改进，但这些改进没有包含本次回归所需修复；
不能仅根据“版本更新”判断已经等价。上游发布说明见
[Taffy v0.15.1](https://github.com/DioxusLabs/taffy/releases/tag/v0.15.1)。

## Blitz：新增接口和原生正确性补丁

当前 fork 比已发布 beta.4 基线多 26 个提交，差异涉及 57 个文件，包含测试、
锁文件和维护记录。主要维护内容包括：

- 有预算的资源接收、缓存、取消、文档关闭和迟到图片结果退休。
- 原生 viewport top layer、浮层命中和滚动边界，保留逻辑 DOM 父链。
- 隐藏子树删除、connected move、派生布局/绘制缓存与动画的正确失效。
- 焦点、密码显示、IME、独立 placeholder、三态 checkbox 和 form-scoped radio。
- 固定规模的阶段计时与平台生命周期集成。

当前 Glory 实际调用了发布版中没有的 `ResourceLimits`、`ImageRequestId`、
`ImageLoadStatus`、`TopLayerConfig`，以及 `set_top_layer`、`top_layers`、
`revoke_image_url`、`set_profile`。调用位置包括
[document.rs](../crates/blitz/src/document.rs)、
[layers.rs](../crates/blitz/src/layers.rs)、
[image_observation.rs](../crates/blitz/src/image_observation.rs) 和
[files_io.rs](../crates/blitz/src/files_io.rs)。

独立 API 探针使用同一份源文件：发布包有五项编译错误（缺失导出/方法），
fork 编译通过。探针仅证明接口兼容性，不把编译当作窗口/IME/设备验收。
见 `blitz-results.json`、`blitz-registry-check.txt`、`blitz-fork-check.txt`。

因此当前替换 Blitz 会先遇到编译问题，之后还需要处理引擎内部行为差异。
单纯删除 Git 地址或改回同名版本号不能保留这些能力。

## winit：移动端补丁，桌面可单独收敛

fork 相对发布基线多七个提交。除测试锁文件外，修改集中在 `winit-android`
和 `winit-uikit`：

- Android Activity Destroy 后正确结束旧事件循环，释放互斥许可，让同进程重建。
- UIKit 连接 Scene、UITextInput/marked text 桥接和组合输入期间延迟键盘特性切换。
- Android GameTextInput 的预编辑/提交转换和输入缓冲区边界。

Git tree 对照确认 `winit`、`winit-core`、`winit-common`、`winit-win32`、
`winit-wayland`、`winit-x11` 在 fork 与发布基线间逐字节一致。
因此不能把 winit 描述成“所有平台都离不开发行外代码”：Windows/Linux 有
独立换回发布包的空间，但尚未在本次调查中做完整桌面替换验收。

若要调整，必须同步 Blitz fork 中的 winit、Glory 直接依赖和锁文件，保证
相同平台只有一套类型身份。只改 Glory 一处依赖可能与 Blitz shell 的类型
来源冲突。整体切换会丢失移动端修复，本次没有更改依赖或重做设备验收。

## 对 crates.io 发布的影响

Cargo 允许 `git + version`，但发布后的依赖会使用**注册表版本**，不会把 fork
源码随库一起分发。当前清单中的 `version = "=0.3.0-beta.4"` 等字段不能将
Glory 的 fork 变成 crates.io 上的同名包。规则见
[Cargo：Multiple locations](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#multiple-locations)。

所以 Git 工作副本构建通过，不能代替发布后依赖图的验证。必须先让所需补丁
进入可分发的依赖版本，或明确设计、维护另一条可发布依赖路径，再验证打包后
的实际产物。当前没有通过关闭原生功能或删除回归来强行发布。

Sapid 的 `publish = false` 已按要求移除，但 `glory` 仍是没有 version 的 path
依赖。`cargo package --allow-dirty --list` 能列出文件，实际执行
`cargo package --allow-dirty --no-verify` 则在清单验证阶段失败（缺少 version），
还未进入编译验证。后续发布需要为它指定
与实际发布的 Glory 匹配的版本，并按依赖顺序发布。此处先记录问题，不擅自
选择一个尚未发布或接口不匹配的版本。

调查后的下一步应围绕补丁的上游合并/发布路径，以及 winit 的平台拆分取舍来
决定。当前发布和 benchmark 继续暂停，等待这项依赖决策。

另外，实际查询 `glory --no-default-features --features dsl --target
wasm32-unknown-unknown` 的 normal/build 依赖树，三个原生引擎均未出现，
见 `web-graph.txt`。这些 fork 的维护与发布问题不应混同于浏览器 DSL 的
运行路径或此前 WASM 体积问题。
