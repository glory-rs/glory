2026-10-29 更新：组件统一为 `Widget`；两种 Each 也已合并为同一个 API，见
[统一 Each](v2-unified-each-2026-10-29.md)及 [Widget 说明](v2-widget-dsl-unification-2026-10-29.md)。

# v2 keyed 数据、Each 和组件内容

显示模板使用独立命名空间，Widget 和 Each 接口与标准宿主共享。
只写 XML 时启用 `glory/dsl`，使用 `glory::display::prelude::*`；
手写模式启用 `glory/web` 即可，不引入 DSL 编译器。

## Each 的统一命名

公开概念统一为 `Each`：XML `<Each>`、手写 `each(...)`、配置类型 `display::Each`。
内置 DSL 不提供 `For` 别名。Leptos 的 `<For>` 是参考来源，不是 Glory 的公开名称。

`ui::Each` 与 `display::Each` 现在导出同一个 Widget 和控制器，直接接受
Vec/Cage/Bond/Vary 及 KeyedList。向量使用 `each(source).key(key).children(factory)`；
KeyedList 使用 `each(source).children(factory)`，沿用其成员身份。
协调器和 keyed Cache 只有一份；数据源与物理宿主使用各自的轻量适配。
使用命名空间区分，避免同时 glob 导入旧 `glory::prelude::*` 与新 display prelude。
原始 HTML 不隐式创建高层 Button/View Widget。

## 业务数据与字段

```rust,ignore
use glory::display::prelude::*;
struct Row { id: usize, label: String }
let rows = scope.keyed_list(Vec::<Row>::new(), |row| row.id)?;
rows.replace(vec![Row { id: 1, label: "Hello".into() }])?;
rows.extend(vec![Row { id: 2, label: "World".into() }])?;
rows.update_at(0, |row| row.label.push('!'))?;
rows.update_key(&2, |row| row.label.push('?'))?;
rows.swap(0, 1)?;
rows.remove_key(&1)?;
rows.clear()?;
```

集合句柄是 Owner 所有的本地 Copy 句柄，继续使用 Cage/Bond/Vary。
业务值在 generational arena 中只存一份；业务键索引用于成员操作，顺序有独立的响应通道。
字段访问使用现有投影及成员代次，不建立额外的业务键哈希登记或中间行 Cage 别名。
重复键在 replace/extend 改变成员前被拒绝。

移动保留成员身份。同 key 原位替换更新数据且保留行状态；删除后重插具有新代次。
旧行句柄读写会返回 MissingField，不会写入复用槽中的新成员。
key 必须纯且在成员存活期间稳定；修改值中的 id 字段不会自动重命名成员。
显式删除并插入用于建立新身份。

`update_at/update_key` 编辑整行，会通知该行字段；它们不创建永久字段别名，也不触发顺序观察者。
直接修改字段 Cage 则只通知该字段分支和整行/祖先读者：

```rust,ignore
let row = rows.at(0)?;
let label = field!(row.label);
// 不启用 DSL 编译器时：
let label = row.field("label", |row| &row.label, |row| &mut row.label);
label.revise(|label| label.push('!'));
```

字段投影归属于构造它的 Owner。Each 自动在行 Scope 中构造它们并负责释放。
临时业务编辑使用 update 方法；不要在根 Owner 下为所有历史 key 创建字段句柄。
多视图可以共享数据；卸载一个视图只释放它自己的投影，数据仍由集合 Owner 所有。
`find/at` 和 `try_with_untracked` 可在不挂载 UI 时访问业务数据。
`with_order` 是响应式结构读取，`len_untracked` 只做非响应式观察。

槽位容量跟随并发峰值复用；清空释放活值和投影，不声称 WASM 线性内存或进程 RSS 回到启动值。
批量写在观察者执行前完成关联发布，不提供任意业务 panic 的自动回滚。

## 手写 API 先于 XML

```rust,ignore
const BODY: Outlet = Outlet::new("body");
const LABEL: Field = Field::new("label");
static PANEL: Template = Template::new(&[div(&[children(BODY)], &[])]);
static ROW: Template = Template::new(&[span(&[], &[field(LABEL)])]);

let result = view(&PANEL).child(BODY, each(rows).children(|row| {
    view(&ROW).bind(LABEL, row.field("label", |r| &r.label, |r| &mut r.label))
}));
```

Each 内部管理 Cache、行 Scope、Binding、事件、放置、移动和退休。
工厂对每次挂载的成员运行一次；保留的成员通过字段更新，不依赖首次 seed 的副本。
应用不需要 RowHost、WeakApp、手动 place/dispose 或 label 注册表。

XML 降低到同一个 API：

```rust,ignore
view! {
    <div class="rows">
        <Each each={rows} children={move |row| {
            let label = field!(row.label);
            view! { <span>{label}</span> }
        }}/>
    </div>
}
```

`Each/Show/Content` 是明确的 XML 结构入口，不分析任意 Rust 函数的返回体来猜测 UI。
值初始化一次、Cage/Bond/Vary 直接订阅、计算闭包跟踪依赖的现有区别继续保留。
`{count.get()}` 是一次性读取，`{count}` 是响应式句柄，`{move || ...}` 是计算绑定。

## class、条件和内容范围

```rust,ignore
view! {
    <div class="item" class:danger={move || selected.get() == Some(id)}>
        <Show when={move || visible.get()} fallback={|| view! { <span>"Hidden"</span> }}>
            <Counter initial={0}/>
        </Show>
        <Content value={some_view}/>
    </div>
}
```

class 开关接受 bool、Cage/Bond/Vary<bool> 和 bool 计算闭包；只缓存 bool，静态 class 仍进入模板。
手写使用 `toggle_class("danger", FLAG)` 声明以及 `.class_when(FLAG, computed(read_bool))` 配置。
初值来自模板；false 也能移除模板里已有的同名 class。开关不能与同元素的动态整条 class 混用。
token 当前限为 1–128 字节 ASCII 字母、数字、连字符和下划线。

Show 离开分支时退休该分支，重新进入创建新状态；兄弟组件保留状态。没有隐式保活。
省略 fallback 时显示空内容。静态 View、组件、Option、Vec 和 lazy 工厂可作为 Content；
`Content value={...}` 用于明确的子视图组合，普通 `{expr}` 继续作为文本。

空内容、多根内容和根级结构使用必要的注释范围，不增加可见包装元素。
移动范围会连同实际子内容一起移动；不能只移动占位注释而留下内容。
组件、动态范围和行可以拥有 Scope，静态 HTML 子节点没有独立 Scope/语义 Entry。

## 组件与根挂载

```rust,ignore
#[component]
fn Counter(scope: &Scope, initial: usize) -> impl Widget {
    let count = scope.cage(initial);
    view! {
        <div>
            <span>{count}</span>
            <button on:click={move || count.revise(|n| *n += 1)}>"+"</button>
        </div>
    }
}
#[component]
fn Card(children: glory::ui::Children) -> impl Widget {
    view! { <div><Content value={children}/></div> }
}
// view! { <Card><Counter initial={0}/></Card> }
let app = App::launch("main", Counter::default().initial(0))?;
// 保留 app，直到需要关闭该显示根。
```

宏生成带类型状态的 Widget builder；参数名、类型及必填参数仍由 Rust 检查。
首个 `scope: &Scope` 是注入参数，不进入 builder；显式写错 Scope 类型会编译失败。
Cargo 重命名后使用 `#[g::component(g)]`，函数式宏自动通过 `$crate` 支持重命名。
目前组件宏要求同步函数和具体属性类型；泛型/impl Trait 属性、默认参数等尚不支持。
普通 Rust 函数仍可以直接返回 View，不需要全部注册为组件。

[普通应用示例](../examples/display-web/src/bin/counter.rs)包含计数器、Each、字段编辑、
条件、class 开关和组件 children，没有手工行生命周期代码：

```powershell
./scripts/build-display-web.ps1 -Bin counter -BuildRoot D:/BuildCache/glory-display-demo
# 向该目录提供普通静态 HTTP 服务后打开 counter/site/。
```

## 执行边界与能力范围

新显示宿主在同一 JavaScript turn 后用 microtask 合并事件和写入，不强制等待下一次 RAF。
队列任务只持有 Weak App；关闭或 fault 后不会执行其工作。显式 flush 仍可用，提交重入仍被拒绝。
字段文字进入共享暂存缓冲，保持 UTF-16 字符串偏移和有序命令；每包的节点/记录/字节预算继续生效。
JS 执行失败只确认成功前缀，停止后缀；恢复边界是关闭并重建，不承诺事务回滚。
绑定由 core Owner 拥有。移除显示实例 BindingId 记录的实验出现清空回退，未保留。
同一包中连续移动只拍摄一次选区端点快照；其他命令和分包会切断该快照的使用范围。
焦点和选区仍在每次相关移动后恢复，快照不保留到下一次刷新。

具名显示模板当前支持 div/span/button/a/h1/table/tbody/tr/td、文本、受限静态属性（包括 id）、
class/title/aria-label 动态属性、class 开关和无参数 click。模板内部的表格内容层级也会校验。
旧 raw-template AST、编译器和 Source 已移除；探针、手写 API 与 DSL 使用同一个
`html::view::Template` 及缓存编译结果，不能用任意 wire 绕过 profile。
标准 Widget 宿主已接通混合树 SSR、hydration、增量流和 Blitz；浏览器专用的
`display::App` 保留稀疏路径。配置执行、Each/Show 和内容控制也已共享，具体区别与验证见
[统一 Widget DSL](v2-widget-dsl-unification-2026-10-29.md)。完整 HTML 事件/property
与设备级 IME 不在这个有限 profile 的承诺内。

[实施计划](v2-display-completion-plan-2026-10-28.md)与本轮最终验证/性能记录分别维护；
旧报告中的测试数和计时只属于各自冻结版本。
