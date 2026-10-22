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
| 从回答分叉 | Codeg `src-tauri/src/acp/fork.rs` 和 `src/components/conversations/conversation-detail-panel.tsx::handleForkFromTurn`，助手原文指纹、重复序号及 AIR fork shape | `NativeTranscriptBody` 的原助手行增加分叉操作；Core 从只读快照再次计算定位，不用本地位置编号猜测原程序 id |
| 独立原生工作副本 | Rovai 原 `CreateThreadCommand`、私聊草稿和 `app-dialog-*`；Codeg 先核对原会话、分叉前保留旧消息 | `NativeContinuationDialog.tsx`；Core 复用 Thread 事务和私聊表，确认后准备原生会话，首次消息单独发送 |
| 原生继续与分叉的 Android / iOS 页 | 固定 Codeg Android 的 `GlassRow` / Compose 表单、Codeg iOS 的 `List` / `Section` / 原生表单；融合期已移植原生会话库 | `RovaiNativeContinuation.kt/.swift`；会话库保留继续、末尾分叉和助手回答分叉；原请求保存在当前 Host 的私有编辑记录 |
| 移动端原生插话 | 同一原生队员设置和已移植反馈面板、原版 Codeg 完整输入选择语义 | `RovaiNativeSteer.kt/.swift`；默认关闭、材料默认不勾选、只核对同一命令；私聊传当前自身附件 metadata |
| 未接收后的完整普通下一轮 | Codeg `src/lib/queued-steer.ts` / `use-session-feedback.ts` 的完整输入保留；Rovai 原普通消息 / 私聊排队 / Domain Command 事务 | `NativeNormalContinueDialog.tsx`、`RovaiNativeNormalContinue.kt/.swift`；Core 仅准入 `not_consumed + promptRequired`，同事务登记唯一消费；正文与材料整份进入原发送链，现有普通草稿和引用不消费 |

当前记录只说明源码来源和作者选择。未启动应用、未做视觉对照、软件 Review 或任何软件测试。
所有实际布局、窗口尺寸、主题、恢复、完整输入与原版同场景对照留待全部源码写齐后的统一 Review 及后续验证步骤。

原生创建准备目前使用用户明确选择的自定义 ACP 程序，实际握手核对程序身份、能力和原生目录。其他内置原生接口的直接恢复资格、创建后的精确私聊导航、完整环境恢复提示和全范围尾项仍在作者队列；这里不据入口或源码存在宣称功能已经可用。
