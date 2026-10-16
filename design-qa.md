# Codeg 画布界面还原：工程核对记录

本记录对应 2026-10-16 客户提供的五张真实 Codeg 截图和融合应用容器问题截图。最新有真实运行证据的界面提交caa03f67、候选fusion-i03-rc.13；其后各类卡片样式代码继续调整，尚未进入新集中运行批次。此前rc10容器修复、rc13私聊工作区修复及定向功能回归通过，整套画布UI仍有明确剩余项。以下前期记录保留，最新结论见末尾，代码检查不替代视觉核对。

## 参考真值

原图已逐字节复制保存在 `../i03/codeg-ui-reference/`，`manifest.json` 记录原始路径、当前路径、尺寸和 SHA-256。

| 状态 | 参考文件 | 原图像素 |
|---|---|---|
| 无画布主页 | `01-board-home.png` | 1890 × 1290 |
| 创建画布弹窗 | `02-board-dialog.png` | 1890 × 1290 |
| 空画布与工具栏 | `03-empty-board.png` | 1890 × 1290 |
| 展开会话 | `04-conversation.png` | 1890 × 1290 |
| 文件与会话并排 | `05-file-conversation.png` | 2559 × 1527 |

固定源码及组件对应见 [画布源码参考](integration/inventory/canvas-ui-reference.md)。

## 当前证据及比较历史

- 客户指出旧融合界面的整体结构与原 Codeg 不符。旧实现可见两排文字工具、管理表单风格及重复会话标题；该反馈仍作为 P1 问题保留。
- 本批已按原源码改为细标题栏、画布列表和空状态、颜色圆点表单、底部悬浮图标工具、右下地图与水平缩放、统一卡片框架；绑定私聊卡片保留真实队员和私有标识，并去掉重复标题。所有节点、原命令、权限和输入恢复链路保留。
- 修正前的顶部文字工具条整理没有解决整体结构问题，已被本批布局替换，不能用那版截图作为本批成果。
- 新实现截图已生成：rc10真实隔离Native的创建、空画布和列表，CSS视口1440×920、DPR1.5、应用缩放1、夜主题；PNG2160×1380。原Codeg参考的CSS视口/DPR未知，不能冒充同视口像素比较。
- 全图比较：rc10工作区已贴边，外层多余横纵滚动均0；底部工具与地图沿用原布局。客户本次原图逐字节保存在../i03/codeg-ui-reference/06-fusion-container-before.png。
- 局部比较：实际创建弹窗为448px、24px内距、28px圆角、胶囊输入及12色；空画布及文件卡片已视觉检查。私聊截图仍有重复标题，不能宣告全部还原。

## 五项视觉核对

| 项目 | 已有依据 | 待实际运行核对 |
|---|---|---|
| 字体和排版 | Codeg 原源码默认 Inter 与系统中文回退；当前融合应用保留原平台字体 | 字号、行高、字重、长标题截断和中英文换行，判断平台字体差异是否造成可见漂移 |
| 间距和布局 | 原组件的 36px 标题、16px 卡片圆角、32px 工具按钮及胶囊布局 | 父布局、最小窗口、浮层与画布坐标、按钮可达性、缩放后是否重叠 |
| 颜色与主题 | 新增画布专用中性色，十二个身份色采用固定供体原值；两套主题使用同一组件树 | 日/夜对比度、菜单弹窗继承、私聊输入区和文件阅读器的实际颜色 |
| 图像与图标 | 同版本 lucide-react 标准图标和真实队员头像；用户内容不作为产品装饰打包 | 图标居中、大小、头像清晰度；文件卡片仍需实际挂载确认 |
| 文案与内容 | 创建表单和空状态采用参考中的层级；真实 Rovai 对象与权限文案保留 | 长画板名称、空/加载/失败/恢复状态、真实队员私聊标识是否清楚 |

## 当前检查和后续动作

低成本检查：前一生产批全量TypeScript、六文件53项、冻结安装及文档治理通过；当前容器批types0、控制器4项、文档0、JS语法0、diff0，CodeGraph同步。rc10 Desktop/Web各一次构建通过，复用2158df02已核验Core/Host/CLI，没有再次编译Core或触发Actions。

当前真实Native：visual-r2=4、files-r1=9、objects-r1=8、private-ui-r3=2，合计23项定向检查通过；四次成功运行均受控退出，无截止超时或强杀。1440×920主容器四边padding0、溢出0、尺寸差0；实际1040×702小窗口仅列表滚动且末项可达。真实创建后核对精确新boardId。该结果不等于整个I03或324场景通过。

截图在../i03/c06/fusion-i03-rc.10/：native-visual-r2的00-board-home.png、01-board-dialog.png、02-empty-board.png；native-files-r1的01-text-and-interactive-html.png；native-private-ui-r3的01-private-attachment-preview.png。最后一张仍停在单聊加载态，DOM及授权读取两项通过不等于完整预览视觉资格。

失败保留：rc9创建保存后未进入新画布，生产回执判断已修复。rc10 visual-r1在动画中测量失败且测试表单挡住退出，原driver180秒终止；修正owner稳定等待及失败清理后r2受控退出。private-ui-r1/r2寻找已被生产恢复链路展开卡片的展开按钮而失败；修正owner按nodeId识别恢复状态后r3通过。测试owner修正没有引发重复生产构建。

继续执行：P1私聊卡片重复标题及内容容器贴合、稳定可见附件预览；Codeg列表真实缩略预览/节点数量仍未接入，禁止假数据。日主题、完整八类创建、错误状态、C05冷恢复/同Thread多卡及准备好Git工作区的Diff继续按有限清单补证。当前只取得Diff诚实不可用分支资格。Mac/移动真机及适用平台验证待资源，最终MVP不豁免。

源码/产物哈希和实际指标见[本批验证记录](integration/execution/i03-c06-source-ui-verification.json)，全页面执行[原源码UI规则](integration/ui-source-policy.md)，工程检查由研发负责。

## rc13 私聊及原生鼠标复核

按固定Codeg CardFrame只保留一个36px拖动标题栏，行内正文贴合卡片，侧边模式保留独立标题；编辑、移至侧边/放回画布、打开完整工作区、收起等入口保留。节点及正文统一层级，相邻折叠卡片不再遮住操作。私聊目标引用按身份稳定，绑定私聊使用Rovai原timeline-pane紧凑预览替换。

真实结果：私聊4、原对象8、文件HTML9、列表/窗口4，共25个不同Native断言通过；私聊同owner在两个独立App冷启动进程均通过，不把重复运行累计成8项。鼠标实际命中标题操作，行内/侧边往返保持同一个textarea，草稿仍只有乙材料，无消息和AgentRun。成功运行均受控退出。全量types0、原会话/恢复/控制器三文件39项未变规则复用；实际鼠标与可见性由Native直接验证。

当前截图：../i03/c06/fusion-i03-rc.13/native-private-ui-r2/01-private-attachment-preview.png与02-private-conversation-returned.png。前者实际显示乙材料正文，鼠标命中正文可见区域；后者真实回到行内，重复内标题已消除，操作和材料仍在。不是只依赖DOM存在。两个相邻同名卡片是实际两个node，不是同一卡片重复标题。

失败自审保留：rc11截图暴露标题操作被遮挡；rc12真实鼠标切换触发选中状态在两条回调间交替写回，React185使页面空白。r5调试记录确证只有selected在空/原nodeId间交替；改由onNodesChange同批维护后，rc13同用例通过。临时断点诊断代码已撤除，保留错误采集。rc13 files-r1/visual-r1因调度失误共用目录而排除结论；进程退出后入口增加独占锁，两个r2串行通过。

console核对：原作者HTML的connect-src none阻止诊断SSE，安全策略保持；尺寸切换仍出现ResizeObserver延迟通知，原始消息保留并列入后续整改，不宣告零控制台问题。空私聊中恢复入口挤压正文、局部滚动仍需整理。其余未完包括真实列表预览/数量、双主题/八类路径、同Thread多卡/崩溃/规模及适用平台。

源码/产物/失败证据和console记录见[私聊工作区验证](integration/execution/i03-c06-private-workspace-verification.json)。本批修复通过不等于I03或MVP完成。

final result: blocked

原因：整套画布视觉核对仍有上述未完成项；容器修复已经通过，持续目标仍active，没有新增客户阶段技术验收门槛。

## 各类卡片源码对照后的第一批调整

客户继续要求所有卡片沿用原Codeg设计。本批已逐项定位摘要、展开会话、文件、终端、四类区域、便笺，以及真实Rovai对象的对应源码，见画布源码参考的新表。已改摘要四行标题及紧凑元信息、区域透明度和胶囊、便笺轻边框、淡色底与选中环，保留真实操作。全量types0、Native owner语法0、diff0。

未进行新的Desktop/Web构建或Native运行；新代码视觉资格仍待下一集中批次，rc13旧证据仅适用于原提交。明确剩余包括原便笺整纸拖动/行内编辑、终端控制栏、空私聊恢复入口高度、双主题/缩放/长标题及尚缺的真实投影，不宣告完整还原。详情见 integration/execution/i03-c06-card-source-style-result.json。

final result: blocked

仅代表整套视觉核对未完；持续目标active，不需要客户阶段技术验收。

## 便笺原交互与终端控制栏

按原NoteNode恢复整纸拖动、双击/键盘行内编辑和失焦/Escape保存；保留安全Markdown，将标题/颜色编辑保留在选中工具条。Core仍按捕获版本只写content；冲突保留本机原文和草稿，明确核对后再保存。草稿单独按Host/Core/板/便笺隔离，不写入视口偏好或模型输入。终端六操作改为24px图标，原处理函数及输入异常恢复保留，路径沿原版在标题悬停展示。

全量types0、草稿与graph两文件24项通过、文档测试10项/门禁0；实际鼠标/输入法/保存失败/离开/冷恢复和终端运行尚未验证。原rc13截图不授予本代码资格，待集中候选执行。记录见integration/execution/i03-c06-note-terminal-interaction-result.json。

final result: blocked

## 空私聊高度与本组集中验证入口

已实际查看rc13返回私聊截图，结合原Rovai CSS定位到空状态300px min-height及闲置默认材料外框占位。仅画布绑定私聊改为自适应空状态，并减轻未展开材料入口；实际正文/展开材料仍保留原滚动和完整操作。本次types0与语法/差异检查通过，尚未运行新Renderer。

已有Native owner新增notes六项真实操作检查，以及最小260px终端六按钮可达和空私聊viewport真实无溢出检查；源码入口已经准备，不能记为通过。该组代码已齐，下一步冻结单候选集中构建/运行，复用既有原生程序，不继续堆新的界面范围。记录见integration/execution/i03-c06-private-empty-fit-result.json。

final result: blocked

## rc14 本组真实运行结果

生产c4463813、Desktop/Web各一次构建0，复用2158df02原生。串行终端5、私聊5、文件9、原对象8，共27不同Native断言通过；全部成功及三次失败run均受控退出、无deadline或强杀。已查看终端最小宽与私聊返回截图：六操作可见，真实输入2+3得到5并停止回收；私聊空状态显示完整、viewport extraY0，原附件/同textarea模式切换保留。

便笺r1/r2/r3均在首拖动断言未过。r3真实Core位置从0/0变48/24，而owner预期72/48；不能继续说拖动完全无效，也不能把它算通过。下一步采集浏览器实际缩放、DOM/RF矩阵及window捕获mousemove，区分坐标输入/对齐和产品问题。后续行内编辑/冲突/导航验证未执行。原有ResizeObserver及作者CSP诊断限制保留。

证据见integration/execution/i03-c06-source-card-group-verification.json。

final result: blocked

仅整套画布工程视觉仍有待补项；目标active，本轮无需客户介入。

## 便笺定因与同私聊双卡真实检查

复用rc14生产c4463813，未增加构建。r4实测缩放/矩阵1、可信鼠标坐标正常；按XYDrag的激活起点和原computeAlignment边缘吸附，最终应80/48。r7沿原产品行为通过便笺六项，保留r1-r6旧失败；没有关闭吸附或修改拖动代码。

实际双私聊卡片六项和新App进程冷路由三项通过：同一conversation在两个node的正文/材料scope分开；第二张打开、模式往返与单卡收起重开保留另一textarea；暖态输入框滚动保持；冷态恢复精确两个pin及两份正文，无消息和AgentRun。三次成功均受控退出，三个截图已实际查看。本次不同断言新增14，连同上批当前候选共41；重复无发送断言只计一次。

便笺截图保留普通导航被阻止时的通知，核对后已保存；该画面不当作无错误全视觉资格。IME、硬崩溃、公开/多Thread、真实规模/资源释放、ready Git Diff及完整主题/字号仍待。证据见integration/execution/i03-c05-notes-mirrors-native-verification.json。

final result: blocked

仅完整画布视觉/阶段验证未完；目标active，无本轮客户事项。

## 卡片输入在界面进程硬中断后恢复

复用rc14的c4463813，无新编译。Native renderer-crash-r3实际两次render-process-gone=crashed/exitCode2，进程326000→317920→328112，Main保持277224。显式reload及选择画布后，精确原板的两张私聊卡恢复不同正文及最后甲；便笺未经失焦保存的最后乙作为本机副本可见，Core原文仍保留。三场景通过，明确保存自有便笺后受控退出，无deadline/forcedSignal。已实际查看2520×1500截图。

r1错误要求固定启动快照的reload直接进画布；r2的helper又返回列表，触发已恢复便笺的正常离开保护，未取得正常退出回执。旧失败与截图保留；r3修helper并在重试前通过原UI核对保存自有草稿，未改产品行为。

截图仍发现便笺document字号15px覆盖原Codeg 13px；完整主题/八类状态及ResizeObserver通知继续记录。本次不授予自动恢复入口、整App/Core硬杀、未知请求或IME资格。证据integration/execution/i03-c05-renderer-crash-native-verification.json。

final result: blocked

完整视觉/阶段检查仍未完；持续目标active，无本轮客户事项。

## 公开多卡、焦点与64摘要卡真实检查

复用rc14/c4463813，无新编译。public-multi-r5八场景通过：两Thread三张公开卡独立输入和实际保存记录，原生鼠标焦点/可信输入在另一卡布局往返时保持同DOM；一Thread一个读者，0/1/1/2/2/1/0/0序列、全部关闭后的focus/visibility完整监听签名回到baseline。公开卡收起重开保持原正文，无消息或AgentRun。另板64个持久摘要节点实际渲染，可信滚轮按原panOnScroll平移，比例63%不变、节点全集保持。两张2520×1500截图已查看。60静态rAF样本不代表全交互性能或64展开工作区资格。

r1漏只读traceId、r2把每卡发布监听也当成读者、r3忽略原350ms防抖、r4把滚轮平移当缩放均为夹具预期错误；定因后修owner，未改产品或放宽保护。r1-r5全部正常受控退出，旧失败保留。仍有ResizeObserver通知、便笺document字号、完整主题/八类状态及真实列表投影待处理。证据integration/execution/i03-c05-public-multi-native-verification.json。

final result: blocked

仅完整视觉/阶段验证未完成；目标active，本轮不需要客户介入。

## 真实可用Git精确Diff与公开卡新进程恢复

复用rc14/c4463813及2158df02原生。固定ACP协议脚本经原Runtime准入和调度，真实Core创建Mission worktree，无数据库种植或外部模型。ready-diff-r4五项通过：固定基准、三文件真实Git变化、中文#路径精确进入原Diff并显示甲删除/乙新增；索引物理SHA/纳秒mtime与条目均不变，源checkout干净；目标恢复原文后明确显示原路径不在清单中，无默认选中其他文件。r3同五项保留但索引证明较窄，r4不是重复累计。

public-cold-r1三项通过，新App显式打开原64节点和三张公开pin，恢复包含原生焦点追加文字的三份独立正文，原Thread无消息/AgentRun。r1/r2失败分别是夹具null准入及重试显示名冲突，均受控退出；全部本批run无deadline/强杀。已查看r4精确Diff、r3无替代和公开冷恢复截图。

失效态搜索框双层边框不贴合需随下一批按原样式纠正；便笺字号/完整主题八类与ResizeObserver原问题继续保留。本批通过的是真实Core/Git/界面和受控协议，不是模型推理、自主工具或自动展开恢复。报告integration/execution/i03-c02-ready-diff-public-cold-native-verification.json。

final result: blocked

完整视觉/阶段工程验证未完，目标active，本轮无需客户介入。

## 卡片源码对照补正（待集中运行）

再次对照固定 Codeg `nodes/card-frame.tsx:13–18,64,98–110` 与 `nodes/note-node.tsx:139–146`。现有摘要、实时会话、文件、终端、区域和便笺继续沿用既有原组件映射，不新增另一套卡片设计。本次只补正文普通光标/文字选择、标题拖动时禁选及便笺 Markdown 局部13px/1.625；普通文件文档仍保留原阅读样式。

CSS解析和差异空白检查通过，没有新增构建、Native运行、推送或Actions。报告 `integration/execution/i03-c06-card-source-followup.json` 绑定本次CSS；rc14已有60个Native场景不覆盖新CSS。下一冻结候选再检查日夜主题、八类、长标题、小窗口、正文选择和必要滚动；不把代码对齐当作视觉通过。原Diff搜索框与ResizeObserver问题仍保留。

final result: blocked

完整视觉/阶段工程验证未完，持续目标active，无新增客户确认事项。
