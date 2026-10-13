# I02-T01 工程首页切片

本切片将Codeg工程工作台入口行为接入Rovai既有导航。使用Project菜单“进入工程”，保留原项目行展开/折叠和目录偏好行为；首页显示该工程名、规范路径和最近对话，继续使用既有创建/打开Thread操作。

页面历史的quick_chat增加可选客户端projectPath，公共Core RestorableLocation保持原合同。进入工程先调用现有workspaces.inspect，在共享离开保护成功且导航意图仍最新时提交规范路径。取消或迟到结果不改变当前工程；已移除的历史目录回退，损坏相对路径偏好被拒绝。未增加数据库、业务身份或Runtime。

新增两项回归先失败，再与既有导航用例一起通过。最终导航、工程偏好及语言覆盖41项通过，类型检查通过。原生Electron组件夹具验证菜单键盘进入、工程内对话过滤、Back/Forward及两主题，并保留既有导航回归；它不启动Core或Runtime，不代表完整业务验收。

Windows隐藏窗口首次在旧的菜单淡入断言处失败；未改断言或产品CSS。原始基线夹具在本机另一次执行中推进到折叠场景后100秒超时，不能写成同一断言稳定复现。为测试窗口采用Electron支持的[离屏渲染](https://www.electronjs.org/docs/latest/tutorial/offscreen-rendering)，完整组件夹具通过，失败、跳过均0；不涉及生产窗口设置。

双主题截图已人工核查，使用夹具目录与投影数据；无实际AI执行。41单元、组件截图与构建不能直接把324个融合验收场景标记通过。

I02-T01仍进行中。下一项验证生产BusinessApp与真实隔离Host的工程进入、重开、失效及迟到请求，再推进成员/Provider、Skills/MCP。缺少的客户资源见customer-todos.json，不阻塞独立开发。
