# 引擎依赖维护

第三方源码修改在 glory-rs 组织的真正 GitHub fork 中维护。Glory 仓库不保留
vendor、副本补丁树或子模块；应用无需添加全局 Cargo patch。

| Fork | 上游基线 | Glory 固定提交 | 维护范围 |
|---|---|---|---|
| [glory-rs/blitz](https://github.com/glory-rs/blitz) | `92b11f1b` / beta.4 | [b40cdfc2](https://github.com/glory-rs/blitz/commit/b40cdfc2c9b5a000759275b673e4ed8aadd68f78) | 资源生命周期、增量失效、焦点、密码显示、IME、阶段计时和 inline 样式差分失效 |
| [glory-rs/taffy](https://github.com/glory-rs/taffy) | `eb645069` / 0.15.0 | [dd7914fb](https://github.com/glory-rs/taffy/commit/dd7914fba2e6e0b5d8a2c8854d764a2ade817efd) | 小数 flex 因子的 gap 计算与回归测试 |

恢复上游包名 blitz-dom/traits/paint/shell、stylo_taffy、taffy；Glory 自己的
适配 crate 继续叫 glory-blitz。Blitz fork 的 workspace 统一固定同一个 Taffy
Git revision，各包间保留上游正常的 workspace/path 依赖。因此没有修补前后
类型混用，也不将第三方 crate 加入 Glory 的十包 workspace。上游代码、许可证、
仓库结构及 Git 历史保留在各自 fork。原有实现的 146 个源码/资源文件在迁移时
逐字节核对；Cargo 规范化 manifest 改回上游 workspace manifest。

运行环境先按 AGENTS.md 将 CARGO_HOME、CARGO_TARGET_DIR 和临时目录设到 D:。
`python scripts/check-engine-forks.py` 使用 Cargo metadata 和 Cargo.lock 检查
实际来源、完整提交、重复类型身份及 workspace 边界；加 `--test` 会在 D: 构建
目录的独立 Git checkout 中执行各 fork 固定锁的库测试，并确认 checkout 清洁。
Windows/Linux semantic gate 均调用该检查。独立 checkout 是正常 Git 工作副本，
不参与 Glory 的 path 依赖，也不修改 Cargo 管理的源码缓存。
Blitz fork 的 Linux/FreeBSD 测试依赖启用 Fontique 的 fontconfig-dlopen，
与适配器一致；运行时仍需 Fontconfig 和字体，不需要安装其开发头文件来跑库测试。
`extra_tests` 还启用 `parallel-construct` 执行计时器测试，验证并行聚合、reset
代次和真实文本构建。计时器只保留 13 个固定聚合值，不缓存节点、文本或帧。
样式差分测试也在此配置执行：top/height 和等价声明复用文本，Clear 恢复
stylesheet；16种连续样式变化与强制完整重建对照，属性选择器仍重算关联节点。
inline style 保留 snapshot、selector/inherited restyle 与祖先标记，只去掉
无条件 ALL_DAMAGE，交给 computed-style difference 分类；CustomWidget 继续
保守完整失效。其余属性修改仍保持原路径。
默认不安装 collector；开启后使用引擎自身的单调时钟，Glory 在 resolve 返回
后导入其固定快照，不跨线程传递 Glory 的本地响应式对象。

修改流程：在 fork 上从维护提交建分支，提交并推送修复与测试；若修改 Taffy，
先更新 Blitz fork 的统一依赖和测试锁，再更新 Glory 的四个 Blitz Git revision、
主/示例 Cargo.lock 及 [engine-forks.json](engine-forks.json)，运行 fork 检查与
Glory 的语义、原生几何/窗口回归。合入上游后也通过审查过的完整提交升级。
不得改成浮动 branch、未提交本地 path、在应用端打 patch 或重新引入 vendor。

这些依赖尚未发布到 crates.io；当前保证的是 Git 源构建。早期实施记录中的本地
源码校验和命令仅为历史证据，当前复现以本页和检查脚本为准。
