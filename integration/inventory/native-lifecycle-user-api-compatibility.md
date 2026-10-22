# 原生继续分叉插话的用户 API 编号兼容

客户已明确确认原生继续分叉插话 r1。该确认的模型输入、继续/分叉/插话行为与恢复要求保持。

附录的 `NativeContinuationPreviewV1.newThreadId` 和 `NativeContinuationReceiptV1.threadId`
误用了新命令 UUID 的格式。现有 `crates/rovai-core/src/camp_id.rs` 明确限定 Thread 唯一持久和公开身份为
`rvcamp_` TypeID，内部载荷为 UUIDv7。裸 UUID 不能传给现有会话 API，也不能存成另一类 Thread。

本次按既有 Core 约束修正上述两项用户 API 输出，不创建别名，不改既有编号，也不替换已交付的 r1 Word、PDF
或冻结提案。修正 shape 由 `integration/contracts/native-lifecycle-user-ui-v1.json` 独立记录。
这两项只在客户操作预览及回执出现，不进入模型工具目录。`NativeSessionOriginV1` 自身没有 Thread 编号字段；
其来源、模式、分叉点、工程摘要和原命令字段不变。NativeInputDeliveryV1 的目标原本即沿用有界 Core 标识，亦不变。
因此不改变已确认模型输入的结构、选择、文本、版本轴或历史证据，不为这项内部兼容选择新增客户技术审批。

`runtime.steer.materials.prepare` 是用户侧准备快照的辅助接口。它只接收已认证编辑端当前草稿中的明确文件编号，
可携带既有私聊卡片 scope；返回 r1 已确认的 `NativeSteerMaterialV1`。路径、URL、目录扫描和模型工具不跨此接口。
实际 NativeSteerInput 仍只含用户确认的原文与固定快照引用。

源码仍处于集中编码，尚未统一 Review、编译或运行验证。后续统一 Review 必须核对用户 API 修正没有传播到模型合同。
