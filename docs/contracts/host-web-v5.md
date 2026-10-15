---
document_type: protocol-contract
contract: host-web-v5
authority: shared-host-web-transport
status: accepted
version: 5
source_version: v1.72
last_updated: 2026-10-16
---

# Host Web v5

继承 [v4](host-web-v4.md) 的认证、Owner、allowlist、超时和命令回执。已认证 User 的变更操作新增
`agentRuns.continue`，输入与准入由 [AgentRun Continuation v1](agent-run-continuation-v1.md) 拥有。
Web 仅转发闭合请求并通过既有 `commands.reconcile` 核对原 commandId；不得因运输失败换 ID 重发。
该增量不改变登录与 Session schema，传输 `protocolVersion` 保持 4。Agent 权限不扩大。

## 私聊上传原回执与恢复能力（2026-10-16）

登录及编辑端恢复响应增加可选 `privateUploadRecoveryVersion: 1`；没有该声明的 Host 不启用新的私聊准备接口。
这是 API 能力声明，不新增凭据、持久 Session 字段或传输大版本。旧上传及 reconcile 的请求/响应保持兼容。

新增认证 `POST /api/v1/uploads/receipt`，请求为原 UploadIntent，只准私聊 target。
Host 从已验证 Session 构造 DraftClient，通过内部 `host.upload.receipt` 读取同 UUID 的持久结果；
不查当前草稿、编辑租约或源文件，不登记 scope，不写业务数据，不因原材料移除/编辑取消而换编号。
未知返回 `{state:'unknown'}`，已记录返回 `{state:'recorded',receipt}`；错误返回通用不可核对结果，不降级重放到其他编辑端。
Core 内部入口允许 Host 已验证的 Desktop/Web 编辑端，浏览器 `/request` allowlist 仍不允许直接调用该内部方法。

HTTP receipt 只投影 commandId、commandType、status、code、recordedAt、空 resultEntity，以及 payload 中的 attachmentRefId/revision；
拒绝结果的 payload 为空。即使同编号原结果来自旧公开上传，也不能把 SourceRef、sourcePath、draftId 或内部实体带到此接口。
该值是原结果的安全视图，不伪造新的 Core 回执。归属及首次结果规则继续由 Domain Command Gateway 拥有。

共享 Web 页面通过原准备接口保留 request/intent/phase/receipt，文件字节只留有界内存；刷新或淘汰后显式重选，
名称、大小和 SHA-256 全部一致才可用原 UUID/target/revision 重试。恢复/重复 prepare 只查询，不发送新文件。
首次明确 HTTP 拒绝可记为本次 rejected，不伪装成 Core recorded；已有未知请求的重试拒绝仍保留未知，等待原回执。
准确的输入位置和界面边界见[私聊输入合同](single-chat-composer-scope-v1.md#web-原请求准备与原文件重选2026-10-16)。
