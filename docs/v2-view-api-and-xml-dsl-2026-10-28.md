> 历史实现记录：文中的 Component/Props/IntoView 接口已于 2026-10-29 被统一的 Widget 契约取代；旧 raw-template AST、Source 和独立编译器也已移除。当前 API 见 [统一 Widget DSL](v2-widget-dsl-unification-2026-10-29.md)。

# v2 手写显示 API 与 XML DSL

日期：2026-10-28。状态：手写 API 与首版 XML 前端均已实现，并完成真实浏览器等价验收。
实现顺序：先定义并实现手写模板／实例契约，再让 XML 前端生成同一 API。

## 手写 API

共享 `Template` 是静态数据。`div`、`span`、`button` 是显示数据构造函数，不是旧语义 widgets。
`Field`、`Click`、`Outlet` 是具名且分种类的槽句柄，不暴露 DOM 路径或数字槽号。
一个模板内名称唯一；名称只在声明编译／实例配置时解析，字段刷新仍使用数字槽。

```rust,ignore
use glory::display::*;

const COUNT: Field = Field::new("count");
const INCREMENT: Click = Click::new("increment");
static COUNTER: Template = Template::new(&[
    div(&[class("counter")], &[
        span(&[], &[field(COUNT)]),
        button(&[on_click(INCREMENT)], &[text("+")]),
    ]),
]);

let app = App::new(root)?;
let count = app.scope().cage(0);
app.render(
    view(&COUNTER)
        .bind(COUNT, count)
        .on_click(INCREMENT, move || count.revise(|n| *n += 1))
)?;
```

`bind` 接受常量、Cage、Bond、Vary，显示值使用 Display。常量只初始化；响应句柄直接绑定，
不额外生成中间 Bond。`watch(field, move || ...)` 表示纯计算，自动跟踪实际读取的依赖。
绑定按字段安装，沿用原型的独立观察者策略；不因 DSL 自动合并整个区域。

`view(template)` 的配置链是短命 Rust 值；挂载后只保留模板、必要目标与观察者／处理器。
配置先验证槽类型、重复配置、响应句柄的 Runtime／存活状态，再创建实例。
`mount(app, scope)` 返回未挂接实例，供 keyed 协调使用；`App::render` 负责根挂接与首次 flush。
作用域仍决定生命周期，事件闭包不得强持有 App。已有底层数字槽原型保留为测量基线。
mount/place、直接 set 和 Scope 退役会暂存宿主操作，直接操作后调用 App::flush；事件与响应式写入自动安排观察。
根只 render 一次，后续修改 Cage；子实例显式持有其 Scope，丢弃 Mounted 句柄本身不会卸载。
挂载字段和事件不是必填项：未配置字段保留空文本／默认属性，未配置事件不执行处理器，
可后续通过 Mounted 的具名 set/on_click 配置。已经绑定的字段由响应源驱动。

## XML 前端

```rust,ignore
app.render(view! {
    <div class="counter">
        <span>{count}</span>
        <button on:click={move || count.revise(|n| *n += 1)}>"+"</button>
    </div>
})?;
```

字符串文字是静态文本；花括号是 Rust 表达式。Cage/Bond/Vary 保持响应性；
`{move || count.get() * 2}` 是计算；`{count.get()}` 是一次性求值，不猜测任意 Rust 程序的 UI 语义。
已有函数／闭包变量使用 `{computed(read)}`；手写时可直接 `.watch(FIELD, read)`。
花括号也接受普通 Rust 语句块，例如 `{let value = value.clone(); move || value.to_string()}`，
准备捕获的代码只在构造时执行，返回的闭包才是持续计算。
属性同理：`class="constant"` 进入共享模板，`class={class_cage}` 或闭包成为字段槽。
只生成当前显示 profile 支持的标签／属性／click，未知标签和指令给出定位到源码的错误。

`template!` 仅产生共享声明；用 `field:class={FIELD}`、`<field slot={FIELD}/>`、
`on:click={CLICK}`、`children={OUTLET}` 显式声明槽，再使用同一手写配置 API。
`view!` 自动命名动态槽，并生成 `.bind/.watch/.on_click` 配置链。

显式组件使用 `Component::Props` 契约：大写标签调用组件，小写 HTML 不创建组件。
组件在自己的 Scope 中构造状态，返回同一显示配置。嵌套组件使用真实动态边界；
不为了静态标签增加 Scope，也不偷偷恢复旧 Widget/Memory。
首版组件必须自闭合；复杂 children、控制流和编辑器不靠任意字符串或旧 widget 回退实现。

```rust,ignore
struct Counter;
struct CounterProps { initial: i32 }
impl Component for Counter {
    type Props = CounterProps;
    fn view(scope: &Scope, props: Self::Props) -> impl IntoView {
        let count = scope.cage(props.initial);
        view! {
            <div>
                <span>{count}</span>
                <button type="button" on:click={move || count.revise(|n| *n += 1)}>"+"</button>
            </div>
        }
    }
}
// 根组件：app.render(view! { <Counter initial={0}/> })?;
// 嵌套组件：app.render(view! { <div><Counter initial={0}/></div> })?;
```

组件工厂在自己的 Scope 上下文中调用，包含从外部 Cage 创建的隐式 map；派生值不会错误归属给外部源。
根组件复用根 Scope；嵌套组件才增加 Scope、一个实例和必要的动态边界。
普通 Rust 函数可直接返回 Template 节点或 View 配置，不要求所有组合函数都成为 Component。

## 实现细节与边界

- `glory-html::view` 提供纯数据构造与具名句柄；`glory-web::view` 提供泛型配置链，
  `glory::display` 汇总新 API，与原语义 widget prelude 分开。
- 配置链是元组／闭包，不为每个静态标签建立 builder 对象或独立运行时记录。
  编译后的槽表按声明共享，挂载时解析名称；观察者之后按数字槽更新。
- 编译器放在新的可选 `glory-view-macros`；`glory` 开启 `dsl` 才引入它，默认依赖图经检查未包含它。
  宏通过 `$crate` 转发，支持 Cargo 重命名 facade，不要求使用者另行依赖内部 crate。
- `view!` 在编译期提取静态 Rust Template 数据；首次使用仍进行 profile 校验、目标路径编译和宿主原型登记，
  并在 App 内缓存。运行时没有 XML 解析，也不在字段刷新时重新执行整个 view 或组件函数。
- `template!` 的具名句柄由 Rust const 契约检查；`view!` 自动生成槽名。
  错配标签、重复属性、非法内容、未知标签／指令等在宏阶段报错；Props 名称和类型由 Rust 检查。
- Source 是封闭实现，仅接受经过校验的两种声明，外部代码不能提供任意 wire JSON 绕过标签、属性和预算。
- 现有受限 profile 仅支持 div/span/button。动态属性为 class/title/aria-label，事件为无参数 click；
  未声称拥有完整 HTML 属性、事件对象、属性/property 或输入法协议。
- 组件可以单独作为根，或嵌套在 div 内。嵌套使用注释边界，保留真实位置而不增加包装元素；
  目前不支持 span/button 内组件、包含组件的多根顶层片段或组件的子标记内容。
- `children={OUTLET}` 声明空 div 容器；`template!` 还可用 `<outlet slot={OUTLET}/>` 在 div 内声明位置边界。
  列表可复用 Rust keyed 协调；首版未提供 if/for/each 的 XML 语法糖。
- 实例配置的结构／句柄验证在创建前完成；用户计算、组件工厂和宿主提交仍可能失败。
  沿用原型的失败前缀、fault、关闭重建契约，不宣称事务回滚或跨平台编辑器已交付。

## 验收记录

[完整示例](../examples/display-web/src/main.rs)提供手写与 XML 两种构建，
[构建脚本](../scripts/build-display-web.ps1)、[浏览器驱动](../examples/display-web/acceptance.cjs)可直接复现。

本轮相关 crate 的 all-feature Rust 测试 **612 项通过**；Native／WASM 严格 Clippy 通过。
额外 **12 个实际下游编译用例**验证重命名依赖、标签错配、未加引号文本、非法 HTML 上下文、
未知标签、缺失／拼错／错误类型 Props、错误槽句柄、不可显示值、空片段和封闭 Source。
见[诊断驱动](../scripts/check-display-diagnostics.py)。

Chrome 155.0.8059.86 的两个实际页面验证：

- 手写与 XML 输出的完整 DOM、状态快照和截图一致；静态 Element/Text 身份不被更新替换。
- Cage 数值、计算闭包、一次性 get、动态 class、Unicode 与 HTML 字符串按文本显示。
- 条件分支切换后，修改原分支的源不会再运行该计算；独立子组件保持自己的计数状态。
- 同时挂载 600 行后清理，图、实例、目标和 Owner 回到基线；退休节点的事件不再触发。
- 反复创建／退休从外部 Cage 派生数据的组件，确保派生值随子 Scope 清理。
- 等待渲染生命周期并强制四次 GC 后，退休根与 Text 的 WeakRef 消失；关闭后清空根。
- 上一轮 raw-template 的 seeded 万行数据、密度、焦点／选区、memory.grow、四类提交中途失败和 GC 验收继续通过。

[仓库验收汇总](v2-dsl-validation-2026-10-28.json)、
[浏览器证据](D:/BuildCache/glory-dsl-20261028/completed-browser/evidence.json)、
[编译诊断](D:/BuildCache/glory-dsl-20261028/diagnostics.json)、
[Rust 日志](D:/BuildCache/glory-dsl-20261028/tests-verified.txt)、
[WASM Clippy](D:/BuildCache/glory-dsl-20261028/wasm-final.txt)。
本轮没有重新给出框架性能排名；上一轮性能数字属于其冻结原型，不套用到新 API／DSL。

## 参考与验证

Dioxus 0.7.10 的 `BodyNode` 分离 Element／Component，`TemplateBody` 提取静态结构和动态位置。
Leptos 0.8.22 对应的 leptos_macro 0.8.19 使用 rstml 解析 XML；普通表达式和响应闭包语义不同，
`template!` 也不同于普通 `view!` 的返回类型。本实现借鉴这些边界，生成 Glory 自己的模板／字段 API。

参考：[Dioxus RSX](https://dioxuslabs.com/learn/0.7/essentials/ui/rsx/)、
[Leptos 宏源码](https://docs.rs/crate/leptos_macro/0.8.19/source/src/lib.rs)、
[Leptos 动态内容](https://book.leptos.dev/view/01_basic_component.html)。
本地对照使用发布包版本，Leptos 宏归档校验与 benchmark lock 中 checksum 一致；
源码摘要见[源码审计](v2-dsl-source-audit-2026-10-28.json)。不把本地 sibling 仓库的开发版本混入这些结论。
