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

后续统一验证时按同状态记录：空列表、分页、筛选、新增/编辑、密钥替换/清空、九项 Claude、Codex 高级目录、断线/重连、切换服务器、窄屏/键盘、编辑放弃、迁移部分文件待写/冲突。每个页面使用其原容器滚动，不额外固定外层高度或隐藏超出的真实内容。具体画面效果及全部功能是否通过，以后续真实运行对照为准。
