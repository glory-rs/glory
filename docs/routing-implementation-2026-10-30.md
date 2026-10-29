# Glory 路由首轮实现和验证

日期：2026-10-30。当前 API 见 [routing README](../crates/routing/README.md)，完整路线见[架构报告](routing-architecture-2026-10-30.md)。实现位于 `crates/routing`，包名 `glory-routing`，facade 通过可选 `glory/routing` 暴露。

## 实现范围

| 能力 | 实际交付 |
| --- | --- |
| URL 和匹配 | 统一 Rust URL/编码语义；静态、参数、末尾可选、catch-all、index、无路径布局；路径段优先级；歧义和预算检查；完整匹配失败不泄漏参数 |
| 类型化目标 | RouteKey/P codec、反向生成、可选/重复 query、fragment；相对目标按当前节点已消费路径解析；内部导航不能逃出 mount base |
| 状态和历史 | 单一只读快照、弱 RouterHandle、有限 MemoryHistory、独立 EntryKey、replace/截断 forward、活动路径判定、失败不提前发布 |
| 嵌套视图 | RouteViews/RouteContext/Outlet 沿用 Widget；默认保留同声明的布局和页面；参数动态读取；显式 key_by 重建；缺失/重复 outlet 报错 |
| 条件与并发 | 同步和异步 guards、有限重定向、重新校验；supersede 立即释放 pending Owner 并唤醒旧 waiter；重入同步导航返回 Busy |
| Web | 独立 BrowserHistory、真实 href 与 scoped raw anchors、历史事件队列、有界 window 滚动/锚点恢复、释放监听器和恢复 scrollRestoration |
| SSR | 纯 RequestLocation/RequestHistory；Salvo 200/400/404/3xx、拒绝和 HEAD；拒绝或重定向不构建页面 |
| Hydration | 有限、版本化、无可执行内容的路由 bootstrap；先采用 SSR 路由，再处理加载期间变化的浏览器地址；保留脏编辑身份与选区 |
| Native | 不依赖渲染器的 Back/Forward/Open、显式 scheme/host 深链接转换、交互式返回的准备/取消/提交；Apace WindowApp 帧边界接线与返回键路由 |
| 工程接入 | 可选 feature、默认依赖图审计、发布顺序、工作区清单、CI 浏览器验收和共享示例 |

Routes 保存可跨请求共享的纯声明和 codec 函数指针，RouteViews 单独保存本地 Widget 工厂。实际 API 使用 `Routes::builder()` 加 `RouteViews::new(routes.clone())`，没有重复路径定义，也没有新组件或渲染协议。参数投影返回 `Bond<Result<Rc<P>>>`，退休上下文以 Gone 表示失效。

平台服务和渲染服务分别接入。Android/iOS 或替换渲染器可以直接使用 routing 的 MemoryHistory 与 NativeNavigation；Apace 是其中一个 Widget 宿主，不是路由核心的依赖。

## 验证结果

| 检查 | 结果与证据范围 |
| --- | --- |
| routing Rust 测试 | 20 项通过：编码/优先级/相对目标/base、状态、guards、取消、历史、嵌套生命周期与 bootstrap |
| Salvo 路由测试 | 2 项通过：真实 Response 状态/响应头/HEAD、拒绝与 redirect 不运行工厂 |
| Apace 路由测试 | 2 项通过：实际原生文档内容、布局保留、取消返回；WindowApp 输入在 observe 才提交 |
| Chrome 浏览器 | 3 个完整场景通过：CSR、SSR hydration、WASM 前 URL 改变；涵盖输入身份/脏值/选区、query/hash、返回、重复 URL 滚动、raw anchor、fallback、清理 |
| canonical 工作区脚本 | CLI、引擎来源/测试、工作区全 feature 测试、package-only 检查及严格 Clippy 通过 |
| 最终源码检查 | routing 默认 graph 严格 Clippy；工作区 Windows 和 wasm32 全 feature/target 严格 Clippy 通过 |
| 示例 | browser WASM、SSR generator 和 native binary 编译检查通过；复用同一个 page Widget 工厂 |
| 发布工具 | 11 项 release 工具测试通过；更新后的依赖发布顺序校验通过，未执行发布 |

本地日志和浏览器报告位于 `D:/BuildCache/glory-routing`。重跑入口为 `scripts/check-semantic-ui.ps1`，以及 [示例 README](../examples/routing/README.md) 的浏览器构建和验收命令。最终 URL 边界完善后，已补跑 routing、Salvo、Apace 目标测试及 Windows/WASM 全工作区 Clippy。

浏览器用例等待真实导航提交再继续，符合 Glory 将原生点击排队到帧边界处理的约定；同页面 id 不能单独作为点击已处理的证据。

## 后续边界

本轮没有 Android/iOS 真机或模拟器验收，没有新增系统 scheme 注册、UIKit 手势桥或通用平台转场。现有 Windows 原生文档/WindowApp 测试不作为这些 OS 能力的完成证据。

任意嵌套滚动容器/虚拟列表业务锚点恢复、浏览器已发生 traversal 的取消、独立 stream bootstrap/End 接管、Routable 派生、并行命名 outlet、keep-alive、预取/分包、增强 GET Form、动态 head 和官网迁移继续按架构路线推进。本轮不宣称与 Leptos/Dioxus 的全部外围功能对齐，也没有吞吐、WASM 体积或 RSS 的比较结论。
