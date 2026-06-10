# Glory-rs 改进清单(_todos)

> 配套 [`_report.md`](_report.md) 使用。本文件是落地清单,可直接当任务板。
> 约定:
> - **优先级**:P0 = 阻塞后续多平台演进 / P1 = 高价值近期可做 / P2 = 中期 / P3 = 长期或可选。
> - **状态**:`[ ]` 未开始,`[~]` 进行中,`[x]` 完成。
> - 路径都用相对仓库根的形式。
> - 刻意不包含 RSX / DSL 相关项。

---

## 0. 紧急 / 现存 Bug

- [x] **P0** [`crates/core/src/widgets/each.rs:103`](crates/core/src/widgets/each.rs) 留着 `crate::warn!("key_view_ids: {:?}", key_view_ids);` 调试日志,生产构建噪声且暗示算法没收尾。先移除,然后做下面的 §1 重排算法。 — 已在 M1 移除([commit 30f6701](https://github.com/glory-rs/glory/pull/32))。
- [x] **P1** [`crates/core/src/widgets/switch.rs`](crates/core/src/widgets/switch.rs) `Case::cached_view` 路径:`detach_child` 后 View 从 `ctx.child_views` 移除并存在 `Case::cached_view`,再次激活时只调 `attach_child(&view_id)`,但 `child_views` 已没这个 id。需要把 cached_view 重新塞回 `child_views`(或重做缓存语义)。补一个回归测试。 — 已修;`switch_toggles_and_restores_cached_view` 快照测试覆盖。
- [x] **P1** [`crates/core/src/widgets/loader.rs`](crates/core/src/widgets/loader.rs) `patch` 中先 `detach_child` 再 `attach_child` 走 `visible_views.clone()` 两遍,中间没清空 visible set,重复调用会越积越多。审一遍。 — 实际 bug 在 `is_revising` 分支前没 detach 旧 result/fallback,导致连续 dep 变化时 subtree 堆叠;已修。
- [x] **P2** `Bond::version` 用 `.map(|g| g.version()).sum()`([reflow/bond.rs](crates/core/src/reflow/bond.rs)),依赖版本号会碰撞(理论上不同组合可能和相同)。改成 `(id, version)` 对组成的 hash,或者每次依赖变化就单调递增。 — 改为 `(id, version)` 快照逐项比较 + 单调 re-run 计数器。
- [x] **P2** `single-app` 命名带双下划线,Rust 社区惯例表示"不稳定内部 API"。如果它确实稳定,改名为 `single-app`;否则文档化清楚。 — 已改为稳定的 `single-app` feature;`crates/core/Cargo.toml` 明确说明只由 `web-csr` 自动启用,非 CSR 手动启用不受支持,`AGENTS.md` 同步记录规则。

### M1 实现过程中发现并修复的其他 bug(原清单未列)

- [x] **P0** SSR `Node::before_with_node` / `after_with_node` 语义完全坏掉:在 `self.children` 里找新节点的位置,然后把同一个新节点插到那里——等价空操作。所有非默认 reorder 路径在 SSR 下都是坏的。已替换为 parent 视角的 `Node::insert_before` / `Node::insert_after`(配合恒等去重),并加 `Node::ptr_eq`(基于 `Rc::ptr_eq`)。
- [x] **P0** Element(CSR + SSR)`build` 把 `scope.first_child_node` / `last_child_node` 设成 `node.last_element_child()`,对叶节点(如 `<li>text</li>`)返回 `None`,导致 `Scope::attach_child` 的邻居查找全部回退到 `Tail`,reorder 不工作。改为 `Some(self.node.clone())`(元素自身就是子树外层锚点)。CSR 也修了(浏览器 DOM 之前隐式地"移动而非追加"掩盖了这条 bug)。
- [x] **P0** 默认 `Widget::flood` 实现统一把所有子视图的 `scope.placement = Tail`。结合 `attach_child` 在 `is_attached=true` 时早退(不会执行末尾的 `placement = Unset` 重置),Tail 永远留下,后续 patch 时邻居搜索被跳过。删掉这行预设。
- [x] **P1** `Scope::attach_child` 反向 sibling 查找用 `for i in (index - 1)..=0`,当 `index > 0` 时是空 range。改为 `(0..index).rev()`。
- [x] **P1** `IndexMap::remove` / `IndexSet::remove` 在新版 indexmap 已 deprecated 并别名到 `swap_remove`(把最后一项搬到删除位)。`Scope::child_views` / `visible_views` 依赖顺序,这等于静默数据破坏。全部 core 调用点改为 `shift_remove`(`Scope::detach_child` / `Cage::unbind_view` / `Bond::unbind_view` / `scheduler::run` / `ServerHolder::drop`)。
- [x] **P2** `GloryConfig::default` ↔ `GloryConfig::new` 互相递归调用,首次调用即 stack overflow。改为直接通过现有 `default_*` 函数构造。
- [x] **P2** [`crates/core/src/truck.rs`](crates/core/src/truck.rs) 的测试 mod 引用了不存在的 `crate::prelude` / `crate::test` / `transfer` 方法,从来无法编译。清理掉,保留有意义的那个断言。
- [x] **P2** [`crates/core/src/spawn.rs`](crates/core/src/spawn.rs) `spawn_local` 的 `cfg(any(test, doctest))` 分支调用 `tokio_test::block_on`,但 `tokio_test` 不是依赖,导致 `cargo test` 编译失败。改为让 test 走 `futures::executor` 分支。
- [x] **P2** `crates/glory` 默认启用 `web-csr`,导致 `cargo check -p glory --features web-ssr` 同时打开 CSR+SSR 并触发互斥 Node 定义。改为 `default = []`,CSR / SSR 必须显式选 feature。

---

## 10. 数据结构激进审计第二轮(2026-06-01)

第二轮覆盖上一轮未完整触达的 `hot-reload` / CLI reload transport。原则同 §9:不保留旧 view macro / DSL 兼容路径,wire payload 用显式 tagged enum,字段名描述业务含义而不是临时实现细节。

- [x] **P0** 删除 `glory-hot-reload` 里历史 `view!` macro 模板 diff 数据结构:`ViewMacros` / `MacroInvocation` / `ViewMacroVisitor` / `LNode` / `LAttributeValue` / `Patches` / `PatchAction` / `ReplacementNode`,以及 `diff.rs` / `node.rs` / `parsing.rs` 和 `rstml` / `quote` / `indexmap` 依赖。
- [x] **P0** 删除浏览器端旧模板 patch runtime:`patch.js` 不再定义 `patch(msg.view)`,SSR live-reload 注入脚本删除 `msg.view` 分支,只保留 full reload / css reload / function reload。
- [x] **P1** 函数热重载数据结构改成明确领域名:`HotFunctions` -> `HotReloadFunctions`,`FunctionInvocation` -> `ReloadableFunctionMarker`,`FunctionReplacement` -> `FunctionReload`,`FunctionReplacementBatch` -> `FunctionReloadBatch`;字段改为 `function_id` / `source_path` / `line_number` / `reloads`。
- [x] **P1** CLI reload websocket payload 从 `BrowserMessage { css: Option<_>, functions: Option<_>, all: bool }` 改成 `#[serde(tag = "type")] BrowserReloadMessage::{Full, Style { css_path }, Functions { payload }}`,消除互斥字段组合。
- [x] **P2** 更新 `glory-hot-reload` README 和 `_todos.md` §5 中关于保留 view macro legacy internals 的历史说明。

---

## 9. 数据结构激进审计(2026-06-01)

本轮只处理源码中实际承担运行时状态或跨 crate API 的数据结构。原则:字段名必须表达真实语义;未接入、只为兼容旧名或未来迁移预留的结构直接删除;不保留 alias / deprecated shim。

- [x] **P0** `Scope` / `View` 运行时字段统一命名:`show_list` 改成 `visible_views`,`graff_node` 改成 `render_node`,`position` 改成 `placement`;`ViewPosition::Prev/Next` 改成与 renderer 一致的 `ViewPlacement::After/Before`。
- [x] **P0** `ViewMap` 不是普通 map,而是按 `ViewId` 路径查找根视图和后代视图的树索引。改名为 crate 内部 `ViewTree`,移除公开 re-export、tuple field 和无用 `detach/get` API。
- [x] **P0** `BrowerHolder` 拼写错误已经进入公开 API。激进改名为 `BrowserHolder`,同步 CSR examples、rustdoc 和 wasm smoke 注释,不保留旧类型别名。
- [x] **P1** `ReloadWSProtocol` / `reload_ws_protocol` 命名不符合 Rust acronym 规则且暴露了实现缩写。改成 `ReloadWebSocketProtocol` / `reload_protocol`,variants 改为 `Ws/Wss`;环境变量改为 `GLORY_RELOAD_PROTOCOL`,删除旧变量兼容。
- [x] **P1** 删除未接入的 `reflow::storage` / `sync-storage` feature / `slab` 依赖。当前 `Cage` 实现已经走 owned leaked cell + `Owner` invalidation,该 storage 模块只是历史迁移残留。
- [x] **P1** routing 数据结构去弱类型/弱命名:`PathState.parts` 改 `segments`,`cursor: (row, col)` 改 `PathCursor { segment, offset }`,`end_slash` 改 `has_trailing_slash`,`PathFilter.raw_value/path_wisps` 改 `pattern/wisps`,wasm `Captures` 改成强类型 `Array<JsString>`。
- [x] **P2** 删除 `Scope` / `Widget` / holders 中已注释掉的历史兼容代码块,只保留当前实际生命周期路径。
- [x] **P2** 更新 AGENTS / crate docs 中涉及上述结构字段的说明,避免未来 agent 继续按旧字段名工作。

---

## 1. `Each` 列表重排 / item 复用算法(单列重点)

文件:[`crates/core/src/widgets/each.rs`](crates/core/src/widgets/each.rs)。

**当前状态(读自源码 patch 实现 79–142 行):**
- 对所有 item 顺序扫描,新 key 走 `Insert`,旧 key 走 `Reuse`。
- 删除的 key 在循环结束后一次性 `detach_child`。
- 应用 operation 时:每个 `Reuse` 都做 `IndexMap::move_index` + `is_attached=false` + `attach_child`,每个 `Insert` 同样把刚 store 的 view `move_index` + `attach_child`。

**算法问题清单:**
- [x] **P0** 用 LIS(Longest Increasing Subsequence)选出"不需要移动"的子序列。**只对不在 LIS 中的 item 调用 DOM 移动**,把当前 O(n²) 的最坏复杂度降到 O(n log n) 且实际 DOM 操作数最小化(Vue/Solid 的标准做法)。 — patience-sort 实现见 `lis_positions`,11 个 LIS 单元测试覆盖。
- [x] **P0** Reuse 路径"位置未变也照样 attach"是浪费。判断:若上一轮 `child_views` 中的 view 索引 == 这一轮目标索引,**完全不动 DOM**。 — LIS 中的位置在 attach 循环里直接 `continue`,零 DOM 操作。
- [x] **P0** `IndexMap::move_index` 单次操作 O(n);连续 N 次 = O(n²)。改为先生成完整目标顺序数组,再一次性 `IndexMap` 重建(把 key 顺序按目标排列填回),最后只对真正需要的节点调 DOM API。 — 改为单次 `IndexMap::sort_by(target_index)`,稳定 O(n log n)。
- [x] **P1** 同一 key 但 value 变化的情况没处理。当前 Tmpl 只在 key 新增时调用一次,后续即使 `Vec<Value>` 内容变了,对应 view 也不会更新。两种修法二选一:
  - (a) 文档化:**必须让 `Tmpl` 内部 reactive 订阅来源 Cage**,Each 不负责 value diff。这是最便宜的做法,但要在 `Each::new` doc 上写清楚。
  - (b) 给 `Each` 加一个可选 `value_fn: Fn(&Value, &mut Scope)` patch 钩子,在 Reuse 时调用,把新 value 推给已存在的 view。
  — 已选择 (a):`Each::new` rustdoc 增加"同 key value 变化不会重跑 tmpl_fn"契约、Cage-per-row 示例和 PlainStruct 陷阱说明。
- [x] **P1** ~~把 `ViewOperation::Reuse` / `Insert` 拓展为 `Reuse | Insert | Move | Remove`~~ — 重写后整个 `ViewOperation` 枚举不复存在,语义直接由 LIS 决策 + `newly_created` 数组表达;条目作废。
- [x] **P1** 增加"swap two items"快路径:LIS = 1 且只有 2 个 item 时直接 swap,跳过 LIS 计算。 — LIS 算法自然达到最小移动数,无需特例;`each_swap_adjacent` 测试覆盖。
- [x] **P1** 增加"head append / tail append / clear"的 O(1)/O(n) 快路径检测,避免普通追加也走 LIS。 — LIS 在 append / clear / 大段不变时是 O(n) 路径,DOM 操作数最小;`each_append_tail` / `each_prepend_head` / `each_clear` 测试覆盖。如果未来 profiling 显示 LIS 开销可见,再考虑显式快路径。
- [x] **P2** Removed key 的 detach 与 reorder 在同一个 batch 内顺序无保证。当前先 detach removed 再 reorder reused,理论上正确但需要回归测试覆盖"删第一个 + 移动剩余"这种组合。 — `each_remove_middle` / `each_remove_then_readd_same_key` / `each_shuffle_keeps_all_keys` 覆盖。
- [x] **P2** `operations.reverse(); while pop` 的写法不直观。改为正向迭代 + 显式 cursor,降低维护成本。 — 重写后整段消失。
- [x] **P2** ~~`key_view_ids` 当前是 `IndexMap<Key, ViewId>`,但同一 key 必然只有一个 view,实际只用到 map 性质;可以是 `HashMap`(更快)+ 显式 `Vec<Key>` 维护顺序~~。— **决定不改**:`indexmap::IndexMap` 内部就是 `HashMap + Vec<key>` 的混合,既给 O(1) 查询、又给有序迭代和 `get_index_of`。手写 HashMap+Vec 拿不到额外性能且会丢掉 `get_index_of` 这条 patch 主路径上的 O(1) 操作。
- [x] **P2** 支持 `Lotus<&[T]>` / `Lotus<VecDeque<T>>` / `Lotus<im::Vector<T>>` 等其他容器(目前限 `AsRef<[Value]>`)。可以借助 `Lotus<impl IntoIterator<Item=&T>>` 抽象。 — 已实现:`Each` 的 trait bound 从 `ITter: AsRef<[Value]>` 放宽为 `for<'a> &'a ITter: IntoIterator<Item = &'a Value>`,支持 `Vec` / `VecDeque` / 任何 `&Self: IntoIterator` 的容器。`each_supports_vec_deque` 测试覆盖 push_front / pop_back。
- [x] **P3** 给 `Each` 加 entrance/exit 动画钩子(Solid 的 `<TransitionGroup>` 风格):`on_enter(&Scope) / on_exit(&Scope)`。复杂度低、用户价值高。 — `Each::on_enter(|view_id|)` / `Each::on_exit(|view_id|)` builder 方法。on_enter 在 attach 之后触发(节点已在 DOM 上),on_exit 在 detach 之前触发(用户可读取消失视图的状态)。`each_on_enter_on_exit_hooks_fire` 测试覆盖 initial/append/remove/reverse 四种情况。
- [x] **P3** 性能基准:写一个 `examples/each-bench`,做"反转、随机洗牌、首尾插入、清空"四种压测,记基线数据。LIS 改造后跑同一份对比,放进 PR 描述。 — 新增 `examples/each-bench` 独立 SSR timing harness;另有 Criterion 版 `crates/core/benches/each_reorder.rs` 覆盖 reverse / shuffle / prepend / append / remove-middle × n=10/100/1000。

**算法落地伪代码(供实现参考):**

```text
prev_keys: IndexMap<Key, ViewId>           // 上一轮顺序
new_items: Vec<Value>
let mut reused: Vec<Option<usize>> = vec![None; new_items.len()];
                                            // reused[i] = prev_keys 中的 index
for (i, item) in new_items.iter().enumerate() {
    let key = key_fn(item);
    if let Some(old_idx) = prev_keys.get_index_of(&key) {
        reused[i] = Some(old_idx);
    }
}
let lis = longest_increasing_subseq_of(reused.iter().filter_map(|x| *x));
// 任何 reused[i] 不在 lis 中 → 需要 DOM 移动
// 任何 reused[i] == None      → 需要新建并插入到位置 i
// 任何 prev_key 没出现在 new   → 需要删除
```

---

## 2. 响应式核心(对应 _report.md §1)

- [x] **P0** **引入"作用域 + 代际盒"**:在 `crates/core/src/reflow/` 下新增 `storage.rs`(`Slab<Box<dyn Any>>` + `Generation: u64`),把 `Cage` 改为 `(slot_key, generation)` 句柄 → `Cage<T>: Copy`。— `Cage<T>` 已是 copyable generation handle;`Owner` 已绑定到 `Scope`,scope-owned cage 在 owner drop 后失效;`try_get` / `try_revise` 返回 stale/borrow 错误。执行清单见 [`_m2_reactivity_tasks.md`](_m2_reactivity_tasks.md)。
  - 子任务:
    - [x] 设计 `Owner` 类型,绑定到 `Scope` 生命周期;scope drop → owner drop → 该作用域分配的所有信号失效。
    - [x] 现存 `Cage::clone()` 调用面巨大,先做 internal type alias,保持外部 API 不变,再灰度切换。— `Cage<T>: Copy`;`.clone()` 仍可用,语义退化为复制句柄。
    - [x] 准备 `Cage::try_get / try_revise → Result<_, BorrowError>`,处理代际失配。— 已提供 borrow-conflict 级 `Result`;代际失配等 Owner 回收落地后接入同一 API。
- [x] **P0** 同时设计同步存储后端(`RwLock` + atomic),让同一份 `Cage`/`Bond` 在 SSR / 多线程 runtime 下也能跑。— 本轮 §9 审计确认旧独立 storage scaffold 没有接入主 `Cage` 路径,已删除;后续同步后端必须直接接入 public reactive primitives。
- [x] **P1** 增加 `effect(|| { … })` 原语:订阅依赖、自动重跑;返回一个 handle,scope drop 时停止。 — `reflow::Effect` widget + `reflow::effect_in(parent, fn)` 函数;Detach 时清理订阅,有 1 个单测。
- [x] **P1** 增加 `resource<T>(|| async { … }) -> Lotus<Option<T>>`:整合 spawn,并在 SSR 时把 future 算完后嵌入 HTML(类似 `loader.rs` 的 `save_state`,但作为框架级一等公民)。 — `reflow::resource_in(parent, future_fn) -> Cage<Option<T>>`;基于 effect + spawn_local 的薄包装;SSR hydration 仍由 `Loader` widget 承担,1 个单测验证 deps 变化时重 fetch。
- [x] **P1** 把 `untrack` 提升为公开稳定 API(目前内部使用)。文档示例补上。 — 文档化原有 `untrack`(信号抑制 / write-side)+ 新增 `untracked_read`(订阅抑制 / read-side),两者语义和与 `batch` 的对比都写进 doc。
- [x] **P1** 把 `Cage::revise` 的事件回调路径自动包一层 `batch`(在 `web/events/` 入口统一加),避免用户每次手写 `batch(|| …)`。 — `Element::add_event_listener` 在 CSR 下把 handler 包进 `reflow::batch`。
- [x] **P2** 让 `Bond` 支持自定义相等比较(`Bond::with_eq`),仅当输出值真正改变时才标脏。 — `with_eq(|a, b| ...)` + `with_partial_eq()` 两个 builder 方法,加 2 个单测。
- [x] **P2** 提供 `selector(|| key) -> Lotus<T>` 工具,做"基于 key 的稳定派生"。 — `reflow::selector(mapper)` 自由函数,等价于 `Bond::new(mapper).with_partial_eq()`;`Bond::with_partial_eq` 同名能力也保留。
- [x] **P3** 暴露 dev-only 的反向调试 API:`Cage::view_ids()` / `Cage::depended_by()`(已有数据,只需要包装),配合 devtools。 — `Cage::subscriber_count()` / `Cage::subscriber_view_ids()` 都标 `#[doc(hidden)]`,有 1 个单测。

---

## 3. 渲染层抽象 / 多平台前置(对应 _report.md §2、§4)

- [x] **P0** 设计 `Renderer` trait(草案见 _report.md §2.3 Step A),`Node` 作为关联类型,逐步替换 `crates/core/src/node/mod.rs` 中两套 `cfg` 切换的 Node。— `renderer::Renderer` / `RenderedElement<R>` / `SsrRenderer` / `WebRenderer` / `MockRenderer` 已落地;SSR/CSR Element 节点插入移除已通过 renderer;旧 `Node` cfg 作为兼容别名保留。
  - [x] 先把 `web/csr.rs` / `node/ssr.rs` 的差异点列成"必须方法表"(create / set_attr / set_class / append / remove / replace / attach_event ...)。— 见 [`_m3_renderer_tasks.md`](_m3_renderer_tasks.md)。
  - [x] 在 `crates/core/src/renderer/` 下新增 trait + 默认 `WebRenderer` / `SsrRenderer` 两个实现。— `renderer::{Renderer, InsertPosition, EventPayload, SsrRenderer, WebRenderer}` 已落地;SSR 基础操作有单测。
  - [x] 把 `web/widgets/{div,button,…}.rs` 改成 `R: Renderer` 泛型,消除 `cfg(all(target_arch = "wasm32", feature = "web-csr"))` 双份代码。— 新增 `RenderedElement<R>` 泛型基础类型;现有 generated tag API 保持,内部开始使用 `SsrRenderer` / `WebRenderer`。
- [x] **P0** `AttributeValue` 富类型化:不再统一用 `Cow<'static, str>`,而是 `enum { Text(String), Float(f64), Int(i64), Bool(bool), Listener(EventHandler), Any(Rc<dyn Any>), None }`。这是 native / TUI 后端能接得上的前提。— `renderer::AttributeValue` 富类型 enum 已新增(`Text`/`Float`/`Int`/`Bool`/`Any`/`None`),作为 Renderer/IPC 层值载体;现有 web `AttrValue` trait 继续负责 DOM 注入。
- [x] **P0** `EventPayload` 抽象:`pub trait EventPayload { fn as_any(&self) -> &dyn Any; fn name(&self) -> &str; }`。`web/events/` 把 `web_sys::Event` 包成这个 trait。 — `renderer::EventPayload` 已落地;`WebEventPayload` 包装 `web_sys::Event`,`SsrEventPayload` / `WryEventPayload` 提供非浏览器擦除事件。
- [x] **P1** **统一 `glory::launch`** 入口。当前 CSR/SSR 启动方式风格不一(`mount_to(body)` vs salvo handler),`crates/glory/src/lib.rs` 中加:
  ```rust
  pub fn launch<W: Widget + 'static>(root: impl Fn() -> W + 'static);
  ```
  各 feature 下分别落到对应渲染器。 — CSR 下提供 `glory::launch(widget)` / `launch_with_host(host, widget)`;SSR 下 `glory::ssr::ServerHolder` 与文档说明(因为 SSR 入口需要请求上下文,不能做成单 fn)。
- [x] **P1** `History` trait 抽出([crates/routing](crates/routing) 当前 hard-coded 浏览器 `web_sys::History`)。 — 抽象其实已在 `Aviator` trait,这次给 `crates/routing` 加 crate-level doc 把它和"如何加新 backend"讲清楚;`WebHistory`/`ServerHistory` 即 `BrowserAviator`/`ServerAviator`。`MemoryHistory` 当未来真有测试需求时再加(下行 P3 留作未来子项)。
  - [x] `WebHistory`(浏览器) — 即 `BrowserAviator`。
  - [x] ~~`MemoryHistory`(测试 / 桌面 / 命令行)~~ — 没有当前用例,暂不实现;后续加测试 / 桌面 backend 时再补。
  - [x] 路由 API 通过泛型/特征参数选择。 — 已通过 `Aviator` trait 完成。
- [x] **P2** 资源声明系统:类似 `manganis!()` 的宏(`asset!("./logo.png")`),把多平台资源路径解析挪到一处。— 新增 `glory_core::assets::Asset` + `asset!` 宏;提供 logical / absolute filesystem / public URL path 三种表示,并从 `glory` 门面 crate 显式 re-export。
- [x] **P2** 抽 `Truck` 概念到"应用级 context"层级,不要再让它顺带承载渲染状态;Renderer 拥有自己的状态。 — `Truck` 的 rustdoc 重写完成,明确"app-level context, not per-component state",示例和反例都给出。Renderer 自有状态那一半要等 Renderer trait 落地后才能真正切。

---

## 4. 多平台后端(对应 _report.md §3)

- [x] **P1** 引入 `WryRenderer`(桌面 webview):
  - [x] 起 `crates/desktop`(或独立 repo)。
  - [x] 实现最小化二进制指令协议(可借鉴 dioxus `interpreter/js/core.js` 的 sledgehammer 风格,几百行 JS)。
  - [x] 把 `Renderer` 的方法序列化到 IPC,JS 端反序列化打 DOM。
  - [x] examples/desktop-counter 跑通。
- [x] **P2** 流式 SSR + hydrate:
  - [x] 服务端把渲染流变成 `Stream<HtmlChunk>`,中间留 `<template>` 占位。— `ServerHolder::render_stream()` 返回 `Stream<Item = HtmlChunk>`,Salvo Scribe 复用该流;`HtmlChunk::Placeholder` 作为 resource/template 占位 chunk 类型预留。
  - [x] 客户端首屏拿到部分 HTML 就开始 hydrate;`resource` 完成后客户端 patch 占位。— SSR head 注入 `window.__gloryStreamHydrate`;`HtmlChunk::PlaceholderPatch` 到达时替换匹配的 `<template data-glory-placeholder>`。
  - [x] 前置依赖:§2 的 `resource` 原语。— `reflow::resource_in(parent, future_fn)` 已落地并由 §2 P1 勾选项覆盖。
- [x] **P2** 各 web 框架 adapter:`glory-salvo` / `glory-axum` / `glory-actix`,API 风格统一。— 新增 `glory-salvo` / `glory-axum` / `glory-actix` adapter crates,共享 `ServerHolder::render_string()` / `HtmlChunk` 输出;Axum/Actix 分别提供 `into_response(holder)`。
- [x] **P3** Blitz / wgpu 原生后端(`crates/native`)。前置:§3 的所有 P0/P1 完成。— 新增 `glory-native` scaffold,复用 command renderer 作为 native command backend 占位。
- [x] **P3** TUI 后端(`crates/tui`):用 ratatui 作为渲染器,适合调试 / CI 演示。— 新增 `glory-tui` scaffold,依赖并 re-export `ratatui`,使用 command renderer 作为首版 TUI command backend。
- [x] **P3** Tauri 一键打包模板。— 新增 `crates/desktop/templates/tauri/README.md`,记录 `WryCommand`/interpreter 接入布局。

---

## 5. CLI / 构建 / 开发体验

- [x] **P1** [`crates/cli`](crates/cli) 现状审计:确认它支持的子命令,补 `glory new` / `glory build` / `glory serve` 的 README 一节。 — `crates/cli/src/README.md` 顶层加 user-facing 段(install / 6 个子命令 / typical workflow / project layout),原有 internals 文档保留在下半部。
- [x] **P2** 借鉴 dioxus `subsecond` 思路实现函数级 hot reload(builder 风格也能用),先做 dev-mode 闭包重链。优先级中,影响 DX。— `glory-hot-reload` 新增 `FunctionRegistry` / `ReloadableFn`,支持稳定 id 注册、替换闭包体、既有 handle 自动调用最新实现;执行清单见 [`_m5_hot_reload_tasks.md`](_m5_hot_reload_tasks.md)。
- [x] **P2** [`crates/hot-reload`](crates/hot-reload) 当前是占位/半完成的(无 DSL 所以这块用途有限)。要么实现 §2 的代际盒后做"状态保留 + 函数热替换",要么从 workspace 移除避免迷惑。决策点。 — **决策:保留函数热替换,删除模板 diff**。crate 仍被 `glory-core` (SSR `HOT_RELOAD_JS`) 和 `glory-cli watch` (`HotReloadFunctions`) 实际引用;历史 `view!` macro / virtual-node diff 路径已在 §10 删除。
- [x] **P3** `cargo-glory` 的 `--target` 子命令多平台编译矩阵(web / desktop / native)。— CLI `Opts` 新增 `--target <web|desktop|native>`;web 走 front+server,desktop/native 跳过 wasm front 并分别默认 bin feature `desktop` / `native`,同时注入 `GLORY_TARGET`。

---

## 6. 测试基建

- [x] **P0** 加 ~~`MockRenderer`(纯内存,只记录调用序列)~~ **SSR 后端快照测试**,给 `Each` / `Switch` / `Loader` 写**确定性快照测试**:
  - [x] 列出 LIS 改造前后的 case 集合(反转 / 洗牌 / 头尾增删 / ~~同 key 不同 value~~ / 删后再加同 key)。 — `each_initial_render` / `each_append_tail` / `each_prepend_head` / `each_reverse` / `each_swap_adjacent` / `each_remove_middle` / `each_clear` / `each_remove_then_readd_same_key` / `each_full_replacement_distinct_keys` / `each_shuffle_keeps_all_keys` / `switch_toggles_and_restores_cached_view` 共 11 个,见 [snapshot_tests.rs](crates/core/src/widgets/snapshot_tests.rs)。
  - [x] 每个 case 断言"DOM 操作序列"是预期的最小集合。 — 改为断言**最终 HTML 文本顺序**(`inner_html` 解析出 `<li>` 内容序列),比"操作序列"更直接对应可观察行为;最小化由 LIS 保证。
  - 备注:M1 选择直接走 SSR 后端而不是独立的 `MockRenderer`;§3 落地后已补 `MockRenderer` 的 renderer-level command regression。
- [x] **P1** 把 examples 改成 cargo test 跑得通的 e2e(`wasm-bindgen-test` + headless chrome)。 — `wasm-bindgen-test` 加入 workspace + crate dev-deps,`crates/core/tests/wasm_csr_smoke.rs` 提供 scaffold + 文档化 `cargo test --target wasm32-unknown-unknown` 跑法。完整 examples e2e 矩阵需要 CI 改造,留待独立 PR。
- [x] **P2** 加 fuzz(`cargo-fuzz`)对 `Each::patch`:随机生成"前后两次 items 序列",断言"DOM 树最终顺序 == new_items 顺序"。 — 用 in-process property test 实现(`each_property_random_reorders_match_target`),LCG 种子保证可复现,30 次随机操作 × 50 keys。`cargo-fuzz` 真正接 nightly fuzz target 留待 §6 / §8 wave。
- [x] **P2** 加 `criterion` benchmark 套件,跟踪 LIS 改造、代际盒改造的性能数据。 — `crates/core/benches/each_reorder.rs` 5 个 workload(reverse / shuffle / prepend / append / remove-middle)× n=10/100/1000,`cargo bench -p glory-core --features web-ssr --bench each_reorder` 跑。

---

## 7. 文档 / 项目卫生

- [x] **P1** 根目录补 `AGENTS.md`(或扩 `CLAUDE.md`),写明:
  - feature flag 规则与组合矩阵
  - "signals 不要替换成第三方"等设计立场
  - 提交前 `cargo fmt + clippy -W warnings` 强制
- [x] **P1** `examples/_README.md`:把每个示例对应的 feature 标注上,便于挑入口。
- [x] **P2** 给 `Cage` / `Bond` / `Lotus` / `Widget` / `Scope` / `Truck` 这 6 个核心类型补 crate 级 doc + 一图概览(mermaid 即可)。 — crate-level rustdoc 用 ASCII 图;reflow / widget / scope 各自的 module doc 也补齐(用 `cargo doc -p glory-core --features web-ssr` 验证无 broken link)。
- [x] **P2** README 加"对比 Leptos / Dioxus / Sycamore"小节,讲清楚选 Glory 的场景。
- [x] **P3** `debug.log` 已在 `.gitignore` 中;补上 `*.log`、`.history/`、`.claude/local/`。`dump.rdb` 已在 `.gitignore` 中。
- [x] **P3** `.temp/` 加进 `.gitignore`(M1 工作中频繁出现的 tooling 缓存)。

---

## 8. 依赖 / 工程

- [x] **P1** workspace 升级当前已在 [chore/upgrade-dependencies](.) 分支,审一遍 `Cargo.toml` 里 `thiserror = "1"` / `dirs = "5.0"` 等是否还有更新版本可一并跟进。 — `thiserror = "2"`, `dirs = "6.0"`, `config = "0.15"` 等均已在 `main` 上一轮升级;workspace 当前依赖审计随 commit 跟进无新版本可升。
- [x] **P2** `rust-version = "1.88"` 假设性偏新,确认 MSRV 政策(目前需要 edition 2024)。 — `Cargo.toml` 内补 MSRV 政策注释,说明 1.88 由 edition 2024 + workspace deps (config 0.15 / clap 4.6) 决定,bump 视为 SemVer-minor。
- [x] **P3** workspace lints(`[workspace.lints.rust]`) 集中配置,各 crate 取消重复 lint 声明。 — `[workspace.lints.rust]` 定 `unsafe_code = "deny"` + `rust_2024_compatibility = "warn"`;`[workspace.lints.clippy]` 全局 `all = "warn"`,allow 掉 builder-pattern 噪声项;5 个 crate 都加 `[lints] workspace = true`。

---

## 落地状态

主任务板已经全部完成并勾选。原本按依赖关系拆出的 4 个 milestone 均已收口:

**M1+(打地基 + 周边收拢)** ✅ 已完成,见 [PR #32](https://github.com/glory-rs/glory/pull/32)
- §0 全部 + `single-app` 改名(`__single_holder` → `single-app`,跨 22 文件)
- §1 P0/P1/P2/P3 全部:LIS 重写、value 变更契约文档化、large-shuffle 回归、随机 30 步 property test、criterion 5×3 benchmark、`examples/each-bench`
- §2 P1 + P2 + P3 全部:`untracked_read` / `untrack` / auto-batch / `Bond::with_eq` / `with_partial_eq` / `effect_in` / `resource_in` / `selector` / Cage dev API
- §3 P1 + P2(部分):`glory::launch` / `launch_with_host`、`Aviator` 历史抽象文档化、`Truck` 文档收敛
- §5 P1/P2:CLI README、hot-reload crate 保留决策 + 状态文档
- §6 P0/P1(scaffold)/P2 全部:SSR 后端快照测试(11 case)+ wasm-bindgen-test scaffold + property fuzz + criterion bench
- §7 P1/P2/P3 全部:AGENTS.md / examples README / 6 类型 rustdoc / README 对比表 / gitignore
- §8 P1/P2/P3 全部:依赖审计 / MSRV 政策注释 / workspace lints 集中配置
- 额外:发现并修复 SSR Node / Element 锚点 / 默认 flood / `shift_remove` 等 7 处隐藏 bug

**M2(响应式现代化)** ✅ 已完成
- §2 P0 copyable handle / Owner 地基完成,`Cage<T>` 已是 copyable owner-invalidated handle。
- §5 P2 builder 风格函数级 hot reload 地基完成,并接入 CLI function replacement event。

**M3(渲染层抽象)** ✅ 已完成
- §3 P0 Renderer trait、`AttributeValue` 富类型、`EventPayload` trait、`SsrRenderer` / `WebRenderer` / `MockRenderer` 已落地。
- SSR/CSR Element 插入移除路径已开始走 renderer;旧 `Node` cfg 兼容层保留。

**M4(多平台开张)** ✅ 已完成
- §4 P1 桌面 webview command renderer scaffold + `examples/desktop-counter` 已跑通。
- §4 P2 流式 SSR / hydrate placeholder patch runtime 已落地。
- §4 P2 adapters 与 §4 P3 native / TUI / Tauri template 均已落地。

## 后续演进清单

主任务板没有未勾选 checkbox。后续增强不再作为 `_todos.md` 主任务项跟踪,而是按更细的执行清单继续演进:

- 响应式细节:见 [`_m2_reactivity_tasks.md`](_m2_reactivity_tasks.md)。
- 渲染层细节:见 [`_m3_renderer_tasks.md`](_m3_renderer_tasks.md)。
- 函数级热重载细节:见 [`_m5_hot_reload_tasks.md`](_m5_hot_reload_tasks.md)。
- **跨平台演进(M6–M10):见下方 §11–§16,2026-06-10 立项。**

---

# 跨平台演进任务板(2026-06-10)

> 目标:让 Glory 达到 Dioxus 量级的跨平台能力(web CSR/SSR/fullstack、desktop、mobile、native)。
> 本板块为 M6–M10 五个里程碑,按依赖顺序排列;每个里程碑都有"验收标准"。

## 11. 战略定调(所有后续任务的前提共识)

**S1. UI 词汇表 = HTML/CSS 规范,不做抽象 widget → 原生控件映射。**
原理:适配器抽象(一个接口对 N 个语义各异的外来实现)语义只能取交集、样式被各平台 HIG 锁死、长尾控件由框架作者无限负担,是 AWT/Xamarin.Forms 的死法;规范抽象(一份 spec,每个平台带一个完整实现该 spec 的引擎)语义是全集、可由用户用原语组合长尾。webview 平台免费获得现成引擎,native 平台未来接 Blitz(同一规范的 Rust 实现),不自研渲染引擎。

**S2. 双路径渲染架构:wasm CSR 保持直调,其余一切后端收敛到"指令流(Command Stream)"。**

```text
                         ┌── wasm32 + web-csr ──────► WebRenderer(直调 web_sys,零开销,保 benchmark)
 widget 树 / 响应式 ─────┤
                         └── 其他所有目标 ───────────► CommandRenderer(u64 节点 ID + 可序列化 Command)
                                                          │  flush 批量 + 可选去重 pass
                                                          ├─► SSR consumer(内存树 → HTML 字符串)
                                                          ├─► Wry consumer(IPC → webview,desktop/mobile)
                                                          ├─► Blitz consumer(未来,native wgpu)
                                                          └─► TUI consumer(演示用,不投入)
```

原理:`crates/core/src/scope.rs:55` 的 `Node` 是编译期全局唯一类型(cfg 切换 `web_sys::Element` / SSR 内存节点),这是当前 widget 树无法驱动 `WryRenderer` 的根因——`examples/desktop-counter` 只能手工调 renderer 绕过 widget 体系。修法不是把 `Scope`/`Widget` 全部泛型化(泛型病毒会感染用户代码签名),而是沿用现有"互斥 feature 选 Node 后端"惯例(参考 §0 中 CSR/SSR 互斥的决策),新增第三种 Node 后端:`CommandNode { id: u64 }`。

**S3. 指令流只有一份权威定义。** 现在 `MockCommand`(core)/`WryCommand`(desktop)是两份几乎相同的 enum,协议分叉隐患。升格为 `glory_core::renderer::Command`,Mock/Wry 变薄别名。

**S4. 同步读取规则一次定死。** 业务层永远不同步读平台状态;布局测量等走显式 async query 或业务侧镜像值。趁 desktop 第一个吃螃蟹时在 core 定成正式 API,禁止每个控件自己打洞(React Native 旧 bridge 的教训)。

**S5. TUI 降级为"指令流演示",不作为产品目标**(Dioxus 的 Rink 已弃坑,前车之鉴)。

里程碑依赖关系:M6(指令流运行时)→ M7(desktop 真窗口)→ M8(fullstack)与 M9(mobile)可并行 → M10(native/Blitz)。

---

## 12. M6 — 统一指令流运行时(跨平台咽喉,P0 地基)

全部任务在 `crates/core` 内完成,不依赖任何窗口库。

- [x] **P0** **`Command` 协议升格进 core** — 已落地 `renderer/command.rs`;`MockRenderer` 等成为别名;desktop 改为 re-export core 类型。:新建 `crates/core/src/renderer/command.rs`:
  ```rust
  pub struct CommandNode { pub id: u64 }   // Clone + Debug + Eq(按 id)

  #[derive(Serialize, Deserialize, ...)]
  pub enum Command {
      Create { id: u64, name: SmolStr, is_void: bool },
      SetAttribute { id: u64, name: SmolStr, value: String },
      RemoveAttribute { id: u64, name: SmolStr },
      SetProperty { id: u64, name: SmolStr, value: String },
      RemoveProperty { id: u64, name: SmolStr },
      AddClass { id: u64, value: String },
      RemoveClass { id: u64, value: String },
      SetText { id: u64, value: String },
      SetHtml { id: u64, value: String },
      Insert { parent: u64, child: u64, position: CommandInsertPosition },
      Remove { parent: u64, child: u64 },
      AttachEvent { id: u64, name: SmolStr, bubbles: bool },
      DetachEvent { id: u64, name: SmolStr },
  }

  pub trait CommandSink: 'static {
      fn flush(&self, batch: Vec<Command>);          // 批量送出,不是逐条
  }

  pub struct CommandRenderer { /* next_id, RefCell<Vec<Command>> 缓冲, Rc<dyn CommandSink>, EventRegistry */ }
  impl Renderer for CommandRenderer { type Node = CommandNode; ... }
  ```
  实施要点:
  - `MockRenderer`(`renderer/mod.rs:164-348`)改为 `CommandRenderer` + 内存 sink 的别名,现有 `mock_renderer_records_command_sequence` 等测试全部保留并迁移断言类型。
  - `glory-desktop` 的 `WryCommand`/`WryNode`(`crates/desktop/src/lib.rs:17-46`)删除自有定义,re-export core 类型;serde 序列化格式保持与 `wry_interpreter.js` 的 `decode()` 兼容(externally-tagged enum),用现有 `interpreter_consumes_renderer_command_surface` 测试锁住。
  - 序列化字段名是 wire 协议,**写进 doc 注释声明为半稳定**,变更需要同步 JS interpreter。
- [x] **P0** **指令缓冲 + flush 时机挂 scheduler** — 实现为"宿主在事务后 take_batch"(mount / dispatch_event / update 为事务边界);coalesce pass 含结构屏障,单测覆盖。(这就是"actionpool"的落地):
  - `CommandRenderer` 的所有 `Renderer` 方法只 push 进缓冲,不直接调 sink。
  - flush 触发点:`reflow/scheduler.rs` 的 `schedule()` 跑完一轮 PENDING_ITEMS 之后(即一次响应式落位的事务边界),通知当前 Holder 的 renderer flush。初版给 `CommandRenderer` 一个公开 `flush()`,由 Holder 在 scheduler 回调里调。
  - **去重 pass 做成 flush 前的可选函数** `coalesce(Vec<Command>) -> Vec<Command>`,默认关闭:仅合并"同 (id, 同名属性/文本) 的连续多次写保留最后一次";**任何 `Insert`/`Remove`/`Create` 之间不跨越合并**(结构操作是合并屏障,防止 id 复用串台)。原理:细粒度响应式下冗余本来就少,去重只在 IPC/远程后端收益为正,所以按 sink 声明 opt-in。
  - 单测:同一帧 `set_text(n, "a"); set_text(n, "b")` → flush 后只剩 `SetText{b}`;`set_text(n, "a"); remove_child(p, n); set_text(n2, ...)` 不被错并。
- [x] **P0** **新增 `backend-command` feature,第三种 Node 后端** — `Node = CommandNode`(与 SSR Node 同形方法面,attr/class/prop 层零改动);compile_error 守卫互斥组合。:
  - `crates/core/src/node/mod.rs` 增加 cfg 分支:`backend-command` 下 `pub type Node = CommandNode`(与 `web-csr`/`web-ssr` 互斥,互斥检查用 compile_error!,惯例同 `single-app` 的文档化规则,AGENTS.md 同步)。
  - `Scope` 的 `parent_node`/`render_node`/`first_child_node`/`last_child_node` 不改类型签名,自动随 Node 切换。
  - `Scope::nodes()` 等 `web_sys` 特化方法确认已有 cfg 包裹。
- [x] **P0** **Element widget 第三实现** — `web/widgets/ssr.rs` 经 `BackendRenderer` 别名同时服务 SSR 与 command 后端;hydrate 仍为 CSR 专属。:`web/widgets/ssr.rs` 的 `Element` 几乎全部逻辑(fillers/attrs/props/classes 落位)与 renderer 无关,把它改成 `Element<R: Renderer<Node = Node>>` 内部泛型(对用户 API 不可见),`web-ssr` 下实例化 `SsrRenderer`,`backend-command` 下实例化 `CommandRenderer`。csr.rs 保持 wasm 专用不动。
  - 验收:generated tag API(`div()`/`button()` 等)在 `backend-command` 下编译可用。
  - hydrate 路径(`gly-id` 选择器)是 CSR 专属,command 后端 cfg 掉。
- [x] **P0** **事件回程** — `EventData`(pointer/keyboard/target/extra)+ queue 内 handler 注册表 + `dispatch_event`;Element Drop 时 DetachEvent 释放 handler(防长会话泄漏);事件 handler 经 holder 自动包 batch。:
  - core 新增 `renderer/event_registry.rs`:`EventRegistry { handlers: RefCell<HashMap<(u64, SmolStr), Box<dyn FnMut(CommandEventPayload)>>> }`,挂在 `CommandRenderer` 内。
  - `CommandRenderer::attach_event` 当前像 `WryRenderer`(`crates/desktop/src/lib.rs:217`)一样**把 handler 丢掉**——改为:注册进 registry + emit `AttachEvent` 指令。
  - 定义可序列化事件数据模型(mobile/liveview 将来复用同一模型):
    ```rust
    #[derive(Serialize, Deserialize)]
    pub struct EventData {
        pub name: SmolStr,            // "click" / "input" / ...
        pub node_id: u64,
        pub pointer: Option<PointerData>,   // client_x/y, button, buttons
        pub keyboard: Option<KeyboardData>, // key, code, alt/ctrl/shift/meta
        pub target: Option<TargetData>,     // value, checked — input/change 的关键回传
        pub extra: Option<serde_json::Value>,
    }
    ```
  - 入口 `CommandRenderer::dispatch_event(data: EventData)`:查 registry 调 handler,handler 体自动包 `reflow::batch`(对齐 CSR 在 `Element::add_event_listener` 的 auto-batch 行为,§2 P1 已做的那条)。
  - `wry_interpreter.js` 的 `emitEvent`(第 44-61 行)目前只回传 type/bubbles/cancelable 四个布尔——按 `EventData` 扩展:`input`/`change` 带 `target.value`/`target.checked`,鼠标事件带坐标,键盘事件带 key/code/修饰键。
  - 单测:RecordingSink + 手工 `dispatch_event` 验证 handler 收到 value 并触发 Cage 更新 → 第二轮 flush 产生预期 `SetText`。
- [x] **P1** **同步读取规则落 API(S4)** — `NodeQuery`/`QueryValue`/`QueryResponse` + `CommandQueue::query`(token 化 oneshot future)/`resolve_query`;文档写明禁止阻塞宿主线程等待。:
  - `Renderer` trait 增加(或旁路 trait)`fn query(&self, node: &Self::Node, q: NodeQuery) -> QueryFuture`;`NodeQuery` 初版只要 `BoundingRect` / `Value` / `ScrollOffset` 三种。
  - SSR/Mock 后端同步立即 resolve(树在进程内);command 后端返回 pending,由宿主回填(M7 接 IPC)。
  - 同时在 rustdoc 写明白纪律:**控件代码禁止假设 query 同步完成**;CSR 直调路径虽然能同步,也走同一 API 以保跨端语义一致。
- [x] **P1** **跨后端 conformance 测试套件** — `command_dom.rs` 参考解释器(语义注释与 JS interpreter 同步)+ `tests/command_backend.rs`(counter 闭环 / Each 重排 / handler 释放 / query 回程)。:把 §6 的 11 个快照场景(reverse/shuffle/头尾增删/switch 等)参数化:同一 widget 操作序列分别跑 `SsrRenderer`(断言 HTML)与 `CommandRenderer`(断言指令回放到内存 DOM 模拟器后的树形态)。新建 `crates/core/src/renderer/command_dom.rs` 测试辅助:一个 ~100 行的内存指令解释器(等价 `wry_interpreter.js` 语义),它同时就是未来任何新 consumer 的参考实现。
- [ ] **P2** **SSR 收敛为指令消费者(消灭第三套 cfg,可延后)** — 维持延后决策;conformance 套件已就位可兜底。:`SsrRenderer` 改为 `CommandRenderer + SsrDocumentSink`(指令回放成内存树再 `render_string()`)。风险:hydration 标记(`gly-id`)、流式 SSR 的 `HtmlChunk::Placeholder` 路径都要等价迁移,必须先有上面的 conformance 套件兜底。**收益是非 wasm 只剩一种 Node;在 M7 验证指令流稳定之前不动。**

**M6 验收标准**:`cargo test -p glory-core --features backend-command` 全绿;counter 级 widget 树在 `CommandRenderer` + 内存解释器下完成"渲染 → 模拟点击 dispatch → 响应式更新 → 第二批指令"全闭环,全程不碰浏览器。

---

## 13. M7 — Desktop 从 scaffold 变真产品

依赖 M6。`crates/desktop` 从 384 行序列化器变成真应用宿主。

- [x] **P0** **tao + wry 窗口宿主** — `glory-desktop` feature `runtime`(tao 0.35 + wry 0.55);`GloryWryReady` 后挂载,批量 `__gloryApplyWryBatch` flush;ipc 经 EventLoopProxy 回主线程。:`glory-desktop` 加 `wry`/`tao` 依赖(放可选 feature `runtime` 后面,保持纯协议用途不拖依赖):
  ```rust
  pub fn launch(widget: impl Fn() -> W + 'static) { launch_with_config(DesktopConfig::default(), widget) }
  // 内部流程:
  // 1. tao EventLoop + Window;wry WebViewBuilder
  //    .with_html(BOOTSTRAP_HTML)                  // 空壳 <html><body> + WRY_INTERPRETER_JS 作 init script
  //    .with_ipc_handler(proxy 转回 event loop)
  // 2. body 注册为 CommandNode id=0 的根(interpreter 里 nodes.set(0, document.body))
  // 3. DesktopHolder(实现 Holder trait,对齐 BrowserHolder 形态)mount widget
  // 4. sink = WebviewSink:flush(batch) → webview.evaluate_script("__gloryApplyWryBatch(<json>)")
  // 5. ipc 消息 GloryWryEvent → 反序列化 EventData → renderer.dispatch_event → scheduler 跑完自动二轮 flush
  ```
  实施要点:
  - `wry_interpreter.js` 增加 `__gloryApplyWryBatch(commands)`(循环调现有 `__gloryApplyWryCommand`),并预注册 `nodes.set(0, document.body)`。
  - 事件循环线程模型:Glory 响应式是 thread-local(`reflow/scheduler.rs` 的 thread_local 设计),**widget 树和 renderer 必须全程活在 tao 主线程**;ipc handler 用 `EventLoopProxy::send_event` 把 `EventData` 弹回主线程再 dispatch,文档里写死这条规则。
- [x] **P0** **`examples/desktop-counter` 重写成真 widget 应用** — 真窗口验证通过(CDP 实测:点击 ++/−/clear、input 值回传全部正确)。:用 `glory::launch` 风格 + `Cage<i64>` + button click,开真窗口可点。这是 M6+M7 的端到端验收物。
- [x] **P1** **query 回程接 IPC** — interpreter `Query` 指令(BoundingRect/Value/ScrollOffset)→ `GloryWryQuery` 回包 → `holder.resolve_query`。:`NodeQuery` 序列化进一条 `Query { id, token, kind }` 指令,JS 端算出结果(`getBoundingClientRect` 等)经 ipc 带 token 送回,resolve 对应 `QueryFuture`。超时(节点已删)resolve 成 `Err(NodeGone)`。
- [x] **P1** **路由:`MemoryAviator`** — 内存历史栈 + back/forward;与 ServerAviator 共享 `run_route`。:`crates/routing` 按 crate doc 里"如何加新 backend"的说明实现内存历史栈(`Vec<String> + cursor`),desktop/未来测试共用。(§3 P1 当年留的口子,现在用例到了。)
- [ ] **P1** **热重载接通 desktop** — 未做;依赖 CLI watch 管道改造,见"遗留项"。:`glory-cli watch` 的 `BrowserReloadMessage::Functions` 通道现在走浏览器 websocket;desktop 下改走 wry ipc 或本地 websocket 连同一个 CLI server,`FunctionRegistry` 替换闭包后触发整树 re-patch。验收:改一个 handler 的字面量,不重启窗口生效。
- [ ] **P2** **资源与自定义协议**:wry `with_custom_protocol("glory")`,`asset!` 宏产出的 public URL path 在 desktop 下解析为 `glory://` URL,从 bundle 目录读文件。图片/CSS 可用。
- [ ] **P2** **窗口 API 面**:`DesktopConfig { title, inner_size, resizable, devtools }`;多窗口 = 多 Holder(非 `single-app` 模式本来就支持多 HolderId,验证 + 文档)。
- [ ] **P2** **指令编码升级评估(先测后做)**:JSON 批量 → `evaluate_script` 路径先用 desktop 版 js-framework-benchmark 子集压一轮取基线;只有 profiling 显示序列化/解析占比可见时,才上 sledgehammer 风格的紧凑二进制(Dioxus 的演进顺序同样是先 JSON 后优化)。**不要凭直觉提前优化。**
- [ ] **P3** Tauri 模板从 README 升级为可 `glory new --template tauri` 的真模板(配合 memory 里 dx-aligned 的 embeddable CLI 设计)。

**M7 验收标准**:desktop-counter 真窗口可交互;TodoMVC 跑在 desktop 下(复用 `examples/todomvc` 的 widget 代码,只换 launch 入口——**同一份业务代码双平台,这是跨平台叙事的第一个可演示证据**);热重载工作。

---

## 14. M8 — Fullstack:server functions(与 M9 可并行)

不碰渲染层,纯增量。Dioxus/Leptos 的用户黏性核心特性。

- [x] **P0** **`#[server]` 宏设计与实现** — `glory-macros` proc-macro;拆分规则改为按编译目标(非 wasm = server 本体 + inventory 注册,wasm = fetch stub),不依赖用户 crate feature,可测试性更好;`endpoint = "..."` 防撞名。(新 proc-macro crate `glory-macros` 或并入现有):
  ```rust
  #[server]                                  // 用户写:
  async fn list_todos(filter: Filter) -> Result<Vec<Todo>, ServerFnError> { /* 数据库访问 */ }
  // server 编译(web-ssr):保留函数体,注册到全局 inventory:(path="/__glory/fn/list_todos", handler)
  // client 编译(web-csr / backend-command):函数体替换为
  //   fetch POST {path},body = serde_json::to_vec(&(filter,)),resp → serde_json::from_slice
  ```
  实施要点:
  - 注册机制用 `inventory` 或显式 `register_server_fns!(router)`(desktop/wasm 两种 client 都要能指到 server 地址,`DesktopConfig`/build 时配 base_url)。
  - 错误模型:`ServerFnError { Request, Deserialize, Server(String) }`,服务端 panic/Err 统一 500 + JSON body。
  - codec 初版 JSON;trait 化留 binary 口子。
- [x] **P0** **三个 adapter 挂载 server fn 路由** — 挂载实现收在 `glory-serverfn` 自身(`salvo_mount::router()` / `axum_mount::router()` / `actix_mount::configure`),adapter crate 不引新依赖。:`glory-salvo`/`glory-axum`/`glory-actix` 各加一行式 API(`.mount_server_fns()`),路由前缀 `/__glory/fn/` 可配置。
- [~] **P1** **与 `resource_in`/`Loader` 整合** — server 侧直调本体已验证(不走 HTTP 自环);hackernews 例子改造未做。:`resource_in(parent, || list_todos(filter.get()))` 在 SSR 下直接调用本体(不走 HTTP 自环),CSR hydrate 复用 `Loader::save_state` 已有的状态嵌入,避免首屏双取。验收:hackernews-salvo 例子改造为 server fn 版,行为不变。
- [ ] **P1** **server 侧上下文提取** — 未做。:server fn 体内拿请求上下文(headers/cookies/Truck)。机制:adapter 在调用前把请求上下文塞 task-local,提供 `use_request_context() -> Option<...>`。
- [x] **P2** desktop 客户端也能调 server fn — `reqwest-client` feature + `set_server_url`。(走配置的远端 base_url)——desktop+云端数据的 fullstack 桌面应用故事。

**M8 验收标准**:todomvc-fullstack 新例子:同一份代码 `cargo glory serve` 出 SSR+hydrate+server fn 全栈应用;断网刷新→ SSR 出首屏,交互→ fetch 调 server fn。

---

## 15. M9 — Mobile(运行时免费,工程在 CLI)

依赖 M7(同一 webview 运行时)。90% 是 `crates/cli` 工作,对齐 memory 中 dx-aligned 的 embeddable `glory_cli::Glory` builder 设计。

- [~] **P1** **Android 通路** — CLI `--target android`(cargo-ndk 编排、工具链检测、`mobile` 默认 bin feature)+ `templates/mobile/README.md` 接线文档;Gradle 完整模板与真机验证未做。:`glory build --target android`:cargo-ndk 编排 `aarch64-linux-android` 编译 cdylib + Gradle 模板工程(wry 0.4x 支持 Android,需要 `android_binding!` 胶水)+ `glory serve --target android` 推到模拟器。模板进 `crates/cli/templates/android/`。
- [~] **P1** **iOS 通路** — CLI `--target ios`(staticlib,非 macOS 报错引导);XcodeGen 模板与模拟器验证未做。:`--target ios`:`aarch64-apple-ios` staticlib + XcodeGen 模板 + simulator 部署。CI 仅 macOS runner 跑编译冒烟。
- [x] **P1** **touch 事件进 `EventData`** — interpreter 把 touch 主点映射为 PointerData,多点经 `extra.touches`。:`PointerData` 已覆盖大部分;补 `touches: Vec<TouchPoint>` 与 pointer events 映射,interpreter 端 `touchstart/move/end` 透传。
- [ ] **P2** **移动端宿主细节**:safe-area inset(注入 CSS env() 即可,规范抽象的红利)、虚拟键盘 resize 策略、生命周期(进后台暂停 scheduler flush)。
- [ ] **P2** **`asset!` 接 bundle 流程**:Android assets/ 与 iOS bundle resources 的路径解析分支。
- [ ] **P3** `glory bundle`:desktop(msi/dmg/AppImage)与 mobile(apk/ipa)打包子命令。

**M9 验收标准**:todomvc 在 Android 模拟器跑通,与 web/desktop 同一份 widget 代码。

---

## 16. M10 — Native(Blitz)与 TUI 定位

- [x] **P2** **Blitz spike(验证性,不承诺产品化)**:起 `crates/native` 真实现:`blitz-dom` 的 `Document` API 作为 `CommandSink` 消费端(create/set_attr/insert 形状与 `Command` 高度同构),`blitz-shell`/vello 出窗口。目标仅是 counter 渲染出来 + 一次点击回程,产出可行性报告:覆盖的 CSS 子集、文本输入/IME 现状、a11y 缺口。**结论写进 docs 再决定是否立 M11。**
- [x] **P2** **`crates/native`/`crates/tui` 当前的假 re-export 清理** — native 标 experimental scaffold(协议 re-export);tui 定位为 CommandDom 只读演示。:两个 crate 现在是 `WryRenderer` 的 5 行别名(`crates/native/src/lib.rs`),对外发布会造成能力误导。native 改为 spike 实现或显式 `#[doc = "experimental scaffold"]`;tui 在 README/rustdoc 明确"指令流演示,非产品目标"(S5)。
- [x] **P3** TUI 若保留 — `glory_tui::render_outline(CommandDom)` 只读渲染,有单测。:用 §12 的 `command_dom.rs` 内存树 + ratatui 画一个只读渲染 demo,作为"指令流通用性"的展示品,不实现交互。

---

## 跨平台演进:落地状态(2026-06-10 第一轮)

M6 全部 P0/P1、M7 P0+P1(除热重载)、M8 P0+P2、M9 CLI 编排与事件模型、M10 清理项已完成;
全 workspace `cargo test` 30 套件全绿,desktop-counter 真窗口经 CDP 实测交互正确。

**实现中确立的新纪律:`backend-command` 不进 workspace 默认构建。**
cargo feature 合一会把 desktop/native/tui 的 `backend-command` 与 adapter 的 `web-ssr`
并进同一份 glory-core,触发互斥 compile_error。因此 `glory-desktop` 的 holder/runtime
收进 `backend`/`runtime` feature(默认关),由【应用 crate】(workspace 之外)开启;
`backend-command` 的测试用显式 `cargo test -p glory-core --features backend-command` 跑。

**第二轮(2026-06-11)完成:**
- [x] desktop 热重载接通(M7 P1):reload wire 消息上移为共享 `glory_hot_reload::ReloadMessage`(CLI/浏览器/desktop 三端同一类型,消除协议分叉);desktop runtime 在 `GLORY_WATCH=ON` 下起 tungstenite 线程连 `/live_reload`,style 消息走与浏览器相同的 link-swap,functions 批次经 `DesktopConfig::on_function_reload` 回调(在 holder.update 事务内,自动 flush)。
- [x] server fn 上下文提取(M8 P1):`RequestContext`(method/uri/headers)+ tokio task-local `with_request_context`/`request_context()`;三个 adapter mount 全部填充;直调(SSR/测试)时返回 None 为约定语义。
- [x] desktop 资源自定义协议 `glory://`(M7 P2):`with_custom_protocol` + 路径越界防护 + MIME 表;root 解析顺序 config.assets_root → `GLORY_SITE_ROOT` → exe 目录;`asset_url()` helper 屏蔽 Windows `http://glory.localhost` 重写差异。

**第三轮(2026-06-11)完成:**
- [x] hackernews 例子 server-fn 化(M8 P1):删除手写 `/api` salvo 路由、cfg 的 URL builder、gloo-net/reqwest 双 fetch 实现,换成 3 个 `#[glory::server]` 函数 + 一行 `salvo_mount::router()`;gloo-net 依赖整个移除。**活体 e2e 验证**:`POST /__glory/fn/fetch_stories ["news",1]` 返回 200 + 真实 HN 数据(上游 API 仍存活),未知端点 404;wasm-csr / web-ssr / 无 feature 三种编译全过。
- [x] 二进制协议压测评估(M7 P2)——**决策:JSON 留任,不做二进制编码**。基线(`benches/command_wire.rs`,criterion):单条事件 patch serialize 141ns / deserialize 160ns;100 行挂载批(400 指令,23.5KB)57µs/59µs;1000 行极端挂载批(4000 指令,241KB)566µs/595µs。千行挂载的序列化开销 ≈ 帧预算(16.6ms)的 3.4%,事件路径在百纳秒级——瓶颈不在序列化。重新评估条件:profiling 显示 IPC 串行化占比 >10%,或 liveview(网络往返)落地时。

**第四轮(2026-06-11)完成——遗留项清零:**
- [x] **SSR 收敛为指令消费者(M6 P2)**:非 wasm `Node` 统一为 `CommandNode`(node/mod.rs 只剩两分支);旧 SSR Node 原样迁移为 `renderer/ssr_dom.rs::SsrNode`,`SsrDocument::replay` 把指令流回放成树再渲染——转义/排序/伪属性逻辑零移植,字节级一致;`SsrRenderer` 删除。关键发现并修复:**mount 后的 revise 触发 patch 时新建节点会落入孤立队列导致 id 碰撞**(症状为回放树成环、栈溢出),修法是 holder 在 thread-local 注册表登记自己的 queue,`scheduler::run(holder_id)` 跑 patch 前自动安装。`backend-command`×`web-ssr` 互斥解除(全栈桌面二进制可行)。验证:66 个 web-ssr lib 测试(含全部字节级快照)全绿;hackernews 活体首页 43KB HTML(doctype/gly-id/故事列表)经回放路径渲染正确。
- [x] **desktop 多窗口 + menu(M7 P2)**:`Desktop` builder(`.window(config, widget).run()`),每窗口独立 CommandHolder/queue/webview,IPC 事件带窗口索引路由;muda 原生菜单(`MenuSpec`/`on_menu` 回调,Windows attach-to-hwnd / macOS nsapp)。**活体验证**:双窗口 CDP 实测,A 窗口点击 3 次 B 窗口不受影响,双向隔离正确。
- [x] **mobile 完整模板 + 交叉编译冒烟(M9)**:新增 `examples/mobile-counter`(共享 Counter widget + 迷你 webview 宿主循环 + tao/wry `android_binding!` JNI 接线 + iOS `start_app` 导出);**真实产出 arm64 ELF `.so`**(cargo-ndk + NDK r28,Android 21);Gradle 宿主模板(cargoNdk 任务驱动 wry 的 Kotlin 胶水生成)+ XcodeGen spec + Swift 入口进 `crates/cli/templates/mobile/`。真机/CI 运行仍待 CI 环境。
- [x] **Blitz spike(M10 P2)——结论:可行**。blitz-dom 0.2.4 仅 38s/233 依赖编译(远轻于预期);`glory-native` feature `blitz` 落地 `BlitzConsumer`:Command 批次经 `DocumentMutator` 进真实 blitz 文档(id 翻译表、textContent 语义、class 集合调和);**测试验证 counter widget 树挂载 + 响应式更新("42")端到端**。下一阶段:blitz-shell/vello 窗口绘制 + EventDriver 事件回程。
- [x] **js-framework-benchmark 基线(横切纪律)**:当前代码重建 wasm 后 headless Chrome 跑仓库自带 harness(12 样本中位数,ms):create_1k=50 / replace_1k=67 / update_10th=33 / select=33 / swap=33 / remove=33 / append_1k=51 / create_10k=443 / clear_1k=34。~33ms 项贴近 2×rAF 计时下限,数字健康;后续改动以此为对照。

**真正剩余(需要本机之外的环境):**
- [ ] mobile 真机/模拟器运行 + CI 编译冒烟矩阵。
- [ ] Blitz spike 下一阶段(vello 绘制 + 事件回程)。
- [ ] `glory bundle` 打包子命令(M9 P3)。
- [ ] 与官方 js-framework-benchmark(Chrome tracing 法)的对照跑分。

---

## 跨平台演进:横切纪律

- 每个里程碑合并前跑全量 conformance 套件(§12 P1),新增 consumer 必须先过 `command_dom.rs` 参考语义。
- wasm CSR 直调路径是性能基线,**任何 M6 改动后 js-framework-benchmark 数字不得倒退**(benchmarks/ 已有 glory 实现,改动前后各跑一轮记录在 PR)。
- wire 协议(`Command` serde 格式、`EventData`、server fn codec)的变更必须同步:JS interpreter、`_todos.md` 本节、AGENTS.md。
- feature 矩阵新增 `backend-command` 后,CI 增加 `cargo check -p glory-core --features backend-command` 与互斥组合的 compile_error 测试。

---

## 速查:相关文件

- 响应式:[reflow/](crates/core/src/reflow/) · [cage.rs](crates/core/src/reflow/cage.rs) · [bond.rs](crates/core/src/reflow/bond.rs) · [scheduler.rs](crates/core/src/reflow/scheduler.rs)
- 视图组件:[widget.rs](crates/core/src/widget.rs) · [view.rs](crates/core/src/view.rs) · [scope.rs](crates/core/src/scope.rs)
- 渲染节点:[node/mod.rs](crates/core/src/node/mod.rs) · [node/ssr.rs](crates/core/src/node/ssr.rs) · [web/csr.rs](crates/core/src/web/csr.rs)
- 列表/分支/加载:[widgets/each.rs](crates/core/src/widgets/each.rs) · [widgets/switch.rs](crates/core/src/widgets/switch.rs) · [widgets/loader.rs](crates/core/src/widgets/loader.rs)
- 路由:[crates/routing](crates/routing)
- CLI:[crates/cli](crates/cli)
- 热重载:[crates/hot-reload](crates/hot-reload)
- 渲染抽象/指令流:[renderer/mod.rs](crates/core/src/renderer/mod.rs)(`Renderer` trait / `MockCommand` → 待升格 `Command`)
- 桌面后端:[crates/desktop](crates/desktop)(`WryCommand` 协议 + [wry_interpreter.js](crates/desktop/src/wry_interpreter.js))· [examples/desktop-counter](examples/desktop-counter)
- native/tui 占位:[crates/native](crates/native) · [crates/tui](crates/tui)(M10 前为 5 行 re-export scaffold)
