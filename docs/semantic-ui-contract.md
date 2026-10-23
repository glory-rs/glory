# 语义组件契约与适配器审计

日期：2026-10-23。对应破坏兼容重构的 R0；本页记录当前实现及明确的剩余审计，不将基础矩阵通过当作 R0 全部完成。架构与实施证据分别见[架构报告](semantic-ui-rendering-architecture-report-2026-10-21.md)和[实施记录](semantic-ui-implementation.md)。

## 八组件及内容

| 名称 | 内容与配置 | 子节点 | 当前 HTML 默认 |
|---|---|---|---|
| view | 样式盒、role/name/focus/item | 接受；继续校验交互祖先约束 | div；有 name 的默认 group；Main/Navigation/Section 分别 main/nav/section |
| group | 无盒子的逻辑范围、身份与寿命 | 接受，保持逻辑祖先；不产生样式或可访问性 wrapper | 注释范围 |
| text | content，Inline/Paragraph/Heading(1–6)，selectable；Heading 可设置 name | 不接受 | span/p/h1–h6 |
| button | label、enabled | 仅 inline text、image、group；不能包含交互控件或显式 focus 节点 | button，type=button |
| input | 字符串 value/revision、purpose、multiline、enabled | 不接受 | input / textarea |
| check | bool checked、label、enabled | 不接受；label 是独立的受控文字部分 | label + checkbox + 文字 |
| image | source、alternative | 不接受 | img |
| link | target、label | 接受；不能嵌套交互控件或显式 focus 节点 | a |

Group 不接受 classes、style、role/name、显式 focus 或 item；需要盒子或语义区域时使用 view。移动已有 Group 时也校验整个被移动子树，不能通过先组装再移动绕过祖先内容约束。普通列表行移动没有交互祖先时不遍历子树。

Input的值始终是字符串。Number/Email/Url在HTML分别使用type=text + inputmode=decimal/email/url；purpose表达呈现/键盘意图，不隐式解析数值、裁剪空白或替业务校验。实测原生HTML的type=email/url会裁剪两端空白，因此这里不用它们改变文本值。multiline仅支持Purpose::Text；单行程序值不允许CR/LF。

存入语义树的Input值使用LF，CRLF/CR转LF、NUL转U+FFFD；单行程序换行继续报错，不静默过滤声明。共用normalize_text对已规范文本返回借用，主机输入可在事件边界移除单行换行。业务Cage不会因为渲染自动改写：它可以保存原始文件换行，显示描述符和回调值为规范文本。HTML、Web与Blitz复用同一规则。

Image 的 None alternative 表示装饰性，不代表漏填；Rust builder 要求显式 alt(...) 或 decorative()，低层 Descriptor 的默认 ImageProps 已是装饰性值。Text 的 selectable=false 是呈现属性，实际选择/辅助技术行为仍需相应宿主验证。

## 角色、名称与焦点元数据

Role 是 Glory 的有限结构角色，现为 Group/Main/Navigation/Section/List/ListItem/Status/Alert，只属于 View。`.role(...)` 只出现在 View builder，低层 Descriptor/Patch 同样拒绝给 Text、Button、Input、Check、Image、Link 替换结构角色；Group 组件仍没有物理/可访问性角色。此限制保留控件的固有语义，未引入通用 ARIA 字符串接口。

`Descriptor::effective_role()` 给 name=Some、role=None 的 View 返回 Group；显式角色优先。HTML lowering 使用该结果，布局标签仍为 div。Name Clear 后，无显式角色的 View 恢复 generic；显式 Role::Group 则继续保留。命名策略参考 [WAI-ARIA 的 generic](https://www.w3.org/TR/wai-aria-1.2/#generic) 和 [group](https://www.w3.org/TR/wai-aria-1.2/#group)，实际结果另通过 Chromium AX 树验证。Glory 的有限角色词表比 [HTML 允许的 ARIA 映射](https://www.w3.org/TR/html-aria/)更窄。

| 元数据 | Set / 有值 | Keep / Clear | 错误与恢复 |
|---|---|---|---|
| View role | 显式结构角色；Main/Navigation/Section 映射结构标签 | Keep 保留；Clear 采用有名 group/无名 generic 默认 | HTML 标签改变仍须重挂；已有 item 不能失去 ListItem 角色 |
| View name | 默认可命名 group，或显式角色的名称 | Keep 无物理修改；Clear 去掉作者名称及隐式 group 角色 | Group 组件不接受 name |
| Heading name | 覆盖作者名称，content 仍独立保留 | Clear 回退到可见标题内容 | Inline/Paragraph 不接受 name，使用 content；带 name 时不能清除为普通文本 |
| Button/Input/Check/Link name | 映射到实际 control；Check 为内部 checkbox | Clear 回退到原生 label/content 算法，Input 可无作者名称 | 结构角色覆盖在模型/排队前拒绝；name 不重写 value、label 或 children |
| Image name | 有 alternative 的图片可设置作者名称 | Clear 恢复 alternative 的原生命名 | 装饰性 alternative=None 拒绝 name 与显式 focus；切换装饰前先清理 name/focus |
| Focus | Programmatic=-1，TabStop=0，作用于 control | Keep 保留；Clear 移除显式 tabindex，恢复宿主默认 | Group/装饰图不接受显式 focus；装饰图的 Focus 动作入队前 Unsupported |
| Item | ListItem 上使用逻辑 index+1/总数，未知总数=-1 | Keep 不改属性；Clear 移除两项位置元数据 | 继续校验角色、索引溢出与 index/count，不以窗口内顺序替代 |

名称遵循宿主可访问名称算法，空/空白作者名称不保证强制覆盖原有内容。相等/Keep 元数据不写 DOM；Clear 是移除作者配置，不清空可见文字、编辑值或物理控件。初始 builder 配置作为整体检查，低层单项 Patch 拒绝冲突并保持原描述符；需要同步清理多个字段时按依赖顺序提交，或使用创建前 Descriptor::patches。

本节证明有限词表、共享 lowering、实际浏览器 AX 及原生引擎属性/编辑状态；Blitz 尚无平台辅助技术桥，不能把引擎保留 aria-label/role 等属性当成 OS 无障碍支持。Live region 播报、屏幕阅读器宣布逻辑位置与总数仍独立验收。

HTML 扩展只改变映射，不得绕过内容模型。Inline text 可以在一般流内容中显式映射为 p；在 button 中必须拒绝，包含 Group 的 Place/Resume 路径。Paragraph/Heading 与冲突 tag 覆盖也拒绝。Text、Button、Check、Link 自己的文字部分与用户 children 分开更新，不使用 textContent 清空后者。

kind、multiline、purpose、role 等 builder 配置为构建期值。低层 Patch 按具体后端检查：Render::preflight 在语义模型修改/排队之前拒绝需要改变 HTML tag 的 TextKind、View Role 与 Input Multiline。tag 不变的更新允许；调用方需要改变结构时显式重挂。不能因为参考解释器接受描述符更新，就宣称物理后端可保留身份修改结构。

preflight(None) 检查创建；preflight(Some(patch)) 使用当前语义描述符，包含尚未提交的修改。它不得读宿主树、布局、I/O 或执行业务代码。HTML 的冲突 Text 选项也在创建前检查；拒绝不占 NodeId。直接 adapter.apply 仍必须检查同一限制，不能绕过 App 后静默接受。

父子关系采用 `Render::preflight_place(tree, child, parent)`，tree 是借用的当前语义 Memory，包含尚未提交的祖先、分组与修改；不能改读 adapter 已提交的树。`Subtree::New` 借用待创建描述符，没有身份或子节点；`Existing` 指向存活子树，含单独暂停的后代。共享 `Memory::validate_child/validate_placement` 先检查身份、叶节点、祖先内容、环与 sibling/Place/Resume 规则，adapter 再检查自己的内容限制。UI 创建在分配身份之前完成检查，移动/恢复在修改模型与排队之前完成检查；拒绝不消费身份、不改变层级/暂停状态、不留下待输出操作，后续合法操作仍可提交。

HTML 的同一只读助手供 SSR/Web/Blitz 预检和直接 Mapper.apply 使用，按钮中的显式 p 映射无法经 Group 移动/恢复绕过。这里只检查本次父子关系，不克隆语义树、全文或样式；普通列表移动只沿祖先链，限制性按钮/交互祖先才检查被移动的存活子树，不访问列表数据源。多根 widget 的多个操作及直接 apply 的整个批次仍不承诺原子回滚；预检也不保证后续宿主 I/O 或布局成功。

同步 SSR 与渐进 SSR 是独立 Renderer；二者分别实现上述预检及事件注册/动作/查询目标检查。流式 Renderer 也在直接 apply 后续项拒绝时同步 Mapper 的成功前缀水位，不把之前有效操作报告为旧水位。它运行相同的八目标/32查询 conformance，物理能力仍全部关闭。

本地固定 Blitz 引擎已实现 Password 的扩展字素簇遮罩，创建与 Purpose 修改不再拒绝。原始编辑器保留 value/IME 字节与选区；独立圆点布局用于绘制、指针、键盘导航、光标、选区和滚动，经同一索引表映射回 raw UTF-8 边界，Query::Selection 仍回读原值 UTF-16。复制/剪切不导出原文，粘贴正常；双击和 word 导航按整个密码字段处理。SSR/Web 保持各自的 HTML password 映射。

Password preedit 只在圆点区间标记组合状态/下划线；不将原始 preedit 放入显示缓冲。用途切换保留物理 control、值、选区与组合输入；新值或 DPI/视口变化后，仅当前焦点编辑器重算可见光标、滚动和 IME 区域。宿主接收 Normal/Password 意图，桌面 winit 同时声明 hint/purpose 与 cursor-area 能力并合并重复更新。此处交付自绘和输入协议，实际 OS IME/辅助技术继续按平台验收。

## 事件目标

`EventKind::validate_target` 是 render 层的共同语义规则，UI 注册/派发与参考、SSR、Web、Blitz 的订阅路径均复用。它不依赖 DOM 或布局引擎。

| 事件 | 允许注册的语义目标 | 派发 |
|---|---|---|
| Activate | 所有节点，含 group | 最近逻辑 handler；disabled 祖先阻止激活 |
| ValueChanged / Commit / Composition | input | 直接输入控件 |
| CheckedChanged | check | 直接复选控件 |
| FocusChanged | 有物理表示的节点；不含 group | 直接焦点来源 |
| FocusWithin | 所有节点，含 group | App 从 FocusChanged 沿逻辑祖先派生 |
| Navigate | 所有节点，含 group | 来源明确，最近逻辑 handler |
| Scroll / Resize / Anchor | 有物理表示的节点；不含 group | 相应观察/接管目标；是否可测仍由宿主决定 |

订阅能力与产生事件的能力分开：SSR 可记录合法编辑/焦点订阅，供其组件在浏览器执行；这不代表 SSR 拥有输入驱动或物理观测。错误类型的编辑事件注册是 InvalidProps，Group 的物理事件注册是 Unsupported；UI 在替换 handler、登记订阅或排队 listener 之前检查。禁用一个未成功启用的 interest 可作为清理 no-op，但仍先校验节点身份。

暂停保留逻辑身份和 Scope，非终止输入被丢弃；FocusChanged(false)/Composition(false) 仍送达存活节点，释放离屏编辑与 pin。Dispose 后旧代际事件不回调，不能触达复用 slot。禁用控件不接收激活/用户修改；重新启用不需要重新注册合法 handler。

## 事件载荷、来源与批处理

`Event::validate_payload` 检查消息的局部数值和形状；`Memory::validate_event` 在当前语义树检查嵌入的来源身份/祖先关系。App 在进入业务handler前复用它们，后端不必复制另一套列表校验。

| 消息 | 已实施的检查 |
|---|---|
| Scroll | x/y有限，允许原生负偏移；viewport为局部x=y=0、有限非负宽高 |
| ScrollAck | requested有限且非负；accepted x/y有限，可以保留原生负值；不改写真实坐标 |
| Resize | size/viewport均为局部矩形；有限非负宽高，允许零尺寸及小数 |
| Anchor | within有限且非负；viewport合法；row是当前目标的存活、有物理表示的严格后代 |
| FocusWithin | 来源有物理表示，且为目标自身或后代；不能用无关树或Group伪造来源 |
| Navigate | 显式from必须等于事件目标，即使错误from已被销毁也诊断不匹配 |
| ValueChanged / Commit | 事件为规范文本；单行不能含CR/LF，多行允许LF；二者都携带revision |

foreign来源返回ForeignRuntime，活着但无关的来源/错误数值返回InvalidEvent。retired来源或不再活跃的非终止来源作为迟到消息忽略；slot即使被重用，也不把旧generation解释成新节点。Anchor的within不在无布局的语义层推测实际行高；列表仍保留自身锚点与估值规则。局部矩形和身份关系也不证明OS确实产生过这个输入，不能当作外部消息认证。

App对主目标已失效的迟到事件仍直接丢弃；Memory显式push_event对不存在的主目标仍返回身份错误。参考队列在drain/相关清理时重验嵌入来源，避免入队后移动、暂停、删除改变其关系。`Event::is_terminal_loss`统一FocusChanged(false)、Composition(false)、FocusWithin(false)：存活暂停Scope可以清理，Dispose后不可复活。

直接焦点handler可能删除或移动来源；后续派生FocusWithin在模型stats变化后重验，不把失效来源发布给祖先。正常路径只收集一次祖先链，不为每个观察者重新扫描；无访问行历史或业务源扫描。

`App::pump`现在处理完整个已drain批次，错误事件不进入handler，但不会提前丢掉后面的有效编辑/查询响应。有效写入最后仍flush，query waiter正常移除。若flush失败，优先返回该错误；否则返回第一项route错误。单独dispatch仍直接返回它自己的错误。这里没有增加整个mutation batch回滚，也没有改变业务panic语义。

程序revision在同一存活NodeId内单调：Set/Clear不能回退，相同revision只能重放规范值相同的写入；Keep完全忽略revision。相同值可用更大revision表示新屏障，幂等重放不能代替新屏障。builder从当前节点分配下一版本，可以与显式Scope.patch混用；耗尽返回InputRevisionExhausted，不回绕或panic，重挂新节点建立新寿命。

ValueChanged和Commit { value, revision }都携带产生输入时的程序屏障：小于当前版本丢弃，大于当前版本报InputRevision，等于当前版本才可进入回调。Commit不再使用无版本tuple。终止composition/focus清理不因旧程序值而被屏蔽。SSR接管创建新Runtime/NodeId与版本，不能把服务器计数器当客户端身份；dirty输入带客户端接受时的版本。

Web读值时修复非规范主机文本并保留UTF-16选区；正常规范值不写回DOM。hydration验证阶段只计算规范观察，整个形状验证成功后才修复值，不让后续节点错误破坏先前输入。规范等价程序回写只确认版本，保留选区/preedit；原生输入的UTF-8游标偏移由共用规则转换。

## 查询、动作与能力

| 目标/请求 | 共同规则 | 实际宿主差异 |
|---|---|---|
| Bounds / Scroll | Group 没有自己的物理目标，明确 Unsupported | 其他节点是否已有布局盒取决于引擎/样式/连接状态 |
| Selection | 只可针对input，统一UTF-16 code units | 还取决于编辑驱动/状态；当前七种HTML purpose均保留文本选区API |
| Focus 查询 | 有物理表示的控件；不是 FocusWithin 或 OS 激活 | Web/WindowApp 读实际 control 文档焦点；Memory/SSR/无交互 Blitz Unsupported |
| Focus | Group/disabled 控件不接受 | 其他目标仍需宿主支持、可聚焦并可见 |
| ScrollTo | 有限且非负的请求坐标；Group 不接受 | 宿主可能 clamp；ScrollAck 分开 requested/accepted/current |

Scroll 查询可返回负值，例如 RTL/弹性滚动；当前 ScrollTo 的非负请求范围是明确限制，不等于已经实现完整 RTL 定位协议。

`Capabilities` 是宿主级上限，不是每个节点或每种 CSS 状态的保证。Memory 与 SSR 四项均 false；无窗口 Blitz measure/scroll=true，focus/input=false；WindowApp 与 Web 均已启用四项。不能由 input=false 推导“不能生成受控 value HTML”。

有效查询接受后返回 token，结果异步进队列，含接受时的 applied 水位；Unsupported 也是一次对应响应。后来的 commit 不能改变此前结果所属屏障。失效身份直接拒绝，不登记 waiter；已登记后被 Dispose 的等待者只完成一次 NodeGone，迟到响应忽略。App 验证 kind、水位、有限几何、非负宽高与有序选区。applied 只表示已应用，不表示布局已稳定或像素已显示。

动作先查身份，再检查参数/语义目标和宿主 capability，之后才能入队。因此 foreign/retired 目标不会被“不支持 focus”掩盖；错误动作不会污染待提交队列。动作在前序 mutations 后、查询屏障前运行，暂停/销毁竞争仍由宿主和 App 提交规则处理。

Focus 是请求，不保证目标获得焦点。入队后发生 CSS/布局/启用状态变化，adapter 可返回 Unsupported；App 将这项焦点请求视为未达成，继续后面的动作，且不会 fault 或伪造焦点事件。其他 Backend 错误仍 fault；不能把任意动作错误一律吞掉。直接 adapter.action 调用者仍能观察它返回的拒绝。

通过 `App::request(Query::Focus(id), reply)` 确认实际结果：先提交，接受时捕获 `QueryValue::Focus(bool)` 和 applied，再异步交付；后续聚焦不能改写该结果。Group/失去连接的范围 Unsupported；有物理控制但没有焦点是 false。Check 读取内控件，祖先的 FocusWithin 不等于祖先 control 自身获得焦点。Web 使用 control 实际 Document/ShadowRoot.activeElement；Blitz 在解析当前样式/布局后同步并比较 control。bool 不证明 OS 窗口激活、IME 可用或帧已显示。

Focus 动作不会推进 mutation 的 applied；同一水位可以先后有不同焦点。每个 token 对应接受时捕获的结果，不能把 applied 当作焦点版本，或到 drain 时才重新读取最新焦点。

## 错误与验证范围

无效 UI handler 注册及已知不支持的动作目标可立即失败，App 仍可正常挂载/编辑/清理。查询的有效身份不等于几何可用：Group 不返回一段 DOM Range 的聚合 bounds；Web、Blitz、SSR 与参考端口都明确拒绝该物理目标。

mutation batch 允许已应用前缀，不承诺回滚整个 batch；物理 sink 失败后 App fault，不能继续假设模型与宿主一致。描述符预检失败不会先改 UI 模型；HTML 的 Place/Resume 内容检查发生在 mapper 模型与 sink 变更前。直接 apply 若接受前缀后遇到拒绝，SSR/Web/Blitz 保留 mapper 最后成功水位，查询对应这个前缀；这不是整个 batch 原子性。

共用的 `glory-render/conformance` 是可选测试工具，默认不编入生产依赖。当前 verify 覆盖八目标、11类 interest、32个查询、异步响应/水位、错误坐标、Dispose/foreign 身份；由 Memory、SSR、实际 Blitz 文档与交互 WindowApp 运行同一个函数。Focus/Selection 根据宿主声明的 focus/input 能力验证正向值或 Unsupported，不再只验证无输入后端。矩阵显式建立 display:block/64×24px 盒子；不能据此保证所有空 inline 节点都可测。

浏览器另通过 target-acceptance.js 验证实际七个元素加一个无 wrapper Group、32个异步查询、Group Unsupported、四项描述符及两项分组子节点拒绝、合法同 tag 更新，以及拒绝期间的同一输入元素/焦点/选区保留和后续编辑。SSR/Blitz 的 preflight 测试覆盖未提交祖先、错误选项/子节点不分配身份、结构拒绝、失败前缀水位和恢复；新增 Password 验证创建/用途切换保留 control 与 raw value。UI 另覆盖父节点销毁/slot复用与 Place/Resume 拒绝后的恢复，HTML 覆盖实际 Group 移动/恢复。实际 DOM/合成协议证据不能代替 OS/辅助技术验收。

event-acceptance.js另验证13种错误输入、真实slot复用后的旧来源、负偏移保留及错误后的查询/编辑/卸载。render events测试覆盖数值与来源矩阵、暂停终止清理；UI events测试以实际drain批次证明错误事件后仍完成有效编辑和查询，并覆盖直接焦点handler删除来源。一亿行惰性列表专项确认错误消息不增加key读取/节点/动作，随后30次远跳仍有界。这些不是新增FPS或耗时测量。

input-acceptance.js验证七种HTML purpose及多行控件、实际选区、显式100→绑定101→编辑102、旧/未来编辑与Commit、NUL修复、CRLF等价组合回写、错误声明后恢复。inputs.html由原生ssr_fixture生成；制造后续节点不匹配证明原始NUL/选区未被失败接管改写，再重试保持同一元素、焦点、选区与规范dirty值。原生WindowApp另验证Unicode/CRLF/NUL、UTF-8 IME游标、UTF-16查询与显式/绑定版本混用。

focus-result-acceptance.js 验证四种宿主状态（display:none、visibility:hidden、inert、opacity:0）的焦点真假、普通 text/image 的请求未实现、programmatic View、Check 内控件、异步结果不被后续焦点改变、preventScroll、Shadow DOM 根回读、禁用/连接恢复与卸载。WindowApp 验证 CSS 更新后焦点查询、同帧禁用、拒绝后继续后续焦点与编辑；UI 测试还验证 Backend 错误仍 fault。一亿行两种高度模式的30次离屏跳转中，焦点查询不增加 key 读取。

原生 password 像素测试比较八个圆点与同字素簇数量的拉丁文、中文、组合字符、家庭 emoji，要求逐像素相同；正常文本作正向差异控制。另覆盖同节点 reveal/再次受控修改、引擎实际指针点击/双击、原值编辑、组合输入切换、DPI/窄框滚动/IME 区域以及清理。引擎用例覆盖 Hebrew、Arabic ligature、Indic、ZWJ、字素删除、剪贴板与 Apple 命令；一亿行焦点用例扩为两种高度 × 普通/密码编辑器，实窗变高列表也采用 Password。它们不是操作系统密码输入策略或全部平台控件的完成证明。

第十六组 semantics-acceptance.js 用相同 Rust form 覆盖 CSR 与真实 SSR 接管，保留预输入并读取 Chromium 实际 AX 树：16个初始/更新角色名称、Clear 后15个节点的回退结果；Keep 没有 DOM 写入，元数据不重写 value，身份/双向选区/组合协议保持。真实 Tab 验证显式 programmatic 与 Clear 后默认策略；每种入口拒绝15项错误并继续合法命名与键入。原生 WindowApp 同样保留编辑值/选区/preedit、物理身份，并验证元数据清除与 Tab 恢复；它没有运行 OS 辅助技术。

复现：`cargo test -p glory-render --features conformance --test targets`、`cargo test -p glory-ssr --test targets`、`cargo test -p glory-blitz --test targets`、`cargo test -p glory-ui --test targets`、`cargo test -p glory-html --test content`；完整 gate 及浏览器函数见实施记录。

## 可选性能观测边界

`render::profile` 固定保存 31 个阶段聚合值，原有 18 个阶段序号不变；新增
13 个原生引擎阶段。Blitz 的 Layout 包含 style、damage、construct、
post_construct、box_layout、transform、paint_tree、layout_cleanup、hover、
subdocuments；TextShape/Break/Align 是其中的嵌套实际工作。elapsed 为包含
子阶段的时间，不得相加推导帧时间。并行文本累计值可能超过墙钟时间。
TextShape 的单位是 inline builder.build_into 完成后的文本 UTF-8 字节数，既不是字形数，
也不涵盖 builder 前的文本收集、字体准备、编辑器/密码投影或 marker 更新。
其余单位通常为阶段调用次数，不表示遍历节点或缓存未命中数。TextBreak 的
float 分支还包含其浮动流协调；不能解释为纯 Parley 算法时间。

`Profile::recording()` 捕获当前代次；`Recording::record` 只在该代次仍有效时
导入外部固定聚合，并拒绝 max 大于 elapsed、错误时钟次数大于调用次数及
零调用却含非零数据的样本。导入不调用 Profile 时钟，Glory collector 仍为
本地对象。reset 退休旧 span/recording；引擎 reset 在锁内原子退休并行 span。
None 避免计时器分配、锁和时钟读取；挂载、移除、读取及 reset 不触发布局或
重绘。干净帧仍可记录真正执行的 Observe/Resources/Flush，但不得捏造引擎工作。
同 RuntimeId 的两个文档也独立计数。浏览器不暴露这些内部引擎阶段，值保持零。
此观测不包含窗口显示完成、OS 合成等待或 FPS/RSS；resolve 之外的直接引擎
操作和资源读取也不能归入这些细分计时。

瞬时ScrollTo在布局已解析后立即更新引擎offset和geometry generation；仅
像素变化时推进visual revision并保留scroll ack/观察，不强制再跑样式或盒布局。
移动内容后立即refresh_hover，命中改变则仍完整失效；Hover计时另包含这次
调用。资源、动画、CSS/viewport及语义Patch失效保持原契约，不以“滚动通常
不改尺寸”跳过它们。查询仍看到当前坐标，清洁复用不能吞掉实际像素更新。

## 原生 inline profile 与实际滚动

Blitz 使用独立 STYLE_PROFILE（blitz v4，Css::Html），类型化 Position::Sticky、scroll_anchor(true) 返回带 property/value 的 UnsupportedStyle。当前固定引擎缺少相应 sticky 偏移/自动锚定行为；显式 scroll_anchor(false) 继续接受，Glory 虚拟列表使用自己的锚点补偿。Browser/SSR 的 HTML profile 保持该滚动范围；存储型 RECORD 不提供几何承诺。文字/装饰域扩展后，HTML/RECORD profile 为v3，Blitz为v4，独立于HTML格式v3。

创建、Style Set 在 App/显式 preflight/direct apply 三条入口检查，拒绝不改变拟议模型、命令、物理节点、布局或控件值/焦点。direct apply 逐操作保留已经成功的前缀，后续查询使用该水位，随后合法更新可恢复；错误节点身份先于该节点的样式支持检查。Keep/Clear 与无关字段不重验旧 Style，避免重新扫描大文本/数据源。

原生实际几何回归另外验证 relative 偏移及 Visible/Hidden/Clip/Auto/Scroll 的双轴程序滚动：Visible/Clip 的 offset 保持0，其余三种允许内容在盒内滚动。原始 set_css 属于引擎 CSS，仍可包含引擎不实现的声明；它不是 Portable stylesheet 验证器，也不扩大类型化支持范围。七项回归与原始布局诊断见实施记录。尚未对所有属性、CSS 组合或像素完成审计。

## 最大尺寸的三态与布局边界

width/height/min_width/min_height 使用 Length，max_width/max_height 使用 Limit::{None,Px,Percent}。最大尺寸不能传入 Length::Auto；compile-fail用例固定这个类型边界。字段Option::None不产生inline声明；Some(Limit::None)显式输出max-*:none，覆盖样式表的上限。Style Clear删除所有inline声明而恢复class规则；Keep不写style。数值必须有限、非负，min大于max仍合法，由布局引擎执行min优先。

Style/Support检查、HTML序列化、差分Patch lowering、Web/SSR接管与Blitz布局使用同一域。profile沿用现有None/Px/Percent有限值，不新增运行时/引擎/serde依赖。实际Blitz/Chromium验证取消上限、像素/百分比、min优先、Clear与非法值恢复；同一Rust form的SSR接管保留预输入、控件身份、焦点和双向选区。百分比测试使用尺寸确定的包含块；不借此承诺全部不确定高度/内在尺寸组合。

## 存活字段的验证、lowering 与提交

Memory 中的描述符在创建时已规范化/验证；`prepare_patch` 只替换拟议字段，保留旧字段作逆向值，检查受影响的载荷及跨字段/祖先约束。未触及的 value/content、classes、style 不因其他字段更新而复制或重新扫描。返回的只读 guard 可供 adapter lowering，Drop/展开恢复字段而不推进 stats/水位，commit 接受恰好一项修改；其排他借用防止准备期间删除或并行修改该树。原始 Descriptor::patch/patches 仍执行完整构建期验证。

HTML Patch 使用该字段及逆向值生成 edits；create/snapshot/hydration 继续完整 shaping。1,232组差分对照覆盖18类 Patch 的 Keep/Set/Clear、规范化、命名/角色、类型/属性、revision 与结构拒绝；430接受项与原完整 Shape 差分相同，802拒绝项保持模型/输出/stats/水位。另有大载荷缓冲保持、错误 revision/祖先内容恢复、Drop/展开及 commit 水位测试。它们与真实浏览器/原生编辑回归共同验证，不能单凭该 oracle 推导所有引擎像素/无障碍能力。

准备阶段 rollback 不扩展到 host sink；sink 在接受后执行，仍保留成功前缀与 fault 约定。Keep 即使没有物理 edits 仍是一项已接受 mutation，水位推进规则不变；空 apply 继续保持水位。当前优化不改变公共 Patch 数据形状，也不加入节点访问历史缓存。

## 响应传播与读取边界

Cage::map 借用源对象，不要求源T: Clone；`get()`仍是显式拥有值的克隆，indexed大型数据继续使用with/map的借用路径。Bond可选相等门控保持：来源变化需要验证，但输出相等可以不执行后续Binding。未配置比较器的派生值继续遵守原有发布规则，不增加全局PartialEq要求。

同一未刷新的失效前沿只传播一次，源发布本身仍逐次执行、递增版本和source_writes。求值开始重开当前/旧依赖前沿，依赖登记、完成及展开重试使后来的写入重新可达；不以节点处于Check/Dirty就永久剪枝。批次中途读取可取得当前值，刷新过的路径随后仍接收新写入，普通绑定不因此在批次中间执行。

RuntimeStats::marked现在记录实际传播到消费者的工作；重复写入被前沿合并后，不能用它反推业务写入数，业务发布计数继续看source_writes。标记属于存活代际节点，无按访问行/每次修改积累的缓存。新订阅、动态分支、Effect回写、异常重试与销毁/复用有独立回归；这一优化不代表Cage内部字段级依赖已经完成。

## 可选原生 AccessKit 桥

glory-blitz/accesskit（门面blitz-accesskit）在桌面shell中读取已提交的语义树与实际引擎几何。Group透明，装饰图像不导出；Input值和Check状态取实际控件，Password只导出保护角色，不读/缓存原值。Button/Link的标签文本作为后代参与名称计算。AccessKit位置/level为零基；已知一致的item.count放最近List容器，未知/不一致则省略总数。无障碍导出不调用业务source/key/locate。

每文档单调分配可访问ID，不使用可复用的物理slot。只保留存活映射与上一份树，变化时提交差异；停用客户端释放快照但不重置ID计数。平台回调仅投递WindowId+RuntimeId消息，UI循环检查当前身份、连接、显示与启用状态，忽略过期或不支持的请求。支持Focus/Click/字符串SetValue；不宣称TextSelection/TextPattern或全部滚动接口。现有字段更新、受控revision和业务事件路径保持。

Windows实际UIA客户端已对两个窗口验证Invoke/Toggle/Focus/emoji SetValue、禁用拒绝、密码保护、几何、有界列表及关闭回收。定高/变高导出与唯一离屏焦点行另由实际AccessKit consumer验证，不增加源读取。Linux普通/AccessKit实窗不是AT-SPI客户端或屏幕阅读器证据；这些及播报、完整文本接口、系统输入法、Android仍分别验收。

## 已挂载虚拟行的 Reveal

Navigation::Reveal 以事件原点所在的已挂载行作为目标，不移动键盘焦点；只需要scroll能力。Root、空列表或无法解析到当前行的原点没有目标，不能退化成首行；同批次Last/Next后面的Reveal不能复用临时导航游标。已有普通导航继续要求focus+scroll。稳定key、indexed locate、稀疏测量、物理分段与有界焦点pin由同一ui::list控制器维护。

AccessKit仅在具有有效Root/Navigate的虚拟Row wrapper广告ScrollIntoView，None或ScrollHint进入Reveal；其他data拒绝。平台回调只排队，提交逻辑行位置后才按实际物理几何显示原生祖先和文档视口；nearest保持边框与client大小，overflow由引擎执行。重复目标合并，退休/隐藏/停用清理，实际滚动安排后续观察。没有任意源行的物化/检索、百分比滚动或全量列表AX树。

Windows UIA的ScrollItemPattern已实际请求第九千万行附近的离屏pin回到视口，并核对原身份、系统焦点、emoji草稿与几何；算法/consumer测试覆盖定高/变高、有界读取和嵌套祖先。屏幕阅读器、AT-SPI客户端、文本选区仍不由此证明。

## 确定尺寸 flex 的实际引擎契约

grow/shrink允许合法非负小数。固定gap计入初始已用空间一次；未冻结因子之和小于1时，只按该和缩放initial free space，不能再减gap。当前原生Taffy的两个分支已修正，finite style/profile域没有因此收窄或改版本。布局执行权仍属于Blitz，html只输出声明，不做后置几何补偿。

可选render/conformance的46项FlexCase共用语义描述符与预期局部矩形，覆盖双轴六种justify、四种align、零/小数/整grow/shrink以及五种overflow的自动最小尺寸。原生创建/36次存活样式修改与浏览器CSR/SSR接管共用同一数据。Taffy直接回归另覆盖不等因子/冻结项/三子项/gap/margin。该范围不证明不确定/内在尺寸、Grid或完整CSS/文字能力；跨引擎像素完全一致仍不是Glory的承诺。

## 动态属性的字段依赖

`Cage::field(name, get, get_mut)` 返回同一模型中固定内联字段的 Cage；Builder 继续接收 Cage/Bond/Lotus，不增加后端属性绑定类型。字段写入通知整模型、祖先及该路径所有别名/子路径；无关兄弟依赖不求值。根写入保守通知全部存活字段，值相等性仍由可选 Bond 门控。

路径按根模型/父字段/名称共享，每个调用拥有自己的 Owner 别名。当前构建 Owner 结束只清理该别名，路径在最后别名和子路径释放后回收；根模型结束使独立 Owner 中的别名也失效。不存在访问过的行或堆上元素的历史索引。字段投影捕获与模型值均在图借用之外析构。

访问器必须纯且固定位置；框架检查越界、相同名称/类型/位置、兄弟存储重叠、可变位置不一致与最大深度128，拒绝访问器内部的响应读写。普通业务闭包的根 RefCell 借用规则保持。位置稳定的结构体/元组/固定数组受支持；完整 String/Vec 可作为字段，不能通过此接口越出父对象选择堆元素；Vec/Map 独立路径见下节。Deref及动态枚举仍不支持。

普通 Cage 的写入/发布 guard 独立于字段投影路径；没有注册字段的源不创建投影或收集别名。这一边界也适用于 Runtime 内其他模型已经注册字段的情况。

## 集合位置与完整 key

- `Cage<Vec<T>>::item(index)` 跟随位置，扩容时重新解析地址，重排后选中该位置的新元素。稳定实体使用业务 key 与独立顺序模型。
- `Cage<BTreeMap<K,V>>::entry(key)` / `Cage<HashMap<K,V,S>>::entry(key)` 按完整 key 解析当前值，保留原集合的比较/hasher。不要求 K/V Clone；BTreeMap key 额外要求 Hash 以建立活跃路径索引。
- 每种都有try构造方法。缺失位置/key可以注册；try读写返回MissingField，读仍订阅，写不执行业务回调或发布。删除后已有句柄仍属于组件，重插同key恢复；若代表新实体则业务key应含代次。首次注册内联field需要父元素存在，已有field在缺失/重插后恢复。
- 根/集合revise保守通知全部存活子路径；单项写入通知祖先/同路径别名/后代，兄弟不求值。没有不同字段并发可变借用，也没有隐式全集合事务。根可变借用期间禁止注册新路径，保持写入前采集的通知集合有效。
- 内联和集合路径不能是重叠兄弟；先选共同父字段。动态key注册不扫描源或其他活跃兄弟，哈希碰撞仅比较该桶内完整类型化key。用户Hash/Eq在图和注册表借用外运行；源集合Ord/Hash/Eq在图借用外运行。key方法必须纯，key析构在所有相关借用外运行。
- key索引仅保存活跃路径的弱桶，最后路径离开时清理key与空桶；Root释放也使外部Owner别名失效。不缓存访问历史或删除的业务值。RuntimeStats::entry_keys/entry_buckets提供活跃注册量，Vec位置只计入fields，不另建key桶。

Deref、任意自定义容器/动态枚举仍为后续范围；内建集合的结构编辑见下节，不能推导并发借用或任意容器支持。

## 集合结构编辑

这些都是Cage上的普通Rust方法，不构造新的item/entry句柄，也不克隆集合、业务key或value。

| 容器/操作 | 发布范围（另含根、集合及祖先别名） | 返回值 |
|---|---|---|
| Map `insert(key,value)` | 该key的存活别名与全部后代 | 原value或None；保留原已存在的key |
| Map `remove(&key)` | 该key及其后代；不存在时无发布 | 原value或None |
| Vec `push(value)` | 原len位置 | () |
| Vec `pop()` | 原len-1；空时无发布 | Option<T> |
| Vec `insert(index,value)` | index至原len（含新末尾） | () |
| Vec `remove(index)` | index至原len-1（含现在缺失的末尾） | T |
| Vec `truncate(len)` / `clear()` | 实际移除的位置；不缩短时无发布 | () |

均有try_形式，错误为Result<上述返回值,AccessError>：已释放/借用冲突/缺失父集合仍用既有错误，非法Vec位置用OutOfBounds，不可表示的Vec长度增长用Capacity。它们不把分配失败或用户panic变成AccessError。无操作不增加版本/source_writes、取目标缓冲或唤醒Host；相同value替换不隐含PartialEq门控。

Map仅按活跃注册表中的完整类型化key匹配；通常单候选不分配候选Vec，真实碰撞才取其余候选。Hash/Eq/Ord/hasher在图与注册表借用外运行，必须纯。重复传入key、被移除的存储key在正常完成时延后到模型借用及发布结束再析构。Vec使用已注册位置BTree范围，不逐一遍历业务数据或无关位置；底层Vec本身的移位/析构成本仍按标准库执行。

版本与目标检查在实际编辑前完成，panic后的部分修改沿用既有保守发布。目标仍只复用每根的一份缓冲，额外选择器为父路径、内部key身份或位置范围，不保存业务key/value、地址、订阅者、旧版本或访问历史。成员变更撤销guard返回许可；整体revise和每项修改之间切换仍须按当前拓扑重收集。

集合选择参数走独立准备入口，不进入普通字段写入的参数/guard。每根缓存槽至多有一个可复用的数字选择器Box；从集合编辑切换到普通字段时可保留它，类型标签阻止把旧集合目标误作命中。它和目标槽一起随拓扑/Owner失效回收，不按操作或key积累；无操作及版本预检失败也不创建它。

字段/item/entry句柄应在相应Owner构造期保存并Copy使用；显式增删接口不需要为写入临时建立这些路径。大量批内修改或影响整段位置时，单次revise可比多个细粒度调用便宜；不存在全部场景的速度保证。

## 通知目标缓冲的边界

同一canonical路径在一个Owner内只有一个别名；不同Owner的别名仍独立退休。
重复field/item/entry选择在验证访问、类型和layout后复用节点，不改变拓扑或
通知缓存。未采用的getter/setter captures和重复key仍在图/注册表借用外析构。
缺失位置/key按原逻辑订阅并在重插后恢复；复用不绕过pure/借用/Disposed检查。

每个有字段写入的根最多一个目标，只有代际节点身份和可复用遍历缓冲。命中不重建身份集合；失配复用容量，但仍按当前成员重收集。每次写入仍检查全部当前版本并推进源/相关别名，通知读取当前订阅者和前沿状态，不合并版本或缓存数据/元素地址。

投影、借用及版本检查在改动缓存/计数之前通过。guard独占缓冲期间，根中仅留可撤销槽；新增/释放别名、路径或根会删除该槽，旧guard不得放回失效缓冲。同根可变借用阻止新路径注册和嵌套写入，确保槽不会在该guard结束前重建。其他根不受影响。失败访问保持原缓存/统计，部分业务panic仍按既有规则发布；Slot和缓冲只持内部身份，析构无用户回调。

Root成员变化释放缓冲；容量由当前拓扑/别名界定，不按写过或访问过的字段逐项缓存。RuntimeStats::field_target_builds/hits/visits是准备次数与收集身份数，field_target_caches/keys包含guard借用期间的活跃槽/身份；不是完成写入确认或内存字节。集合、Owner和输入语义不因此扩展。

## 注册 Owner 的执行上下文

Runtime::enter(instance, callback)仅接受本Runtime仍存活的代际InstanceId，先
检查再执行；外部id用WrongRuntime、退休id用Disposed。执行时不持图借用，
不延长Owner，不自动flush，嵌套返回或panic后恢复原上下文。该入口只覆盖同步
回调；future的创建/返回不传递上下文，已有owned task poll/TaskToken.commit
继续各自校验和进入Owner。

UI每次送达事件先验证目标、callback身份和当前树，复用该次live检查读取
Subscription.owner，经Runtime.enter执行。归属为注册监听器的Scope，允许该
监听器观察另一个Scope的节点；不会按事件origin/DOM父节点猜测Owner。Scope
在cleanup期间已退休时也不得借助还没清理的callback快照运行用户代码。

实际Binding/Derived/Effect/Structure回调统一进入自身节点的Owner，与调用flush
的Owner分开。依赖验证或旧值析构退休了当前observer时，捕获的旧回调不执行。
纯计算的创建/写入限制保持，执行上下文不授予额外权限。字段/派生值的寿命
由Owner控制；重复map仍创建不同计算，Effect重复运行不会自动清理本Owner
先前显式创建的不同路径。短期跨key修改优先用无路径注册的集合编辑或短Owner。
cleanup及应用级query回调现在按下面的明确契约执行；任意用户值析构、动态投影
和事务等其他边界仍继续审计，不以这些回调规则宣称全部完成。

## 清理回调与查询请求的归属

调度队列取消会立即移除活成员资格，等待中的closure/值随原生命周期清理，
不会由FIFO里的旧数字ID保活。已退休的完整代次Key可短暂留在物理队列，
总entry数始终不超过`2 * queued + 64`，queued归零时entry也归零。取消和
取出活观察者后均压缩检查，回调重入能看到这个界限；保留Structure→Binding→
Effect优先级及每阶段活FIFO顺序。复用slot必须携带新代次，不得执行旧ID。

普通source/derived不查调度队列，没有废弃ID时不做压缩计算。压缩只检查数字
身份，不执行业务访问器/值析构；达到阈值才线性检查，避免每删一行重复扫描
整个页面的待更新绑定。RuntimeStats的queued为活成员，queue_entries含有限
废弃ID，queue_capacity为实际FIFO容量，后者与arena一样可保留峰值分配。
queue_compactions/queue_compaction_visits计实际压缩次数/非空扫描身份数，
不表示RSS、业务数据量或呈现时间。这里不增加业务行、通知目标或渲染树缓存。

`Owner::on_cleanup`在自身及后代身份全部退休后执行，无图借用、untracked，
顺序为子Owner先于父Owner、同Owner逆注册顺序。构造上下文保留已退休Owner，
因此从仍活着的全局Cage隐式创建field/item/entry/map不能逃逸到卸载调用者或
源Owner。可以读写仍活着的其他状态，或显式进入另一个活Owner构造资源。
不解除Pure限制，不自动flush。Task取消回调同样untracked并进入任务Owner：
显式取消时Owner可以仍活着，Owner退出时则已退休。回调panic正常展开并恢复
上下文，不保证剩余回调执行或自动重试；这不是任意用户值Drop的上下文保证。

`App::request`是应用根所有的查询；`Scope::request`是调用该方法的Scope所有，
不由目标节点、当前Effect或pump/flush调用者决定。响应回调untracked进入登记
Owner，里面构造的响应式资源也随这个Owner退出。请求前必须flush，且屏障
可能退休请求者，因此接受前再次验证登记身份。失败不发送请求，不留下索引。

请求者退出时直接移除其所有等待项并释放捕获，不执行用户回调，即使目标还
活着。目标退出而请求者仍活着时，在flush交付一次`NodeGone`。如果两者都退
休，则不交付；待取消队列只保留token，不保留回调。查询表同时维护target和
Owner索引，完成/取消移除空桶，晚到、重复和旧代回复均无效。目标取消优先于
同一已drain批次中稍后的成功回复，applied校验和结果类型/数值校验保持。

```rust,ignore
// 行/组件退出就取消等待。target可以属于另一个长生命周期组件。
scope.request(Query::Bounds(target), move |reply| {
    // 此处的map/field资源属于scope。
    update_measurement(reply);
})?;
// 需要比组件活得更久的请求，显式使用app.request。
```

注册表只保存非拥有的InstanceId，不延长Scope。捕获析构和回复均在图、UI树及
renderer借用之外。UiStats的queries/query_owners/query_targets分别计活等待项
和非空索引桶，不表示引擎内响应队列/RSS；宿主仍需pump排空已接受的晚到回复。

## 窗口化列表的顺序协调

Cache 的位置元数据只属于活跃行，行销毁时一并回收。下一窗口在源 key 验证期间保存旧顺序，重复 key/无效 pin 在任何行刷新、销毁或放置之前拒绝。保留行相对顺序递增时不计算 LIS，不移动保留行；发生逆序时仍用 LIS 选择最少移动子序列。源 index 与窗口 order 分开，焦点 pin 与普通行进入同一次协调。

写回 order 合并到已有 mount/update 遍历，不克隆整窗旧 key 建位置表；不保留访问历史。窗口、revision 与 pin 均相同时保持零 key 读取快捷路径。它不提供数据源排序、结构性集合增删发布、主机回调 panic 回滚或更宽的行预算保证。

## 受限样式表与 HTML 格式 v3

可选`css`提供`glory_style::Sheet`：现有33类类型化属性、class组合、子代/后代关系和hover/focus/enabled/disabled/checked。保留声明和规则顺序，不接受未知属性、任意函数/单位、at-rule、important、物理标签/id/结构选择器。每个compound必须有class。数字域复用Style验证；目标adapter逐条检查profile，不能用后续覆盖来隐藏不支持声明。

默认预算为1MiB输入、4,096规则、8,192选择器、65,536项、32,768声明，单选择器硬上限128项。错误给出一基行和UTF-8字节列。公开AST只读；HTML编译器对每个compound增加零specificity的主节点guard，Group保持透明，内部控件/文字part不参与语义选择器。identifier中的`<`使用CSS十六进制转义，可用于style raw-text而不做HTML实体转义。

HTML格式升至3：enabled/disabled/checked使用Main上的语义状态属性，Check的原生disabled/checked property仍在Control；full shape和字段Patch同步。focus编译为自身焦点或仅Check的focus-within；不会把任意聚焦后代算成View自身焦点。Blitz已补focus-within的祖先状态、移动和清理，原生协议验收不等于OS输入法/读屏验收。

SSR Document的可选sheets按顺序输出并使用nonce；Blitz set_sheet先完成目标检查再替换author CSS，失败不改文档。Browser使用相同编译资产，实际级联/继承/几何仍由引擎拥有；默认core/ui/HTML图不引入CSS parser。颜色支持hex和有限关键词；gap/边距/字号/行高为px，尺寸/偏移按当前类型支持px/%/auto。文字与装饰域见下节；主题token、更多语法及非HTML resolver仍未完成，不能把受限Sheet当作原提案的全部Portable CSS。

## 字体、换行与装饰声明

`Style::font_family`接收有序`Family`列表：Named(String)与Serif/Sans/Mono/System/Cursive/Fantasy区分。空列表、空名和NUL无效；宿主负责字体解析和缺字回退。Named在HTML/CSS中恒为字符串，转义引号、反斜杠、控制符和`<`，防止声明或style raw-text边界被字体名改变。Sheet接受字符串、多词family及这六种generic；其他generic、CSS-wide关键字不在有限语法内。

`font_weight`为有限1–1000（允许450.5）；文本另接受normal/bold，映射400/700。WhiteSpace::Normal/NoWrap/Pre/PreWrap对应normal/nowrap/pre/pre-wrap，控制折叠/换行，不提供word-break。引擎必须在仅wrap mode改变时失效其已收集文本数据；不以单纯重新布局代替重新收集。

`border_width(Edges)`为一到四边非负px，border_style只接受None/Solid，border_color统一四边颜色；三者独立，设置一项不能隐式修改其他两项。`radius`为单一非负px圆角；`opacity`为有限0–1整体合成透明度（含子节点），不改几何。字段None不生成声明；撤销inline值恢复样式表，不写入替代默认值。文本域与Rust值共用验证和支持表，HTML/RECORD v3、Blitz v4；HTML映射格式仍v3。

文字导致行高改变沿用List的有界Resize反馈和锚点补偿，不增加全数据预排版、逐历史行字体缓存或第二布局树。运行时仍只持有当前窗口及有限测量记录。主题token、完整CSS、工具包resolver、实际OS IME/屏幕阅读器不由上述声明获得完成保证。

## 浮层语义与宿主范围（2026-10-23）

`Semantics::layer` 在具名、可程序聚焦的 `Role::Dialog` View 上声明有限浮层，
`hidden` 决定其是否进入栈。Layer 包含 modal、Escape/外部关闭策略、有限位置、
anchor/initial_focus/return_focus 符号关联。锚定位置必须有不同于自身的 anchor；
其他位置不接受 anchor。`has_popup` 仅属于非 Tab Button；described_by 补充描述关联。
字段事务继续验证角色、focus 和 metadata 的交叉约束并在失败时回滚；HTML 的字段
差分 oracle 覆盖新状态的 Set/Clear/Keep，不改变原生 Input 值或选择。

`Event::Dismiss(DismissReason::{Escape,Outside})` 是关闭请求，业务可拒绝；只接受
活跃且允许对应关闭方式的浮层。原生监听器只入队，不调用业务代码。WebApp 内共享
栈处理最上层，manual popover top layer 保留原有逻辑/DOM 祖先，modal 对外部兄弟
分支施加 inert 并约束实际焦点。显式初始焦点失败则选择首个可用 Tab stop，再落到
panel；关闭时恢复存活目标，非模态外部点击产生的新焦点不被覆盖。关闭/卸载清理
宿主引入的 inert 与 top-layer 状态；组合输入不触发 Escape 关闭。

Web 在构造时检测 top-layer API，并在不支持时于描述符预检及逐操作 apply 拒绝；
支持时保留常规批量 mapper 路径。viewport scroll/resize 仅标记并唤醒，布局读在
宿主观测边界合并。SSR 生成具名但未激活的浮层，成功 hydration 后才展示并管理
焦点；失败验证不激活浮层。现有 HTML edits/stream 格式不变，WASM 接管前不提供
浮层交互。当前每个 WebApp 管理自己的栈，多 App 文档应指定一个浮层所属 App；
跨 App 栈、移动 visual viewport、动画与手势关闭仍未实现。

Blitz beta.4 没有 top layer 的物理实现，因此在 preflight 和 direct apply 中明确
拒绝 Layer；逐操作拒绝仍保留成功前缀的 receipt/query 水位。该范围不等于新增原生
浮层、实际 OS IME 或屏幕阅读器验收。Sapid 的三个浮层组件共用这些语义，浏览器
验收与复现入口在同级 sapid 的 scripts/check.ps1 和 overlay-acceptance.js。

## 字段与原生标签（2026-10-24）

`Semantics::field_part` 提供有限的 Label、Fieldset、Legend；它们只属于无显式 role、
无 focus 的结构 View。HTML 对应真实 label/fieldset/legend，节点生命周期内不可通过
metadata 改变结构标签。Legend 必须直接属于 Fieldset；Label/Legend 不接受交互后代，
因此没有首个 legend 内控件绕过组禁用的例外。`.field_part(...)` 在创建描述符时指定结构，
不能先创建普通 View 再用响应绑定改标签。普通 Group 仍无盒子或表单角色。

`label_for` 属于 Label，指向稳定 SemanticId；`labelled_by`、`described_by` 和
`error_message` 分别表示名称、帮助和错误。符号可前向引用，适配器只解析存活目标。
Check 的 ID/命名/状态映射到实际 input，而非包裹 label，外部标签能够原生切换复选框。
错误关联只在 invalid 时输出，同时追加到 aria-describedby，保留已有帮助描述。

`required`、`readonly`、`invalid` 仅接受 Input/Check。required 是状态而非跨宿主校验器；
readonly 保留焦点、选择和程序写入，丢弃用户 ValueChanged/Commit/CheckedChanged，
但仍先验证事件负载及输入 revision。三个状态的补丁都不写控件 value/checked，也不重置
selection/preedit。HTML 字段差分与完整 shape oracle 共同覆盖状态设置、移除及关联。

Fieldset 使用 `Semantics::disabled` 禁用后代；`Memory::enabled` 仅沿实际祖先检查，
无需扫描业务数据或保存访问历史。共享事件路由、Focus action 和原生键盘/指针/AX 行为
遵守组禁用，恢复组状态不覆盖子控件自己的 enabled。Web 由真实 disabled fieldset 与
readonly input 配合宿主事件保护；readonly Check 在默认切换前阻止点击。Blitz shell
实现标签目标聚焦/切换及只读编辑保护，AccessKit 输出 required/read_only/invalid、
有效的标签/帮助/错误关系和继承禁用状态。SSR 和 stream 保留相同属性，浏览器采用前的
原生字段行为不依赖 WASM。纯 SSR 不宣称输入能力，原生字段支持不扩大 Layer 的支持范围。

验证入口：render/tests/fields.rs、html 的字段差分 oracle、Blitz command/accessibility
字段测试，以及同级 Sapid 的 tests/fields.rs 和 examples/web/field-acceptance.js。
浏览器 AX 协议与无窗口 AccessKit consumer 检查不等于真实屏幕阅读器或 OS IME 验收。

## R0 尚未收口

- 全部 props/后端扩展的错误分类与恢复范围；描述符局部检查与借用 staged tree 的父子预检已接通，当前保留 partial-prefix/fault 契约。
- 当前18类字段的语义/HTML差分矩阵已有上述证据；继续审计实际引擎样式/字段行为与扩展，不能以字段 oracle 代替完整 CSS/像素或平台辅助技术验收。
- 受限Sheet核心及上述文字/装饰域已有实现；更广文字/视觉语法、主题与非HTML解析/级联后端继续推进。完整CSS、RTL、transforms/writing modes不因采用Blitz就成为跨后端保证。

实际 OS IME、辅助技术与 Android 设备属于 R3/R5/R6，继续独立验收。上述未完成项不能以本页或绿色基础矩阵勾选完成。
