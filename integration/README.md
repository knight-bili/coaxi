# Codeg 与 Rovai 一期完整融合

I00已自审修正并合入，I01基础构建与契约证据保留草稿；I02继续补齐，按已确认r1补齐实际输入并推进独立I03画布基础，完整一期终点不变。Rovai Core唯一业务与执行权威，Codeg与双原生移动按模块移植；第二阶段才开发自主新增能力。

- [完整计划](planning/plan-v4.md)、[日历基准](planning/schedule.json)、[源码锁定](sources.lock.json)。
- [功能账本](planning/features.json)、[研发工作包](planning/work-packages.json)、[验收场景](acceptance/cases.json)。
- [候选入口库存](inventory/source-entrypoints.json)、[开发规则](development.md)、[I00验收单](acceptance/i00.md)。
- [资源与风险](resources.md)、[许可证保留](licenses/README.md)。
- [免费优先 CI 安排](ci-cost-policy.md)：公开前审查、标准运行机、按里程碑启动全平台验证及产物保留边界。

源Rovai当前版本指针只代表源骨架状态，不证明融合完成。根文档和品牌保留，后续设计阶段再处理。

- [I01执行入口](foundation/README.md)。
- [持续执行状态](execution/goal-state.json)、[需要客户补齐的待办](execution/customer-todos.json)。客户已授权沿I00至I12自动推进，阶段分支持续开发，main仍由客户验收控制。
- [I02技能包配置证据](execution/i02-skill-members-result.json)：目录、安装/更新卸载与队员配置已验；[实际模型投递r1](../docs/versions/v1.72/model-context-change-skill-pack-member-delivery.md)已获客户明确确认，[本地实际输入及恢复](execution/i02-skill-delivery-context-result.json)已接通并验证，逐SP矩阵和真实Gate仍继续。Office及其他一期功能继续实施。
- [I03画布架构](../docs/architecture/canvas.md)：多画板Core元数据及八类节点图已实现；[固定版本节点源码清单](execution/i03-node-source-audit-result.json)与[节点持久化/真实升级验证](execution/i03-canvas-nodes-result.json)可查。交互UI、实时业务绑定、布局、客户端视口和真实临时PTY继续实施，均保留在一期，不能把Core基础当整个I03完成。
- [I02恢复兼容矩阵增补](execution/i02-skill-delivery-matrix-result.json)：数据库边界的执行轮次、旧公开29/33与旧A2A冻结输入补测已通过；[数据库修复](execution/i02-db153-regression-repair-result.json)的 Windows109项与Linux110项全部通过，原失败保留。[进程恢复](execution/i02-skill-process-recovery-result.json)已验真实Core重启保留冻结输入并终结未知结果、禁止重发；明确确认分支、其余逐SP与真实模型Gate继续补齐。

- [I03项目分组排列检查点](execution/i03-project-layout-result.json)：侧栏/画布统一入口、组与工程混合拖放、键盘排列及冲突/未知结果保护已编码；37项相关检查与4项静态导航检查通过，实际交互和当前Host资格仍待阶段候选。

- [I03工作树层级与分组菜单检查点](execution/i03-project-tree-result.json)：真实Git父子/分支展示、置顶去重、显式打开登记和统一分组菜单已编码；42项相关测试与5项静态导航检查通过，新Rust及实际Host/UI仍待阶段候选，活动统计继续实施。

- [I03完整导航活动统计检查点](execution/i03-navigation-activity-result.json)：Core按普通导航Thread完整聚合、窗口外组定位及工程/分组三态徽章已编码；78项相关检查、6项静态导航与7项SQL原文夹具通过，新Rust/真实Host/挂载UI继续按阶段候选补验，不据此标I03完成。

- [I03区域成员拖放检查点](execution/i03-canvas-member-drag-result.json)：派生成员节点、跨源原子分组、精确引用和旧服务保护已编码；52项相关检查及定向类型/文档/格式通过，新增Rust与真实Host/UI仍待阶段候选，原地展开和Composer继续实施。

- [I03画布原地与侧边会话检查点](execution/i03-canvas-conversation-result.json)：复用既有会话/输入/执行控制，补精确私有绑定和离开保护；40项组件及4项离开退出检查通过，定向类型/文案/文档通过，真实UI/Runtime及多卡、重开恢复、通知继续，不作为I03完成依据。

- [I03会话读写归属检查点](execution/i03-thread-scope-result.json)：既有页面改用固定Core client/Thread的共享读写层；13声明保持原文、10旧导出兼容，216项定向检查与类型/文案/文档通过。多卡实例、共享草稿和展开恢复仍继续，实际UI/Runtime待阶段候选。

- [I03私有共享草稿检查点](execution/i03-single-chat-shared-result.json)：既有面板接共享正文、发送互斥、保存失败恢复与保留输入切换，原send和pending edit请求表达式保持；41项相关检查及类型/文案/文档通过。公开草稿镜像和多卡挂载仍继续，真实UI/Runtime未验。

- [I03每卡独立草稿与恢复检查点](execution/i03-canvas-composer-context-result.json)：固定Codeg源码确认每卡输入独立，纠正此前共享草稿计划；已接主会话隔离、Pending输入并集、Web公开归档与精确副本、Native私有正文存储。251项相关检查及类型/文案/文档通过；当前仍单聚焦，多卡同时挂载及私有附件/引用隔离继续，真实UI/Runtime未验。

- [I03按Thread读取与刷新检查点](execution/i03-thread-projection-result.json)：现有页面的只读打开、刷新、缓存和历史分页已用client/Thread owner；刷新尾读、过期页、删除/重启代次、挂载租约和旧错误保护已接。215项相关检查及类型/文案/文档通过，enter参数与原发送/编辑器源码保持；实际多卡UI、事件分发和各卡文件预览继续。

- [I03实际多卡会话检查点](execution/i03-canvas-multi-result.json)：多卡及镜像、独立输入、稳定原地/侧边实例、每Thread事件读取与窗口预览并集已接。296项相关检查与类型/文案/文档通过；浏览器实际挂载三张生产卡片、模拟Core/文件，验证独立草稿、撤销、局部查找、关闭重开及预览归属/缩放。通知、持久展开恢复、私有附件隔离和真实Core/Runtime/完整应用验证继续，不作为I03完成依据。


- [I03多卡通知可见确认检查点](execution/i03-canvas-notification-result.json)：实际卡片来源并集、裁剪/遮挡观察、每Thread确认/独立重试和client围栏已接；245项检查、类型/文案/文档及浏览器9项检查通过。现有隔离Electron通知检查1通过1失败，设置焦点问题在旧Controller对照同样复现并登记研发整改；真实Core通知、公私来源/Host与规模待验。

- [I03展开保存与只读恢复检查点](execution/i03-canvas-recovery-result.json)：精确Host/Owner/Core/Board记录、切页切板保留、显式收起、恢复/停止/重试与saved-input恢复已接；230相关检查及类型/文案/文档通过。真实CanvasWorkspace/Surface/ReactFlow模拟Core完成12步检查，另发现并修复展开白屏；只读运输没有enter/启动Runtime。原编辑器/undo的原子离开、Native完整/异常进程恢复、私有附件隔离及真实平台仍待补齐。

- [I03原编辑器离开事务检查点](execution/i03-canvas-atomic-leave-result.json)：导航失败/拒绝/失效、等待取消与Renderer后续退出检查失败保留同一编辑器和撤销；画板拒绝/未知结果保留实例、核对成功才交接。247相关检查、类型/文案/文档及生产画布mock浏览器验证通过，并修复删除后自动选板循环；Native退出/异常进程/Host与私有附件隔离继续。

- [I03卡片使命交付与累计Diff检查点](execution/i03-canvas-mission-result.json)：复用原Mission说明/启动/交付/活动/累计Diff，核对Thread/Mission标识、来源回原卡片及镜像控件隔离；修复选中节点遮挡真实会话控件。228相关检查与类型/文案/文档、mock前端交互通过；独立持久对象引用、完整私有导航和真实使命运行/平台验收继续。
