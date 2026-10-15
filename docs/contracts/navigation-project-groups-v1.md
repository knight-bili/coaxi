---
document_type: protocol-contract
contract: navigation-project-groups-v1
authority: client-navigation-project-group-preferences
status: accepted
version: 1
last_updated: 2026-10-15
---

# Navigation Project Groups v1

将固定Codeg `aff1e6c278453ed8d0b96a5f664edcdca621aa3f` 的命名分组、颜色、非嵌套成员与布局行为
接入[Navigation Read v1](navigation-read-v1.md#client-directory-preferences)已有导航偏好。
来源为`src-tauri/src/db/entities/folder_group.rs`和`src-tauri/src/db/service/folder_group_service.rs`，
保留Apache-2.0来源通知；不移植SeaORM或另建项目业务注册表。

## 保存与作用域

Navigation Preferences schema7增加`projectGroups: NavigationProjectGroup[]`和`projectGroupsRevision:number`。
组为`{id,name,color,projectPaths}`：id由客户端生成UUID，名称沿现有80字符项目显示名称规则，color为null或
现有画布十二种颜色，projectPaths是有序绝对路径。null表示继承主题；颜色只用于组标题，不能改工程、会话或队员身份。

`projectOrder`的顶层序列同时容纳`directory:<path>`和`folder-group:<id>`，允许组与未归组工程交错。
组内顺序由该组projectPaths拥有，成员不重复出现在顶层。同一路径只能属于一个组，不支持组嵌套。
新建组追加到顶层；明确选择其他组的工程会将其移入本组。删除组将其可见工程按原组内顺序追加回顶层，
不移除登记目录，不删除Thread、使命或文件。移除目录时保留其已有归组意图，恢复同一路径后可重新显示。

旧schema1–6在内存补空分组和版本0，合法旧文件不会因读取而重写。损坏、重复归属、未知字段或未来schema
沿现有降级策略保留原文、禁止写入；不以空默认值覆盖。目录登记、置顶、别名、已读和旧迁移完成标记均保持。
此schema不改变Core数据库、模型输入或应用包版本。

Desktop复用当前隔离profile的Main私有JSON、串行队列和原子保存；Web复用当前Host/Owner范围的localStorage，
并在既有短浏览器事务内读取最新值、修改及保存。跨窗口或标签页只发布成功保存后的完整快照。
与既有目录清单相同，分组不跨客户端同步；另一个客户端缺少该ID时，画布显示未解析绑定，不新建替代组。

## 修改接口

`navigationPreferences.applyProjectLayout(expectedRevision, mutation)`由Native preload/Main和Web偏好适配器共同提供。
这是展示偏好接口，不是Core Domain Command，不分配Run或命令回执，也不赋予路径访问权限。

| mutation.type | 字段 | 行为 |
|---|---|---|
| create | group | 新UUID及完整组内容；仅允许新加入已登记且未移除的目录 |
| update | group | 保留同一ID，修改名称/颜色/有序成员；未恢复的旧成员可以保留或明确移出 |
| delete | groupId | 移除组，保留工程及业务历史，原画布绑定保留为不可用 |
| layout | entries | 有序`{kind:'group',groupId}`或`{kind:'project',projectPath,groupId:null|string}` |

所有参数和嵌套对象拒绝未知字段；读取最新JSON后核对projectGroupsRevision。旧版本写入明确拒绝，不自动重基。
实际改变时版本加一，无变化不加一；安全整数耗尽则拒绝。空layout无变化，未列出的成员和顶层项保留，
不会因客户端只显示部分目录而丢失其他工程。新布局中的未知组、未登记目录、重复项与嵌套意图都拒绝。
其他导航偏好修改保持分组和版本；与新目录登记并发时，最新登记目录仍保留。

## 读取与验收边界

画布group节点仍只保存folderGroupId，实时解析来自当前偏好。worktree包含关系继续由
[Workspace Worktrees v1](workspace-worktrees-v1.md)的真实Git读取决定，不由偏好路径前缀推断。
保存归组意图不证明该路径是主工作树，也不授权文件/终端。侧栏worktree跟随、布局拖放、管理界面和画布接线
须完成各自实施及验收后才能称为完整项目分组功能。

纯共享model owner拥有混合排序、迁移、唯一归属、CAS与无业务删除；Desktop现有私有JSON owner扩展并发与重开，
Web现有跨标签页/作用域owner扩展分组写入、隔离及旧版本拒绝。最小验证为这三个TypeScript测试文件；
Main IPC、实际浏览器锁与挂载UI沿阶段集中验证，不用纯函数或模拟运输结果替代。
