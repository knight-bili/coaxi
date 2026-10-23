# Coaxi 品牌升级与兼容

Coaxi 是当前融合产品名称。开发继续使用 `develop/V1.0.0`，版本仍为 0.4.8，目标为 V1.0.0。

- 安装识别号 `app.codegrovai.workbench.desktop` 和 productId `codeg-rovai-workbench` 保持稳定。
- 默认数据目录固定为原 `Codeg Rovai Workbench`，不随显示名称改变。显式数据目录仍优先。
- Core、Electron、技能库、MCP、加密配置及记录内的绝对路径继续原位使用，不复制或接管上游应用数据。
- `rovai` 命令、URI、环境变量、内部 IPC、移动包标识及备份扩展名保持兼容；它们不是可随品牌盲替换的显示文案。
- Windows 安装器识别旧桌面进程名，但仍校验进程位于本次安装目录；默认卸载不删除数据。
- 更新源只指向 `knight-bili/coaxi`。不会发布或合入 main；上游许可证和固定来源不改变。

升级前应关闭应用并校验备份；先用隔离数据副本启动新安装包，再升级日常安装及快捷方式。
