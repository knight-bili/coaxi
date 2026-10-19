# 许可证和来源保留

Rovai保持根MIT LICENSE和完整Git历史。Codeg桌面及原生移动为Apache-2.0，移植时保留相应声明和NOTICE。
sources.lock.json锁定提交和原样复制的许可证hash，包含科研技能、Playwright和移动第三方通知。当前没有宣称全部间接依赖已完成法律审核。
每批移植补查对应第三方许可、通知和来源，I12核查发布包通知与依赖清单。开源许可不等于商标或服务账号授权。

`runtime_interaction.rs` 的Cursor/Grok问题和计划回复编码依据固定Codeg提交
`aff1e6c278453ed8d0b96a5f664edcdca621aa3f` 的 `src-tauri/src/acp/question.rs`、
`cursor_ext.rs` 和 `plan_approval.rs` 适配为Core类型，保留上述Apache-2.0许可及源码来源注释。
原问题/计划卡作为后续界面移植依据；本批只有协议转换，没有复制供体broker、数据库或引入依赖。

画布界面按固定 Codeg 源码引入同版本 `lucide-react 0.564.0`，其 [ISC 与 Feather MIT 原文通知](lucide-react/LICENSE)随源码保留。锁文件只增加该依赖，不顺带更新其他包；最终发布包的通知装配仍在 I12 核查。

I02-T03补留[Superpowers MIT许可](superpowers/LICENSE)和[固定来源核对](superpowers/source.json)：Codeg专家包52文件对比上游v6.3.0，51份Git blob一致，另一份是供体为单独解包添加的writing-skills/package.json。Codeg原始Apache许可与科研MIT通知随Core目录API保留，不将整个技能集合笼统标为同一种许可。

I03节点图行为依据[固定源码清单](../execution/i03-node-source-audit-result.json)中的Codeg提交移植，
`crates/rovai-core/src/canvas_node.rs`保留来源及[Codeg Apache-2.0许可](codeg/LICENSE)指引。
Core节点图批使用既有Rust/SQLite/Serde依赖。Renderer已按供体固定版本引入`@xyflow/react 12.11.5`；
本次锁文件新增20个包的[版本、完整性与许可摘要](canvas-dependencies/packages.json)和[原文通知](canvas-dependencies/THIRD-PARTY-NOTICES.txt)已保留，
许可为MIT、ISC与BSD-3-Clause，未运行安装脚本。图形库内部状态仅拥有交互呈现，不拥有Core业务或执行事实；
PTY后续移植及I12最终发布包仍分别核对，不将本批通知保留当成完整发布资格。

`canvas-reference-model.ts`依据同一固定Codeg提交的`computeRegionMembers`移植筛选、排序和精确引用行为，
并按Core实际active membership/private Conversation适配；工程父子关系必须明确取得，不用路径前缀推测。
沿用已保留的Codeg Apache-2.0许可，不增加第三方依赖。

`navigation-project-groups.ts`按同一固定提交的`folder_group`实体与`folder_group_service`移植非嵌套、唯一归属、
混合顶层/组内排序和删除组保留工程行为；持久化改用既有导航JSON与版本核对。来源注释及上述Apache-2.0通知保留，无新依赖。

`navigation-project-layout.ts`依据同一固定Codeg提交的`sidebar-conversation-grouping.ts`适配容器移动、拖放目标与布局序列化；
`ProjectLayoutDialog.tsx`保留固定行投放、工作树跟随主工程的交互语义，并接入Rovai主题、键盘和现有偏好版本冲突保护。
上述Apache-2.0来源通知保留；本批不增加拖放库或其他第三方依赖。

`navigation-project-tree.ts`及侧栏树形/分组菜单按同一固定Codeg的`sidebar-conversation-list.tsx`、
`sidebar-folder-group-header.tsx`适配主工程/工作树、置顶去重和统一菜单语义；保留上述Apache-2.0许可。
真实关系及分支来自Rovai只读Git API，不移植供体数据库；本批无新增第三方依赖。

侧栏活动徽章保留固定Codeg `sidebar-folder-group-header.tsx`与`sidebar-conversation-list.tsx`的整组活动语义，
按Rovai Thread/Run和导航可见性重写为`navigation.activity`只读统计；不复制供体会话数据库或用窗口内行数代替完整值。
上述Apache-2.0通知保留，本批没有新增依赖。

`canvas-member-interaction.ts`与派生成员节点按同一固定Codeg `canvas-view.tsx`的applyDrop/selection、
`conversation-card-node.tsx`的卡片和镜像语义接入现有ReactFlow；Core以可选sourceMembers接原子移动，不移植供体数据库。
来源注释和Apache-2.0许可保留，本批不增加第三方依赖。

画布会话呈现参考同一固定Codeg `canvas-conversation-surface.tsx`与`canvas-conversation-drawer.tsx`的原地交互，
在`canvas-conversation.ts`保留来源注释；实际会话、输入、流式、审批和停止复用Rovai既有组件及Core，不复制供体ACP执行层。
保留上述Apache-2.0许可，无新增第三方依赖。

`canvas-composer-drafts.ts`依据同一固定Codeg `canvas-conversation-surface.tsx`的
`draftStorageKey=canvas-draft:<contextKey>`、`attachmentTabId=contextKey`保留每卡独立输入；
按Rovai Host/Owner、Core、画板、真实pin与精确引用适配本机存储，不移植供体执行或数据库。
源码保留上述Apache-2.0来源通知，无新增第三方依赖。

画布临时终端的 `canvas_terminal/state.rs` 与 `helper.rs` 根据固定Codeg提交的 `terminal/manager.rs`、
`terminal/types.rs` 适配代次、整块回放和PTY生命周期，保留上述Apache-2.0来源通知。
新增Rust直接依赖锁定为供体使用的portable-pty0.8.1，crate校验值与[原MIT许可证](portable-pty/LICENSE.md)见
[来源记录](portable-pty/source.json)。该次锁定新增21包，既有278包版本和校验保持；完整传递依赖通知仍纳入I12发布资格，当前不宣称全部通知已完成。

I04原生提问/计划卡片按固定Codeg的`ask-question-card.tsx`、`plan-approval-card.tsx`和`ask-question.ts::splitRecommended`适配，
保留上述Apache-2.0来源注释与许可；组件、样式、交互及运行对照见[源码UI映射](../inventory/runtime-interaction-ui-reference.md)。
生产继续使用已有React、Radix Tabs、lucide-react与Rovai安全Markdown，没有新增依赖或修改锁文件。
原组件隔离视觉对照的测试专用依赖位于仓库外，不打入产品；其版本、安装锁及原源码摘要随本批证据记录。

I04 Core委派查看器按固定Codeg `sub-agent-session-dialog.tsx`、`session-viewer-host.tsx`、`ui/drawer.tsx`和委派行组件适配，
`CodegRuntimeMark.tsx`保留`agent-icon.tsx`静态SVG原区域及Apache-2.0来源通知，业务/状态不移植。
抽屉使用与原源码一致的BaseUI1.7.0；新增四个生产包各自MIT正文、版本、lock完整性及既有777包保持证明见
[BaseUI来源记录](base-ui/source.json)。原生浏览器表面未在本批移植，不声称该能力完成。映射和实际运行边界见
[查看器源码UI映射](../inventory/delegation-viewer-ui-reference.md)。

I04 原生只读查看器继续按固定 Codeg `message/subagent-session-dialog.tsx`、`session-viewer-host.tsx`和原 Drawer/header/角色容器适配。
`codeg-collab-tool.ts`在Apache-2.0来源头之后保留固定`src/lib/collab-tool.ts`完整原文字节，来源摘要与正文等价检查见[UI映射](../inventory/native-session-viewer-ui-reference.md)。复用原已准入BaseUI及现有Rovai安全Markdown，不新增产品依赖；原 ContentPartsRenderer 完整富内容仍在移植队列，参考外壳运行不冒称其完成。

I04 原生富内容的纯适配层、必要类型/引用/工具分类辅助及计划/提案/目标组件来自固定Codeg提交，保留Apache-2.0来源头；[逐文件映射](../inventory/native-content-ui-port.json)记录原Git blob、源码摘要、抽取范围与明确适配。
原样式冲突合并使用供体相同`tailwind-merge@3.4.0`，仅新增该一个无依赖leaf，781旧package/snapshot不变；[MIT正文](tailwind-merge/LICENSE.md)与[锁定记录](tailwind-merge/source.json)保留。生成的隔离卡片CSS使用已有开发缓存中的Tailwind4.1.18；[MIT正文](tailwindcss/LICENSE)及[生成来源](tailwindcss/source.json)保留，不增加生产Tailwind依赖或全局preflight。

I04 原生工具原语沿固定Codeg Apache-2.0组件，出处与声明级比较见[内容来源清单](../inventory/native-content-ui-port.json)。新增直接依赖intl-messageformat11.1.2、ansi-to-react6.2.6以及已在锁中存在的Radix受控状态1.2.3；共16个必要图节点，SRI逐包核对，新增12个包节点，782个既有包/快照及原importers不改，无生命周期脚本或全量重装。逐包原许可/来源位于native-tool-ui/。

许可未结项：ansi-to-react发布tar和精确Git提交145140971e9d83ccac32265f24b1c55dea437896均无LICENSE文件，原package.json及README声明BSD-3-Clause，已原样保存到[native-tool-ui/ansi-to-react](native-tool-ui/ansi-to-react/source.json)。没有伪造版权人或许可证正文，也不把该项记为许可完整确认；阶段对外分发前必须补齐可据来源或替换此依赖。

I04 会话图片/生成图/完整图片弹窗、TurnStats、token-format 与 codex-visualize（含原用例）继续使用固定 Codeg Apache-2.0 来源，逐文件摘要/原 Git blob/唯一只读按钮 guard 见[内容来源清单](../inventory/native-content-ui-port.json)的 mediaStats。双语目录按原 zh-CN/en 精确提取。图片菜单/剪贴板、Dialog、Tooltip 和文件/HTML服务均复用已准入 Rovai 依赖；隔离 source-media.css 仍由已记录的 Tailwind4.1.18 生成，不新增生产依赖或全局 preflight。完整原 Markdown/动态卡引擎仍待，不以本批称全部内容引擎许可已完成。

I04 原Markdown引擎的39份组件/纯模块/原用例及提取闭包继续按固定Codeg Apache-2.0来源；原翻译及globals样式来源见[源码清单](../inventory/native-content-ui-port.json)的markdownEngine。13生产/3开发直接依赖固定供体lock版本，280新增包节点；[逐包许可目录](native-markdown/)保存311个实际下载包的许可原文、精确版本、tarball/SRI及9项补取来源。victory-vendor同时保留原MIT与13份vendor通知（含ISC、BSD和MIT），不以根声明覆盖子包通知。此次补齐不消除此前ANSI完整许可文件缺口；全新完整安装/平台发布资格仍在阶段候选核对。

I04 原文件/类型徽标与右键菜单继续按固定Codeg Apache-2.0组件，7份新来源与2份复用记录见[来源清单](../inventory/native-content-ui-port.json)的referenceActions。完整原菜单JSX/包装保留，Core宿主接缝和未完目录/系统引用明确记录。新增直接Radix ContextMenu2.3.1复用已锁传递节点，新增开发依赖jest-dom6.10.0保留原断言；旧1090包/1114快照不变，9新增包/快照。[本批许可目录](native-reference/)保存12个实际下载包的完整原文、精确版本、tarball/SRI和来源，css.escape保留其LICENSE-MIT.txt。已有Radix许可复用先前native-markdown记录；不声明此前ANSI完整许可缺口已消除。
