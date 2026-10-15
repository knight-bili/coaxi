# 许可证和来源保留

Rovai保持根MIT LICENSE和完整Git历史。Codeg桌面及原生移动为Apache-2.0，移植时保留相应声明和NOTICE。
sources.lock.json锁定提交和原样复制的许可证hash，包含科研技能、Playwright和移动第三方通知。当前没有宣称全部间接依赖已完成法律审核。
每批移植补查对应第三方许可、通知和来源，I12核查发布包通知与依赖清单。开源许可不等于商标或服务账号授权。

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
