# 引擎依赖维护

2026-10-30：Glory 0.6.1 开发源码使用同级维护仓库，尚未发布。
精确版本为 Apace 0.3.0、Takia 0.15.1、Wingy 0.31.1；支持包为
`apace_accesskit` 0.2.2、`apace_debug_timer` 0.1.4、`wingy-dpi` 0.1.3。
请将 apace、takia、wingy 与 glory 放在同一父目录，Sapid 也放在该目录。
Apace 的 Stylo 继续使用固定 Git 修订，保留本次上游合并所需的接口；不另建发布分支。

`docs/engine-forks.json` 记录当前开发版本、合并基线、上游来源及此前发布记录。
`python scripts/check-engine-forks.py` 校验精确版本、本地源码路径、单一类型身份和工作区边界。
`--test` 在这些开发源码上执行测试；开发模式不会宣称已验证注册表归档。
后续发布必须重新固定发布来源并完成原有注册表、校验和及发布门禁，本次不执行发布。
原始版权、许可证和第三方来源保留，无 vendor 或全局 Cargo patch。

所有库、示例和基准的 Cargo.lock 均不纳入 Git；website 独立保留锁文件。
设置 Cargo 构建环境后运行 `python scripts/library_lockfiles.py`，首次生成缺失的锁，
后续 `--locked` 检查复用该解析。Windows 的 Cargo home、target 和临时目录使用 D:/BuildCache。
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
