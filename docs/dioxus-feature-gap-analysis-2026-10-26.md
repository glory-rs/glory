# Glory 与 Dioxus 功能差距及补齐报告

Glory 的主要缺口集中在移动平台交付、桌面分发、应用级导航与文档服务、开发调试工具。响应式内核、浏览器渲染、SSR、水合、流式资源传递和常用交互组件已经有实现；不能把旧架构删除的能力、README 未列出的能力和真实缺口混为一谈。本轮按现有语义架构补齐，SSR 的 HTTP 框架集成只支持 Salvo。

## 当前进度（2026-10-27，移动输入补齐后）

下方“起始状态”和 Windows 会话记录保留为比较基线；当前主机已是 Apple Silicon
Mac，最新证据见文末 Apple/移动输入记录，不能继续把“没有 Mac”当作当前状态。

| 项目 | 当前结果 | 未关闭边界 |
|---|---|---|
| macOS | CPU 真窗口/缩放/清理、实际 AX 读取和编辑、可重定位签名 `.app` | VoiceOver 朗读、Developer ID/公证的凭据验收、外部机器安装 |
| iOS | 模拟器/设备链接、UIScene、CPU 窗口、包资产、真实拼音 IME、Unicode 程序选区、底部键盘避让、同进程恢复/旋转、单窗口场景断开重连、HTTP(S)/原生证书校验与真实连接取消 | 真机网络/权限及签名安装、VoiceOver、任意选择手势/rects、多 Glory 场景及进程终止后的状态持久化 |
| Android | NativeActivity 基线；明确选择的 GameActivity、ARM64 模拟器真实拼音预编辑/提交/取消、Unicode 选区、汉字/emoji 删除、APK AssetManager 读取、HTTP(S)/系统证书校验与真实连接取消 | 真机/16 KiB 内核运行、TalkBack、运行时权限及真机网络、生产分发 |
| 桌面开发 | native watch、错误保留旧进程、成功重启与退出回收 | 状态保持、真正 Rust 热补丁、检查器 |
| Web 发布 | 可复现 gzip、哈希清单、预览协商/HEAD/身份回退 | 指纹命名、Brotli/图片转换、WASM 分包 |

整体差距仍未关闭。签名身份/设备等外部条件和仍需实现的软件功能分别保留，
没有用模拟器或编译结果替代完整平台交付。

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

起始 Windows 会话曾建立隔离在 D: 的 Android 16 x86_64 模拟器，当时没有可用 Mac。
后续 Apple 主机的结果见当前进度与文末记录。Android/iOS 模拟器均不能替代 arm64
真机；其他目标分别记录编译或运行证据，不能因为依赖支持这些 OS 就标为 Glory 已验收。

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

后续需要 Mac 上的构建/安装/启动、UIKit 暂停恢复/尺寸与安全区、软键盘/CJK/VoiceOver、真机签名和 IPA 分发。Android 还缺真机、CJK 组合输入、TalkBack、AssetManager/联网 Loader 和权限等覆盖；嵌入式包资产的后续补齐见下节。桌面缺各系统安装器与 macOS 本机验收。这些均未被本轮勾选完成。

### 检查结果

- Glory 全工作区 `--all-features` 测试通过；native 全工作区 `--all-features --all-targets` 严格 Clippy 通过。
- 三个固定 fork 的独立 checkout 测试通过，包含 Winit core/common 和独立 Activity 生命周期测试；metadata/lock 确认十八个引擎/平台包各只有一份来源身份。
- Android 全图严格 Clippy 的首轮触发 Rust 1.98 对已有 const thread-local 的诊断；未为此改写 core。适配器 `--no-deps -D warnings` 检查单独执行，不能把它写成全图严格检查通过。
- CLI 19 项测试、严格 Clippy 和目录复制后运行验收通过，最后一次原始结果为 `D:/BuildCache/glory-cli-native-acceptance/e1d753309f764221a1b2006f8e1a7756/result.json`。
- 最终固定依赖的 Windows 普通 shell、AccessKit 真窗口和独立 UIA 客户端均通过；保留 `D:/BuildCache/glory-mobile/windows-{shell,accesskit,uia}.log`。Counter release 目录包重新构建成功，日志为 `counter-bundle.log`。
- x86_64 和 aarch64 的原生库均实际链接、打包和 debug 签名验证通过；aarch64 仍只是产物构建，没有真机运行结论。
- 仓库 `start-emulator.ps1` 从全新 AVD 配置启动的 Android 16 复验通过，最终结果为 `D:/BuildCache/glory-mobile/repro-emulator/acceptance-final/result.json`，包含同进程 Activity 重建。初次固定键盘坐标误中工具栏后，脚本改为等待真实键盘画面并按其顶部定位数字键；仍要求软件键点击后实际更新值，没有改用按键注入冒充。

上述新增 gate 可由 `check-semantic-ui.ps1 -Android` 构建两种 ABI；提供 `-AndroidSerial` 才运行所选模拟器。没有请求模拟器验收时，脚本只报告 APK 构建。iOS、真实 Android 设备、AssetManager/网络/无障碍和桌面安装器仍按前述清单继续补齐，整体功能差距不标为关闭。

## 后续补齐：移动端离线 CSS/图片

新增 `glory-blitz/package`（facade `blitz-package`），将已有 Loader 的包字节、限额队列、超时、取消和引擎解码准入抽为 native 共用路径。桌面 `assets` 包含该能力并继续提供原有 HTTP/挂载文件后端；没有复制第二套加载调度器。每个文档独立持有 Loader，关闭后不复用。`asset://app/` 从应用显式提供的不可变字节映射解析资源，默认 runtime、HTML、SSR 和 WASM 不因此引入加载器。

包专用构建拒绝非空 origins/mounts，生产依赖图排除了 reqwest、Rustls 和 cap-std/cap-fs-ext。移动示例用 `include_bytes!` 嵌入 CSS 和 PNG，相对 URL 由配置的 base URL 解析；CSS import 和图片依然经过 `NetHandler::prepare`，遵守引擎请求/解码/完成结果预算。没有 AssetManager、环境路径查找或网络权限的隐式授权。

验证：包专用 6 项测试通过，覆盖独立配置预算、文档归属、异步 decode 准入、队列满拒绝、取消/关闭不迟发，以及实际 CSS 布局和 CPU PNG 像素；原桌面加载器 9 项网络/资源回归通过。native 全工作区 all-features 测试和严格 Clippy、WASM 全工作区严格 Clippy通过。规范检查脚本增加包专用测试和 native/Android 生产依赖图检查。

Android 16 x86_64 实际 APK 通过完整交互、Gboard 数字键、后台恢复、横竖屏和同进程 Activity 重建；每一阶段均检查嵌入式绿色 PNG 的像素及导入 CSS 决定的位置/尺寸。退出文档日志为 completed=2、failed/rejected=0、queued/active=0、closed/stopped=true。证据：`D:/BuildCache/glory-mobile/package-assets-acceptance-retry/result.json`、同目录截图和 application.log。首轮截图保留在 `package-assets-acceptance`，因模拟器启动时 System UI 无响应弹窗遮挡而失败；系统稳定后完整重跑通过，未放宽像素断言。

本轮工作区共 964 项测试通过（含文档测试）；Android aarch64 适配器严格 Clippy（--no-deps）通过，x86_64/arm64 两个 APK 都重新链接、打包并验证签名。指定 -AndroidSerial 的规范 gate 现在会先安装本次 x86_64 APK，再运行像素/交互验收，避免测试残留旧安装包。

这是嵌入式离线资产链路的补齐。Android AssetManager/动态资产、联网及权限、CJK/TalkBack 和 arm64 真机仍待验收；iOS 共用源码但仍没有 Apple SDK 构建/运行结果。SSR 继续仅集成 Salvo。

## Apple 主机后续补齐（2026-10-26）

本节接续上面的 Windows 记录。当前主机为 Apple Silicon Mac，完整 Xcode 27.1
及 iOS 27.0 模拟器运行时均已存在；前文“没有 Mac/Apple SDK”的限制属于前轮，
不再代表当前环境。保留此前证据与尚未完成项，不把模拟器验收合并为真机验收。

### macOS 构建、应用包与系统交互

CLI 与生成的 shell launcher 支持 `/Volumes/Data/Caches/glory` 数据盘缓存隔离，
不再在 Mac 上要求 Windows D$ 挂载或 GNU 工具。路径在 mkdir 前解析并拒绝越界
符号链接；Windows/WSL 原有路径规则保留。Apple 工具通过进程级 DEVELOPER_DIR
选择完整 Xcode，没有修改全局 xcode-select。

`bundle --platform desktop` 在 macOS 目标生成真正 `.app`：可执行文件进入
Contents/MacOS，显式资产进入 Contents/Resources，生成 Info.plist 和可配置
reverse-DNS 包标识。ad-hoc 签名校验后生成包含签名文件的完整哈希清单，再原子发布。
新增测试将签名应用移到含空格目录后执行并重新核对签名、资源和所有清单哈希。
实际 Counter release `.app` 已构建并启动。

AppKit 主线程独立进程验收实际呈现、640×480 尺寸变化、多帧及关闭时 Owner 清理。
外部 macOS AX 客户端实际读取 Heading、Counter value 文本框、Clear/-1/+1 按钮，
执行 +1 得到 1、SetValue 得到 42，真实键盘全选/退格/输入后得到 17。
这建立 macOS AX 树和动作链路证据，不能替代 VoiceOver 朗读或完整文本选择验收。

实际测试还发现 AppKit 文本命令绕过引擎 EventHandler 的缺口。现在 Glory 将
AppleStandardKeybinding 送入同一 DOM 事件分发，补齐 inert/disabled/readonly
默认动作前的保护，以及编辑捕获的删除意图和 UTF-16 选区。真实引擎回归确认
只读控件不能被退格或插入引号修改；测试按 Mac 的真实文本命令协议产生退格。

`scripts/sign-macos.sh` 提供显式 Developer ID 身份与 notarytool keychain profile
的发布步骤：hardened runtime、时间戳、公证 Accepted 校验、staple、Gatekeeper
评估及刷新哈希清单。当前没有应用所有者签名凭据，故只实现并静态检查该链路，
没有声称已完成 Developer ID 签名、公证或外部机器安装。

### iOS 27 场景、CPU 窗口与移动资产

iOS 模拟器和 arm64 真机目标均已实际编译链接。模拟器 `.app` 经签名校验后安装，
初次启动在 UIKit 的 UIScene 强制检查处中止；仅添加场景 manifest 又得到黑屏。
保留失败截图和崩溃记录，未降低 SDK 版本或把链接成功计作运行成功。

应用工程增加 Swift SceneDelegate 和可追踪源变更的 build.rs。场景真实激活/
失活事件转到既有 Winit 生命周期；Shell 在场景连接后创建 Surface，失活释放
CPU Surface，重新激活恢复同一语义 App。WindowApp/Loader 的所有权继续保留。

真正的 [Winit fork 固定提交](https://github.com/glory-rs/winit/commit/eff22893b58d35f31bc65928b365e9863ce1c5ce)
通过安全 UIKit API 在显示窗口前关联已连接 UIWindowScene；传统无场景路径保留。
[Blitz 固定提交](https://github.com/glory-rs/blitz/commit/01e237f5e33b272ecb524cf3a4b1f39ac77f6a03)
同步固定同一 Winit 身份。manifest、全部活动锁文件、来源记录与检查脚本一起更新；
没有修改 Cargo checkout、vendor、全局 patch 或增加 unsafe Rust。

实际 iPhone 17/iOS 27 模拟器通过：Counter/包内 CSS/PNG 显示，触摸 +1，真正的
系统软件键 7 将 1 改为 17，Home/同进程恢复、横屏旋转及后续保留值。
原始截图验证绿色 PNG 是 32×32 point、距安全区边缘 24 point，横屏左右安全区
均为 62 point；恢复与横屏后的标签像素哈希与软件键输入后完全相同。
`capture-ios.py` 保留原始帧、图片位置/尺寸与标签哈希，不伪造触摸或 IME 结论。
CoreSimulator 服务拒绝数据盘写入，因此设备使用其 OS 管理目录；仅截图先在该
自有设备 tmp 中生成，再转存数据盘。编译/Cargo/Swift/temp 缓存全部在数据盘。

设备打包脚本要求显式身份和 provisioning profile，验证有效期/应用标识、派生
entitlements、嵌入 profile，并提供签名 `.app`/Payload IPA/哈希清单。
当前只有 arm64 设备可执行文件链接证据，没有证书签名或真机安装结果。

### 其他缺口：桌面开发 watch/restart

CLI `serve --platform desktop|server` 增加本地依赖文件监控、200ms 去抖、成功构建
后重启。每一代运行独立的可执行文件副本，避免 Windows 正在运行的 EXE 阻塞下轮
链接；构建失败保留原进程，启动失败尝试恢复上一代。Ctrl+C 取消构建并回收自有
进程，关闭后移除临时副本。它是进程重启，不是 Rust 热补丁或状态保持。

真实 Cargo/notify/子进程验收通过：故意写入语法错误后原 PID 仍存活，修复后新
一代产生不同 PID，Ctrl+C 后新 PID 已回收。完整源码和失败/恢复日志保留。

### 本次验证与仍未关闭项

- macOS 全工作区 all-features 单元/集成/文档测试合计 964 项通过；全目标严格 Clippy 通过。
- CLI Apple/watch 阶段 22 项测试通过，后续 Web 压缩补齐后为 25 项；包含签名包重定位、哈希验证和真实 TCP 协商，严格 Clippy 通过；桌面 watch 另有实际端到端结果。
- macOS 真窗口主线程测试和 Counter release 应用包实际运行通过。
- iOS 模拟器应用链接、签名验证、安装、原生交互和资产像素通过；arm64 设备目标实际链接。
- 三个固定引擎 fork 的独立 checkout 测试及 UIKit 模拟器/设备编译检查单独记录。
- 证据总表：`/Volumes/Data/Caches/glory/apple-acceptance-result.json`；原始 iOS 帧与
  JSON 在 `ios-acceptance/`，编译/测试/fork/CLI/watch 日志也在同一数据盘缓存根下。

初次中文拼音触摸未产生组合文本/候选，失败帧 `pinyin-no-composition.png` 保留。
用户随后明确授权仅在 Winit UIKit fork 增加完整 UITextInput 所需的最小 unsafe
协议桥接；Glory 运行时继续禁止新增 unsafe。补齐结果见下节。

### UIKit 中文 IME 后续补齐（用户授权后）

[Winit 最终固定提交](https://github.com/glory-rs/winit/commit/edfaacd7c88feed241d12f7b2bb6b61271e513c4)
实现 UITextInput 所需的 Objective-C selectors，复用一个不进入显示树的 UIKit
文本存储来管理 opaque position/range/tokenizer。新增 unsafe 仅位于该 UIKit
协议 ABI 声明和必要的同类型转发，有主线程、对象生命周期、返回值所有权和弱
delegate 的安全说明；没有裸指针转换或 Glory 运行时 unsafe 例外。

[Blitz 最终固定提交](https://github.com/glory-rs/blitz/commit/54e070d591ec387be40b2a25500376aa796f15ef)
为显式 opt-in 的 UIKit shell 同步当前焦点输入的周边文本：最多 3999 UTF-8 字节，
不截断码点或选区，超过预算的选择改用空上下文，密码始终为空。非 UIKit host
默认不导出上下文。更新按字段合并，离开输入时清理；不扫描其他输入、列表或业务源。
原生 marked text 活跃时不接收旧值确认覆盖；原生 purpose/secure traits 的改变
延后到组合终止，避免 UIKit 销毁正在输入的文本。Native UTF-16 选区转换为经验证
的 UTF-8 preedit 范围，开启/关闭/组合/提交继续进入已有有限 IME 协议。

真实系统拼音键盘通过：`nihao` marked text 与“你好”候选、单次提交得到 `0你好`、
中文提交后退格得到 `0你`、连续候选提交、emoji 的缓冲区插入及整字删除、删除
预编辑恢复原提交值、组合期间点 Clear 后保持 `0`、重新聚焦后上下文无旧值、
中文值的 Home/同进程恢复，以及横竖屏后继续组合并提交为 `0你好`。AX 值和可见
输入框同时核对，原始 PNG/包资产像素记录位于 `ios-ime-acceptance/`。Emoji 缓冲区
与偏移正确不代表彩色 emoji 已正确绘制；此次该 glyph 的绘制未通过。

纯 Rust UTF-16/UTF-8 测试覆盖中文、emoji 代理对、组合附加符、越界/溢出与非法
半代理对。引擎测试验证周边文本限额、选择范围、密码不导出以及 preedit 不被覆盖。
Winit UIKit 严格 Clippy（--no-deps）通过。完整原生选择手势、selection rects 和
超出当前选择的系统自动更正仍不宣称支持；没有用这次 IME 补齐替代完整文本选择
或 VoiceOver 验收。

前轮新增仅应用测试使用的 `ime-probe` 可选视图，沿用当时 ASCII 程序选区契约选择
`ABCDZ` 的 `BCD`。实际系统键盘将其预编辑为 `AniZ`，删除预编辑后恢复
`ABCDZ`、原选区和 Idle；不重新选择即可再次输入并提交为 `A你好Z`。
原值恢复后的标签像素与起始截图相同，原生 AX 文本上下文也回到 `ABCDZ`。
这覆盖了“最终值相等但原生 backing store 已变化”的取消路径：原生 IME
通知现在使上下文确认缓存失效，下一次提交恢复完整文本与选区。记录在
`ios-ime-probe-acceptance/`。任意 Unicode 程序选区没有被偷偷放宽；初次用
Unicode 选区触发的 Unsupported 日志保留，UIKit 的不可返回主循环现在只打印
一次致命错误以便诊断。

普通 Counter 的最终安装包未启用 `ime-probe`；再次实际输入 `0你好` 后，Home/
恢复前后均为 PID 14280，横屏后值继续保留，包资产像素与标签哈希检查通过。
最终截图与报告在 `ios-final-acceptance/`；`ime-probe` 模式另有当前依赖的严格
Clippy 检查。当前钥匙串未发现可用发布签名身份，也没有连接真实 iOS 设备，
这两项仍保留为外部验收条件，没有使用 ad-hoc 模拟器签名代替。

### 继续补齐：Web release gzip 与内容协商

独立 CLI 在 Web release staging 内流式生成 gzip sidecar，跳过 256 字节以下及
已压缩媒体，只有小于原文件时才保留。固定 gzip 时间戳为零、不嵌入文件名，
相同输入与固定压缩器得到相同字节；显式应用 `.gz` 资产冲突会拒绝构建并保留
上一次发布结果。原 URL 和原文件不改名，压缩产物在原子发布前进入完整哈希清单。
压缩依赖仅进入独立 CLI/项目私有工具，没有进入十二包运行时图。

本地预览根据 `Accept-Encoding` 的质量值、显式禁用和 wildcard 选择 gzip/identity，
发送原 MIME、Content-Encoding、Vary 和对应 Content-Length；HEAD 不发送内容。
两种表示均不可接受时返回 406。watch/reload 使用 identity 后插入开发脚本，
避免把脚本插进压缩字节。生产服务/CDN 仍需应用配置同等协商，不能仅凭 sidecar
存在就宣称线上压缩已开启。

真实 Counter release 产物：WASM 2,107,005 → 570,130 字节，JS 59,230 → 9,808 字节。
全部清单哈希和 gzip 解压字节逐一核对；压缩版浏览器页面实际启动并点击 +1
得到 Value: 1。25 项 CLI 测试与严格 Clippy 通过，含真实 TCP gzip/HEAD、禁用
编码、identity 回退、406 和 reload 断言。记录：
`/Volumes/Data/Caches/glory/web-compression-result.json`、`web-compression-build.log`、
`web-compression-browser.png`。指纹命名、Brotli、图片转换和 WASM 分包尚未实现。


还未关闭：iOS VoiceOver、scene disconnect/reconnect、真机权限/
网络/签名安装/App Store 资产；macOS VoiceOver/生产公证及其他系统安装器；应用级
history/嵌套路由/返回恢复、动态文档头、Salvo 类型化调用、发布资产优化、WASM 分包、
GPU 和真正热补丁等。整体差距保持开放，不以本次 Apple 纵向链路或桌面 watch
替代这些实现和验收。

## 2026-10-27：Unicode 选区、键盘视口和 Android 中文输入

### 共用程序选区

移除起始版本的 ASCII 限制。有限协议仍以 UTF-16 code unit 定址，对当前受控值
进行无分配的 scalar 边界转换；半代理对、倒序范围和越界在队列变更前拒绝。
Blitz 在修改编辑器前检查实际 shaped cluster 边界，不能精确表达的组合字符/ZWJ
内部位置返回 Unsupported，保留旧选区，不静默舍入。选区方向和输入 revision
仍有独立验证，不能把过期的选区应用到后来的值。

Web CSR 与真实 SSR adoption 使用同一 Rust 输入表单：文本和密码的 `A中😀Z`
均可选择 UTF-16 1..4 的 `中😀`，正反方向保持；3..3 的半代理对拒绝后选区不变，
后续合法选择可继续，水合保留输入对象。夹具和可重复脚本分别是
`examples/semantic-web/unicode-selection.html`、`unicode-selection-acceptance.js`。
浏览器记录在 `/Volumes/Data/Caches/glory/unicode-selection-web/`。iOS 真实拼音
也完成同一非空 Unicode 选区的预编辑、取消恢复和再次候选替换，原始截图与 JSON
在 `/Volumes/Data/Caches/glory/unicode-selection-ios/`。此处补齐 supersedes
前轮 ASCII 探针的能力边界，前轮失败和验收记录继续保留。

### iOS 底部键盘避让

应用拥有的 Swift scene delegate 只观察本场景 key window 的本地 docked keyboard，
将底部遮挡换算为 root controller 的额外 safe-area inset，复用 Winit 布局事件刷新
内容视口。浮动键盘不缩小整页；无效/极端矩形拒绝或裁剪，至少保留一个 point。
监听器在 disconnect/deinit 解除，闭包弱引用 scene delegate。没有向 Glory
运行时加入 unsafe、平台键盘尺寸或另一套模型。

最终复验暴露并修复了真实触摸时序问题：按下底部输入框会打开键盘并使控件上移，
手指在旧位置抬起时，Blitz 原先合成空白 Click、立即使编辑器失焦。现在仅对
“静止手指、原先按下并聚焦的文本编辑器已移出释放目标”抑制该错误点击；后续
主动点击空白仍正常失焦。fork 回归测试使用真实布局/视口缩小，在关闭修复时
明确失败、开启后通过；普通引擎 117 项测试通过。

`keyboard-probe` 与 `ime-probe` 分开选择。前者将输入框固定在视口底部，检验
实际系统键盘显示/收起、输入框随可用高度移动和输入继续可用；十二项 Swift
几何测试已纳入 Apple 规范 gate。最终无诊断构建还通过竖屏键盘显示、
带键盘旋转和旋转后继续提交中文。横屏输入框中心实测上移 204pt，收起后复位。记录在 `/Volumes/Data/Caches/glory/keyboard-ios-final/`。
这不代表任意遮挡区域自动滚动、完整 scene 重连或真机验收。

### Android GameActivity 与真实拼音

保留默认 NativeActivity 路径，新增显式的互斥 Activity 选择。`android-game-activity`
及 facade 对应 feature 使用同版本 GameActivity 4.4.0 Java AAR/Rust glue。
应用 Java 子类只加载库；实际 UI 仍由 Rust → Blitz/Stylo/Taffy/Parley →
AnyRender/Vello CPU → 原生 surface 绘制，没有 WebView/HTML 页面套壳。
引擎内部复用 DOM/CSS 数据结构不等于在浏览器中运行。

Winit genuine fork 用安全 Rust 将 GameTextInput 的 UTF-16 state 转成经过校验的
UTF-8 Preedit/Commit/Cancel/DeleteSurrounding，提供受限的 focused-editor context，
密码不导出周边文本，输入通知先限制大小再复制。拒绝无法表达的原生选区移动和
超出当前选区的自动更正；Android/JVM 调用在 session mutex 释放后执行。Glory
运行时没有新增 unsafe，既有 UIKit 窄桥接例外没有扩大到 Android。

用户同意许可后，NDK r27d、Android 36 ARM64 Google Play 镜像和 Gradle 9.1.0
安装在数据盘隔离缓存，下载核对官方摘要。Mac GameActivity 打包脚本验证 JNI
入口、AArch64、16 KiB ELF LOAD 对齐和 APK 签名；Gradle lock 与 SHA-256 依赖
验证元数据已提交到源码。普通构建严格验证，不临时放宽校验。最终 APK 使用
调试签名，不作为商店发行包。

实际 Android 36 ARM64 模拟器、Gboard 简体拼音（开启 Inline composing）、CUA
原生点击/物理键验收：`A中😀Z` 选中 1..4 → `AniZ`/Composing → 删空预编辑恢复
原值、Idle 与原生 1..4 选区 → 无需重选再次输入并选择 `你好` 得到 `A你好Z`。
随后真实退格得到 `A你Z`，物理 `x` 只产生一次预编辑，取消后恢复；末尾退格从
`A中😀Z` → `A中😀` → `A中`，未残留半代理对。Home/恢复及旋转保留 PID 4244
和值/包资产。软件键验收没有用 `adb input` 注入替代。记录在
`/Volumes/Data/Caches/glory/android/unicode-ime/result.json`、同目录原始 PNG 和日志。
接入最终触摸修复后的 APK 另以 PID 4592 复验预编辑/取消恢复/再次提交通过。

该 AVD 的内核页大小是 4096，不能把 ELF 对齐检查写成 16 KiB 运行通过；ARM64
模拟器也不是真实 ARM64 设备。ADB 37.0.1 在本机启动出现 libunwind 崩溃，隔离
使用官方 ADB 36.0.2 后可运行；没有据此宣称上游通用故障。模拟器只修改测试
所需键盘设置，镜像及 Gradle/JDK 写目录位于数据盘缓存。

### 验证与下一批缺口

本轮全工作区含文档测试共 967 项通过，native/WASM 全工作区严格 Clippy 通过；
三个固定 SHA fork 的独立测试、UIKit 两个 target 及 Android Native/Game 两条
编译路径通过。Apple 模拟器和 Android Game APK 均实际重新链接、打包、签名验证。
这些检查与真实输入验收分别记录，不能互相替代。

继续优先补软件可完成的 iOS scene disconnect/reconnect、移动资产/联网与权限接口，
随后处理应用级 history/嵌套路由/返回恢复、动态文档头、Salvo 类型化调用与开发
检查器。VoiceOver/TalkBack、签名真机安装、16 KiB 设备、商店分发、GPU、真正热补丁
和完整原生选择手势仍开放。整体差距没有关闭。

## 2026-10-27：iOS 单窗口场景重连

应用自有 Swift 宿主现在在 scene disconnect 时分离 Winit 持有的原窗口，暂停
CPU surface，清除键盘 inset；新 scene 连接时重新挂接同一窗口并恢复尺寸。
仅当前 scene 可以驱动生命周期，重复及迟到回调不会暂停后继 scene。原生焦点以
弱引用保存，激活后恢复仍存活的 responder；不保留旧 scene，不创建第二个 Rust
event loop、文档或运行时，也没有增加 unsafe。

独立编译的 simulator scene-probe 使用不同 bundle ID，通过真正的 UIKit
requestSceneSessionDestruction/Activation 验收；普通包不包含测试按钮或多场景配置。
一个纯 UIKit anchor 保持进程存活，不持有 Rust 文档。iPad Air M4/iOS 27 实测两次
断开重连均保持 PID 12711 和原窗口标识，文本 17 恢复后软件键继续编辑为 177；
再次断开期间旋转，新场景恢复 177、原生键盘和 safe-area inset。原始截图/JSON
位于 `/Volumes/Data/Caches/glory/scene-final-acceptance/`，日志为
`scene-clean-final-console.log`。两组 label 像素相等，CSS/PNG 位置与方向一致。
Swift 回归覆盖重复/过期回调和 1,000 轮状态转换；普通模拟器包及 arm64 device
可执行文件均已实际链接。设备链接仍不等于签名真机运行。

失败证据继续保留：iPhone 模拟器拒绝多场景销毁；销毁最后 scene 可能终止进程；
一次运行遇到系统 backboardd respawn，没有把该运行计入成功。当前能力限于同进程
单 Glory 窗口重连，不声称进程死亡持久化或多个独立 Glory scene。后续继续补
Android AssetManager、移动联网/权限及原生控件，读屏和正式分发仍未关闭。

## 2026-10-27：Android APK AssetManager

新增独立可选 `glory-blitz/android-assets`（facade `blitz-android-assets`），由应用
显式传入当前 AndroidApp 的 application AssetManager 和 APK 前缀。`asset://android/`
映射前缀内路径；`asset://app/` 内存包保持独立。URL 分段单次 UTF-8 解码，拒绝
分隔符、控制字符、空/dot 段；前缀单独拼接，不会被 URL 的父目录归一化越过。
无全局 AndroidApp、环境文件权限或新 unsafe；网络和权限仍是独立缺口。

挂载文件与 APK 共用有界 blocking admission/分块读取。异步取消不能提前归还仍
运行的 OS 读取许可；长度和实际字节预算、deadline、逐块取消、prepare/解码准入
及关闭撤权继续有效。`file_active`/`file_peak_active` 包含 APK 读取，不声称 OS 调用
可中断，也不把 Loader stopped 等同于所有阻塞调用已退出。

GameActivity 工程把原 CSS/PNG 打包到 `assets/glory/`；应用启用该功能时清空嵌入式
包，强制 CSS 和图片走 AssetManager。ARM64 Android 16 模拟器实际读到 deflate CSS、
中文路径文件和 PNG，missing/非法 URL/body limit/prepare/close 探针通过。独立只读
脚本核对已安装 APK 的 SHA-256 与本次 build.json 相同，再校验进程日志、绿色 PNG
和 CSS 尺寸/位置；不使用按键注入。证据为
`/Volumes/Data/Caches/glory/android/asset-manager-acceptance/probe.json`、同名 PNG、
probe-logcat.txt 和保留的 probe.apk。小数 DPI 的 logical edge rounding 与抗锯齿
边缘单独处理；没有把看见任何绿色当作样式定位通过。

当前全工作区含文档测试 970 项通过，Android 应用/适配器严格 Clippy 和
实际 ARM64 APK 链接/签名通过。新增通用回归覆盖增长文件的实际字节限制、提前
拒绝、分块取消和 APK 路径；原 200 个取消等待者不扩张 blocking worker 的回归
继续通过。网络/权限、真机/16 KiB runtime、TalkBack 和生产签名分发仍未关闭。

## 2026-10-27：移动 HTTP(S) 资产与平台 TLS

新增可选 `glory-blitz/network`（facade `blitz-network`），将原桌面 HTTP worker
提取为 Windows/Linux/macOS/Android/iOS 共用传输。桌面 assets 包含 network 后再
加入 mounted files；网络单独启用不引入 cap-std，包专用图仍不含 HTTP/TLS。
原始及每跳 redirect origin、HTTPS 降级拒绝、body/queue/deadline、prepare 和取消
契约保持。空 origins 不创建 HTTP client；未显式允许的 origin 不产生网络请求。
原生 reqwest/Rustls socket 不使用环境代理或自动 retry/redirect/decompression；
明文 HTTP 需应用显式允许 origin，HTTPS 证书链及 hostname 交平台校验。

Android 新增 AndroidTls 能力，构造时只接受显式 Application JNI 对象，拒绝 null/
非 Application 和缺少 Java verifier class 的配置。版本匹配由应用构建校验。Loader 的非空 origins 必须携带
该能力，不能依赖别的 Loader 已初始化的全局状态。示例从 android-activity 已准备
的安全 JVM/class-loader API 获取应用桥接对象；Java 仅保存 Application，未保存
Activity。没有新 unsafe、原始指针转换或全局 AndroidApp。iOS 使用 Security.framework。

GameActivity 构建从 Cargo metadata 取得 verifier-android 版本，匹配 AAR/POM
SHA-256 及独立 gradle-network.lockfile；Maven archive 固定 upstream Git SHA。
原始 raw GitHub 下载失败后使用同一固定提交的干净本地 archive 镜像，未替换依赖
或跳过校验。生成的 manifest 仅为网络构建添加普通 INTERNET 权限；运行时权限
弹窗服务仍未实现，不能把 manifest 权限当作该缺口完成。

Android ARM64 与 iOS 27 模拟器的实际原生请求均通过：可信公开 HTTPS、自签证书
拒绝、同源跳转、跨 origin/循环拒绝、声明长度及 chunked body 限额、编码拒绝；
Counter 清空内存包后通过 HTTP 取得 CSS/PNG 并核对实际像素。独立后台探针等待
loopback server 确认 slow request 后再取消/关闭，服务端均观察到连接 EOF/reset；
deadline 同样关闭实际 socket，未发生迟到 prepare/publication。证据位于
`/Volumes/Data/Caches/glory/mobile-network/`：两端日志、截图/JSON、server.jsonl 和
result.json。初次 Android 250ms 在连接建立前已过期，观察失败记录保留；使用
三秒预算后重新通过实际连接的到期关闭检查，没有用“无回调”冒充 TCP 取消。

全工作区含文档测试仍为 970 项，native/WASM 严格 Clippy 通过；network-only 的
HTTP 测试单独运行，package-only 和 desktop assets 回归通过。iOS 模拟器包与 arm64
device 可执行文件实际链接，Android 网络 APK 链接/签名及锁定依赖构建通过。
这关闭的是移动资产 Loader 的 HTTP(S)/平台 TLS/取消实现缺口；运行时权限、
真机网络/离线及无线环境、TalkBack/VoiceOver、原生控件和正式分发继续保留。
