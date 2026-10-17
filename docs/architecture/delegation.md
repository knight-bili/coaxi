---
document_type: architecture
authority: core-delegation-ownership
last_updated: 2026-10-18
---

# 独立帮手

Core 是委派业务与执行权威。DelegationId 关联真实发起成员、父/根 Conversation、原 Run/epoch/Binding/tool-call 和独立 child Conversation；恢复增加 attempt，不替换逻辑编号。它不是 Task、Mission、正式成员或提供方原生子代理。
User 查询依据 Host 身份及确切持久 Thread/child 关系，父 Run 终态不撤销用户查看历史的能力；模型侧仍每次核对原活跃运行与接管水位。
两类读取复用原固定结果与有界分页，但签名游标隔离受众。全局设置走原User命令/CAS/同事务事件回执，不改变旧冻结输入；具体入口见当前合同。
User恢复以原UUID和持久父Conversation归属授权，允许父Run已经终态，复用同一原任务/Session/清理与writer检查和attempt工厂。候选SQL只凭对应新Run/attempt的同事务User恢复事件准入该分支；模型的活跃父身份门禁保持。因果依据仍来自上一attempt，不伪造模型执行或另一套broker。
现有 Scheduler/Fleet、Runtime Adapter、输入接收、清理和命令回执承担执行；不能另运行具有业务权威的 Codeg broker。原生提供方的子代理通过只读证据/引用投影，缺可靠来源时显示未知，不伪造 Core 子 Run。

当前[输入、设置和持久身份原语](../contracts/delegation-v1.md)已编码，候选身份 DDL 在隔离 Core 夹具验证；启动迁移尚未登记。下一步按[实施计划](../versions/v1.73/implementation-plan.md)合齐格式/冻结资格/严格迁移与执行全链路；既有 Host 或 Runtime 尚不会因为这些模块而获得新能力。
候选结构验证现覆盖 settings、全部新增对象、五张扩展旧表和十个改动旧 guard；安装与验证共用同一转换。159结构参考只记录既有DDL，不复制业务数据或建立第二套数据权威。旧159来源和完整候选分别校验，部分安装、同名空壳guard、放宽的CHECK或字段属性均拒绝；启动版本仍为159/209，完整升级链另行登记。
新Run从可信Conversation和完整合同开通标记冻结角色及资格；Runtime刷新沿用该Run的冻结资格，不读取当前设置授予旧Run新能力。角色绑定摘要保留原Runtime模型、权限及无原生Session恢复能力的事实。子Bootstrap按已批准文本使用独立身份、空记忆/平台技能，不查询父Mission或渠道事实；模型输入和原生技能/MCP的完整隔离仍须格式35与执行准入共同实现。
格式35继续使用原Core装配器、受管Blob、Manifest和恢复入口。child有独立顺序与闭集数据，不查询父历史、任务、Mission、A2A或跨Thread历史；初始任务核对原Blob摘要，恢复仅固定继续提示，User后续正文必须来自本child且在接管边界之后。父指南计入相同UTF-8预算；超限不保存部分任务。已有Manifest回放不重选当前设置，child身份也不回退到父Profile。此处是格式与存储接线，材料授权、原生目录投影和真实执行准入仍须单独取证。
完整行为、上下文隔离和原界面职责在[已确认 r1](../versions/v1.73/model-context-change-native-delegation.md)，取舍理由见[V1.73-D01](../versions/v1.73/decisions.md#v1-73-d01)。
