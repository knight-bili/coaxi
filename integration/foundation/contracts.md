# I01 Core 与宿主融合契约

## 唯一业务权威

Rovai Core继续拥有队员、公共Thread、Task、Mission、Run、记忆、自动化、命令回执及SQLite迁移。Electron、Web、原生移动和渠道只发起已准入操作和显示投影。
Codeg的完整数据库及执行管理器不作为第二业务底座启动。其可复用协议、解析器、UI、Git、浏览器及工具实现按模块接入；宿主工具调用保留Actor、Run和目标授权上下文。

## 当前可验证的源接口

当前Host Web协议为4，登录POST /api/v1/login的字段为protocolVersion、administratorToken和可选editor。管理员长期Token用于登录，不直接作为业务请求Bearer Token。登录返回Session Token，业务调用使用Session Token。
POST /api/v1/request仅接受operation与params，operation来自rovai-web的逐项准入enum；不把所有内部Core RPC自动发布到网络。额外顶层字段按现有deny_unknown_fields拒绝。
GET /api/v1/events是失效通知流。首次、重连及滞后收到resync后重新读取授权快照，不能当作完整消息或原始Runtime事件重放。

Core内部CommandEnvelope序列化字段为commandId、actor、campId、expectedVersions、executionEpoch和payload。expectedVersions为数组，每项含entityType、entityId、version；不是v4草案中简写的单个expected_version。
ActorRef的type为user、agent或system；嵌套字段仍为user_id、agent_id、source_agent_run_id或component_id，不能机械地把整个JSON改成camelCase。
正式适配必须调用已有Core网关及鉴权，不能在兼容层另写业务状态机。网络兼容入口不得从客户端任意字段推导Agent或System权限。

## Codeg 和原生移动适配

Codeg现有客户端POST /api/<name>及WebSocket事件，不直接等价于Rovai Host的operation/params与SSE。其acp_prompt携带connectionId、blocks、folderId、conversationId和clientMessageId；ACP connection不是Core Run身份。
兼容层在I06/I10按具体操作建立白名单映射，先完成Host会话认证，再调用统一Core。仅改变URL或改字段名不算移植完成。错误、审批、停止、分叉和插话必须保持源支持行为。
legacy整数folder/conversation身份以sourceHost、Runtime和来源记录为作用域；公共Thread使用Core身份。导入映射必须显式、去重和可追溯，不能把同名目录或数字ID直接作为另一Host的对象。
公共Thread沿用现有rvcamp_加26位Crockford Base32的UUIDv7身份；不能暴露普通UUID作为别名。迁移必须使用Core创建命令返回的threadId，设计夹具中的合法格式示例不代表已创建对象。物理camp表和内部Conversation保持原有身份边界，禁止额外建立重复的公共Thread表。

## 工程与工具服务

画布布局和节点引用独立持久化，业务状态查询Core；PTY为临时进程，文件指向真实工程。Git暂存和提交复核HEAD/index，Mission累计Diff固定base_sha并覆盖四类变化。
浏览器、电脑helper、OfficeCLI服务由宿主管理，其许可、授权、撤销和进程回收在对应阶段实现。远程客户端不能隐式授权共享桌面。
独立Server当前源能力声明不支持渠道，Linux当前Rovai sidecar只准入Server。完整一期还必须补Linux桌面宿主和Server渠道适配，这些差异保留在一期任务中，不能视为已支持或删到二期。

## 数据和升级

I01固定来源的Core数据合同为v1.72、投影schema为146；当前融合工作树以 [current-protocol.json](current-protocol.json) 和真实Core常量为准，不覆盖历史来源快照。应用版本、数据合同、投影schema和Host Web协议是不同版本轴。融合数据字段进入既有迁移体系，不创建第二Thread/Run数据库。
画板/节点、标签、导入来源映射、Forge关联是后续新增或扩展模型；I01不预先落未经核对的重复DDL。工程目录偏好继续按既有客户端配置管理。
旧数据导入先对来源副本做快照和校验，保留原记录与来源hash。未知版本、非法路径、重复来源及中断结果有明确处理；恢复采用一致性快照或停机窗口，不能复制仍在写入的db/WAL充当安全备份。
当前migration-fixtures为设计夹具，生产数据迁移尚未执行。真实Host协议夹具只创建隔离空库，用于验证源协议及当前schema准入，不代表全部历史数据库升级通过。

## I04 原生会话中的工具文件引用

在既有 `filePreview.resolveSource` 中增加显式来源；不新增模型工具或向客户端发布任意 Core 方法：

```ts
{ kind: 'native_child_file', threadId: string, readId: string, rawReference: string }
```

Core 从 Current User 和已完成的原生读取派生授权，逐次解析原 Thread/Run/证据与绑定、校验读取范围和 Ready 状态，
只使用原 Host 捕获的绝对工作目录。Renderer 不传目录、原生 Session 或 actor；未知字段拒绝。
旧记录缺少捕获目录、读取已关闭/过期、原绑定失效或读取尚未完成时拒绝；不能补用当前 Camp 目录。

返回既有 `file_target`，`sourceKind='native_child_file'`、`sourceIdentity='native-child:<readId>'`，
`rootPath/basePath` 均为捕获目录。Main 核对回复的 Thread、来源种类、精确 readId 身份和原引用，再校验真实文件路径。
绝对路径、相对路径和目录联接均不能越出捕获目录。文件内的后续引用继承该读取身份，不能转成普通工程来源。
后续文件读取、重读、原生操作与 HTML 资源仍须重核原授权；字节读取前后均校验，拒绝在读取中失效的请求。

原生文件的预览缓存按 Thread、真实路径和 readId 区分，不与当前工程或另一次读取合并。
readId 是瞬时资源，不持久恢复、不自动改绑当前工程；恢复界面可保留失效标签，需重新打开原会话获得新读取。
刷新期间保留旧完整正文和对应旧读取，只有整份新正文通过分页/摘要检查后才切换文件来源。隐藏/关闭停止读取后不再授予新点击。
失败使用现有错误码及脱敏提示，不能把无权限点击处理为成功。

本批原来源 owner、Main 受控文件和生产 Renderer 的替身运输验证分别记载；实际 Core.handle→Native/Web→UI 联通及完整原 App 对照仍属 I04 阶段候选验证，不由局部通过替代。
