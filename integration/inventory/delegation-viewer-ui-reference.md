# I04 Core 委派查看器源码映射

固定 Codeg `aff1e6c278453ed8d0b96a5f664edcdca621aa3f`（Apache-2.0），固定 Rovai `cc465dda562536b94f23787759e3206324baa29e`（MIT）。两类查看器都来自 Codeg：有连字符的 `sub-agent-session-dialog.tsx` 是跨 Runtime 委派；无连字符的 `subagent-session-dialog.tsx` 是 Runtime 自带子代理只读历史。后者尚无可信 Core Session 读取链，本批没有用前者冒充后者。

| 来源组件 | 融合位置 | 保留与适配 |
|---|---|---|
| Codeg `sub-agent-session-dialog.tsx:57`、`ui/drawer.tsx` | `DelegationViewer.tsx::DelegationDrawer`、`delegation-viewer.css` | 原 Base UI 1.7.0，非模态、点击父页面不关闭、右滑关闭、同宽嵌套缩退与逐层 Escape；8px 边距、576px 最大宽、26px 圆角、16px 正文列；日夜颜色沿 Rovai 主题 |
| Codeg `session-viewer-host.tsx::SessionViewerHost` | `DelegationViewerHost` | Host 在虚拟消息行之上，卸载发起行不关闭；每层一个槽，同层替换、子层归父层；client/Thread 变化丢弃旧呈现，原按钮仍存在才归还焦点 |
| Codeg `delegated-sub-thread.tsx`、`delegation-card-row.tsx` | `DelegationChildrenContent` | 36px 圆头像、14/12 字号、短编号/状态和分隔的查看按钮；列表仅元数据，不预取任务正文或内联铺结果 |
| Codeg `agent-icon.tsx` 静态 IconProps 至 Per-agent color override 前 | `CodegRuntimeMark.tsx` | 静态 SVG 原字节区域复制；不带自定义注册表、图片 URL 或源应用状态。未编译品牌与 custom 使用明确初始字母回退 |
| Codeg `LiveTranscriptView` 的只读与三类阻塞答复职责 | 原 Rovai `SingleChatRunHistory`、`ApprovalDock`、既有 `RuntimeInteractionDock` | 只呈现 Core 精确 child 的 Run/Epoch/执行证据、pending 审批与问题/计划；两类原生答复沿原 Core 命令，不 attach、启动或隐式接管 |
| Rovai `ThreadClientProvider`、私聊/公开 Conversation 边界 | `delegation-viewer-controller.ts`、原 `ThreadWorkspace`/`SingleChatPanel` 挂载 | 唯一 Core User API；history 原签名翻页、断线后追到缓存重叠才衔接；错误保留同归属内容，换父读取失败不能标记旧列表为新父 |

业务适配：编号取 Core ID 尾八位，避免统一前缀不可区分；状态来自 Core，而非提供方 UI 推断。共享执行组件以 trigger/final Message ID 对齐，正文去重时保留最终消息附件名称。刷新失败保留已读取内容并可重试。初始焦点放安全标题，不落在审批决定上。打开和关闭不写业务数据。

## 实际验证与资格边界

本批 User get 的新增活动投影在原 Core 同库 Context owner 内实际运行，校验 exact child Run、证据/审批归属、列表没有 activity 与既有只读断言。没有新增 Rust owner 或整库夹具。TS 控制器及 Desktop/Web 运输回归另计，不能当完整 UI 往返。

生产组件使用隔离 Electron Renderer 和模拟传输；检查父页面操作、发起行卸载、单层替换、嵌套逐层退出、Radix 兄弟模态的 Escape 保护、可信 Chromium 触控右滑、精确 child 审批/答案、工具过程、最终附件、失败重试、Host 迟到答复及换父失败缓存隔离。隐藏窗口会停在 CSS 动画中间，测试在独立窗口捕获帧并结束抽屉有限过渡后核对静止几何；未修改产品动效，也不授予真实设备触控/帧率资格。

仓库外 `work/i04/render-source-viewer.mjs` 直接构建未修改的原 SubAgentSessionDialog、Drawer、AgentIcon 和依赖组件；原 root/neutral/day/night CSS 与 Tailwind 4.1.18 保留。仅语言 hook 读原中文、无原生浏览器时 occlusion 为 noop；LiveTranscriptView **明确替换为标记夹具，只资格抽屉外壳/标题，不资格原完整会话**。原全文 App/Tauri、原生会话、相同真实全文/工具链接等仍待阶段候选。原源码及截图摘要在[本批证据](../execution/i04-delegation-viewer-result.json)。

原外壳与生产组件各保留日夜宽/窄四图。已目视检查夜间宽屏、日间窄屏：容器边距/圆角/标题对齐，无外层横向滚动；长结果只有正文滚动且关闭入口固定。主题、Core 状态/刷新、原提交核对与共享正文是明确业务差异，不声称像素相同或优于完整原 App。真实隐藏但保留的会话切换、完整各卡片挂载、接管后的普通编辑/材料与控制入口、第二类原生子代理读取，均继续留在 I04 有限队列。

新增生产依赖仅 Base UI 1.7.0 及其三项必要传递包，共四包；原 777 个包与快照逐值保持，离线 frozen-lockfile 安装通过。[许可与完整性记录](../licenses/base-ui/source.json)保留各 MIT 正文。Codeg 来源许可沿既有[原通知](../licenses/README.md)保留。固定批准 r1、Core v1.73/Schema161/Migration211、81 簇/48 包/324 正式场景与 I04 84 场景不变，本批新增正式场景通过数为零。

## 接管后的原聊天复用（2026-10-18）

保留上述 Codeg 抽屉外壳和嵌套规则，内部复用固定 Rovai 的 SingleChatPanel 布局与已有融合输入组件；没有重画聊天页。新增明确的 child 模式和嵌入呈现：去掉外部单聊入口及重复标题，取消独立浮层定位/边框，阅读区和输入区填满原抽屉；仅正文滚动，输入与结束入口固定。主题仍取同一组 Rovai token，未新增框架、依赖或主题分支。

User 显式接管后，Core 的 takenOver、cleanupPending 和真实成员共同决定编辑准入；原父任务的 system/delegation-parent 消息保留原 Run。固定目标不能改成默认主队员，附件、引用、排队编辑和保留材料各取 child owner。取消/恢复原任务与接管后聊天分别走既有 Core 命令，不由 UI 计时判定业务完成。

控制请求可在保存后关闭并重开核对；正文发送/整份移回未结时，真正的输入 guard 阻止抽屉关闭、同层替换和 Thread 离开，失败回滚已准备层。隐藏保留的父页面将所有子层标 hidden/inert；内层结束弹窗同步隐藏，回到原页面保留编辑器、草稿和阅读位置。画布按当前 board 过滤实际挂载实例，换板卸载旧 Frame；相机移出视口不被当作新的业务归属或结束事件。

隔离生产 Renderer 已覆盖接管丢回执关闭/重开、清理门禁、固定身份、普通私聊草稿隔离、child 上传、发送与整份移回的丢回执、离开保护、隐藏/恢复、结束历史保留；日夜宽窄图逐批查看，含移回附件的输入区仍贴合容器。模拟 Core 传输只资格 UI，不代表真实提供方推理、完整 App/原全文 transcript 对照或逐平台结果。原 Codeg 外壳对照记录保持，完整源应用同场景仍保留到阶段候选。分项证据见[本批记录](../execution/i04-child-chat-ui-result.json)。
