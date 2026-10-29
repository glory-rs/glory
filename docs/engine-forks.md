# 引擎依赖维护

2026-10-29：维护的 fork 使用独立发布名称，Glory 0.6.0 改用 crates.io 的精确版本。
原生适配器为 `glory-apace`，门面模块为 `glory::apace`，功能开关以 `apace` 开头。
Widget、Each、DSL、SSR、hydration 和 native 的接口保持同一套实现。

| 维护项目 | 原始上游 | 首次发布版本 | 维护范围 |
|---|---|---|---|
| [Apace](https://github.com/glory-rs/apace) | DioxusLabs/blitz | 0.3.0-beta.4 | 文档、资源、焦点、IME、布局/绘制失效、顶层、编辑器、动画和节点回收补丁 |
| [Takia](https://github.com/glory-rs/takia) | DioxusLabs/taffy | 0.15.0 | 小数 flex 因子与 gap 计算补丁 |
| [Wingy](https://github.com/glory-rs/wingy) | rust-windowing/winit | 0.31.0 | Android Activity 生命周期、GameTextInput 和 UIKit UITextInput 补丁 |

支持包保留其原来的版本：`wingy-dpi` 0.1.2、`apace_accesskit` 0.2.1、
`apace_debug_timer` 0.1.3。Apace 与 Glory 使用同一组 Takia/Wingy 类型。
原始许可证、版权说明、Git 历史和上游来源保留；旧名称出现在历史报告中。
补丁与官方发布包的差异见 [发布前调查](engine-fork-release-investigation-2026-10-29.md)。

各项目的 `main` 是维护分支。每日同步任务只更新 `upstream-main`：
Apace/Takia 对应官方 main，Wingy 对应官方 master；不会自动覆盖或合并到 main。
各库的 `RELEASING.md` 说明 tag 发布、验证门禁和凭据配置。

`docs/engine-forks.json` 记录发布版本、完整源码提交与官方基线。
`python scripts/check-engine-forks.py` 验证 Cargo metadata、lock checksum、
注册表归档的 `.cargo_vcs_info.json`、单一类型身份和工作区边界。
加 `--test` 会在规定缓存目录中的独立、清洁 Git checkout 上执行发布源码测试。
这些 checkout 不作为应用 path 依赖，也不修改 Cargo 管理的缓存。
Windows 的 Cargo home、target 和临时目录均使用 D:/BuildCache。

后续修复先在对应维护仓库完成测试并发布新版本，再更新 Apace/Glory 的精确版本、
lockfile 与来源记录。应用不需要全局 Cargo patch，也不引入 vendor。
实际窗口、IME、无障碍和设备验收仍是独立门禁，不能用编译或模型测试代替。

以下保留既有补丁的验证记录，历史提交和旧构建路径对应当时的源码。

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
`native-top-layer-final-provenance.log` 的独立干净检出验证通过 Takia185、Apace113、
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
