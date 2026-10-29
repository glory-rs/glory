# 引擎依赖维护

2026-10-29：发布前重新核对了 crates.io 下载包、源码提交和替换回归；
见 [fork 与发布版本调查](engine-fork-release-investigation-2026-10-29.md)。
当前发布包不能整体替换这些 fork；winit 的桌面源码与发布基线一致，
可以另行验证桌面端的依赖收敛，移动端补丁仍需保留。

2026-10-25：维护分支保留既有修复并升级 winit 至 0.31.0-beta.4，移除已不可达的
IME 事件兜底并增加五类事件转换回归。当前完整提交见下表；依赖和开放 PR 的
核对记录见 [dependency-update-2026-10-25.md](dependency-update-2026-10-25.md)。

第三方源码修改在 glory-rs 组织的真正 GitHub fork 中维护。Glory 仓库不保留
vendor、副本补丁树或子模块；应用无需添加全局 Cargo patch。

| Fork | 上游基线 | Glory 固定提交 | 维护范围 |
|---|---|---|---|
| [glory-rs/blitz](https://github.com/glory-rs/blitz) | `92b11f1b` / beta.4 | [ee125981](https://github.com/glory-rs/blitz/commit/ee125981e0e5a217e758be5c470df23cdd9e2fc3) | 资源生命周期、增量失效、焦点、密码显示、IME、阶段计时、inline 样式差分失效、隐藏子树销毁与移动缓存回收、真正的文档顶层、UIKit 键盘视口触摸保护、三态 checkbox、按 tree/form/name 分组的 radio checked property、独立 placeholder 布局/绘制、显式程序滚动取消旧触摸惯性、按地址撤销图片复用缓存/迟到结果、隐藏动画与 contents 盒退休，以及固定 Winit Activity 生命周期修复 |
| [glory-rs/taffy](https://github.com/glory-rs/taffy) | `eb645069` / 0.15.0 | [dd7914fb](https://github.com/glory-rs/taffy/commit/dd7914fba2e6e0b5d8a2c8854d764a2ade817efd) | 小数 flex 因子的 gap 计算与回归测试 |
| [glory-rs/winit](https://github.com/glory-rs/winit) | `d597ced7` / 0.31.0-beta.4 | [3b6f5a09](https://github.com/glory-rs/winit/commit/3b6f5a09697acb936567af64e53c915b0b1a2af9) | Android Destroy 退出当前事件循环，释放 Activity 循环独占许可后允许同进程重建；UIKit UITextInput、Android GameTextInput；保留上游包和锁定的测试依赖 |

恢复上游包名 blitz-dom/traits/paint/shell、stylo_taffy、taffy；Glory 自己的
适配 crate 继续叫 glory-blitz。Blitz fork 的 workspace 统一固定同一个 Taffy
Git revision，各包间保留上游正常的 workspace/path 依赖。因此没有修补前后
类型混用，也不将第三方 crate 加入 Glory 的十二包 workspace。上游代码、许可证、
仓库结构及 Git 历史保留在各自 fork。原有实现的 146 个源码/资源文件在迁移时
逐字节核对；Cargo 规范化 manifest 改回上游 workspace manifest。

运行环境先按 AGENTS.md 将 CARGO_HOME、CARGO_TARGET_DIR 和临时目录设到 D:。
`python scripts/check-engine-forks.py` 使用 Cargo metadata 和 Cargo.lock 检查
实际来源、完整提交、重复类型身份及 workspace 边界；加 `--test` 会在 D: 构建
目录的独立 Git checkout 中执行各 fork 固定锁的库测试，并确认 checkout 清洁。
Windows/Linux semantic gate 均调用该检查。独立 checkout 是正常 Git 工作副本，
不参与 Glory 的 path 依赖，也不修改 Cargo 管理的源码缓存。
Blitz fork 的 Linux/FreeBSD 测试依赖启用 Fontique 的 fontconfig-dlopen，
与适配器一致；运行时仍需 Fontconfig 和字体，不需要安装其开发头文件来跑库测试。
`extra_tests` 还启用 `parallel-construct` 执行计时器测试，验证并行聚合、reset
代次和真实文本构建。计时器只保留 13 个固定聚合值，不缓存节点、文本或帧。
样式差分测试也在此配置执行：top/height 和等价声明复用文本，Clear 恢复
stylesheet；16种连续样式变化与强制完整重建对照，属性选择器仍重算关联节点。
inline style 保留 snapshot、selector/inherited restyle 与祖先标记，只去掉
无条件 ALL_DAMAGE，交给 computed-style difference 分类；CustomWidget 继续
保守完整失效。placeholder 修改同样由样式差异决定编辑器损伤；提示文字单独更新布局，不重写值或预编辑。其余属性修改仍保持原路径。
默认不安装 collector；开启后使用引擎自身的单调时钟，Glory 在 resolve 返回
后导入其固定快照，不跨线程传递 Glory 的本地响应式对象。

修改流程：在 fork 上从维护提交建分支，提交并推送修复与测试；若修改 Taffy，
先更新 Blitz fork 的统一依赖和测试锁，再更新 Glory 的四个 Blitz Git revision、
主/示例 Cargo.lock 及 [engine-forks.json](engine-forks.json)，运行 fork 检查与
Glory 的语义、原生几何/窗口回归。合入上游后也通过审查过的完整提交升级。
不得改成浮动 branch、未提交本地 path、在应用端打 patch 或重新引入 vendor。

这些 fork 的维护补丁尚未随对应包发布到 crates.io；同名版本的注册表包不含这些
补丁，当前保证的是 Git 源构建。早期实施记录中的本地
源码校验和命令仅为历史证据，当前复现以本页和检查脚本为准。

2026-10-24 的隐藏子树销毁回归来自 Sapid NavLink 的 Unmount 面板。销毁节点时
按记录的 layout parent / out-of-flow containing block 清除布局、hoisted 和派生
paint 缓存；不扫描文档，也不经已经删除的 DOM 祖先传播失效。公开的非空
remove-all 入口另标记仍存活的父节点结构失效，防止祖先 stacking context 重放
已删除条目。旧提交的混合 inline/block、隐藏祖先及 hoisted 回归实际失败；新提交
100 项库测试、parallel profile 5 项、样式差分 5 项及销毁矩阵 4 项通过。
矩阵覆盖 block/flex/grid/contents、root-first、隐藏前后时序、remove/remove-all、
匿名盒、Parley embedded box、祖先 stacking 缓存和代次复用；日志在
`D:/BuildCache/sapid/engine-hidden-all.log` 与 `engine-hidden-parallel.log`。
适配器/组件真实原生验收仍独立执行，不由 fork 库测试推断。

同日的图像终态扩展为每个存活 img consumer 分配不回绕的 ImageRequestId，独立于
URL 共享 fetch。只有实际消费、缓存命中或明确 admission/no-provider 失败后才记录
一次 Loaded/Failed；旧 consumer、旧请求、移除/复用身份和已 drain 后的重复结果均
不能重新发布。每个存活 img 至多保留一个 consumer 和一个待领取终态，无历史 URL
集合。计数耗尽返回 None；适配器安装时映射该已捕获语义 revision 的 Failed。
真实 fork 的 106 项库测试分别在顺序与 parallel-construct 配置全部通过，含六项
图像消费/ABA/共享请求/预算/退休/FIFO/重复完成回归。对应日志：
`D:/BuildCache/sapid/engine-image-observation.log`、`engine-image-observation-parallel.log`。
宿主事件、Web decode Promise 和 Sapid fallback 仍由各自的实际验收确认。

原生浮层扩展由
`7e0b5f85727da016285028f656a437d89ca10c46` 引入，当前固定提交见上表。保留逻辑 DOM/Stylo 继承父链，
引擎独立进行 viewport 布局、最后绘制和逆序命中，modal backdrop 参与真实绘制；
不使用 portal。当前 lease 上限为 1024，detach/drop 退休，connected move 保留。
显式 `contain_scroll` 复用 wheel/touch/fling/editor 的原生滚动链。
`native-top-layer-final-provenance.log` 的独立干净检出验证通过 Taffy185、Blitz113、
parallel/writing-mode114、既有 focused 及 CPU actual paint1；focused 与全库重叠，
不合并为独立用例数。宿主焦点/AX/组件验收见 [native-top-layers.md](native-top-layers.md)。

2026-10-25 的 connected move 修复来自 Sapid OverflowList 的真实原生弹出验收：
旧隐藏面板的 DOM children 已空，但派生 layout children 仍引用移入测量区的条目。
当前固定提交在改变 DOM parent 前退休跨移动子树边界的 layout / hoisted 引用，
清除旧 owner 的布局、绘制和 stacking 缓存；anonymous children 也是派生列表。
内部布局关系、真实 DOM children、NodeId、焦点、原始文本、selection 和 preedit 保留。
最小基线失败保存在 `D:/BuildCache/sapid/overflow-engine-baseline.log`；
默认和 parallel-construct 全库各 126 项通过，新增矩阵覆盖 24 配置、每配置 16 次
往返移动及同父重排。完整日志为 `overflow-engine-full-tests.log` 和
`overflow-engine-parallel-full-tests.log`（均在 `D:/BuildCache/sapid`）。
`engine-forks.json` 的并行附加检查包含 `layout_move_tests`；实际适配器和组件仍需独立验收。
