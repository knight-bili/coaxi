# I02-T01 工程首页切片

本切片将Codeg工程工作台入口行为接入Rovai既有导航。使用Project菜单“进入工程”，保留原项目行展开/折叠和目录偏好行为；首页显示该工程名、规范路径和最近对话，继续使用既有创建/打开Thread操作。

页面历史的quick_chat增加可选客户端projectPath，公共Core RestorableLocation保持原合同。进入工程先调用现有workspaces.inspect，在共享离开保护成功且导航意图仍最新时提交规范路径。取消或迟到结果不改变当前工程；已移除的历史目录回退，损坏相对路径偏好被拒绝。未增加数据库、业务身份或Runtime。

新增两项回归先失败，再与既有导航用例一起通过。最终导航、工程偏好及语言覆盖41项通过，类型检查通过。原生Electron组件夹具验证菜单键盘进入、工程内对话过滤、Back/Forward及两主题，并保留既有导航回归；它不启动Core或Runtime，不代表完整业务验收。

Windows隐藏窗口首次在旧的菜单淡入断言处失败；未改断言或产品CSS。原始基线夹具在本机另一次执行中推进到折叠场景后100秒超时，不能写成同一断言稳定复现。为测试窗口采用Electron支持的[离屏渲染](https://www.electronjs.org/docs/latest/tutorial/offscreen-rendering)，完整组件夹具通过，失败、跳过均0；不涉及生产窗口设置。

双主题截图已人工核查，使用夹具目录与投影数据；无实际AI执行。41单元、组件截图与构建不能直接把324个融合验收场景标记通过。

生产Web与真实Windows Rovai Host的11项检查已通过，见[i02-navigation-live-result.json](i02-navigation-live-result.json)。使用I01已核实的原生Core二进制、当前编译Web和全新私有数据根；没有Desktop桥接mock或Runtime。通过真实Core API创建两个工程各一条活跃空Thread，验证工程内对话、前后退、刷新、目录缺失保留原页面/历史、恢复重试及浏览器进程重新打开。仅延迟真实Core返回的A响应，在较新的B请求完成、旧A响应交付完成后断言B仍有效；不替换Core结果。

真实回归发现“目录恢复后进入成功但旧错误残留”，在工程进入动作开始清除旧错误，原失败断言随后通过。浏览器验收工具改为正常退出后重开，避免强制终止导致刚保存的偏好尚未落盘；重开后还等待真实对话投影加载，截图不取启动中间态。数据库完整性为ok，agent_run为0；当前类型检查、Web构建及37项定向回归通过。截图已人工核查，完整原始证据保留工作区，提交记录只含哈希、检查结果和限制，不提交凭据、数据库或浏览器配置。

复跑使用tools/integration/smoke-workbench-live.mjs，设置绝对路径ROVAI_HOST_BIN、ROVAI_WEB_UI、WORKBENCH_EVIDENCE_ROOT、WORKBENCH_BROWSER_BIN；只支持本次新建隔离数据根。Windows由Host原生prepare命令建立私有根，明确独立skill library和MCP路径。浏览器应使用支持当前CDP的Chrome/Chromium，无真实AI运行。

I02-T01仍进行中，桌面进程/Host进程重开及实际AI执行未验证，324融合场景仍not_run。后续补齐项目启动器、会话地图、通知及首次使用差异，再推进成员/Provider、Skills/MCP；原生Web脚本的旧草稿API记入development-todos.json，由研发修正。缺少的客户资源见customer-todos.json，不阻塞独立开发。
