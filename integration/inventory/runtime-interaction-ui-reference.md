# I04 原生提问与计划卡片源码映射

固定 Codeg：`aff1e6c278453ed8d0b96a5f664edcdca621aa3f`，Apache-2.0。Rovai固定来源、Core唯一权威及全量一期范围保持。

| 原组件/规则 | 融合实现 | 保留或适配 |
|---|---|---|
| `src/components/chat/ask-question-card.tsx` | `RuntimeInteractionDock.tsx::NativeQuestionCard` | 原题目、完整选项与描述；单选重复点击清除、多选、Other、空选项自由输入；多题Tab/进度/自动前进/下一题；折叠；每题完成后可提交、独立跳过 |
| `src/lib/ask-question.ts::splitRecommended` | 同名展示转换 | 仅展示拆推荐后缀，提交仍是原完整label；真实`__other__`不与宿主Other混淆 |
| `src/components/chat/plan-approval-card.tsx` | `NativePlanCard` | 长计划局部滚动、空计划；批准/修改/放弃；修改空白不可提交；取消保留草稿；三个决定完整提交 |
| Codeg `ui/{button,badge,tabs,radio-group,checkbox,progress,textarea}` | `runtime-interactions.css` +既有Radix Tabs/原生表单 | 14/12/10字号、20/16行高、14/10圆角、选项10内距、32按钮、36胶囊Tab、稳定多题正文高度；原键盘操作与焦点提示；使用既有Rovai日夜语义色，不另建主题 |
| Rovai `ThreadWorkspace` / `SingleChatPanel` / `ThreadClientProvider` | 原组件内挂共享Dock | Desktop/Web/画布复用真实client；公屏只选原四种公开invocation，显式排除single_chat/delegation/delegation_user；私聊精确Conversation，客户端身份变更重置整个呈现 |
| Rovai既有命令回执与Web未知提交核对 | `runtime-interaction-controller.ts` | 仅Core list/get/resolve；不构造原生RPC/Session/Run归属；同请求保留UUID/完整Answer/expectedVersion，先读回执，显式重试原提交；不自动重发 |

Core元数据按50条翻页扫描，旧待回答请求不会被新历史遮掉；仅打开的请求读取正文。多个待回答以最早pending优先，历史/等待投递不会压住新问题。手动切换请求及暂时隐藏保留各自未提交选择；更换Host/Owner client或Conversation不沿用。此呈现缓存不是跨进程草稿库，也不承诺未提交选择在App重启后恢复。

`resolved`只展示答案已保存、尚未确认收到；`dispatching`展示正在投递；`sent`展示已发送，不声称提供方已执行或任务完成；`closed`禁止再次提交；`delivery_unknown`明确不会自动重发。Grok修改反馈的队列衔接仍未完成，绝不把保存的反馈标成已投递。

本批优化针对源码级适配产生的实际问题：Rovai默认行高会把原来能放下的选项挤出滚动条，已对齐原20/16行高；短题正文`clientHeight=scrollHeight=208`。内容超出时保留局部滚动、尾部操作可达，不隐藏全局滚动条。

## 本批运行对照及限界

自动检查只启动测试独立Electron Renderer和其userData/session；不启动Core/真实Runtime，不接模型或日常数据。`scripts/lib/runtime-interactions.test.mjs`加载生产共享组件、生产主题与模拟传输；这是组件交互资格，不是完整Desktop/Web原生往返资格。

固定Codeg原组件及八个原UI依赖在仓库外隔离构建：直接执行原组件，替换本地语言hook以读取原zh-CN，使用原root/neutral/theme/base/text-3xs规则和Tailwind4.1.18。依赖安装在`work/i04/source-ui-deps`，没有修改供体或生产锁文件；来源摘要和渲染路径见本批结果JSON。原问答组件完整渲染；原计划仅资格为空计划外壳及决定控件，未使用的MessageResponse直接抛错，未用替代Markdown冒充原正文。原App/Tauri/WebKit及完整非空计划同场景对照仍须阶段候选。

日/夜、1000×920及390×720、系统像素比例1.5的同输入问答/空计划截图已保留。外部框架差异（Rovai主题、队员用语、共享队列与原提交核对）显式列在映射；不声称像素相同或优于完整原App。最终真实挂载、非空计划/工具链接、多个来源挤压及其余原生格式保留在I04候选，不能用本批测试替代84正式场景。
