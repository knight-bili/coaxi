# 原生继续、分叉与插话 UI 来源

固定 Codeg：`aff1e6c278453ed8d0b96a5f664edcdca621aa3f`，Apache-2.0。
固定 Rovai：`cc465dda562536b94f23787759e3206324baa29e`，MIT。

| 表面 | 原版依据 | 融合实现 |
|---|---|---|
| 原生插话接收条件 | Codeg `src/hooks/use-session-feedback.ts`、`src/lib/queued-steer.ts`、`src-tauri/src/acp/connection.rs` 中广告、轮次及完整输入规则 | Core `native_lifecycle/acp_policy.rs`、`acp_transport.rs`；用户看到实际当前任务列表 |
| 队员独立开关 | Rovai 原队员设置的行、开关及低频保存操作；同一成员编辑平面 | `MemberNativeSteerSettings.tsx`，复用 `member-editor-section`、`interaction-setting-row`；不增加替代主题 |
| 完整内容确认弹窗 | Rovai 原 `app-dialog-*` 与源码弹窗结构；Codeg 以完整 prompt blocks 发送图文的行为 | `NativeSteerDialog.tsx` 复用现有 Radix 弹窗和反馈正文布局，默认不勾选任何材料 |
| 公聊/私聊/画布材料选择 | 原版 Rovai 各 Composer 及已确认私聊材料隔离 r1 的卡片归属 | 各父组件只传当前草稿 metadata；Core 按 Host DraftClient、Scope、原有 composerScope 及明确 sourceIds 冻结 |
| 断线记录 | Codeg 保留完整未消耗输入与确定/未知回执；Rovai Core 原命令账本 | 原预览、投递账本及同一 UUID 只读核对；不把原生回执说成模型已读 |
| 从回答分叉 | Codeg `src-tauri/src/acp/fork.rs`，助手原文指纹、重复序号及 AIR fork shape | Core 已写纯定位和请求投影；继续/分叉创建和最终原生绑定仍在编码队列 |

当前记录只说明源码来源和作者选择。未启动应用、未做视觉对照、软件 Review 或任何软件测试。
所有实际布局、窗口尺寸、主题、恢复、完整输入与原版同场景对照留待全部源码写齐后的统一 Review 及后续验证步骤。
