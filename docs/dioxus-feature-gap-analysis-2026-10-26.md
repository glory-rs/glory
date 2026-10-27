# Glory 与 Dioxus 功能差距及补齐报告

Glory 的主要缺口集中在移动平台交付、桌面分发、应用级导航与文档服务、开发调试工具。响应式内核、浏览器渲染、SSR、水合、流式资源传递和常用交互组件已经有实现；不能把旧架构删除的能力、README 未列出的能力和真实缺口混为一谈。本轮按现有语义架构补齐，SSR 的 HTTP 框架集成只支持 Salvo。

## 当前进度（2026-10-27，移动服务与原生控件补齐中）

下方“起始状态”和 Windows 会话记录保留为比较基线；当前主机已是 Apple Silicon
Mac，最新证据见文末 Apple/移动输入记录，不能继续把“没有 Mac”当作当前状态。

| 项目 | 当前结果 | 未关闭边界 |
|---|---|---|
| macOS | CPU 真窗口/缩放/清理、实际 AX 读取和编辑、可重定位签名 `.app` | VoiceOver 朗读、Developer ID/公证的凭据验收、外部机器安装 |
| iOS | 模拟器/设备链接、UIScene、CPU 窗口、包资产、真实拼音 IME、Unicode 程序选区、底部键盘避让、同进程恢复/旋转、单窗口场景断开重连、HTTP(S)/原生证书校验与真实连接取消 | 真机网络/权限及签名安装、VoiceOver、任意选择手势/rects、多 Glory 场景及进程终止后的状态持久化 |
| Android | NativeActivity 基线；明确选择的 GameActivity、ARM64 模拟器真实拼音预编辑/提交/取消、Unicode 选区、汉字/emoji 删除、APK AssetManager 读取、HTTP(S)/系统证书校验与真实连接取消 | 真机/16 KiB 内核运行、TalkBack、运行时权限及真机网络、生产分发 |
| 桌面开发 | native watch、错误保留旧进程、成功重启与退出回收 | 状态保持、真正 Rust 热补丁、检查器 |
| Web 发布 | 可复现 gzip、哈希清单、预览协商/HEAD/身份回退 | 指纹命名、Brotli/图片转换、WASM 分包 |
| 运行时权限 | 有界 Owner future/driver API；Android 安全 JNI；iOS Swift 宿主；SDK 控制的授权状态查询 | iOS Rust 应用连接待最小 FFI 声明授权；真实系统弹窗选择、真机及其他权限类别 |
| 原生剪贴板 | 显式可选桌面 worker 与 Android 安全 JNI；CopyText 与原生编辑器复制/剪切/粘贴；实际 Sapid 四主题消费端、macOS 系统往返和 Android ARM64 前台系统读写/ClipData 恢复 | iOS 系统 driver、Windows/Linux 实际 OS 往返、移动系统菜单及真实键盘/读屏 |
| 原生文件服务 | 显式可选 driver、桌面系统选择器、Winit 文件拖放通道、Android 安全 JNI/ContentResolver/PFD、稳定句柄分块读取及私有 PNG/JPEG 预览；桌面实际文件/消费端与 Android 自有源 SDK 链路通过 | 实际 OS 选择器/拖动输入验收、iOS driver、Android 非 seekable provider、macOS sandbox security scope 与平台读屏 |
| 原生控件 | Checkbox/Switch/Mixed 和 Radio 的绘制、受控更新、点击/Space/AX 动作；Radio 分组与键盘导航；placeholder 独立提示布局与绘制；combobox/typeahead 协议及 listbox/tree 弹层；原生 Select 单选/多选/分组、选择属性与 AX；五项实际选择控件消费端通过；菜单/菜单栏/右键菜单/Palette 协议、八项实际消费端与浏览器回归通过；TagsInput/PillsInput 的原生输入与 provider 粘贴协议、五项实际消费端通过；两端模拟器初始显示 | 富文本、系统文件服务、iOS 剪贴板/移动系统菜单、其他弹层及系统菜单服务仍开放；实际输入/读屏单独验收 |

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

## 2026-10-27：有界权限服务与原生宿主（部分完成）

新增可选 permissions/facade blitz-permissions，提供六类权限的显式 driver、query/
request future、状态和错误。默认最多八个 pending future、120 秒 deadline；只对
存活等待者登记 timer。构造 future 不请求 OS，首次 poll 才准入。准入在 foreign
driver 调用前保留，防止重入绕过预算；native poll 内关闭会撤销未交付结果。
Owner 退休直接丢弃 future，取消回执，不保留 Owner 或恢复已关闭服务。

Android 的应用自有 Java host 弱持有 Activity，合并主线程消息并限制每轮工作，
native prompt 同时最多一个。取消 reply record 后，实际 OS prompt 仍保留许可到
回调；Activity 销毁关闭 host，过期/外来 request code 和 record 身份不能发布结果。
非权限构建不声明敏感权限或初始化 coordinator；Android 29+ PhotosAdd 表示 scoped
MediaStore 添加自己的媒体，不是任意文件写权限。rationale 返回原始 OS 值，不凭
false 推断永久拒绝。见 [Android 官方请求流程](https://developer.android.com/training/permissions/requesting)。

iOS 原生实现位于应用 Swift 宿主：AVFoundation、Photos、CoreLocation、通知；
校验用途描述，区分 Limited/Restricted/Provisional/Ephemeral，弱 client 防迟到回执，
单 prompt 与 32 个不可中断查询独立计数。Swift ledger 实测取消不提前归还许可、
旧回调不覆盖复用 ID、32 query slot 和 1,000 次 client 退休。Apple SDK 模拟器和
arm64 device 都实际链接。Rust 应用连接仍等待一处仅含整数的最小 FFI 声明授权；
尚未新增该声明或其他 unsafe。不能把 Swift 宿主等同 Rust iOS 权限全链路完成。

ARM64 Android SDK 状态测试验证初始 Denied、照片部分选择/近似定位 Limited、全部
Granted 及恢复；Swift iOS query probe 验证 SDK grant/revoke/reset 的真实 native
状态。证据为 `/Volumes/Data/Caches/glory/permissions/{android,ios}-query-result.json`
与各阶段日志/截图，测试后已恢复权限。它们不证明真实弹窗选择；相机/通知等
未由 iOS SDK 调整的类别也没有伪称完成。UI 自动化上次因 Mac 锁屏中断，尚未确认恢复。

八项新 Rust 回归覆盖 Owner 取消、重入准入、poll 内关闭、deadline、200 个预算拒绝
及恢复、scalar 校验和 ID 耗尽。当前全工作区 978 项、native/WASM 严格 Clippy 通过。
权限缺口继续开放，直到 Rust iOS 连接及真实授权弹窗/设备流程有对应证据。

复核补上 Android 照片状态的边界：完整图片/视频权限均 Granted 时，不要求同时
授予 selected-only 权限；只授予一种媒体或仅所选媒体仍为 Limited。八种组合的
独立 JVM 回归已接入 GameActivity 构建，ARM64 模拟器实际验证 full-without-selected、
image-only、selected-only、none，结束后恢复原授权状态。证据为
`permissions/android-photo-fix-result.json`。普通 Android/iOS 包重新构建，分别确认
无敏感权限且 coordinator 关闭、无权限用途描述及桥接导出；功能不会隐式进入普通包。

## 2026-10-27：原生三态 checkbox

真正的 [Blitz fork 251e4e28](https://github.com/glory-rs/blitz/commit/251e4e288b287c77c59538425b09f25824c35821)
为 checkbox 增加独立的 indeterminate property、CPU 横杠绘制及 `:indeterminate`
样式失效。checked 的程序更新不会清除 mixed；用户激活会清除 mixed 并翻转一次
checked。类型离开 checkbox 时清除该状态，不增加历史记录、全局表或 unsafe。
Glory 不再拒绝 CheckMode::Mixed，保留控制节点身份、模式 Clear 和已接受查询水位；
键盘 Space、标签与 AccessKit 激活复用正确的状态变化，禁用/只读控件不改变值。
AX 树读取实际 native mixed/boolean 状态，不仅根据声明输出 Mixed。

旧实现下两项新适配器回归明确因 Unsupported 失败，日志为
`permissions/mixed-adapter-before.log`；修复后通过。新回归还覆盖 checked/mixed 独立性、
模式切换/清除、1×/2× CPU 像素、原生 Click/标签/Space、只读/禁用和 AX 动作。
全工作区 981 项、native/WASM 严格 Clippy 通过；三个固定 fork 的干净独立检出测试和
来源检查通过，Blitz 默认/并行各 118 项及两项真实 CPU 绘制测试通过。Fork 单独严格
Clippy 仍有原有 needless_return/result_large_err 诊断；仅命令行排除这两项后通过，
没有给运行时代码新增 lint 例外，也没有把它写成 fork 全图严格 Clippy 通过。

共享 `controls-probe` 构建在 Android ARM64 和 iOS 模拟器实际启动，原始截图可见
空框、勾选和横杠三种状态，证据为 `/Volumes/Data/Caches/glory/native-checks/result.json`
及同目录 PNG。普通两端包另已构建，arm64 iOS device 可执行文件按部署目标 15.0
实际链接；这些分别是模拟器初始显示和设备链接证据，不替代真实触摸/读屏/签名设备。
UI 自动化尚未从上次锁屏中断恢复；未用 SDK 注入点击代替。radio、select、placeholder、
菜单、文件/剪贴板、富文本及应用服务等其他缺口继续保留。

## 2026-10-27：原生 radio 与活跃分组

真正的 [Blitz fork eb375a02](https://github.com/glory-rs/blitz/commit/eb375a029330b6179d5276a41dda5643b6077a25)
提供独立的 checked property 和当前 radio group 查询；Glory 的 Radio preflight/direct
apply 不再拒绝。相同非空 name、同一物理 tree/form owner 的 radio 互斥，空名独立；
同名 checkbox、其他表单及断开的树不被误改。关联只从当前宿主树解析，没有存活组
以外的历史表，也不遍历业务/虚拟数据源。checked 程序写入不再依赖 HTML 默认属性。

Click、标签、Space、AccessKit 激活只向新选中的目标发布一次 true；旧项隐式取消
不发布 false，重复选中不产生值变化。受控组件可以通过相等 true property 屏障恢复
被拒绝的提案。name/type/form 变化、重排、挂起/重连按当前关联重新协调；批量插入
保留最后一个初始 checked 项，退休的引擎代次不能选中后继控件。分组依据见
[HTML radio 定义](https://html.spec.whatwg.org/multipage/input.html#radio-button-state-(type=radio))。

无显式 Navigate listener 时，原生方向键在可编辑项中循环，Home/End 到端点；隐藏、
禁用、只读和不在当前焦点/模态范围的目标不被选择。Auto focus 只有一个可用 Tab stop，
从只读项仍能导航到可编辑项。显式 Navigate 和显式 tab-stop 策略继续由组件控制，
没有另建组件状态机。键盘模式参考 [APG Radio Group](https://www.w3.org/WAI/ARIA/apg/patterns/radio/)。

新增七项 Glory 回归覆盖单向变化、受控恢复、动态关联、原生键盘/Tab、显式导航、
Owner/节点挂起退休、实际 CPU 像素及 AccessKit 两个 peer 的当前状态和稳定身份。
全工作区含文档测试 988 项通过，最终 radio 聚焦七项和 native/WASM 严格 Clippy
通过；普通 Tab 保持惰性，只在遇到 radio 时解析该组，并仅在本次 Tab 搜索期间
暂存数字身份，未新增持久组缓存或全量控件 eligibility 扫描。
Blitz 默认/并行库各 121 项通过，三个固定 fork 的干净检出和来源 gate 通过；fork
Clippy 仍只在命令行排除前节记录的两类既有诊断，不新增代码例外。普通移动包和
radio-probe 两种构建均通过；两端模拟器初始选中圆点/未选中圆环的实际截图和 APK
安装哈希证据保留在 `/Volumes/Data/Caches/glory/native-checks/radio/`。
最终记录同时核对已安装 iOS 可执行文件与本次包的哈希，保存运行时代码文件摘要；
arm64 iOS device 也按部署目标 15.0 链接，仍不代表签名设备运行。

这是有限原生 radio 协议的实现与模拟器初始显示；原生输入回归通过框架事件驱动，
不冒充真实触摸/物理键盘或读屏。移动无障碍桥、typeahead、select、placeholder、菜单、
文件/剪贴板、富文本和应用服务仍待补齐。整体缺口继续开放。


## 2026-10-27：原生 input placeholder

真正的 [Blitz fork 0811cfd9](https://github.com/glory-rs/blitz/commit/0811cfd9e7ca3ddcc126ad7ef42f453dfba5be8d)
为单行、多行及空密码输入框提供独立提示布局，沿用 4096 UTF-8 字节的有限协议。
提示仅在空值且没有预编辑时塑形；填值/组合时释放提示布局，清除属性释放提示。
动态 Set/Keep/Clear 不改真实值、revision、选区、编辑器身份或 IME；密码提示可见
不意味着密码正文可见。默认灰色提示继承字体/方向/对齐，单行不换行、多行保留
换行并按内容宽度折行，绘制及下划线均裁剪到内容框，不沿用长值的旧滚动偏移。

`:placeholder-shown` 使用当前编辑器状态及属性存在性，含显式空提示；样式 snapshot
能刷新关联 sibling 选择器。属性选择器改变字体时仍走 Stylo 正常差异失效；纯提示
更新不强制重建真实编辑器。仅本地节点同步，无全量输入/业务源扫描或新 unsafe。
AccessKit 提供独立 placeholder metadata，不替代名称或值，密码原值仍不导出。
语义边界参考 [HTML placeholder](https://html.spec.whatwg.org/multipage/input.html#attr-input-placeholder)；
任意 `::placeholder` CSS 伪元素样式不在当前有限提示策略内。

全工作区含文档测试 991 项通过，native/WASM 全工作区严格 Clippy 通过。
新增适配器回归验证 admission/direct apply 的接受前缀和查询水位、Set/Keep/Clear
不改值 property、当前编辑器选区/预编辑保持、真实 CPU 绘制/受控清空以及 AX 独立
提示与密码保护。固定 fork 的干净检出与来源检查通过：Blitz 精简配置 125 项、
并行/writing-mode 126 项；新增提示 CPU 测试与既有三态/顶层绘制分别通过。
Fork 本身 Clippy 仍仅在命令行排除既有 result_large_err/needless_return 后通过，
没有新增源码 lint 例外，也不把它写成全 fork 无例外严格通过。

Android ARM64 APK 与 iOS 模拟器 `placeholder-probe` 包均实际安装启动；两端原始帧
可见单行、空密码和双行提示，填值参考框隐藏提示。SDK 启动/截图没有注入触摸或
按键。安装 APK/iOS 可执行文件与本次 probe 产物 SHA-256 相同，probe 包和源码摘要
留存在 `/Volumes/Data/Caches/glory/native-input-placeholders/result.json` 及同目录。
Android 首帧是系统启动画面，保留为 android-initial.png；后续实际绘制帧为
android-rendered.png，没有把首帧计为成功。iOS arm64 device 按部署目标 15.0 链接；
普通 Counter 两端包另行构建，移动 probe 的 iOS target Clippy（--no-deps）通过。

这是有限原生 placeholder 实现及模拟器初始显示；真实触摸、软件键盘/读屏与签名
设备验收仍分别保留。select、combobox、typeahead、菜单、文件/剪贴板、富文本、
移动无障碍及应用服务等缺口继续开放。


## 2026-10-27：组合框、typeahead 与选择弹层连接（仍有 Select 依赖）

原生适配器已连接共享四种 ComboboxKind、Choice 命令、Unicode scalar typeahead 和
mouse/pen Pointer/PointerWithin。组件继续拥有匹配、活动游标与受控选择；宿主不另建
搜索/选择控制器。仅当前焦点控件或明确 navigation item/boundary 参与；编辑器、
预编辑、修饰键与最近的独立边界保留原输入。多个按键在 metadata 刷新前排队时，
Select 类型仍交同一个 Next/Previous/Commit 命令，由组件读取当前源处理。

Listbox 顶层保留输入焦点，鼠标/笔移动不选择值，点击选项不会把焦点送到随后关闭的
按钮。Tab 跳过选项，Escape 交组件关闭。Tree 顶层采用已有管理焦点和返回策略，实际
TreeItem 获焦、Escape 返回触发器。RevealOption 仅滚动最近匹配的活跃 listbox，不
滚动外部文档。macOS 空 Multiple 输入的 deleteBackward: 只保留一个待配对节点，由随后的
真实按键按修饰键决定 RemoveLast；删除最后字符不会顺带删除选中项，密码 Backspace
也不会在原始按键中重复删除。其他输入/焦点退休清除待配对节点。所有这些复用现有引擎，无新 fork 或 unsafe。

AccessKit 区分 ComboBox/EditableComboBox、multiple listbox 和当前 active descendant。
隐藏、禁用及退休的活动选项关系被撤销；密码组合框仍使用受保护 PasswordInput 角色，
原值不进入 AX 缓存。完整读屏及移动无障碍桥继续独立验收。

全工作区含文档测试 1002 项通过，新增九项原生事件/CPU 绘制回归及两项 AX 回归；
native/WASM 全工作区严格 Clippy 通过，headless 无默认 feature 构建通过。
Android ARM64 与 iOS 模拟器 choice-probe 包均构建、安装启动；初始画面可见真实
顶层 listbox、禁用项和中文选项。安装 APK/iOS 可执行文件哈希及原始帧位于
`/Volumes/Data/Caches/glory/native-choices/result.json`；这些是 SDK 启动/显示证据，
没有用注入触摸/按键替代真实输入。普通两端包另行构建；iOS device 目标按 15.0
部署版本实际链接，移动 probe 的 iOS Clippy（--no-deps）通过。

实际 Sapid 消费端发现仍须完成原生
Props::Select：Select、Combobox、MultiSelect 的隐藏表单代理会被当前有限拒绝挡住。
保留失败记录 `/Volumes/Data/Caches/glory/choice-consumer-tests.log`；没有仅对 hidden
代理放行。Autocomplete 和 TreeSelect/Cascader 的四主题挂载、命令及 CPU 绘制通过，
不把这两项通过写成五类消费端全部通过。完整后续要求见 [native-choice.md](native-choice.md)。


## 2026-10-27：原生 Select 与实际消费端解阻

原生适配器现已支持 `Props::Select` 的可见单选折叠显示/弹层、多选及 rows 列表框，
包括中文标签、placeholder、分组、禁用选项/分组、真实 CPU 布局/字体/滚动和命中。
复用共享的 1024 选项、32 分组、128 KiB 声明上限；选中键独立于业务表单值。
私有物理选项只属于当前控件，不建立语义别名、全局选项注册表或业务源扫描。
选择补丁保留声明 Rc 和物理身份；等值显式回写能拒绝原生提议，Keep 不写属性。
程序修改不发用户事件，用户修改携带当前 revision 和声明顺序的唯一键。

键盘支持方向/Home/End/Page、Enter/Space、Unicode typeahead、多选切换/范围/
Control 或 Meta 游标移动及全选。共享契约仍明确拒绝 readonly Select。单选弹层
游标在提交前不改变值，Escape 关闭后继续交父对话框处理后续 Escape。Tab、外部
指针/失焦、禁用、挂起和销毁回收当前弹层；恢复保留选项身份。复用引擎触摸拖动、
滚轮和边界滚动，拖动后的抬起不误选。弹层跨出父模态物理矩形时仍遵守原语义边界，
只对当前弹层刷新视口位置。AccessKit 导出分组/选项、选中、多选、禁用、当前活动项
和经存活/可用性校验的动作；其值使用标签，不泄露业务表单值。

原先失败的 Sapid Select、Combobox、MultiSelect 已全部通过，完整五项消费者的
四主题挂载、选择命令及 CPU 绘制均通过；保留旧失败日志，不改成只允许隐藏代理。
新增原生事件、生命周期、触摸/滚轮、模态边界、1x/2x CPU、AX、Plan 和前缀回归。
两端 `select-probe` 展示单选、rows 单选和多选，已按 SDK 安装启动并核对已安装
APK/iOS 二进制哈希及初始画面。证据集中在
`/Volumes/Data/Caches/glory/native-select`，消费端当前日志为
`/Volumes/Data/Caches/glory/select-consumer-tests.log`。

最终全工作区含文档测试 **1015 项**通过，native/WASM 全工作区严格 Clippy 通过，
headless 无默认 feature 检查通过。普通 Android/iOS 包另行构建；iOS device 目标
按 15.0 部署版本实际链接，iOS probe 应用 Clippy（--no-deps）通过。引擎 fork
及依赖固定 SHA 未改变，当前 provenance 检查通过。源文件、测试、日志和帧摘要
记录在 `/Volumes/Data/Caches/glory/native-select/result.json`。

这些是原生协议/CPU 与 SDK 初始显示证据；完整系统输入、VoiceOver/TalkBack 和
签名真机安装另行验收。菜单、TagsInput、富文本、文件/剪贴板、移动权限后续连接、
应用服务和报告其他缺口继续开放。实现边界见 [native-choice.md](native-choice.md)。


## 2026-10-27：原生菜单、菜单栏、右键菜单与 Palette

Blitz shell 连接共享 Menu/ContextMenu/Palette 与 RTL/Typeahead 命令，复用 Sapid
的成员、子菜单、受控开关及勾选/单选控制器。ContextMenu 使用最近显式 region，
保留 Native/Override 编辑器策略、预编辑及修饰键保护；ContextPoint 以真实 CSS
视口像素定位并保留父模态/焦点范围。Menu Layer 管理实际焦点，Tab 按当前物理
顺序退出整个菜单栏或上下文 region，不进入其他 Menu 子树；关闭和 ExitMenu
同批提交时允许本 Menu 自身隐藏，挂起/断连/禁用目标仍拒绝。遍历预算为 65,536
已挂载物理节点，无访问历史、选项注册表或另一套宿主菜单控制器。

实际长菜单消费者发现两处缺口：滚动后的 Layer 命中误用内容偏移，导致底部点击
被当作外部关闭；Sapid 导航只发 Focus，没有显式请求滚动显露。适配器现补偿被
查询节点自己的 scroll，保留祖先/视口偏移；Sapid 只保留一个当前 NodeId 意图，
在可见绑定后发 Focus/ScrollIntoView，关闭及 Tab 撤销旧意图，包括受控关闭拒绝。
Focus 的不滚动契约不变。同批菜单导航后打开 Palette modal 不夺回输入焦点。

真正的 [Blitz fork f7178424](https://github.com/glory-rs/blitz/commit/f717842413c46125d0d9bd425890fbe62777a559)
另修复显式程序滚动没有取消旧触摸惯性的问题。原先 Home 显露首项后会被后续 fling
覆盖；修复后该回归通过，惯性帧自己的滚动链继续工作。固定 SHA、活动锁文件与
来源记录同步，无 Cargo checkout 修改、vendor、全局 patch 或新 unsafe。

最终 Glory 全工作区含文档测试 **1024 项**通过，Sapid **1153 项**通过；native/WASM
严格 Clippy、headless 检查及 CLI 25 项与严格 Clippy 通过。八项实际菜单消费端
覆盖四主题（适用时）、子菜单、受控拒绝、中文 Command 搜索/执行、模态恢复、
长列表显露/滚动点击和同批焦点隔离；原有五项选择控件消费端仍通过。
既有四组浏览器验收共 **49 个报告**通过，含四主题 CSR/水合、流式协议与清理。
Home 检查验证默认事件未被拦截，实际光标行首移动使用对应平台快捷键；原失败
记录保留。Fork 精简配置 127 项、并行/writing-mode 128 项及来源检查通过；fork
Clippy 仍显式排除既有 result_large_err/needless_return，未伪称全 fork 无例外严格通过。

两端 menu-probe 实际安装、SDK 启动并核对菜单/中文/勾选项初始画面与已安装二进制
哈希。Android 首帧仍是启动画面，保留原帧并另存实际绘制帧。普通 Android/iOS 包
另行构建，iOS device 以 15.0 部署目标链接；这些不替代实际触摸/系统键盘、读屏
或签名真机验收。源文件、日志、浏览器报告与图像摘要见
`/Volumes/Data/Caches/glory/native-menus/result.json`；实现边界见
[native-menus.md](native-menus.md)。

这里完成自绘语义菜单与 Palette 连接。平台系统菜单栏/托盘、TagsInput、富文本、
文件/语义剪贴板、Tooltip/navigation Group/Sheet 等弹层、移动权限 Rust 后续连接、
读屏/真机/分发、应用服务和发布工具仍按本报告继续补齐，整体差距不标为关闭。


## 2026-10-27：原生 TagsInput / PillsInput 输入连接

Blitz 解除 tags facet 的拒绝，接通共享 revisioned TagsCommand。输入仍是普通
单行 Input，Sapid 保留原有原子 state、分隔/重复/数量规则、移除项和成功值代理，
不增加宿主标签控制器、业务列表扫描或访问历史。策略补丁保留编辑器身份、选区
和值；正常光标、Space/Tab、未配置分隔符及空 Enter 的表单语义继续工作。

AppKit 使用一个当前 NodeId/命令种类配对空退格及粘贴与原始按键，尊重实际修饰键，
删除最后字符不会顺带删除标签。粘贴已经处理后，即使回调撤销 tags 策略，随后
原始按键也不能再次粘贴。回归在修复前实际得到 3 次读取而非 2 次；保留
`/Volumes/Data/Caches/glory/tags-pair-retirement-before.log`，最终完整测试通过。
其他输入、焦点变化及退休清除标记，标记不保存文本/剪贴板正文。

未组合的软件 IME 单个分隔标点使用同一提交协议；软件 Return 复用 Enter 路径，
包括空 Enter 的表单提交。组合中或空 Preedit 后的最终候选文本不会被分隔，随后
普通 Enter 可提交候选。消耗软件标点后使 native IME context 失效，保证受控拒绝
也能回写接受值。这是原生输入协议验证，不替代实际软键盘/中文 IME 的该控件验收。

粘贴只读取一次 document ShellProvider 快照，返回后在规范化/复制进事件或修改
编辑器前执行 16 KiB UTF-8 检查，再检查规范化结果；4096 字节当前草稿/普通粘贴
结果也受限。Provider 的原始 OS 读取/分配发生在返回 String 之前，不把其内存
算作已受 UI payload 限额保护。分隔/换行粘贴产生一个含 UTF-16 选区的原子提议，
保留物理编辑器直到组件接受。普通粘贴走同一编辑器/文本事件；超限只发送无正文
拒绝，保留选区并允许后续恢复。百万字节和 UTF-16 emoji 选区已有实际消费端证据。

新增七项输入回归及一项 AX 回归。AccessKit 保留 TextInput/value/SetValue，没有
虚构 ComboBox/ListBox；AT 赋值不会分隔，退休动作失效。五项实际 Sapid 消费端的
四主题验证自由值、重复/上限、受控拒绝和光标、已选中的成功值代理、原子粘贴/Pills
重复项、IME/空 Return、同批输入提交、256 标签下少于 1100 节点及清理，并输出
CPU 控件/文字像素。Sapid 运行时没有本批改动；既有菜单八项和选择控件五项回归仍通过。

最终 Glory 全工作区含文档测试 **1032 项**、native/WASM 全工作区严格 Clippy、
headless 无默认 feature 以及 CLI 25 项/严格 Clippy 通过。引擎固定 SHA 没有变化，
当前来源检查通过。两端 tags-probe 构建、安装、SDK 启动和初始中文输入框画面及
已安装二进制哈希单独核对；iOS device 以 15.0 部署目标链接，普通两端包另行构建。
详情与摘要见 [native-tags.md](native-tags.md) 和
`/Volumes/Data/Caches/glory/native-tags/result.json`。

这批完成原生输入和注入 provider 的 Tags 协议连接。普通 Glory shell 尚未暴露
系统 clipboard provider，因此实际系统剪贴板/语义 CopyText、移动粘贴服务、真实
设备输入/读屏、文件选择与富文本、其余平台和应用服务缺口仍开放，继续逐项补齐。


## 2026-10-27：显式原生剪贴板与桌面系统连接

新增可选 `glory-blitz/clipboard`（facade `blitz-clipboard`），由应用在 Config 中
显式提供 Clipboard。默认不开启 OS 服务，也不增加默认/SSR/WASM 生产图依赖。
桌面使用固定 Arboard 3.6.1 的一个 worker，首次 poll 才打开系统剪贴板；同一服务
保留一个真实执行许可和当前结果，不保存访问历史。无图片剪贴板或额外 unsafe。

CopyText 复用既有 node/source revision/request 协议。源替换/节点退休撤销结果；
首 poll 前失效不执行 OS 写入。每文档只允许一个当前操作，忙碌请求发 Failed，
默认 30 秒、可配置至 120 秒 deadline。已开始的 OS 调用不能被撤销，超时或取消
不能提前归还许可，迟到结果不覆盖新来源。驱动 future 的构造、poll、drop 均在
文档借用外，完成只排回受校验的语义事件；稳定等待 I/O 不触发无休止 observe。

WindowApp 的复制、剪切、粘贴及 AppKit 配对键使用同一条通道。密码不导出复制/
剪切，readonly 仅允许复制，IME 保持自己的输入。剪切在写入成功且 receipt 仍
有效后删除；粘贴核对实际/语义焦点、输入 revision、选区和当前策略，编辑、焦点
或程序选区 ABA 会撤销旧结果。仅名称/布局更新不会因 Parley layout generation
改变误撤销有效操作。普通粘贴保持 InputEdit 的 before-selection，并在编辑前
检查结果大小；Tags 使用已有 16 KiB/4 KiB、UTF-16 原子提议与无正文预算拒绝。
65,536 字节普通剪贴板预算不包括 OS/provider 返回 String 前的分配。

macOS 实际 worker 写读、超限读取及语义 CopyText 往返通过。Swift guard 在 RAM 中
保留原剪贴板全部条目与类型，检查当前测试 token 后恢复并逐项核对字节；不记录
或落盘用户剪贴板正文。当前记录为 `clipboard-macos-system.log`。Sapid 三项新
消费端测试在四主题覆盖 CopyButton 全部结果/源撤销、CodeHighlight 精确源、两种
JsonViewer 源复制、TagsInput 异步原子粘贴和超限恢复；默认不可用测试仍通过。
这些是驱动/CPU 消费端证据，OS 键盘、菜单和读屏仍分别验收。

最终 Glory 全工作区含文档测试 **1052 项、221 个结果报告**通过；普通 gate 中
一项会修改系统剪贴板的测试显式忽略，再由 guard 单独运行通过。native/WASM
全工作区严格 Clippy、headless、无 shell 的 clipboard 协议测试、CLI 25 项/严格
Clippy 和依赖图隔离检查通过。iOS 模拟器及 Android GameActivity 的新 feature
适配器 Clippy（--no-deps）通过，仅证明可编译，并无移动 OS clipboard driver。
固定引擎 SHA 不变，来源核对通过。实现及复验命令见
[native-clipboard.md](native-clipboard.md)；日志/源摘要见
`/Volumes/Data/Caches/glory/native-clipboard/result.json`。

桌面系统服务与共享异步输入连接在此完成；Android/iOS 剪贴板 driver、其他桌面
OS 往返、文件服务、富文本、权限后续连接、平台交付和应用服务继续开放。


## 2026-10-27：Android 前台系统剪贴板

AndroidClipboard 以安全 JNI 接入应用自有 GloryClipboard/ClipboardLedger，无新
native 导出或 unsafe。构造只验证协议并认领一个 Host，不访问系统剪贴板；克隆
共享 ID 序列，重复包装同一个 Host 明确拒绝。请求首 poll 才提交，Java 主线程在
Activity resumed 且窗口获焦时调用 ClipboardManager，系统 API 不在 ledger 锁内。
Activity 仅弱引用，销毁关闭 coordinator，迟到回执不能进入后继 Activity。

每 coordinator 最多 32 个当前弱 client、一个排队/执行中的真实操作，每 client
一个请求/结果；取消清除回执，但排队任务实际跳过或系统调用返回前不归还执行
许可。当前 ID 不复用，无完成历史或 clipboard listener。只取第一项 plain text，
不对 URI/HTML 做 coercion。Java 分别限制 UTF-16 工作量、原始和规范化 UTF-8，
Rust 在复制 byte array 前再限长/验证 UTF-8；OS ClipData/CharSequence 分配仍在
传输预算之外。8 ms timer 只属于当前等待 future，空闲没有定时轮询。

纯 JVM 测试使用真正的 ledger，覆盖取消前/执行中许可、200 次 busy 拒绝、源字节
快照、一次性结果、client/Activity 关闭、ID 耗尽和中文/emoji/CRLF/NUL/孤立代理
字符预算。GameActivity 专用 clipboard-probe 实际 ARM64 SDK 安装/前台启动通过，
含真实系统读写、超限读取/写入拒绝、语义 CopyText 和原 ClipData 恢复；安装包与
构建产物哈希相同。Guard 仅在 RAM 中持有原对象的全部 item/type，并核对恢复后的
数量/类型计数，不声称 URI 目标字节快照。不保存用户正文或可能含原剪贴板系统
预览的截图，没有注入触摸/按键。最新可重复记录见
`/Volumes/Data/Caches/glory/android-clipboard/final/sdk-result.json`。

普通 GameActivity APK 另行构建并核对 clipboard/guard/permissions 三个 BuildConfig
flag 均为 false，未初始化服务/guard；功能由应用显式选择，不增加权限声明。
Glory 1052 项测试、native/WASM 严格 Clippy、Android 驱动与 probe 应用 Clippy
（--no-deps）、headless、iOS 新 feature 编译检查及引擎来源核对通过，固定引擎 SHA
未变。详细接口和复验命令见 [native-clipboard.md](native-clipboard.md) 与移动示例。

这里完成 Android 前台系统服务，尚不等于实际移动键盘/剪贴板菜单、选择手势、
TalkBack 或真机验收。iOS 系统 driver、原生文件服务和报告其他缺口继续推进。


## 2026-10-27：原生文件选择、读取与私有预览（仍待平台验收）

新增显式 `glory-blitz/files` / facade `blitz-files`，默认构建不安装任何文件服务。
桌面 Rfd 选择器与应用注入的安全 driver 共用不透明令牌；最多一个选择器、四个读取，
克隆服务共用 128 个引用、128 KiB 元数据、1 GiB 声明大小预算。替换失败保留前次选择，
文件名仅作显示，分块读取始终使用最初打开的常规文件句柄，EOF 不返回部分正文。
真实文件测试验证路径替换不改读取身份，以及丢弃回执不提前归还实际 OS 读取额度。

PNG/JPEG 预览先检查共享的 64 KiB 头部、8 MiB 编码和像素/尺寸限制，再有界读取正文。
待完成、已发布及取消后仍被 decoder 持有的编码体共用 32 MiB 服务预算；文档私有地址
精确绑定整个令牌，无路径/外部 transport 回退。删除/清除/配置退休会撤销源与后续读取。
实际测试还发现引擎图片复用缓存可能绕过源撤销，已在真正的
[Blitz fork 6a9ca05b](https://github.com/glory-rs/blitz/commit/6a9ca05b246ecec6a7844e18dc6d264eb8f076cf)
增加按地址撤销缓存和迟到结果的接口与独立回归，全部活动 manifest/锁及来源记录同步。
已经交付到存活元素的像素不承诺安全擦除，编码/解码/绘制预算也不代表总 RSS。

另一项先失败后修复的共享 UI 回归证明：同一批已 drain 的输入里，前一个事件清除文件后，
后面的成功读取/预览回执也必须重新校验当前令牌。foreign driver 的构造、poll、drop 和
退休句柄析构均在文档借用外，取消/超时不虚称中断不可取消的 OS 操作。Sapid 实际
FileInput/FileAttachment/FileButton 四主题消费端覆盖读取、显式预览、移除、取消/拒绝
保留原选择及清理，未新增宿主业务控制器或 unsafe。实现与复验见 [native-files.md](native-files.md)。

最终全工作区含文档测试 **1071 项、222 个结果报告**通过；一项会修改系统剪贴板的
既有 opt-in 测试在普通 gate 中仍显式忽略，没有重做该系统操作。native/WASM 全工作区
严格 Clippy、headless 及默认依赖图隔离通过；Android/iOS 新 feature 编译检查、CLI 25 项
及严格 Clippy、三个 fork 的固定来源和独立测试检查通过。Sapid 原有文件模型 6 项、新增
原生四主题消费端 2 项、默认不可用回归 1 项及消费端严格 Clippy 通过。
证据摘要为 `/Volumes/Data/Caches/glory/native-files/result.json`；这些检查与 OS 输入验收分开。

这批仍不关闭完整文件服务缺口：系统选择器真实输入验收、macOS sandbox security scope、
原生拖放、移动 driver、读屏和真机流程继续保留。没有将协议/CPU 或编译结果当作这些验收。


## 2026-10-27：桌面原生文件拖放通道

桌面交互 WindowApp 现接通 Winit DragEntered/Position/Dropped/Left 和异步数据回执；
`Files::desktop()` 显式安装 opener，注入服务可用 `with_drop_driver` 选择自己的执行端。
只有当前可用且配置允许的 File 控件接受 Copy，悬停只查询类型，不读取 URI 或打开文件。
确认投放后核对当前传输 ID/回执编号，拒绝外来/重复/迟到数据，并在文档借用外读取 URI。
只保留本地绝对 file URI；数量、单项 4096 字节和合计 128 KiB 在路径复制/打开前检查，
系统 API 返回原始 URI 集合之前的分配不在该预算内，也不把 URI 来源当作 OS sandbox。

选择器和投放共用令牌/元数据/filter/大小/预览预算与稳定句柄；投放另有跨文档共享的
一个当前许可，取消/超时不提前归还尚未结束的 opener。成功选择器撤销旧投放，成功投放
撤销仍待返回的选择器，投放失败不清除原选择或挂起的选择器。生命周期、配置、禁用、
模态/焦点范围及实际作者 CSS 可见性在读取/交付前重新核对。未开始的操作直接跳过，
已经开始的实际 OS 工作保留到结束；不保存完成 ID、历史路径或 UI Owner。

十三项专项通过，覆盖带偏移的实际非零布局与 1x/2x 命中、回执匹配、URI 拒绝、
配置/Owner 退休、CSS 隐藏、模态/焦点边界、foreign 回调和 future/析构重入、跨文档额度、
超时及选择器竞争。Sapid 实际 Dropzone 四主题消费端通过：自有原生事件源使用真实 Winit
接口及生产桌面 opener，文件改名并替换原路径后仍可读取/预览原始 PNG，拒绝保留选择，
Remove/Owner 清理释放引用与预览。该事件源没有执行系统鼠标拖动，OS 输入验收仍独立。

最终全工作区含文档测试 **1084 项、222 个结果报告**通过（其中包含上述 13 项专项）；
既有会修改系统剪贴板的一项 opt-in 测试仍在普通 gate 中忽略，没有新增 OS 剪贴板操作。
native/WASM 全工作区严格 Clippy、无 shell 的 files 严格 Clippy、默认依赖图隔离、
Android/iOS 适配器严格 Clippy（--no-deps）及固定 fork 来源核对通过。Sapid 原生
文件消费端共三项、四主题循环及严格 Clippy 通过，widget inventory 重新生成和检查通过。

引擎固定 SHA 不变，无新 unsafe。详细边界见 [native-files.md](native-files.md)，
本批证据为 `/Volumes/Data/Caches/glory/native-file-drop/result.json`。移动 picker/driver、
OS 选择器/拖动、sandbox security scope、真机/读屏以及报告其他缺口继续推进。

## 2026-10-27：Android 文件选择与 ContentResolver 读取

显式 `AndroidFiles` 用安全 JNI 连接应用自有 Java protocol-v1 Host；移动示例的
`files` 功能将其接入 `Config::files`。普通宿主从 resumed/focused Activity 发起
`ACTION_OPEN_DOCUMENT`，只接收当前 request code、read grant 和有界 content URI。
过滤提示不代替回执校验，不增加存储权限、不持久化 URI 授权，也不将显示名当路径。
ContentResolver 在有界 worker 查询元数据并打开 PFD；目前只接受大小已知、可 seek 的
常规文件。读取保留原句柄，不接受未知大小、pipe 或非 seekable 云端流。

一个进程期原生额度账本跨越 Activity 重建，限制一个实际选择/打开、四个读取（包含
已完成未取走的正文）、32 个弱 client、160 个 charged handle、160 KiB 显示元数据及
2 GiB 声明大小；共享 Rust 文件服务的 128 引用/1 GiB 限额另行保持。五个有界 worker
执行打开、读取和回收，空闲线程退出。取消/销毁先撤销交付，实际工作结束前不归还额度；
最后一个实际读取返回后才关闭退休句柄。没有把这些预算描述为总 RSS 或强制中断 OS 调用。

Rust 对并发请求的数字 ID 分配与 JNI 提交排序；Java 只保留当前记录与不回绕水位。
Activity 保存的只有 request-code 水位，没有 URI、授权或完成历史。终态 Future 即使
被调用者继续持有，也立即释放取消/失败回执；迟到旧 Activity 结果不能打开文件或投递
到后继文档。回调不进入 Rust UI，foreign provider 工作不持有 ledger/document 锁。

严格 Java 编译及真实 ledger 的 JVM 回归覆盖实际调用未结束的额度、跨 session 阻塞、
四个未取走结果、160 句柄/元数据/声明大小上限、无效 opener、UTF-8、未 adopt 清理、
主线程投递失败、ID/request-code 耗尽。两个 Rust 协议测试覆盖精确报文、截断、重复 ID、
非法 UTF-8 域和预算。无新 unsafe，引擎固定 SHA 未改。

`files-probe` 专用 APK 实际 ARM64 SDK 安装/启动通过：自有 selector Activity 交付真实
Activity result，私有 ContentProvider 返回真实 PFD；验证单选/多选、取消、数量/type/
grant/filter/大小拒绝、保留原句柄、语义读取中文/emoji、私有 PNG 解码、令牌/cache
撤权、原生额度及 provider descriptor close callback 归零。没有注入触摸或按键。
该来源不等于系统 DocumentsUI 的用户操作；普通 APK 的 manifest/DEX 排除两个验收组件。

全工作区含文档测试 **1086 项、222 个结果报告**通过；既有系统剪贴板 opt-in 测试一项
仍忽略。native/WASM 严格 Clippy、Android 适配器/probe、CLI 25 项及固定 fork 检查通过。
证据与 APK 哈希在 `/Volumes/Data/Caches/glory/android-files/`；接口和复验命令见
[native-files.md](native-files.md) 与移动示例。系统选择器真实操作、非 seekable provider、
iOS driver、sandbox security scope、真机/读屏和其他报告缺口继续开放。
