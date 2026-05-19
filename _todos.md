# Glory-rs 改进清单(_todos)

> 配套 [`_report.md`](_report.md) 使用。本文件是落地清单,可直接当任务板。
> 约定:
> - **优先级**:P0 = 阻塞后续多平台演进 / P1 = 高价值近期可做 / P2 = 中期 / P3 = 长期或可选。
> - **状态**:`[ ]` 未开始,`[~]` 进行中,`[x]` 完成。
> - 路径都用相对仓库根的形式。
> - 刻意不包含 RSX / DSL 相关项。

---

## 0. 紧急 / 现存 Bug

- [ ] **P0** [`crates/core/src/widgets/each.rs:103`](crates/core/src/widgets/each.rs) 留着 `crate::warn!("key_view_ids: {:?}", key_view_ids);` 调试日志,生产构建噪声且暗示算法没收尾。先移除,然后做下面的 §1 重排算法。
- [ ] **P1** [`crates/core/src/widgets/switch.rs`](crates/core/src/widgets/switch.rs) `Case::cached_view` 路径:`detach_child` 后 View 从 `ctx.child_views` 移除并存在 `Case::cached_view`,再次激活时只调 `attach_child(&view_id)`,但 `child_views` 已没这个 id。需要把 cached_view 重新塞回 `child_views`(或重做缓存语义)。补一个回归测试。
- [ ] **P1** [`crates/core/src/widgets/loader.rs`](crates/core/src/widgets/loader.rs) `patch` 中先 `detach_child` 再 `attach_child` 走 `show_list.clone()` 两遍,中间没清空 `show_list`,重复调用会越积越多。审一遍。
- [ ] **P2** `Bond::version` 用 `.map(|g| g.version()).sum()`([reflow/bond.rs](crates/core/src/reflow/bond.rs)),依赖版本号会碰撞(理论上不同组合可能和相同)。改成 `(id, version)` 对组成的 hash,或者每次依赖变化就单调递增。
- [ ] **P2** `__single_holder` 命名带双下划线,Rust 社区惯例表示"不稳定内部 API"。如果它确实稳定,改名为 `single-app`;否则文档化清楚。

---

## 1. `Each` 列表重排 / item 复用算法(单列重点)

文件:[`crates/core/src/widgets/each.rs`](crates/core/src/widgets/each.rs)。

**当前状态(读自源码 patch 实现 79–142 行):**
- 对所有 item 顺序扫描,新 key 走 `Insert`,旧 key 走 `Reuse`。
- 删除的 key 在循环结束后一次性 `detach_child`。
- 应用 operation 时:每个 `Reuse` 都做 `IndexMap::move_index` + `is_attached=false` + `attach_child`,每个 `Insert` 同样把刚 store 的 view `move_index` + `attach_child`。

**算法问题清单:**
- [ ] **P0** 用 LIS(Longest Increasing Subsequence)选出"不需要移动"的子序列。**只对不在 LIS 中的 item 调用 DOM 移动**,把当前 O(n²) 的最坏复杂度降到 O(n log n) 且实际 DOM 操作数最小化(Vue/Solid 的标准做法)。
- [ ] **P0** Reuse 路径"位置未变也照样 attach"是浪费。判断:若上一轮 `child_views` 中的 view 索引 == 这一轮目标索引,**完全不动 DOM**。
- [ ] **P0** `IndexMap::move_index` 单次操作 O(n);连续 N 次 = O(n²)。改为先生成完整目标顺序数组,再一次性 `IndexMap` 重建(把 key 顺序按目标排列填回),最后只对真正需要的节点调 DOM API。
- [ ] **P1** 同一 key 但 value 变化的情况没处理。当前 Tmpl 只在 key 新增时调用一次,后续即使 `Vec<Value>` 内容变了,对应 view 也不会更新。两种修法二选一:
  - (a) 文档化:**必须让 `Tmpl` 内部 reactive 订阅来源 Cage**,Each 不负责 value diff。这是最便宜的做法,但要在 `Each::new` doc 上写清楚。
  - (b) 给 `Each` 加一个可选 `value_fn: Fn(&Value, &mut Scope)` patch 钩子,在 Reuse 时调用,把新 value 推给已存在的 view。
- [ ] **P1** 把 `ViewOperation::Reuse` / `Insert` 拓展为 `Reuse | Insert | Move | Remove`,语义化区分**位置变化**和**节点变化**;现在 Insert 和 Reuse 都隐含 "需要 attach_child",信号不清。
- [ ] **P1** 增加"swap two items"快路径:LIS = 1 且只有 2 个 item 时直接 swap,跳过 LIS 计算。
- [ ] **P1** 增加"head append / tail append / clear"的 O(1)/O(n) 快路径检测,避免普通追加也走 LIS。
- [ ] **P2** Removed key 的 detach 与 reorder 在同一个 batch 内顺序无保证。当前先 detach removed 再 reorder reused,理论上正确但需要回归测试覆盖"删第一个 + 移动剩余"这种组合。
- [ ] **P2** `operations.reverse(); while pop` 的写法不直观。改为正向迭代 + 显式 cursor,降低维护成本。
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
- [ ] **P1** 把 `untrack` 提升为公开稳定 API(目前内部使用)。文档示例补上。
- [ ] **P1** 把 `Cage::revise` 的事件回调路径自动包一层 `batch`(在 `web/events/` 入口统一加),避免用户每次手写 `batch(|| …)`。
- [ ] **P2** 让 `Bond` 支持自定义相等比较(`Bond::with_eq`),仅当输出值真正改变时才标脏。
- [ ] **P2** 提供 `selector(|| key) -> Lotus<T>` 工具,做"基于 key 的稳定派生"。
- [ ] **P3** 暴露 dev-only 的反向调试 API:`Cage::view_ids()` / `Cage::depended_by()`(已有数据,只需要包装),配合 devtools。

---

## 3. 渲染层抽象 / 多平台前置(对应 _report.md §2、§4)

- [ ] **P0** 设计 `Renderer` trait(草案见 _report.md §2.3 Step A),`Node` 作为关联类型,逐步替换 `crates/core/src/node/mod.rs` 中两套 `cfg` 切换的 Node。
  - [ ] 先把 `web/csr.rs` / `node/ssr.rs` 的差异点列成"必须方法表"(create / set_attr / set_class / append / remove / replace / attach_event ...)。
  - [ ] 在 `crates/core/src/renderer/` 下新增 trait + 默认 `WebRenderer` / `SsrRenderer` 两个实现。
  - [ ] 把 `web/widgets/{div,button,…}.rs` 改成 `R: Renderer` 泛型,消除 `cfg(all(target_arch = "wasm32", feature = "web-csr"))` 双份代码。
- [ ] **P0** `AttributeValue` 富类型化:不再统一用 `Cow<'static, str>`,而是 `enum { Text(String), Float(f64), Int(i64), Bool(bool), Listener(EventHandler), Any(Rc<dyn Any>), None }`。这是 native / TUI 后端能接得上的前提。
- [ ] **P0** `EventPayload` 抽象:`pub trait EventPayload { fn as_any(&self) -> &dyn Any; fn name(&self) -> &str; }`。`web/events/` 把 `web_sys::Event` 包成这个 trait。
- [ ] **P1** **统一 `glory::launch`** 入口。当前 CSR/SSR 启动方式风格不一(`mount_to(body)` vs salvo handler),`crates/glory/src/lib.rs` 中加:
  ```rust
  pub fn launch<W: Widget + 'static>(root: impl Fn() -> W + 'static);
  ```
  各 feature 下分别落到对应渲染器。
- [ ] **P1** `History` trait 抽出([crates/routing](crates/routing) 当前 hard-coded 浏览器 `web_sys::History`)。
  - [ ] `WebHistory`(浏览器)
  - [ ] `MemoryHistory`(测试 / 桌面 / 命令行)
  - [ ] 路由 API 通过泛型/特征参数选择。
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

- [ ] **P0** 加 `MockRenderer`(纯内存,只记录调用序列),给 `Each` / `Switch` / `Loader` 写**确定性快照测试**:
  - [ ] 列出 LIS 改造前后的 case 集合(反转 / 洗牌 / 头尾增删 / 同 key 不同 value / 删后再加同 key)。
  - [ ] 每个 case 断言"DOM 操作序列"是预期的最小集合。
- [ ] **P1** 把 examples 改成 cargo test 跑得通的 e2e(`wasm-bindgen-test` + headless chrome)。
- [ ] **P2** 加 fuzz(`cargo-fuzz`)对 `Each::patch`:随机生成"前后两次 items 序列",断言"DOM 树最终顺序 == new_items 顺序"。
- [ ] **P2** 加 `criterion` benchmark 套件,跟踪 LIS 改造、代际盒改造的性能数据。

---

## 7. 文档 / 项目卫生

- [ ] **P1** 根目录补 `AGENTS.md`(或扩 `CLAUDE.md`),写明:
  - feature flag 规则与组合矩阵
  - "signals 不要替换成第三方"等设计立场
  - 提交前 `cargo fmt + clippy -W warnings` 强制
- [ ] **P1** `examples/_README.md`:把每个示例对应的 feature 标注上,便于挑入口。
- [ ] **P2** 给 `Cage` / `Bond` / `Lotus` / `Widget` / `Scope` / `Truck` 这 6 个核心类型补 crate 级 doc + 一图概览(mermaid 即可)。
- [ ] **P2** README 加"对比 Leptos / Dioxus / Sycamore"小节,讲清楚选 Glory 的场景。
- [ ] **P3** 把 `debug.log` / `dump.rdb` 从仓库根移走,加进 `.gitignore`(目前 git status 显示二者未跟踪但反复出现)。

---

## 8. 依赖 / 工程

- [ ] **P1** workspace 升级当前已在 [chore/upgrade-dependencies](.) 分支,审一遍 `Cargo.toml` 里 `thiserror = "1"` / `dirs = "5.0"` 等是否还有更新版本可一并跟进。
- [ ] **P2** `rust-version = "1.88"` 假设性偏新,确认 MSRV 政策(目前需要 edition 2024)。
- [ ] **P3** workspace lints(`[workspace.lints.rust]`) 集中配置,各 crate 取消重复 lint 声明。

---

## 落地顺序建议

按依赖关系拆 4 个 milestone,大致 1–2 个月一档:

**M1(打地基,2–4 周)**
- §0 全部
- §1 P0/P1 全部(`Each` 重排算法做完做对,这是 Glory 用户最容易碰到的痛点)
- §6 P0(`MockRenderer` + Each 快照测试)— 和 §1 配对做

**M2(响应式现代化,3–4 周)**
- §2 P0/P1(代际盒、`Sync` storage、`effect`、`resource`)
- §6 P1(e2e)

**M3(渲染层抽象,4–6 周)**
- §3 P0/P1 全部(Renderer trait、AttributeValue、EventPayload、launch、History)
- §1 在 Renderer trait 上的回归测试

**M4(多平台开张,2–3 个月)**
- §4 P1(桌面 webview MVP)
- §4 P2(流式 SSR / hydrate)
- §5、§7 同步推进

---

## 速查:相关文件

- 响应式:[reflow/](crates/core/src/reflow/) · [cage.rs](crates/core/src/reflow/cage.rs) · [bond.rs](crates/core/src/reflow/bond.rs) · [scheduler.rs](crates/core/src/reflow/scheduler.rs)
- 视图组件:[widget.rs](crates/core/src/widget.rs) · [view.rs](crates/core/src/view.rs) · [scope.rs](crates/core/src/scope.rs)
- 渲染节点:[node/mod.rs](crates/core/src/node/mod.rs) · [node/ssr.rs](crates/core/src/node/ssr.rs) · [web/csr.rs](crates/core/src/web/csr.rs)
- 列表/分支/加载:[widgets/each.rs](crates/core/src/widgets/each.rs) · [widgets/switch.rs](crates/core/src/widgets/switch.rs) · [widgets/loader.rs](crates/core/src/widgets/loader.rs)
- 路由:[crates/routing](crates/routing)
- CLI:[crates/cli](crates/cli)
- 热重载:[crates/hot-reload](crates/hot-reload)
