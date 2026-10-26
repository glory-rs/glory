# Glory 与 Dioxus 功能差距及补齐报告

Glory 的主要缺口集中在移动平台交付、桌面分发、应用级导航与文档服务、开发调试工具。响应式内核、浏览器渲染、SSR、水合、流式资源传递和常用交互组件已经有实现；不能把旧架构删除的能力、README 未列出的能力和真实缺口混为一谈。本轮按现有语义架构补齐，SSR 的 HTTP 框架集成只支持 Salvo。

## 对比范围

源码基线为 2026 年 10 月 26 日的两个本地工作树：

- Glory `D:/Works/glory-rs/glory`，起始提交 `463272a9ea90ae2f7a544a46fcd903e03e225002`。
- Dioxus `E:/Repos/dioxus`，提交 `d358d58dc1a100d058e5a6d14d0724ae843f7a65`。

以下 Dioxus 路径均相对于该工作树，比较的是其当前源码，不把主分支能力当作已发布版本承诺。Glory 的平台证据以 [实施记录](semantic-ui-implementation.md)、当前测试和 manifest 为准；历史记录中的验收不代表本轮重新运行了全部平台。

## 功能对照

| 功能 | Dioxus 源码 | Glory 起始状态 | 差距及处置 |
|---|---|---|---|
| UI 声明与更新 | `packages/rsx`、`core`、`core-macro`，模板和 VirtualDOM | 手写 builders、显式 Plan、有限语义补丁 | 保持架构选择，不引入 RSX、VDOM 或兼容别名 |
| 响应式状态 | `signals`、`stores`、`hooks` | Cage/Bond/Lotus、Owner、Binding、逻辑字段和集合编辑 | 已覆盖核心用途；Deref/自定义容器投影仍有差距 |
| 异步状态与取消 | `hooks/src/use_resource.rs`、`use_future.rs` | Owner resource/task、Host、原生 ready-only executor | 已有生命周期与过期请求撤权；不需要另一套异步运行时 |
| Web | `packages/web`、`interpreter` | WASM DOM、RAF、Plan 批处理、受控输入 | 有实现和 Chromium 验收；浏览器覆盖范围仍需持续扩大 |
| 同步 SSR 与水合 | `packages/ssr`、`web`、`fullstack-server` | 共用 HTML mapper、Snapshot、显式资源恢复 | 已有实现；SSR HTTP 集成仅 Salvo |
| 渐进 SSR | `fullstack-server`、`web/src` 的 suspense/hydration | Shell/Patch/End、预编辑捕获、资源交接、Salvo 背压与断开取消 | 保留现有契约；不新增 Axum/Actix 等适配器 |
| 桌面 WebView | `desktop` 使用 Wry/Tao，含窗口、菜单和托盘等系统服务 | 活动后端为 Blitz CPU 桌面窗口 | WebView 是独立引擎路线；不把增加别名当作增加平台 |
| 原生自绘 | `native`、`native-dom`，Blitz，多个可选绘制器 | Blitz、AnyRender、Vello CPU、softbuffer、多窗口 | 缺少 GPU 交付路线；CPU 能力不等同于 GPU 完成 |
| Windows | `desktop`、CLI Windows 构建/分发代码 | 有真实窗口、恢复、输入协议和 UIA 客户端记录 | 分发工具不足；真实 OS IME、读屏和完整文本选择另验收 |
| Linux | desktop/native、CLI Linux 分发 | 有 WSL X11/Wayland、字体、CPU 及窗口记录 | AT-SPI 客户端、桌面发行版分发与安装仍需补齐 |
| macOS | desktop 的 AppKit 集成、CLI Apple 构建 | winit/AccessKit 等依赖有对应平台路径 | 没有当前项目的 macOS 实机验收；需原生构建、签名、公证与安装验收 |
| Android | `desktop` 的 JNI/NDK、CLI `build/android.rs` 和 Android assets | 现有记录只有 aarch64 编译检查，桌面 Loader/AccessKit 有平台限制 | P0：Activity 入口、Surface 生命周期、资产、软键盘/IME、APK 和设备验收均缺 |
| iOS | desktop UIKit 路径、CLI `build/apple.rs` | 没有完整移动宿主和应用工程 | P0：入口、生命周期、资源、输入、签名及模拟器/设备链路均缺 |
| 路由与历史 | `router`、`history`，嵌套路由、导航和历史 provider | 语义 Link/Navigate 与应用处理入口；旧路由已删除 | 应用级 URL/history、嵌套匹配和返回恢复缺；如新增，应是当前语义架构的显式独立能力 |
| 文档头 | `document/src/document.rs`，标题/meta/script/style provider | SSR `Document` 可输出标题与 head；语义 DocumentPart 负责正文结构 | 缺跨 Web/SSR/桌面一致的动态标题/meta 所有权；DocumentPart 不能充当 head 管理 |
| 服务端函数和传输 | `fullstack` 的 server 调用、SSE/WebSocket/表单/文件等 | Salvo SSR/stream；应用可直接使用 Salvo 业务服务 | 缺类型化客户端/服务端调用约定；不恢复旧 serverfn 宏或引入其他 HTTP 框架 |
| 组件与交互 | `html` 事件/元素与组合组件入口 | 除八基础节点外，已有 select、combobox、tree、grid、table、menu、layer、focus scope、range、rich editor 等有限语义 | 不能按八个节点误判只支持八种组件；需按宿主核对每项能力和拒绝行为 |
| 文件与剪贴板 | HTML/desktop 的 file、dialog、clipboard 路径 | 已有 FileToken/分块读取/preview 和 clipboard 协议及各宿主测试 | 继续核对原生系统选择器、权限、拖放和跨平台行为，协议测试不能替代系统操作 |
| 样式与资产 | Web CSS、Manganis、CLI asset 处理 | 有限 Sheet/typed style、desktop 显式 Loader/资源预算 | 更广 CSS/主题、静态资产指纹/压缩/图片转换、移动包资产仍缺 |
| 国际化 | 对比树以应用/生态集成为主 | 独立可选 Fluent catalogs、Owner 消息、显式语言源 | 已有；不把自动 locale/global cache 当作必须补齐项 |
| 开发体验 | `cli`、`devtools`、`rsx-hotreload`、`subsecond` | 项目内 CLI：doctor/print/build/run/serve/check/fmt/bundle/init；Web watch/reload | Rust 热补丁、状态保持、检查器、桌面 watch 和调试信息仍缺；完整重载不等同热替换 |
| 构建目标与打包 | CLI 把 target、renderer、bundle-format 分开 | 起始状态固定 host/WASM，desktop bundle 直接拒绝 | 本轮先补目标选择、交叉运行拒绝、目标诊断和桌面目录交付；安装器与移动包独立推进 |
| WASM 优化 | `wasm-split`、CLI 资产/压缩流水线 | 已有体积基线和独立 benchmark | 缺应用代码拆分与发布优化；性能比较不能替代功能覆盖 |
| LiveView | `packages/liveview` | 已退役，不属于活动架构 | 不恢复旧适配器；如有实际需求，应另行定义远程语义协议 |

## 平台支持的完成条件

“能选择 target”“Cargo check 通过”“能启动窗口”“能完整交付应用”分别记录。移动平台至少要有 Activity/应用入口、Surface 销毁重建、暂停恢复、DPI/安全区、触摸和滚动、软键盘与真实 IME、字体和包内资产、联网取消、无障碍及打包安装的端到端证据。

桌面安装器还要验证资源定位、可执行权限、系统依赖、图标、启动入口、更新/卸载、macOS 签名和公证。可执行文件加 assets 的目录交付可以解决手工部署，但不代表这些工作完成。

当前会话为 Windows，已建立隔离在 D: 的 Android 16 x86_64 模拟器。用户确认暂时没有可用 Mac，iOS 保留明确待验收项；Android 模拟器也不能替代 arm64 真机。其他目标分别记录编译或运行证据，不能因为依赖支持这些 OS 就标为 Glory 已验收。

## 补齐顺序和边界

1. **P0 跨平台工具**：显式 Rust target、按 target 隔离产物、目标环境诊断、拒绝在本机运行外国目标；桌面可重定位目录和完整哈希清单。保持 CLI 独立 workspace，不进入运行时依赖图。
2. **P0 移动宿主**：先 Android 单条纵向链路，复用 WindowApp 的语义/调度能力并传入平台事件循环，禁止用桌面无参入口替代 AndroidApp；之后补 iOS 应用工程。引擎修改遵循 glory-rs 真正 fork 和固定 SHA 规则。
3. **P1 平台验收与分发**：macOS 原生窗口和辅助技术；Linux AT-SPI 实际客户端；真实 OS IME；Windows/macOS/Linux 安装和移动设备安装。验收矩阵须保留失败与平台依赖。
4. **P1 应用服务**：新架构下的显式 history/navigation 和文档头服务，Owner 清理、SSR 请求隔离及 hydration 一致性。服务端传输只基于 Salvo。
5. **P2 开发和发布优化**：桌面 watch、诊断面板、资产优化、WASM 分包与按收益选择 GPU。热补丁须单独处理代码替换和 Owner 生命周期，不能宣称 Web reload 保持状态。

## 本轮实施与验证

### 跨平台构建和桌面目录交付

`tools/cli` 增加 `--target` 和每宿主的 `package.metadata.glory.<platform>.target`；命令行优先。构建、check 和完整依赖 metadata 都采用所选 target。Web 仍只接受 wasm32；桌面/服务器只接受有对应宿主的内置 Rust 目标，不把 Android/iOS triple 当作移动实现。`doctor` 核对目标标准库，外国目标的 `run` 在构建前拒绝。

桌面 `bundle` 现在生成 release 可执行文件、显式 assets 和 SHA-256 清单，目录为 `bundle/<package>/<platform>/<target>`，不同架构互不覆盖。原子替换失败保留上一次产物；资产不得覆盖可执行文件或清单。目录包可复制后运行，安装器、签名、公证和更新器仍独立待办。

真实 Counter release 构建暴露并修复 Windows 路径问题：`canonicalize` 的 `\\?\` 前缀让 Stylo Python 生成器无法处理 `OUT_DIR/..`。现在先验证实际 D: 路径，再给 Cargo 普通绝对盘符路径；子进程 Python 缓存也归入 D:。原始失败后同一实际 Counter 构建成功，未改第三方生成器。

`tools/cli/tests/native-acceptance.ps1` 创建使用真实 Glory 的独立小应用，验证配置/命令行 target、跨目标拒绝、资产和清单哈希，再复制到带空格的新目录运行。它同时检查 Cargo.lock 和其他架构产物保留，已接入 Windows 常规检查脚本。控制台 fixture 只证明工具和资源定位；Counter release 包另行构建。

### Android 宿主和生命周期

`glory-blitz/shell` 的真实依赖与模块开放 Android/iOS 平台；`Shell::android(AndroidApp)` 显式接收本次 Activity，`Shell::from_event_loop` 接受应用配置的事件循环。`WindowApp`、有限事件、Owner 和 ready-only executor 继续复用。AccessKit 和桌面 Loader 仍按平台限制，没有通过 feature 名称宣称移动能力。

[移动示例](../examples/mobile/README.md) 独立于十二包运行时 workspace，挂载原有 `counter::Counter`，提供 NativeActivity manifest、NDK 链接、APK 对齐、D: debug key 签名、产物哈希、隔离 AVD 启动和验收脚本。用户明确授权的 `#[unsafe(no_mangle)]` 仅出现在应用入口；没有新 unsafe 块或运行时 lint 例外。SDK/NDK 为已有只读工具，编译、Java/Python 缓存和模拟器可写磁盘均在 D:。

实际测试发现上游 Winit 的 Activity 生命周期缺口：关闭后同进程重开被永久的 `EVENT_LOOP_CREATED` 标记拒绝，Destroy 也未退出旧循环。修复位于真正的 [glory-rs/winit 固定提交](https://github.com/glory-rs/winit/commit/07fe20dec93211df240050f76dada687c0d705ee)：Destroy 退出当前循环，独占许可在其字段全部析构后归还，重叠循环仍拒绝。包含正常析构/构造 unwind 回归和固定测试锁。

[glory-rs/blitz 固定提交](https://github.com/glory-rs/blitz/commit/80938ef8369257e82080ebd7d477b254487accec) 与 Glory 同时固定该 Winit 源，避免两份窗口类型。manifest、活动锁文件、[来源记录](engine-forks.json)和检查脚本一起更新。没有修改 Cargo 管理的 checkout，没有 vendor 或应用级 patch。

Android 16 x86_64 的 APK 已安装并实际运行。`D:/BuildCache/glory-mobile/acceptance-fixed/result.json` 记录绘制、加减触摸、注入编辑、Gboard 显示与真实软件键点击、同进程 Home/恢复、横竖屏恢复、恢复后的清空和 **Back 关闭后同进程 Activity 重建再点击** 均通过。截图人工核对显示值 `0 → 1 → 17 → 178`；脚本同时以固定值标签区域的像素哈希检查回退/恢复，保留完整截图和进程日志。

旋转动画会暂时拉伸旧缓冲；原先固定 1 秒抓帧误报，最终改为有上限地等待最终目标尺寸和相同标签像素。未放宽最终像素相等要求，也未以编译代替实际显示。初次启动测试还补上旋转锁与启动前屏幕方向检查；失败截图保留。

### iOS 入口和明确边界

示例提供 UIKit Rust 入口、Info.plist 和 `build-ios.sh`。脚本验证 macOS/Xcode、D: 远程挂载与 Cargo 实际目录，生成 ad-hoc 签名的模拟器 `.app`。当前仅检查 shell 语法和 plist 内容；没有 Apple SDK 编译结果。用户确认暂时没有 Mac，故不声称 iOS 可运行或已验收。

后续需要 Mac 上的构建/安装/启动、UIKit 暂停恢复/尺寸与安全区、软键盘/CJK/VoiceOver、真机签名和 IPA 分发。Android 还缺真机、CJK 组合输入、TalkBack、包资产/联网 Loader 和权限等覆盖；桌面缺各系统安装器与 macOS 本机验收。这些均未被本轮勾选完成。

### 检查结果

- Glory 全工作区 `--all-features` 测试通过；native 全工作区 `--all-features --all-targets` 严格 Clippy 通过。
- 三个固定 fork 的独立 checkout 测试通过，包含 Winit core/common 和独立 Activity 生命周期测试；metadata/lock 确认十八个引擎/平台包各只有一份来源身份。
- Android 全图严格 Clippy 的首轮触发 Rust 1.98 对已有 const thread-local 的诊断；未为此改写 core。适配器 `--no-deps -D warnings` 检查单独执行，不能把它写成全图严格检查通过。
- CLI 19 项测试、严格 Clippy 和目录复制后运行验收通过，最后一次原始结果为 `D:/BuildCache/glory-cli-native-acceptance/e1d753309f764221a1b2006f8e1a7756/result.json`。
- 最终固定依赖的 Windows 普通 shell、AccessKit 真窗口和独立 UIA 客户端均通过；保留 `D:/BuildCache/glory-mobile/windows-{shell,accesskit,uia}.log`。Counter release 目录包重新构建成功，日志为 `counter-bundle.log`。
- x86_64 和 aarch64 的原生库均实际链接、打包和 debug 签名验证通过；aarch64 仍只是产物构建，没有真机运行结论。
- 仓库 `start-emulator.ps1` 从全新 AVD 配置启动的 Android 16 复验通过，最终结果为 `D:/BuildCache/glory-mobile/repro-emulator/acceptance-final/result.json`，包含同进程 Activity 重建。初次固定键盘坐标误中工具栏后，脚本改为等待真实键盘画面并按其顶部定位数字键；仍要求软件键点击后实际更新值，没有改用按键注入冒充。

上述新增 gate 可由 `check-semantic-ui.ps1 -Android` 构建两种 ABI；提供 `-AndroidSerial` 才运行所选模拟器。没有请求模拟器验收时，脚本只报告 APK 构建。iOS、真实 Android 设备、移动资产/无障碍和桌面安装器仍按前述清单继续补齐，整体功能差距不标为关闭。
