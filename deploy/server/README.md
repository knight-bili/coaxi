# 独立 Server 容器入口

这是一期 C35/I10-T01 的容器部署源码，沿用固定 Codeg Dockerfile 的 Node 24、Debian bookworm、Git、SSH、Python 和 OfficeCLI ICU 依赖。业务仍由同一个 Rovai Host/Core 负责。当前只编写源码，未构建、启动或验证镜像，不代表已发布版本。

镜像使用后续构建阶段产出的完整 `out/server/linux-x64/`，不下载来历不明的主程序，不在镜像构建过程中初始化数据库、登录模型或启动队员。Server 包的 manifest、程序权限、渠道资源、原生历史 helper、Web 客户端与许可在镜像组装时一起核对。Linux x64 包继续受现有 glibc 2.35 基线约束；ARM64 不套用这个镜像。

后续获准构建和测试时，在仓库根目录执行：

```sh
pnpm build:server --target-key linux-x64
docker compose -f deploy/server/compose.yaml build
docker compose -f deploy/server/compose.yaml up -d
```

这些命令现在没有执行。构建需 Linux x64 构建环境、Rust、Node 24、pnpm 与对应依赖；构建期网络下载和空间按既有工程规则管理。普通镜像运行不需要源码或 Rust 工具链。

默认只映射宿主 `127.0.0.1:8767`，管理令牌仍由原 Server 私有数据目录生成和保存。需要连接时由管理者明确读取，不能写入镜像、Compose 文件、Git 或公开日志：

```sh
docker compose -f deploy/server/compose.yaml exec workbench rovai-server --data-dir /data token
```

需要公网使用时在部署端配置 HTTPS 反向代理、明确公开地址和现有 Host 认证；不要直接把默认本地端口改成公开监听后当作已完成安全配置。

| 容器目录 | 用途 | 保留方式 |
|---|---|---|
| `/data` | Core数据库、画布、附件、技能、安装锁和操作回执 | `workbench-data` 独立卷 |
| `/workspace` | 工程文件、Git工作树和终端起始位置 | `workbench-projects` 独立卷；也可明确绑定自己的工程目录 |
| `/home/node` | 当前容器账号的模型程序配置、凭据和用户级安装 | `workbench-home` 独立卷；不自动读取宿主账号 |
| `/opt/workbench` | 与源码版本一致的主程序及静态资源 | 镜像只读层 |

默认非root运行。若使用已有宿主绑定目录，管理员需要让UID/GID 1000拥有所需访问权限；应用不会自动递归修改宿主文件权限。Runtime程序、模型账号和OfficeCLI需要在容器环境明确配置，不把宿主Windows/macOS绝对路径视为有效Linux程序。

容器沿用一次Core调度、五种渠道和CLI，缺少已认证Desktop执行端时浏览器/电脑工具明确不可用。没有把容器权限扩大成宿主桌面权限。

容器通过替换镜像升级：先保留加密备份和原镜像版本，再停止当前容器、装载新镜像并保留三个数据卷。容器内原生二进制下载/安装被拒绝，避免只改写临时层后在重建时丢失升级。发生数据库升级失败时保留失败证据，使用原备份和匹配镜像恢复；不能把旧镜像直接覆盖已升级的数据当作回滚。Compose停止会发送SIGTERM并保留60秒有序退出时间；超时、重启和持久化效果留待后续真实验证。

移动真机、容器构建运行、模型与渠道账号、OfficeCLI刷新、同场景UI和长时间可靠性仍在后续验证范围，未计通过。不得执行 `down -v` 清除唯一数据卷。
