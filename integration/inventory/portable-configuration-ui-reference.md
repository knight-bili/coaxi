# 完整模型配置迁移 r1 的页面源码依据

状态：源码编写中；本步未运行 App、未做截图或视觉通过判定。

固定来源由 sources.lock.json 拥有：Codeg aff1e6c278453ed8d0b96a5f664edcdca621aa3f；Rovai cc465dda562536b94f23787759e3206324baa29e；Android 33dba91b1522330fe519c27c4833066e35c9495e；iOS e2d75f918a64bc8a450216155cbc3ae4a820b0f1。

| 融合页面 | 原组件和交互依据 | 保留与新增 |
|---|---|---|
| Desktop/Web PortableProviderSettings | Codeg src/components/settings/model-provider-settings.tsx，add-model-provider-dialog、edit-model-provider-dialog；Rovai AppDialog 与 RuntimeInstallationsPanel | 类型筛选、紧凑列表、原编辑弹窗；Claude 九字段、Codex 完整目录、密钥私有输入与原请求核对 |
| Desktop/Web PortableConfigurationMigration | Codeg src/components/settings/config-sync-settings.tsx；Rovai GeneralSettings/BackupSettings | 候选文件、逐项选择、冲突、原回执、撤销；不自动选择同名目标或队员 |
| Desktop/Web PortableMemberConnection | Rovai MemberManagement 的原 MemberRuntimeForm | 队员原运行设置内明确选连接，使用队员实际版本、安装及代次 |
| Android RovaiPortableProviderSettings | 固定 Android app/src/main/java/app/codeg/android/feature/settings/ModelProvidersScreen.kt 的 ModelProvidersContent/ProviderEditorTarget；core/designsystem/component/TextFields.kt 的 CodegTextField/SecretField；既有 RovaiRuntimeSettings | 分组列表、类型筛选、单编辑弹窗、密码字段；九项 Claude、结构化 Codex 模型与高级覆盖、原编号恢复 |
| iOS RovaiPortableProviderSettings | 固定 iOS CodegiOS/Features/Settings/ModelProviders/ModelProvidersSettingsView.swift、ModelProviderEditorSheet.swift；既有 RovaiRuntimeSettingsView | 原列表、编辑 sheet、原生 Form/SecureField、移除确认与原编号恢复 |
| Android/iOS 配置迁移与完整导出 | Codeg src/components/settings/config-sync-settings.tsx 的范围、预览和原请求交互；固定移动端设置容器、文件选择与系统保存流程 | 明确选择每条来源与本机目标，原编号核对及撤销；完整副本和原候选副本分别保存，原候选可能含秘密时必须明确勾选 |
| Desktop/Web 和双原生配置同步 | Codeg config-sync-settings.tsx 与 Rovai 原 WebDAV 设置 | 完整配置及包含密钥默认关闭；手机操作同一主机调度器，原上传编号及修订继续核对 |
| Desktop/Web 和双原生日志设置 | Codeg src/components/settings/logs-settings.tsx 的级别、目标覆盖；Rovai DiagnosticsCenter 与原移动设置 Form/GlassRow | Desktop Main 和独立 Server 分别由原所有者处理，不混入模型状态、审批或操作回执；移动级别选择使用紧凑菜单 |
| Desktop/Web 和双原生任务模板管理 | Codeg src/components/tasks/task-editor-dialog.tsx 的模板列表、保存和删除；Rovai 现有设置容器与原生表单 | 明确 ID 与修订保存，保留原程序、选项及非文字材料；原文件计划继续与整批导入分开，未知结果不自动重发 |
| Desktop/Web 和双原生使命草稿选模板 | Codeg task-editor-dialog.tsx 的模板弹层和 applyTemplate；Rovai MissionWritingPlane 与移动 RovaiMissionCreator | 先把原材料对应到本次已加入的材料，原执行设置由 User 明确改用当前工程和队员，再填写普通使命草稿；没有隐式安装、改队员默认或启动 |
| Desktop/Web 和双原生原文件保存衔接 | 沿用配置迁移原回执区的操作行 | 数据库已提交、文件仍 pending 时明确继续原计划；失败、冲突及来源路径变化不会被覆盖，Desktop 偏好仍等待 Main 原写队列 |

后续统一验证时按同状态记录：空列表、分页、筛选、新增/编辑、密钥替换/清空、九项 Claude、Codex 高级目录、断线/重连、切换服务器、窄屏/键盘、编辑放弃、迁移部分文件待写/冲突。每个页面使用其原容器滚动，不额外固定外层高度或隐藏超出的真实内容。具体画面效果及全部功能是否通过，以后续真实运行对照为准。
