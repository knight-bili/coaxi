---
document_type: contract
contract: automation-run-control-v1
authority: user-requested-exact-automation-occurrence-stop
status: accepted
target_version: v1.72
last_updated: 2026-10-17
---

# Automation Run Control v1

补齐 Codeg 原定时任务历史的单次停止入口，复用 Core 的原子取消、执行隔离和命令回执。定义、调度与结果仍由
[Scheduled Automation v3](scheduled-automation-v3.md)拥有；本合同不把停止定义、停止 occurrence 和物理进程已回收混为一个事实。

## 用户命令

Desktop Main 与认证后的 Web Owner 单独准入 `automations.cancelRun`：

```json
{
  "commandId": "original-unique-command-id",
  "command": {
    "automationId": "immutable-definition-id",
    "runId": "immutable-occurrence-id"
  }
}
```

命令名 `automation.run.cancel`，只接受 User actor。命令体为闭合 shape，两个 ID 必填，不接受客户端指定 AgentRun、
Camp、执行代次或“最新一次”。Core 验证 occurrence 属于指定定义；未知或错归属返回 `automation.run_not_found`，
非 User 返回 `automation.cancel_user_required`。这不是 Agent CLI/Built-in Tool operation，不加入模型工具目录。

同一 `commandId` 与原输入返回原持久结果；同 UUID 改输入走原幂等冲突规则。Web 原提交未知时仅查询
`commands.reconcile`，不自动重发。历史按钮失败后保留原 UUID、定义和 occurrence，再次明确点击仍使用原请求。

## 精确停止与终态竞争

Core 在同一命令事务内读取原 occurrence 的持久关联：

- 当前 Delivery-first occurrence：只处理原 `trigger_delivery_id`。waiting 时取消这一条；claimed 时只结算其原
  AgentRun，不扫描 Camp 中的后来运行、不撤销其他 waiting Delivery、不停止其他 occurrence。
- 历史 CampTurn occurrence：使用现有双向归属检查，只取消这次 occurrence 拥有的原 Turn。
- 原 Run/Delivery/Turn 已有终态但 occurrence 尚未同步时，先结算原结果。已完成/已失败/已跳过不会被迟到点击改成
  用户停止；新 UUID 再次停止旧 occurrence 也不能影响该定义的新 occurrence。

实际停止采用既有 abortive settlement，保留 prepared/accepted/unknown 输入与外部效果证据、关闭待审批并撤销旧写权限。
Runtime 取消通知、清理确认及 lane/execution-root 后继门禁仍由现有 supervisor/adapter 处理。
命令成功只证明 Core 已持久结算本次停止，不证明物理进程已回收，不能手工绕过清理门禁。

成功使用现有 StoredCommandResult，`status = applied`；code 为 `automation.run_cancelled` 或
`automation.run_already_terminal`，payload 是：

```json
{
  "automationId": "immutable-definition-id",
  "runId": "immutable-occurrence-id",
  "threadId": "original-thread-id-or-null",
  "status": "failed",
  "reason": "user_requested_automation_stop"
}
```

`threadId` 可为 JSON null；`status`/`reason` 返回该 occurrence 的真实最终值，已终态路径不强制改为示例值。
用户停止沿用 occurrence 现有 `failed` 存储分支，以新原因区别于应用退出、超时和执行错误。UI 显示“已停止”，灰色状态，
保留完整历史与打开原执行对话的入口；只对仍 running 的行显示紧凑停止按钮。若该次配置了通知，通知说明用户已停止本次运行，
不把它描述为应用退出或成功完成。

## Schema159 / Migration209

严格接收 v1.72/schema158 和原闭合 reason CHECK；仅增加 `user_requested_automation_stop`。复用既有表重建工具，
复制 `automation_run` 全部列和原值，保留索引、定义/执行/通知引用及原触发器。迁移登记、marker 和重建在同一事务提交；
失败回滚旧表、数据、结构和 marker，并恢复外键检查。新 schema 的闭合原因集合或 marker 不符时拒绝准入。

历史回执、ContextManifest、输入投递、绑定、格式版本、容量/选择策略与公开消息原文不改写。实际用户停止产生的是现有
取消链路的业务事实，没有新增模型工具或改变 Bootstrap/Dynamic Context 的 shape、格式和选择规则。
Core/Host/CLI 随阶段候选成套升级；旧二进制不得写 schema159。冻结的 I03 预览继续使用其原二进制与独立数据目录。

## 工程验证

扩展既有自动化事务 owner，覆盖 User/归属、waiting/claimed、原回执、迟到停止与后继隔离，以及 schema158 数据/回执保留、
迁移失败回滚、伪造原因集合拒绝。旧画布迁移夹具按既有链先还原新增约束，再升级至当前 schema，不能放宽生产准入来迁就测试。

Web 运输 owner 验证丢回执后只核对、显式重试保持原 UUID/输入。生产 Renderer 的实际 Chrome 夹具验证日夜原页面对照、
按钮、失败重试、历史分页和打开原对话；夹具的内存 RPC 不证明实际 Host 或 Runtime 清理。后者仍须 I04 阶段集中集成验证。
