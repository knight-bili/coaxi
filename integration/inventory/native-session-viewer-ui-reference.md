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

当前标题、刷新错误与只读说明因 Core 权限/读取生命周期适配；刷新错误条位于滚动体之外。实际第一次界面检查发现插入错误条使250px位置变299.333px，修正后刷新失败及恢复均保持250px。完整工具字符串/meta/stats保留在读取模型；正文后续原普通工具与专用卡片的实施状态分别见下方接续记录，不能把初始通用容器当作最终原工具设计。

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

## 压缩、反馈、MCP 与状态卡接续（2026-10-19）

固定 Codeg 的 `context-compaction-card`、`feedback-check-result-card`、`codeg-mcp-tool-card`、`delegation-status-card/row/group-card/badge` 七个完整展示文件，以及四个完整纯解析文件已接入 NativeSessionViewer。两份原解析测试也完整移植；13文件函数体/JSX与原版本一致（仅import及同名re-export路径适配），7组中英文原文案逐项相同。`ReadonlyToolCard` 只沿用原路由条件，不启用供体store、DB、执行或答复入口；`resume_delegation` 继续留给完整恢复帮手卡，不冒充普通MCP完成。

摘要必须有原 `codeg.compactionSummary` 显式标记才显示。状态组仍一任务一行，完整保留每次结果、原分页和原始输入输出/meta/stats；已结束但报告running的查询使用不旋转的“已查询”。展示历史取消请求不会重新发起取消。Markdown仍用Rovai安全组件，源streaming参数只作为更新正文的展示提示，不执行HTML或启动Runtime。

自审发现Badge少了原SVG尺寸样式，以及旧提取方式拆断含 `&`/`>` 的选择器；已补完整原Badge基础类并改用TS字符串节点提取，实测图标12px。原primary接入实际存在的Rovai `--brand`，反馈列表仅局部补原Tailwind列表reset；不修改App全局样式，也不裁掉Markdown列表。原Badge/Button/Markdown/i18n仍是明确宿主接缝，不把参考夹具称为完整原App。

82项定向、19组生产Renderer实际检查和15组普通工具/搜索/Diff回归通过；类型检查0错误、CodeGraph已sync。产品4张与固定原组件2张截图已检查（两主题；产品含390px窄屏）。运输是显式替身，无真实Core/Host/Native/Web/模型资格；完整原App、帮手主卡/collab、委派/恢复、问答/后台、图片和真实链路仍在原有限剩余中。详见[本批记录](../execution/i04-native-specialized-status-result.json)。

## 帮手、问答、分组与细滚动接续（2026-10-19）

- 原 `agent-tool-call`、`collab-agent-card`、`agent-capsule`、`subagent-session-button`、`tool-group-part`、`ask-question-result-card`、完整 `AskQuestionCard` 与原控件/纯解析已接；18个完整文件保持原函数体/JSX（仅模块导入及Radix聚合导出改等价叶模块），5个原问答类型完整加入，8组双语原文案一致。`Reasoning` 的9个状态/计时/触发声明保持原版；`ReasoningContent` 外层类/props沿原版，正文明确用Rovai SafeMarkdown，不伪造Streamdown插件已接入或资格完整原Markdown/HTML性能。
- 原历史问答控制是只读：标签含逗号、多选和补充文字、无完整问题仍可恢复的答案、忽略结果、键盘切换均由原组件呈现。原交互组件完整保留；实际提交入口继续既有Core运行交互，不把历史记录卡变成新的发送端。
- 原 `session-viewer-host` 与独立Dialog回退都接同一Core查看器宿主。显示编号仅是候选；点击时重新与当前读取中已签发的后代引用相交，校验宿主可见及Runtime，错误在当前卡内显示。卡片的live提示不决定可读权限或轮询状态；Core返回值拥有真源。多个帮手的有效目的地全部可达，名称键名单不生成会话入口。
- 原 `ScrollArea`（OverlayScrollbars）替换早期div接缝，保留延迟初始化、真实viewport回调、横/纵事件、移开自动收起以及原6px/悬停8px主题。问答原生滚动使用源fallback的thin规则；仅局部补Tailwind段落reset以消除浏览器默认间隔，SafeMarkdown原有无layer样式保留。库的body环境探针CSS保留原唯一选择器；所有可见控件CSS限定在codeg-native-rich，不启用供体整页滚动器，不隐藏内容。
- 八个固定直接依赖、十五个新增lock包节点；795个既有包/快照及旧importer保持。26个所需包发布物逐项SRI核验，MIT许可保留，未执行生命周期脚本或全量重装。原shadcn 3.8.4状态CSS只作构建输入，未引入其CLI/runtime；无React副本。

87项定向、28组实际产品Renderer、普通工具/Diff与前批专用卡回归，类型检查及源码审计详见[工程记录](../execution/i04-native-agent-question-result.json)。实际生产与固定原组件分别4张双主题截图已检查；原参考也用同份真实适配后的数据，明确宿主接缝。受控只读运输不资格真实Core/Host/Native/Web、真实模型、全局原App或完整恢复矩阵；客户仍仅在I12工程后使用软件验收。完整四包21簇84场景不减，新增正式通过0。


## 委派、恢复主卡与后台查询接续（2026-10-19）

原 `DelegatedSubThread`、`ResumedDelegationCard`、`DelegationCardRow`、`BackgroundTaskCard` 四份完整展示组件、完整 `delegation-card` 纯解析，以及两份原测试已移植。七份文件仅改import，函数/JSX与固定Codeg相同；两组双语scope一致。五个声明（含既有ALL_AGENT_TYPES、原自定义队员前缀/判断及纯类型DelegationBinding/Status）按原声明核对；不启用供体全局连接store、DB或附着执行。

Core User子列表直接驱动原主卡：当前attempt大于1才出现原恢复角标；Core当前blocked/cleanup优先于历史成功状态，取消与未知不当作成功。Core UUID保持字符串，不伪造供体numeric conversation id。`SourceDelegationProvider`是明确模型/Host接缝，不冒充原全局hook；点击仍打开当前已授权父列表里的确切子会话，保留原trigger供返回焦点。列表未新增正文预取，原Core合同不提供task/reason预览，正文与真实恢复说明仍在原子窗/原始记录，原父工具的完整来源关联继续补齐，不能标为完整原功能已收口。

后台连续查询使用原 `buildBackgroundTaskRows` 与原完整卡：一任务一行、查询次数、最新状态、非零退出码、停止结果；终端沿原ANSI组件，子代理结果沿Rovai Markdown明确接缝。每份原始查询输入/结果/meta/stats独立保留，已结束但报告running不旋转，没有输出就不造空展开。长日志的最后一行在原TerminalContent真实原生滚动容器可达；它不是Overlay viewport，夹具据实际组件修正了检查位置。

定向116项通过、types0，实际生产查看器两模式回归（22组行为及原组件/恢复角标/主题的额外断言）和后台15组检查通过。原查看器定位改为Core稳定ID和原按钮可访问名称，保留全部原断言，没有削弱审批/回答、scope、接管/丢失回执、草稿/附件、排队输入移回、隐藏、返回和窄屏末尾验证。测试进程单独关闭GPU；隐藏窗口主题过渡停在中途的问题用完成有限CSSTransition后取图修复，实际计算色值对照已验证，不更改生产主题或暂停无限状态动画。

原组件参考使用固定供体的真正模块，明确Core UUID模型、Runtime图标/标签、既有Terminal和Rovai Markdown接缝，不是完整供体App。背景查询采用同份真正适配后的payload；两主题及生产窄屏的最终截图按[本批记录](../execution/i04-native-delegation-background-result.json)记录。此次无新依赖、Rust/API/schema/模型输入变更、付费模型、完整编译、push或Actions。完整I04四包21簇84与一期81/48/324不减，本批正式场景通过仍为0，真实Core至Native/Web候选保留。

## 会话图片、统计与材料入口接续（2026-10-19）

用户图片、生成图、完整图片弹窗及逐轮统计使用固定 Codeg 原组件，完整 token/visualize 纯解析与原用例一起保留；[逐文件来源](native-content-ui-port.json)的 `mediaStats` 明确每份原 Git blob、摘要和宿主接缝。除 import/来源头外，TurnStats 只有一处只读适配：没有真正的任务写入 owner 就隐藏创建任务；原可选 fork 不伪造，当前 Thread 的任务与 Mission 继续走原 Core。模型名无原 catalog 时显示原始 ID，非法/不完整可选统计不补零，原始 summary/session_stats/turn metadata 全量可达。

图片沿原 Rovai 编码与右键菜单，原图放大/完整像素复制/完整 PNG 下载、隐藏/刷新与旧客户端迟到隔离均由实际 Chromium 验证；剪贴板 API 显式替身，不冒称系统剪贴板通过。图片、HTML 与文件链接使用当前 Core 原生 readId；客户端不猜工程根。本地图片严格核对类型、MIME、内容代次，每个临时句柄完成或失败后释放。原生来源不能成为可持久恢复路径，PinnedFilePreview 对此走 Core open，其他来源继续原 restore；Main 对直接原生 open 采用只读预览策略，自动呈现不能启动程序、展示目录或索取新根。

普通文本当前是原 Rovai SafeMarkdown/GFM，加原 Codeg 代码头/复制控件及原 visualize 解析；显式 HTML 标记自动展开、普通文件提及手动展开，流式隐式提及不抢先打开。HTML 采用原 Rovai PinnedFilePreview/HtmlPreviewSite，隔离域名与脚本操作、源码切换、收起已实际验证。完整原 Streamdown/math/mermaid/spec/交互卡引擎、完整原 HTML 控制与完整 App 同场景仍是本阶段必需项，不能以本批入口替代。

定向12文件205项通过；原来就因 Windows 不支持冒号文件名而跳过的1项保留，未新增跳过。实际生产查看器34项行为通过并核对日/夜与390px窄屏，固定原组件采用相同图片/统计输入及声明宿主接缝运行；截图人工逐张检查容器贴合、原图比例、卡片与操作布局。此对照不是完整供体 App；真实 Core/IPC/Web Host/provider、生命周期/规模/内存恢复仍随集中候选补齐。[本批工程证据](../execution/i04-native-media-stats-result.json)记录资格边界及原失败检查。完整一期81簇48包324/I04四包21簇84保持，本批正式场景通过新增0。无新依赖、Rust构建、模型输入/HTTP schema/数据库变更、付费模型、push或Actions。
