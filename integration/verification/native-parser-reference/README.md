# 固定原解析器对照夹具

这份配方给研发和阶段工程验证使用，不要求客户执行。原 Codeg 的 `codex.rs`、`grok.rs` 保留固定提交的完整原字节，只在同一模块末尾追加只读夹具入口；消息主体、工具拆分、用量、压缩等 parser body 不改。参考程序没有 Core、SQLite、提供方、模型调用或应用界面。

固定来源是 Codeg `aff1e6c278453ed8d0b96a5f664edcdca621aa3f`，Apache-2.0。准备脚本核对提交与 [源文件摘要](source-lock.json)，Rust edition 沿供体2021。[参考清单](reference-manifest.toml)和[锁定依赖](reference-cargo.lock)只使用固定供体的包版本/校验值，不参与产品 workspace 或产品 Cargo.lock。完整原文件会带入未用路径，因此 `allow(dead_code)` 仅在这个临时参考程序，产品解析器不增加抑制。

外围桥接只提供原纯模型/函数片段、原错误类型的非数据库分支、无状态列表缓存透传及不参与本路径的自定义展示名空实现。没有将融合后的 parser 当成原版；这些边界也不授予原完整 App 或列表缓存资格。

## 复现

先按 [来源锁定](../../sources.lock.json)准备固定 Codeg 工作副本，再在融合仓库执行：

```text
python scripts/prepare-native-parser-reference.py --source-root <固定Codeg源码目录> --output <新的临时参考目录>
cargo build --locked --manifest-path <新的临时参考目录>/Cargo.toml
```

准备脚本拒绝覆盖已有目录。Windows 需要现有 Rust/MSVC 开发环境；其他平台使用其正常 Rust 构建环境。不要改付款方式或默认触发 Actions，仍按阶段候选集中安排。

本项目Windows工作区另有空间约束：执行前先从项目根运行 `work/space-guard.ps1 -Mode Check`，上述Cargo操作经同一脚本的 `Run` 模式执行，继承其项目专用 `CARGO_TARGET_DIR` 和 `CARGO_INCREMENTAL=0`；批次结束再次Check。可执行文件从该目标目录取得，不能再在参考副本单独建立target。阈值不通过时停止构建，不自动删除独有证据。

新建一个绝对路径的结果目录，在其中创建 `.native-parser-differential-root`，内容必须精确为 `controlled-native-parser-fixtures-v1`，无换行。只给本次测试进程设置：

- `ROVAI_NATIVE_PARSER_REFERENCE`：新构建参考可执行文件的绝对路径，Windows 带 `.exe`。
- `ROVAI_NATIVE_PARSER_DIFF_OUTPUT`：上述新结果目录的绝对路径。

然后精确执行既有 owner，不新增 Rust 测试或数据库：

```text
cargo test --locked -p rovai-core --features extended-tests --lib runtime_subagent_session::source::tests::lineage_sources_keep_exact_rollouts_and_lossless_stable_chunks -- --exact
python scripts/review-native-parser-differential.py --directory <新结果目录> --report <新结果汇总.json>
```

参考子进程的 HOME/USERPROFILE/CODEX_HOME/GROK_HOME 都指向测试自建目录，不读日常账户、项目或原生日志。Codex 原断言的辅助入口把 fixture cwd 放入自建工程后，将同一记录交给两套 parser，原断言期望不改。真实文件 case 的 patch 也只读取自建工程。没有设置这两个测试环境变量时，只运行原普通 owner，不得称差分已通过。

## 当前证据范围

当前31个实际对照案例中，Codex24、Grok7；完整 turns/blocks/工具字段、session_stats 和明确的显示摘要30例相等。Grok长正文1例仅两处不同：原版截断任务/TaskOutput，Core保留完整输入。对照不裁剪 Core 内容；汇总脚本另外核对 Core 两个 JSON 与完整夹具精确相等、原版内容是带截断标记的正确前缀，其他字段逐一比较。每例保留双方输出和差异位置/摘要。

这证明已列受控记录上的原组件对照，不等于覆盖所有未来原生格式、用户语料、真实模型、原完整应用、Core/Host/HTTP/GUI 或跨平台资格。实际分项记录见 [Grok统计与原解析器对照](../../execution/i04-native-parser-parity-result.json)，完整I04四包/21簇/84正式场景继续保留。
