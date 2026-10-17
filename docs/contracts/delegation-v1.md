---
document_type: protocol-contract
contract: delegation-v1
authority: core-delegation-identity-settings-and-wire
status: accepted
version: 1
last_updated: 2026-10-17
---

# Delegation v1

完整批准边界见[独立变更说明](../versions/v1.73/model-context-change-native-delegation.md)。本合同区分基础实现和待接线能力，不能据此判断一期或 I04 已完成。

## 严格输入基础

delegation.start/status/cancel/resume/read/history 的输入 shape 由已确认 r1 完整机器附件 after.operations 拥有；Core `delegation_contract.rs` 实现相同解码边界。
六个对象都拒绝未知字段。父身份、epoch、Binding 与授权不能由模型提交。必填及已出现可选值不得为空/null；Runtime 只接受既定 21 个标识或 custom 稳定标识，实际已安装/启用/权限准入仍由执行层核对。
working_dir 须为 Host 绝对路径；语法通过不是目录授权。reason 限 2048 UTF-8 字节且不是新任务。status 保持编号顺序及重复项；省略 wait_ms 是快照，0 是无期限，正数上限60000ms。read 默认16384/范围1..65536字节；history 默认20/范围1..100条。
这里只实现参数与等待数值规范化，不实现等待订阅、业务读取/结果、Core API 或 CLI。

## User 设置存储基础

表 delegation_settings 为 singleton=1，version>=1；enabled 默认 false、depth_limit 默认1且范围1..8、per_runtime_defaults 默认空对象、completed_cache_max_mb 默认512且0不限。每个目标配置沿用 Codeg mode_id/config_values 形状：mode_id 可省略或 null（表示无覆盖），config_values 可省略且空映射不序列化；值是配置字符串，不是凭据或执行授权。嵌套供体默认配置的 null 规则不同于外层严格 patch，不能混用。
User API 更新输入为 {expectedVersion,patch}，patch 仅 enabled/depthLimit/perRuntimeDefaults/completedCacheMaxMb。缺字段保持原值，显式 null/未知字段拒绝。深度/缓存非法时零写入。每次成功更新精确比较版本并递增；开关请求只携带 enabled，不覆盖其他配置。
更新原语接受调用者事务；随后回执/事件失败应回滚同一事务。读取只读，不懒建表、不重设配置。SCHEMA 只在隔离 SQLite 存储夹具执行；启动迁移、Command Gateway User-only 鉴权、幂等回执/事件及 Host 两端入口仍待集齐，不允许生产路径自行执行建表字符串。

## 持久委派与执行待接入

后续必须按批准 r1 引入 delegation/attempt/control、真实父链、child Conversation、task 空值与专用 policy/格式闭集、输入接收和清理围栏、原 Session 恢复及用户接管。Run 仍是执行终态权威；设置或参数通过不得直接启动进程。未启用/无历史关系的旧模型输入不改，关闭后仍可读取/取消历史任务。
目标数据合同 v1.73、Schema160/Migration210 尚未应用；当前数据合同和已有输入仍原样保留。
