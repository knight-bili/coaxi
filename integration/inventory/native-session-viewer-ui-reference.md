# 原生帮手只读查看器：源码映射与有限剩余

Codeg 固定 aff1e6c278453ed8d0b96a5f664edcdca621aa3f，Rovai 固定 cc465dda562536b94f23787759e3206324baa29e；参考目录只读未改。现有 Core 是唯一权限/业务/执行权威，本页不改变已批准独立帮手 r1。

| 原来源 | 当前融合位置及适配 | 本批资格 |
| --- | --- | --- |
| Codeg `src/components/message/subagent-session-dialog.tsx` | `NativeSessionViewer.tsx`：非模态右侧只读、头部16px标记/14px标题/12px副标题、16px内容列/消息间隔、用户输入浅色容器 | 生产组件实际 Renderer 日夜/宽窄；参考运行原 Drawer/header/角色外壳，非完整正文 |
| Codeg `src/components/ui/drawer.tsx`、`button.tsx` | 沿既有 BaseUI/共享抽屉，宽最大36rem/边距8px，原关闭32px区域；Escape/Radix保护抽成共享 helper | 发现当前24px小于原32px并修正，最终两种尺寸实测32×32；既有两组委派 Renderer 回归通过 |
| Codeg `session-viewer-host.tsx` | `DelegationViewerHost` 同层 Core/native 共用一个槽、下级单独层；虚拟行卸载保持，隐藏父页隐藏子层 | 生产受控运输验证同槽往返、单层Esc、行卸载、隐藏、换client关闭旧读取 |
| Codeg `agent-tool-call.tsx::parseChildSessionId`、`src/lib/collab-tool.ts` | `native-transcript-model.ts` 保留原 live/saved/collab 身份规则；纯 helper 完整复制，仅候选ID与 Core 已签名后代映射相交后提供动作 | helper正文原字节等价与原摘要记录；不把 rawInput/路径当权限 |
| Codeg live 读取规则 | `native-child-reader.ts` 2秒且live/不重叠，完整 UTF8分片/来源与整份摘要校验、迟到/取消/StrictMode租约、父快照在途保留 | 两新文件29项，包含长Unicode及工具JSON、混页/摘要/游标/错代次、错误保留、关闭先到、父刷新/子读取交错 |
| Rovai 原 User证据读取与执行卡片 | `NativeSubagentEntries.tsx`、`native-subagent-entries.ts`，公开与私聊原执行记录；已知截断观察显式读完整内容、原receiverIndex保留 | 实际 Renderer 32项、最后索引31；短Grok编号无独立会话只展示状态；终态旧running注明上次记录 |
| Rovai `SafeMarkdown`/原主题tokens/运行标记 | 原安全Markdown与两套同结构主题；不引入新主题或输入/审批/模型入口 | 原/融合外壳各两张已检查；不宣称像素等价或完整原富内容更好 |

当前标题、刷新错误与只读说明因 Core 权限/读取生命周期适配；刷新错误条位于滚动体之外。实际第一次界面检查发现插入错误条使250px位置变299.333px，修正后刷新失败及恢复均保持250px。完整工具字符串/meta/stats保留在读取模型；当前工具正文采用初始展开容器，不能视为最终原工具设计。

## 本批证据范围

[工程记录](../execution/i04-native-viewer-ui-result.json)绑定生产源码、测试日志、原/产品截图、参考原文件摘要及六份r1摘要。生产实际 Electron Renderer 使用受控 ThreadClient，不启动 Core/Runtime/SQLite/MCP/模型；两组既有 Renderer 回归也属于该边界。原版参考程序实际运行原生 Drawer、header与角色容器，但其 ContentPartsRenderer/adapter/API 是明确标记的夹具。因此参考外壳检查不资格完整原 App、原富内容、真实 Core/Native IPC/Web HTTP 或真实模型。

## 同 I04 的有限剩余与完成条件

| 项目 | 当前状态 | 完成条件 |
| --- | --- | --- |
| 原富内容适配和消息布局 | 原纯adapter及计划清单/提案/模式/目标组件已接，完整原工具仍未完 | 继续固定Codeg具体工具/代码模式/搜索/Diff/代理/后台卡片及原实际统计入口，不把通用JSON当最终原设计 |
| 图片及附件交互 | 内联图片初始展示 | 保留原图片组/放大/查看及相关有效动作，正确表达不可访问来源，日夜宽窄/长内容/完整原富内容同场景对照 |
| 真实 Core 与两端 | 前轮 Core/worker及运输准入已分项，本批受控 Renderer | 隔离真实源文件经实际Core handle→Native/Web→生产页，包含失效、取消、后代、终态、列表截断/顶部字段裁剪及规模/内存/每页重授权性能；集成候选集中执行，不逐按钮全量构建 |
| 原完整 App 对照 | 原外壳、原adapter与本批计划/目标JSX已受控运行；部分宿主原语和内容路由明确为子集 | 完整原内容组件在同样语料下实际运行并对照，不用外壳夹具替代；平台/真实Runtime资格保留 |
| 旧格式与其他I04包 | 原既定队列保持 | Claude完整parser/启动来源/Core关联、Qoder独立根/producer、旧无来源显式导入、Grok反馈r1及T02/T03/T04原并集/恢复/记忆/自动化不遗漏 |

本批不增加84正式场景通过数，不改变完整一期81簇48包324场景，也不设置用户技术检查门槛。用户I12工程完成后进行实际软件使用验收。

## 计划与目标组件接续（2026-10-18）

本批接原完整纯适配器和必要依赖，计划清单、提案/模式、目标组件按原body移植，只改import绑定；纯适配器对原生只读显式保留图片伴随的完整工具输入/输出/统计和未配对结果的全部图片。用户输入改为原版字面文字。逐文件原Git blob、摘要、类型AST抽取范围及默认语义保持见[来源清单](native-content-ui-port.json)，实际[本批结果](../execution/i04-native-rich-content-result.json)拥有验证资格。

实际产品Renderer核了长方案288px折叠/完整展开、计划审核结果、目标明细及只读无控制、终态旧目标无动画、用户原文、混合图片正文和未配对多图/统计；同时保留原单槽/嵌套/刷新滚动/隐藏/旧client/32项列表检查。视觉自审修复边框继承currentColor过亮，按原border token衔接。原/产品日夜宽窄截图已看；参考运行实际原NativeDialog、原adapter、Plan/Goal JSX及原Badge/instant disclosure，Markdown/native ScrollArea/Shimmer采用明确宿主适配，内容路由只选本批分支，其他分支明确fixture。它证明本批组件结构/交互参考，不证明完整原主题、ContentPartsRenderer、Markdown管线、App或真实Core/Native/Web/Runtime。完整原富内容和图片查看仍按上表继续，阶段范围不减。

## 工具外壳与内容原语接续（2026-10-18）

已接固定Codeg Tool/Header/Content的完整声明前缀、Terminal、JsonTreeView、CodeBlock UI及完整instant-collapsible。Terminal/折叠主体与导入适配后的原声明一致；代码框UI和复制/行号保留，底层采用Rovai既有语法引擎，未移入Shiki正文缓存或未使用的语言选择器。原JSON字段路径及原代码片段缓存缺陷已重现并修复；真实ICU参数/复数接入，中文“计划2/7”和英文复数不再泄漏占位符。

实际产品24组交互与原组件两张日夜截图已检查；6张产品截图覆盖已有原生窗口及工具状态，复制验证包含Windows系统LF到CRLF的转换。原ToolShell/Terminal/JSON/折叠/Button/Badge在参考窗口实际运行，代码框及翻译是明确宿主适配，完整原ContentPartsRenderer/App和主题管线仍未资格。详情、源码Git blob及依赖在[本批证据](../execution/i04-native-tool-primitives-result.json)和[当前清单](native-content-ui-port.json)。

下一有限队列仍是完整普通ToolCallPart/ToolOutput及命令会话状态、结构化读写/补丁、搜索与Core授权文件跳转、统一/左右Diff，然后代理/MCP/后台卡、统计和图片动作；不得把这一批通用路由当成完整富内容。ansi-to-react原发布包及精确gitHead缺许可证文件，原README/SPDX保留，阶段对外分发前解决。完整I04四包21簇84场景和81簇48包324总范围不变。
