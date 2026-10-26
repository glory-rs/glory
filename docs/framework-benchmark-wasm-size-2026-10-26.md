# 三框架 benchmark 的 WASM 体积（2026-10-26）

后续[具体代码归因与质量去重](framework-benchmark-size-analysis-2026-10-26.md)
保留了三框架最终函数拆解、Rust 编译策略诊断，以及不裁剪能力的 HTML 映射去重对照。

本轮重建仓库当前 `benchmarks/{glory,dioxus,leptos}` 的
**semantic-rows-v1** 应用，比较相同数据生成、全量行和九项操作。
这不是三个最小应用的比较，也没有改变框架实现、功能或 benchmark 的性能配置。

`benchmarks/*/dist` 中仍有 2026 年 6 月的旧产物；README 折叠历史区的体积表
也来自旧架构、旧版本和不同的 wasm-opt 设置，不能作为当前结果。

## 构建和口径

- Glory 0.3.1 当前提交 `6f08a12`；Dioxus **0.7.10**；Leptos **0.8.22**。
  外部框架按当前 manifest / Cargo.lock 精确版本构建，不更新依赖。
- 冻结 527 个输入；原始清单、Git revision 和文件 SHA-256 保存于构建目录。
- 三个独立 Cargo target，均由 `cargo metadata` 核对在 D 盘。
- rustc / Cargo **1.98.0**；wasm-bindgen 库及 CLI **0.2.129**；Node **24.3.0**。
- 保留原 benchmark 的 **opt-level=3、LTO=true、codegen-units=1**。
  compiler artifact 记录核对 opt-level=3、debuginfo=0。
- 原 benchmark 发布流程在 wasm-bindgen 后直接提供 `app_bg.wasm`，没有 wasm-opt。
  分别保留此产物、仅删除调试名称的产物，以及统一 Binaryen **132.0.0**
  `-Oz --enable-bulk-memory --enable-reference-types` 的产物。
- Brotli quality=11；gzip level=9。各资源独立压缩后相加，不能拼接后压缩。
- JS 包括生成入口及其递归引用的相对 JS snippets；Dioxus 的 JS 解释器片段也在内。
  原始生成 JS 未再压缩变量名或改变打包方式。共同测试 harness、CSS、HTML 不计入
  框架 WASM+JS 小计；它们仍是页面发布所需的额外资源。

`-Oz` 的体积结果是补充实验，不能与原 benchmark 未优化的计时成绩绑定。
删除 name section 的收益单列，不能把整个阶段差额当作机器码减少。
压缩大小表示传输成本，不表示运行时内存或渲染速度。

## 实测结果

<!-- bundle-size-results -->

所有数值单位为 **B**。原发布产物指当前 benchmark 的 wasm-bindgen 输出；
统一 `-Oz` 是额外的相同后处理，并非原计时所用文件。

| 框架 | 原发布 WASM | strip-debug 输出 | 统一 `-Oz` WASM | `-Oz` Brotli | `-Oz` gzip |
|---|---:|---:|---:|---:|---:|
| glory 0.3.1 | 1,860,970 | 1,587,767 | 1,425,214 | 376,223 | 512,781 |
| dioxus 0.7.10 | 457,470 | 351,644 | 317,164 | 99,109 | 125,714 |
| leptos 0.8.22 | 279,406 | 172,868 | 155,008 | 43,438 | 54,567 |

| 框架 | JS raw（含 snippets） | JS Brotli | 原发布 WASM+JS Brotli | `-Oz` WASM+JS Brotli |
|---|---:|---:|---:|---:|
| glory | 60,648 | 8,934 | 408,661 | 385,157 |
| dioxus | 49,916 | 9,381 | 121,804 | 108,490 |
| leptos | 30,134 | 5,309 | 58,133 | 48,747 |

在这一相同应用下，Glory 的 `-Oz` WASM 是 Dioxus 的 **4.49 倍**、
Leptos 的 **9.19 倍**。包含 JS 后的 Brotli 传输量是两者的 **3.55 / 7.90 倍**。
Dioxus 的 JS 入口只有 27,174 B，但实际还引用了 22,742 B 的解释器/属性处理
snippets；这里均已计入，没有只统计入口文件。

Glory 的差距主要仍在代码：`-Oz` 后 Code 段 payload 为 **1,334,189 B**，
Data 段为 **76,738 B**；Dioxus 为 277,980 / 33,244 B，
Leptos 为 135,161 / 13,676 B。不能用删除名称节解释整个差距。

Glory 的 strip-debug → `-Oz` 原始文件减少 162,553 B，但 Brotli 只从
376,956 降至 376,223 B，减少 **733 B**。该后处理减少原始指令体积的效果
并不等于同等比例的网络收益。strip-debug 也经过 Binaryen 二进制重写，
其差额不能精确等同于直接裁掉 name section 的长度。

此前 894,092 B 的 Glory 最小应用使用 Rust `opt-level=z`；这里是完整的
行 benchmark，且 Rust `opt-level=3`。两者的原始体积差不能全部算作
“列表组件的边际成本”。此轮保留原性能配置，也不根据这个结果推导渲染速度。

结果来自这三个具体应用和锁定版本，不代表所有应用或所有能力配置的通用体积排名。
没有为体积改动运行时、功能开关或集合表示。

[机器可读报告](../benchmarks/bundle-size-2026-10-26.json)保留原始 B、哈希、工具版本、
各 section 和六项浏览器验收摘要。

## 正确性与复现

构建复用仓库的 WASM all-targets 严格 Clippy 检查。
浏览器检查复用 `benchmarks/acceptance.js`，对原始发布产物和统一 `-Oz` 产物
在 Chrome 155.0.8059.84 中六项均通过，分别验证种子数据、创建/替换/更新/选择/交换/删除/追加/清空、keyed DOM 身份、
全部万行真实布局盒、清理和视口截图。此轮只验证正确性，不采集性能样本。

全部产物：`D:/BuildCache/glory-benchmark-wasm-20261026`。
`bundle-size/report.json` 包含锁文件/产物哈希、工具版本、三个阶段的
WASM/JS raw/Brotli/gzip、code/data/name section 统计；
`bundle-size/browser/` 保留每个框架/阶段的验收数据与截图。

```powershell
# 每次使用新的 D: 构建目录；准备好匹配的 wasm-bindgen 0.2.129 CLI。
$bundleRoot = 'D:/BuildCache/glory-benchmark-wasm-20261026'
$env:TEMP = "$bundleRoot/tmp"
$env:TMP = $env:TEMP
$env:PYTHONPYCACHEPREFIX = "$bundleRoot/pycache"
New-Item -ItemType Directory -Force $env:TEMP,$env:PYTHONPYCACHEPREFIX | Out-Null
./benchmarks/build.ps1 -BuildRoot $bundleRoot -Bindgen D:/path/to/wasm-bindgen.exe
node benchmarks/bundle-size.cjs --build-root $bundleRoot --binaryen-dir D:/path/to/node_modules/binaryen
node benchmarks/bundle-size-browser.cjs $bundleRoot
```

Binaryen 使用此前 WASM 基线固定的 npm 132.0.0 安装。测量脚本拒绝其他版本，
也拒绝覆盖已经保存的报告。源码、Cargo home、编译输出、优化器临时文件和
浏览器临时目录均留在 D 盘。报告源码清单对应原始冻结输入；测量工具和本文不改变
Rust 应用产物。
