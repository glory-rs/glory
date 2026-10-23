# 变高列表滚动余差修复（2026-10-23）

修复变高列表在宿主滚动取整后持续偏移 0.375 px 的问题。保留真实 DOM 行高、
原有 0.1 px 锚点/间隙容差、原有 0.15 px 缩放检查；未通过舍入测量结果或放宽
容差消除失败。正式证据位于 `D:/BuildCache/glory-anchor-residual/`。

## 原因和修复

设逻辑偏移为 L、当前逻辑段起点为 O，请求的物理偏移 P = L − O。
宿主接受的实际偏移为 A，其取整余差 b = P − A。

旧代码记录 b，用它把后续用户滚动转换回逻辑坐标，却仍按 `prefix − O` 放置行。
因此画面中的行顶为 `prefix − O − A = prefix − L + b`。逻辑锚点保存成功，画面
仍然多出 b，这正是原来未修正的 0.375 px。

新代码在收到 readback 后按 `prefix − (O + b)` 放置行，使画面坐标成为
`prefix − O − b − A = prefix − L`。接收同一 ack 时不重新发滚动动作；用户在 ack
到来前后实际滚动一个像素，仍按原有逻辑保留这一像素，不把它当取整误差吞掉。

同时修正两个相关问题：

- 新的测量目标直接请求 P，避免把上一次余差从下一次目标中扣除而累积误差。
- 提交滚动动作不再使用 0.25 px 的阈值。只在受限物理坐标尺度上忽略 f64 运算
  舍入噪声，使 0.125 px 变化也得到真实宿主 readback。待处理目标按投影身份去重。
  误差保护不随巨大逻辑偏移放大；另有接近 `MAX_EXTENT / 2` 的视口小数变化用例。

生产逻辑仅修改 `crates/ui/src/list/widget.rs`，没有增加渲染器特例、私有 DOM
位移、测量缓存或 wire 格式。已有 ScrollAck 同时驱动逻辑坐标和行样式，仍由宿主
帧边界处理输入；没有从原生监听器同步重入业务计算。

## 64 / 1024 缓存验证

未优化快照和修复快照分别在 `before/target`、`final/target` 编译，metadata 验证
实际 target 在 D:。两套 fixture 使用相同 Rust 依赖、业务源、行高定义和缓存参数
入口；业务源为一亿条索引数据。正常行的真实高度包含 24.375 / 32.375 / 40.375 px，
增长通过修改业务样式触发实际 ResizeObserver，未向浏览器测试注入假测量。

| 检查 | 缓存 64 | 缓存 1024 |
| --- | ---: | ---: |
| 修复前增长 30.375 px 后的锚点偏差 | 0.375 px | 0.375 px |
| 修复后同一检查的锚点偏差 | 0 px | 0 px |
| 连续小数变化次数 | 70 | 70 |
| 实际达到的测量缓存峰值 | 64 | 1024 |
| 80 次远跳中存活行峰值 | 28 | 28 |
| 语义节点槽位峰值 | 105 | 105 |
| 响应式图槽位峰值 | 185 | 185 |

70 次变化重复包含还原、+0.125、+0.625、+0.25、−0.375、+0.375 px。
每次都核对真实行高、锚点 key/NodeId、全部存活后代（含 Text Node）的对象身份和
间隙。实际宿主滚动 setter 每步最多一次；每轮还原后再等四次 RAF，写入数不再增长。
两组记录的逻辑偏移变化与业务行高 delta 完全相等，误差为 0；原生物理位置仍保留
浏览器实际 readback（本次最大取整余差 0.5px），没有伪造更精确的 scrollTop。

每个缓存配置记录 73 张视口内容截图：增长前、增长后、70 次小数变化以及远跳后的
Home。前 72 张 PNG SHA-256 完全相同；两种缓存的最终 Home 截图也完全相同。
截图覆盖 client viewport 内容，排除自然发生位移的滚动条；逻辑和原生 scrollTop
另行检查，最终两者都回到 0。这里验证 readback 收敛后的画面，不声称捕获了每个
中间显示帧。

两种缓存都实际填满并经过淘汰，再执行同一个 Home/业务行高还原步骤，比较最终
可见 key、top、height 和物理/逻辑偏移。离屏行仍按既有生命周期退休，不用保留所有
访问过的 DOM 来制造身份一致。

## 回归覆盖

新增 Rust 用例覆盖两种缓存的 70 次小数测量、重复 ack 不重发动作、逻辑与视觉锚点
同时保持，以及极远逻辑段中的 0.125 px 物理投影变化。既有 ack/真实一像素输入竞争
用例继续通过。浏览器扩展同时保留原来的 prepend、删除、1.25 倍缩放、两百万像素
巨行、滚动条 client viewport 和 80 次远跳检查。

完整门禁包括 Windows 工作区、WASM Clippy、Windows shell/accesskit 实窗与 UIA；
Linux 工作区、paint/raster、X11/Wayland 的 shell/accesskit 实窗；全部 24 套浏览器
验收、SSR fixture 四项测试、流式顺序/取消/慢读背压。各项结果及源文件/产物哈希
汇总于 `evidence-index.json`，原始日志继续保留。

Windows 与 Linux 各通过 421 项工作区测试，另各通过 289 项 fork 测试；默认测试中
分别忽略的 2/1 项平台实窗测试已用实际窗口命令补跑。Windows 的两项 shell/accesskit
实窗及一项真实 UIA 客户端通过；Linux 的 X11/Wayland × shell/accesskit 四项通过。
原生和 WASM 严格 Clippy、fmt、CPU raster 输出、24/24 浏览器及四项 fixture 测试通过。

最终门禁在 `validation/source` 冻结副本执行：生产源码与 `final/source` 完全相同，
仅原生 Loader 测试增加下述生命周期等待。两者的文件哈希分别保存；不改写编译用的
原始快照。期间共享工作区有其他任务的键盘/事件改动，首次 Linux 重跑因此在格式
检查退出，保留于 `linux-accepted.txt`；该次不计通过，也不把其他任务代码混入修复证据。

回归还暴露一个独立的 Loader 验收等待竞态：RunningGuard 先公布传输 active=0，
随后函数参数持有的解码租约才析构。测试原来只等传输空闲，就立即断言只剩一个
排队 completion，偶尔看见额外的正在析构租约。现在等待该独立生命周期完成，
不 pump 宿主；原来的 `(requests, completions, bytes) == (1, 1, 256)` 及随后
消费后归零的严格断言不变，没有改 Loader 生产代码或延长既有等待期限。

## 复现与证据

- `baseline.json`：两个缓存配置的未修复真实浏览器复现。
- `before/source/inputs.json`、`final/source/inputs.json`：框架冻结输入；fixture
  和实际验证源码另行记录。`before/metadata.json`、`final/metadata.json` 核对独立 target。
- `browser-accepted/evidence.json`：正式 24 套浏览器结果，adaptive 项内含两个配置
  的几何、身份、缓存上限、写入数和截图哈希；同目录保存全部截图和执行源码。
- `validation/source/validation-inputs.json`：实际完整门禁源码及脚本/引擎 provenance。
- `windows-frozen.txt`、`linux-frozen.txt`：最终完整门禁。较早 `after`、
  `adaptive-first`、`adaptive-stress`、`browser-full` 是过程记录。
- `windows-final.txt` 保留了 Loader 竞态的原始失败；`loader-recheck.txt` 是定位时
  的单项复跑。它们不替代最终完整门禁。

运行浏览器门禁前启动对应的静态/流式 fixture，指定 D: 临时目录：

```powershell
$env:TEMP='D:/BuildCache/glory-anchor-residual/tmp'
$env:TMP=$env:TEMP
$env:NODE_COMPILE_CACHE='D:/BuildCache/glory-anchor-residual/node-cache'
node scripts/test-semantic-browser.cjs --base http://127.0.0.1:8795/final/web-demo --stream http://127.0.0.1:8797 --out D:/BuildCache/glory-anchor-residual/new-browser-run
```

Rust 门禁：`scripts/check-semantic-ui.ps1 -Web -Window`。Linux 使用本轮记录的
D: 镜像挂载 helper，调用 `scripts/check-semantic-linux.sh --window --paint`。
正式日志的运行目录为上述冻结副本，Linux helper 为 `linux-validation.sh`。
完整门禁与实际源文件一致后再生成证据索引；不把较早的绿色结果当作最终代码证明。
