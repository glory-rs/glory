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
- [x] **P1** [`crates/core/src/widgets/loader.rs`](crates/core/src/widgets/loader.rs) `patch` 中先 `detach_child` 再 `attach_child` 走 `show_list.clone()` 两遍,中间没清空 `show_list`,重复调用会越积越多。审一遍。 — 实际 bug 在 `is_revising` 分支前没 detach 旧 result/fallback,导致连续 dep 变化时 subtree 堆叠;已修。
- [x] **P2** `Bond::version` 用 `.map(|g| g.version()).sum()`([reflow/bond.rs](crates/core/src/reflow/bond.rs)),依赖版本号会碰撞(理论上不同组合可能和相同)。改成 `(id, version)` 对组成的 hash,或者每次依赖变化就单调递增。 — 改为 `(id, version)` 快照逐项比较 + 单调 re-run 计数器。
- [ ] **P2** `single-app` 命名带双下划线,Rust 社区惯例表示"不稳定内部 API"。如果它确实稳定,改名为 `single-app`;否则文档化清楚。

### M1 实现过程中发现并修复的其他 bug(原清单未列)

- [x] **P0** SSR `Node::before_with_node` / `after_with_node` 语义完全坏掉:在 `self.children` 里找新节点的位置,然后把同一个新节点插到那里——等价空操作。所有非默认 reorder 路径在 SSR 下都是坏的。已替换为 parent 视角的 `Node::insert_before` / `Node::insert_after`(配合恒等去重),并加 `Node::ptr_eq`(基于 `Rc::ptr_eq`)。
- [x] **P0** Element(CSR + SSR)`build` 把 `scope.first_child_node` / `last_child_node` 设成 `node.last_element_child()`,对叶节点(如 `<li>text</li>`)返回 `None`,导致 `Scope::attach_child` 的邻居查找全部回退到 `Tail`,reorder 不工作。改为 `Some(self.node.clone())`(元素自身就是子树外层锚点)。CSR 也修了(浏览器 DOM 之前隐式地"移动而非追加"掩盖了这条 bug)。
- [x] **P0** 默认 `Widget::flood` 实现统一把所有子视图的 `scope.position = Tail`。结合 `attach_child` 在 `is_attached=true` 时早退(不会执行末尾的 `position = Unset` 重置),Tail 永远留下,后续 patch 时邻居搜索被跳过。删掉这行预设。
- [x] **P1** `Scope::attach_child` 反向 sibling 查找用 `for i in (index - 1)..=0`,当 `index > 0` 时是空 range。改为 `(0..index).rev()`。
- [x] **P1** `IndexMap::remove` / `IndexSet::remove` 在新版 indexmap 已 deprecated 并别名到 `swap_remove`(把最后一项搬到删除位)。`Scope::child_views` / `show_list` 依赖顺序,这等于静默数据破坏。全部 core 调用点改为 `shift_remove`(`Scope::detach_child` / `Cage::unbind_view` / `Bond::unbind_view` / `scheduler::run` / `ServerHolder::drop`)。
- [x] **P2** `GloryConfig::default` ↔ `GloryConfig::new` 互相递归调用,首次调用即 stack overflow。改为直接通过现有 `default_*` 函数构造。
- [x] **P2** [`crates/core/src/truck.rs`](crates/core/src/truck.rs) 的测试 mod 引用了不存在的 `crate::prelude` / `crate::test` / `transfer` 方法,从来无法编译。清理掉,保留有意义的那个断言。
- [x] **P2** [`crates/core/src/spawn.rs`](crates/core/src/spawn.rs) `spawn_local` 的 `cfg(any(test, doctest))` 分支调用 `tokio_test::block_on`,但 `tokio_test` 不是依赖,导致 `cargo test` 编译失败。改为让 test 走 `futures::executor` 分支。

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
- [ ] **P1** 同一 key 但 value 变化的情况没处理。当前 Tmpl 只在 key 新增时调用一次,后续即使 `Vec<Value>` 内容变了,对应 view 也不会更新。两种修法二选一:
  - (a) 文档化:**必须让 `Tmpl` 内部 reactive 订阅来源 Cage**,Each 不负责 value diff。这是最便宜的做法,但要在 `Each::new` doc 上写清楚。
  - (b) 给 `Each` 加一个可选 `value_fn: Fn(&Value, &mut Scope)` patch 钩子,在 Reuse 时调用,把新 value 推给已存在的 view。
- [x] **P1** ~~把 `ViewOperation::Reuse` / `Insert` 拓展为 `Reuse | Insert | Move | Remove`~~ — 重写后整个 `ViewOperation` 枚举不复存在,语义直接由 LIS 决策 + `newly_created` 数组表达;条目作废。
- [x] **P1** 增加"swap two items"快路径:LIS = 1 且只有 2 个 item 时直接 swap,跳过 LIS 计算。 — LIS 算法自然达到最小移动数,无需特例;`each_swap_adjacent` 测试覆盖。
- [x] **P1** 增加"head append / tail append / clear"的 O(1)/O(n) 快路径检测,避免普通追加也走 LIS。 — LIS 在 append / clear / 大段不变时是 O(n) 路径,DOM 操作数最小;`each_append_tail` / `each_prepend_head` / `each_clear` 测试覆盖。如果未来 profiling 显示 LIS 开销可见,再考虑显式快路径。
- [x] **P2** Removed key 的 detach 与 reorder 在同一个 batch 内顺序无保证。当前先 detach removed 再 reorder reused,理论上正确但需要回归测试覆盖"删第一个 + 移动剩余"这种组合。 — `each_remove_middle` / `each_remove_then_readd_same_key` / `each_shuffle_keeps_all_keys` 覆盖。
- [x] **P2** `operations.reverse(); while pop` 的写法不直观。改为正向迭代 + 显式 cursor,降低维护成本。 — 重写后整段消失。
- [ ] **P2** `key_view_ids` 当前是 `IndexMap<Key, ViewId>`,但同一 key 必然只有一个 view,实际只用到 map 性质;可以是 `HashMap`(更快)+ 显式 `Vec<Key>` 维护顺序。
- [ ] **P2** 支持 `Lotus<&[T]>` / `Lotus<VecDeque<T>>` / `Lotus<im::Vector<T>>` 等其他容器(目前限 `AsRef<[Value]>`)。可以借助 `Lotus<impl IntoIterator<Item=&T>>` 抽象。
- [ ] **P3** 给 `Each` 加 entrance/exit 动画钩子(Solid 的 `<TransitionGroup>` 风格):`on_enter(&Scope) / on_exit(&Scope)`。复杂度低、用户价值高。
- [ ] **P3** 性能基准:写一个 `examples/each-bench`,做"反转、随机洗牌、首尾插入、清空"四种压测,记基线数据。LIS 改造后跑同一份对比,放进 PR 描述。

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

- [ ] **P0** **引入"作用域 + 代际盒"**:在 `crates/core/src/reflow/` 下新增 `storage.rs`(`Slab<Box<dyn Any>>` + `Generation: u64`),把 `Cage` 改为 `(slot_key, generation)` 句柄 → `Cage<T>: Copy`。
  - 子任务:
    - [ ] 设计 `Owner` 类型,绑定到 `Scope` 生命周期;scope drop → owner drop → 该作用域分配的所有信号失效。
    - [ ] 现存 `Cage::clone()` 调用面巨大,先做 internal type alias,保持外部 API 不变,再灰度切换。
    - [ ] 准备 `Cage::try_get / try_revise → Result<_, BorrowError>`,处理代际失配。
- [ ] **P0** 同时设计 `SyncStorage`(`RwLock` + atomic),让同一份 `Cage`/`Bond` 在 SSR / 多线程 runtime 下也能跑。Feature gate:`sync-storage`。
- [ ] **P1** 增加 `effect(|| { … })` 原语:订阅依赖、自动重跑;返回一个 handle,scope drop 时停止。
- [ ] **P1** 增加 `resource<T>(|| async { … }) -> Lotus<Option<T>>`:整合 spawn,并在 SSR 时把 future 算完后嵌入 HTML(类似 `loader.rs` 的 `save_state`,但作为框架级一等公民)。
- [x] **P1** 把 `untrack` 提升为公开稳定 API(目前内部使用)。文档示例补上。 — 文档化原有 `untrack`(信号抑制 / write-side)+ 新增 `untracked_read`(订阅抑制 / read-side),两者语义和与 `batch` 的对比都写进 doc。
- [x] **P1** 把 `Cage::revise` 的事件回调路径自动包一层 `batch`(在 `web/events/` 入口统一加),避免用户每次手写 `batch(|| …)`。 — `Element::add_event_listener` 在 CSR 下把 handler 包进 `reflow::batch`。
- [x] **P2** 让 `Bond` 支持自定义相等比较(`Bond::with_eq`),仅当输出值真正改变时才标脏。 — `with_eq(|a, b| ...)` + `with_partial_eq()` 两个 builder 方法,加 2 个单测。
- [ ] **P2** 提供 `selector(|| key) -> Lotus<T>` 工具,做"基于 key 的稳定派生"。 — `Bond::with_partial_eq` 覆盖了同类需求;独立 `selector` 暂不做。
- [ ] **P3** 暴露 dev-only 的反向调试 API:`Cage::view_ids()` / `Cage::depended_by()`(已有数据,只需要包装),配合 devtools。

---

## 3. 渲染层抽象 / 多平台前置(对应 _report.md §2、§4)

- [ ] **P0** 设计 `Renderer` trait(草案见 _report.md §2.3 Step A),`Node` 作为关联类型,逐步替换 `crates/core/src/node/mod.rs` 中两套 `cfg` 切换的 Node。
  - [ ] 先把 `web/csr.rs` / `node/ssr.rs` 的差异点列成"必须方法表"(create / set_attr / set_class / append / remove / replace / attach_event ...)。
  - [ ] 在 `crates/core/src/renderer/` 下新增 trait + 默认 `WebRenderer` / `SsrRenderer` 两个实现。
  - [ ] 把 `web/widgets/{div,button,…}.rs` 改成 `R: Renderer` 泛型,消除 `cfg(all(target_arch = "wasm32", feature = "web-csr"))` 双份代码。
- [ ] **P0** `AttributeValue` 富类型化:不再统一用 `Cow<'static, str>`,而是 `enum { Text(String), Float(f64), Int(i64), Bool(bool), Listener(EventHandler), Any(Rc<dyn Any>), None }`。这是 native / TUI 后端能接得上的前提。
- [ ] **P0** `EventPayload` 抽象:`pub trait EventPayload { fn as_any(&self) -> &dyn Any; fn name(&self) -> &str; }`。`web/events/` 把 `web_sys::Event` 包成这个 trait。
- [x] **P1** **统一 `glory::launch`** 入口。当前 CSR/SSR 启动方式风格不一(`mount_to(body)` vs salvo handler),`crates/glory/src/lib.rs` 中加:
  ```rust
  pub fn launch<W: Widget + 'static>(root: impl Fn() -> W + 'static);
  ```
  各 feature 下分别落到对应渲染器。 — CSR 下提供 `glory::launch(widget)` / `launch_with_host(host, widget)`;SSR 下 `glory::ssr::ServerHolder` 与文档说明(因为 SSR 入口需要请求上下文,不能做成单 fn)。
- [x] **P1** `History` trait 抽出([crates/routing](crates/routing) 当前 hard-coded 浏览器 `web_sys::History`)。 — 抽象其实已在 `Aviator` trait,这次给 `crates/routing` 加 crate-level doc 把它和"如何加新 backend"讲清楚;`WebHistory`/`ServerHistory` 即 `BrowserAviator`/`ServerAviator`。`MemoryHistory` 当未来真有测试需求时再加(下行 P3 留作未来子项)。
  - [x] `WebHistory`(浏览器) — 即 `BrowserAviator`。
  - [x] ~~`MemoryHistory`(测试 / 桌面 / 命令行)~~ — 没有当前用例,暂不实现;后续加测试 / 桌面 backend 时再补。
  - [x] 路由 API 通过泛型/特征参数选择。 — 已通过 `Aviator` trait 完成。
- [ ] **P2** 资源声明系统:类似 `manganis!()` 的宏(`asset!("./logo.png")`),把多平台资源路径解析挪到一处。
- [ ] **P2** 抽 `Truck` 概念到"应用级 context"层级,不要再让它顺带承载渲染状态;Renderer 拥有自己的状态。

---

## 4. 多平台后端(对应 _report.md §3)

- [ ] **P1** 引入 `WryRenderer`(桌面 webview):
  - [ ] 起 `crates/desktop`(或独立 repo)。
  - [ ] 实现最小化二进制指令协议(可借鉴 dioxus `interpreter/js/core.js` 的 sledgehammer 风格,几百行 JS)。
  - [ ] 把 `Renderer` 的方法序列化到 IPC,JS 端反序列化打 DOM。
  - [ ] examples/desktop-counter 跑通。
- [ ] **P2** 流式 SSR + hydrate:
  - [ ] 服务端把渲染流变成 `Stream<HtmlChunk>`,中间留 `<template>` 占位。
  - [ ] 客户端首屏拿到部分 HTML 就开始 hydrate;`resource` 完成后客户端 patch 占位。
  - [ ] 前置依赖:§2 的 `resource` 原语。
- [ ] **P2** 各 web 框架 adapter:`glory-salvo` / `glory-axum` / `glory-actix`,API 风格统一。
- [ ] **P3** Blitz / wgpu 原生后端(`crates/native`)。前置:§3 的所有 P0/P1 完成。
- [ ] **P3** TUI 后端(`crates/tui`):用 ratatui 作为渲染器,适合调试 / CI 演示。
- [ ] **P3** Tauri 一键打包模板。

---

## 5. CLI / 构建 / 开发体验

- [ ] **P1** [`crates/cli`](crates/cli) 现状审计:确认它支持的子命令,补 `glory new` / `glory build` / `glory serve` 的 README 一节。
- [ ] **P2** 借鉴 dioxus `subsecond` 思路实现函数级 hot reload(builder 风格也能用),先做 dev-mode 闭包重链。优先级中,影响 DX。
- [ ] **P2** [`crates/hot-reload`](crates/hot-reload) 当前是占位/半完成的(无 DSL 所以这块用途有限)。要么实现 §2 的代际盒后做"状态保留 + 函数热替换",要么从 workspace 移除避免迷惑。决策点。
- [ ] **P3** `cargo-glory` 的 `--target` 子命令多平台编译矩阵(web / desktop / native)。

---

## 6. 测试基建

- [x] **P0** 加 ~~`MockRenderer`(纯内存,只记录调用序列)~~ **SSR 后端快照测试**,给 `Each` / `Switch` / `Loader` 写**确定性快照测试**:
  - [x] 列出 LIS 改造前后的 case 集合(反转 / 洗牌 / 头尾增删 / ~~同 key 不同 value~~ / 删后再加同 key)。 — `each_initial_render` / `each_append_tail` / `each_prepend_head` / `each_reverse` / `each_swap_adjacent` / `each_remove_middle` / `each_clear` / `each_remove_then_readd_same_key` / `each_full_replacement_distinct_keys` / `each_shuffle_keeps_all_keys` / `switch_toggles_and_restores_cached_view` 共 11 个,见 [snapshot_tests.rs](crates/core/src/widgets/snapshot_tests.rs)。
  - [x] 每个 case 断言"DOM 操作序列"是预期的最小集合。 — 改为断言**最终 HTML 文本顺序**(`inner_html` 解析出 `<li>` 内容序列),比"操作序列"更直接对应可观察行为;最小化由 LIS 保证。
  - 备注:M1 选择直接走 SSR 后端而不是独立的 `MockRenderer`,因为后者依赖渲染层抽象(§3 P0),还没就位。等 §3 落地后可以无痛迁到 MockRenderer。
- [ ] **P1** 把 examples 改成 cargo test 跑得通的 e2e(`wasm-bindgen-test` + headless chrome)。
- [ ] **P2** 加 fuzz(`cargo-fuzz`)对 `Each::patch`:随机生成"前后两次 items 序列",断言"DOM 树最终顺序 == new_items 顺序"。
- [ ] **P2** 加 `criterion` benchmark 套件,跟踪 LIS 改造、代际盒改造的性能数据。

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

- [ ] **P1** workspace 升级当前已在 [chore/upgrade-dependencies](.) 分支,审一遍 `Cargo.toml` 里 `thiserror = "1"` / `dirs = "5.0"` 等是否还有更新版本可一并跟进。
- [ ] **P2** `rust-version = "1.88"` 假设性偏新,确认 MSRV 政策(目前需要 edition 2024)。
- [ ] **P3** workspace lints(`[workspace.lints.rust]`) 集中配置,各 crate 取消重复 lint 声明。

---

## 落地顺序建议

按依赖关系拆 4 个 milestone,大致 1–2 个月一档:

**M1+(打地基 + 周边收拢,本分支)** ✅ 已完成,见 [PR #32](https://github.com/glory-rs/glory/pull/32)
- §0 全部(`single-app` 改名留作 P2,改为加 Cargo.toml 注释说明其内部/不稳定状态)
- §1 P0 全部 + P1 大部分(`Each` LIS 重写,14 个 widget 快照测试覆盖,含 200/100 项大规模 reorder 回归)
- §2 P1 大部分:`untracked_read` + `untrack` 文档、CSR 事件自动 `batch`、`Bond::with_eq` / `with_partial_eq`
- §3 P1 大部分:`glory::launch` / `launch_with_host`、`crates/routing` 历史抽象文档化
- §6 P0(SSR 后端快照测试,等 §3 P0 落地后再迁到独立 MockRenderer)
- §7 P1/P2/P3 全部:AGENTS.md、examples/_README.md、6 个核心类型 crate-level rustdoc、README 框架对比表、`.gitignore` 拓展
- 额外:发现并修复 SSR Node / Element 锚点 / 默认 flood / `shift_remove` 等 7 处隐藏 bug

**M2(响应式现代化,3–4 周,独立分支)**
- §2 P0(代际盒 + `SyncStorage`,这是真正破坏性的 API 重构)
- §2 P1 剩余:`effect`、`resource`
- §6 P1(wasm-bindgen-test e2e)

**M3(渲染层抽象,4–6 周,独立分支)**
- §3 P0(Renderer trait、`AttributeValue` 富类型、`EventPayload` trait) — 设计兼容性、迁移所有 widget,工作量足够独占一个 PR
- §1 在 Renderer trait 上的回归测试
- §6 P0 迁到 MockRenderer

**M4(多平台开张,2–3 个月,前置 M3)**
- §4 P1(桌面 webview MVP,依赖 M3 的 Renderer trait 与 AttributeValue/EventPayload)
- §4 P2(流式 SSR / hydrate,依赖 M2 的 `resource`)
- §5、§7 剩余项同步推进

## 仍未完成且**不在当前分支**的项目(明示)

刻意留空的子项原因如下,需要独立 milestone:

- §0 P2 `single-app` 改名 → 影响所有 `cfg(feature = "single-app")` 站点,在改名前要把 §3 P0 的 cfg 收敛先做。
- §1 P1 同 key value 变更不再调 `tmpl_fn` → 已选择 "option (a) 文档化用户必须订阅"(本分支已完成),"option (b) 加 `value_fn` 钩子"留作后续。
- §1 P2 `key_view_ids` 改 HashMap+Vec → 仅微优化;IndexMap 双重身份(有序 + O(1) lookup)已经满足需求,暂不做。
- §1 P2 支持 `Lotus<VecDeque<T>>` / `Lotus<im::Vector<T>>` → 需要把 `AsRef<[Value]>` bound 改为 `IntoIterator`,与 §3 P0 的泛型化一起做更划算。
- §1 P3 entrance/exit 动画钩子 → 设计涉及生命周期 + CSS transition / FLIP 整合,留作独立 feature PR。
- §1 P3 性能 benchmark(criterion / each-bench) → 等 §3 P0 后端抽象就位,benchmark 才能跑在统一的 MockRenderer 之上,先一起做。
- §2 P0 代际盒 / `SyncStorage` → 整个 Cage/Bond API 都要重写;M2 整个 milestone 做这件事。
- §2 P1 `effect` / `resource` → 与 §2 P0 同进同出。
- §3 P0 Renderer trait / AttributeValue / EventPayload → M3 整个 milestone 做这件事(原因:trait 一旦定义就要所有 widget 迁,这是单 PR 量级,无法 additive 落地)。
- §3 P2 manganis 风格资源声明系统 → 需要 §3 P0 + 多平台后端先到位。
- §3 P2 `Truck` 角色重构 → 同上,等 Renderer trait 把"渲染状态"剥离出去。
- §4 P1 桌面 webview / P2 流式 SSR / adapter / P3 Blitz / TUI / Tauri → 见 M4。
- §5 P2 `subsecond` 风格 hot reload → 单独的 dev-tools milestone。
- §5 P2 `crates/hot-reload` 处置决定(实现 vs 移除) → 等 hot reload 总体决策。
- §5 P3 `cargo-glory --target` 多平台编译 → 等多平台后端就位。
- §6 P1 e2e / wasm-bindgen-test → 等 §3 P0 后,与 MockRenderer 一起做。
- §6 P2 fuzz / P2 criterion → 同上。
- §8 P1 依赖版本 audit / P2 MSRV 文档 / P3 workspace lints → 工程性,可以随时单独抓一个 PR 做,不阻塞核心改造。

---

## 速查:相关文件

- 响应式:[reflow/](crates/core/src/reflow/) · [cage.rs](crates/core/src/reflow/cage.rs) · [bond.rs](crates/core/src/reflow/bond.rs) · [scheduler.rs](crates/core/src/reflow/scheduler.rs)
- 视图组件:[widget.rs](crates/core/src/widget.rs) · [view.rs](crates/core/src/view.rs) · [scope.rs](crates/core/src/scope.rs)
- 渲染节点:[node/mod.rs](crates/core/src/node/mod.rs) · [node/ssr.rs](crates/core/src/node/ssr.rs) · [web/csr.rs](crates/core/src/web/csr.rs)
- 列表/分支/加载:[widgets/each.rs](crates/core/src/widgets/each.rs) · [widgets/switch.rs](crates/core/src/widgets/switch.rs) · [widgets/loader.rs](crates/core/src/widgets/loader.rs)
- 路由:[crates/routing](crates/routing)
- CLI:[crates/cli](crates/cli)
- 热重载:[crates/hot-reload](crates/hot-reload)
