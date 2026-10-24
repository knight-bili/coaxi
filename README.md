<div align="center">
<img src="build/icon.png" width="88" alt="Coaxi 图标" />

# Coaxi

**把 AI 队员、无限画布和真实工程放在同一个工作台。**

[使用说明](docs/guides/coaxi-workbench.md) · [开发指南](docs/development/README.md) · [文档导航](docs/README.md) · [一期融合范围](integration/README.md)
</div>

Coaxi 是基于 Rovai Core、融合 Codeg 工程与画布能力的本地 AI 工作台。你可以选择电脑上已安装的智能体，为队员分配职责，在会话和无限画布中组织工作，并直接查看文件、执行过程和 Git 变更。

## 开发与安装

版本、安装包和各平台验证情况见[项目范围与状态](integration/README.md)。正式下载以维护者为Coaxi发布的安装包为准，上游安装包不代表本产品。下表说明一期融合范围，不代表所有功能已在每个环境完成验证。

## 一期融合范围

| 工作场景 | 范围 |
|---|---|
| 在一张画布上组织工作 | 独立画板、区域、公开/私聊卡片、便笺、文件、终端、分组与布局恢复 |
| 和 AI 队员持续协作 | 队员身份与工具箱、Thread、私聊材料隔离、独立帮手、审批、停止与恢复 |
| 推进较长任务 | Mission、任务与责任、定时工作、文字补充、交互卡片与协作记忆 |
| 操作真实工程 | 项目与 Git 工作区、文件树、编辑与预览、Diff、提交、历史、分支与冲突处理 |
| 连接仓库平台 | 仓库面板、Issue/PR 关联与操作；实际权限和远程动作由用户控制 |
| 选择智能体和模型 | 原生 CLI/ACP 接入、自定义程序、模型与推理参数、配置迁移、原生继续与分叉 |
| 使用工具与资料 | Skills、MCP、浏览器与电脑操作、Office/HTML 预览、科研与生成式卡片 |
| 在其他入口继续 | Desktop/Web、独立 Server、用户 CLI、渠道、移动客户端；各平台资格单独验证 |
| 管理自己的环境 | 加密备份、恢复、WebDAV、主题、快捷键、日志、诊断与桌面宠物 |

## 第一次使用

1. 安装并登录你准备使用的智能体程序，例如 Codex CLI 或 Claude Code。
2. 打开 Coaxi，选择队员和本机智能体。模型目录来自所选程序，同一电脑上不同安装版本可能返回不同模型。
3. 打开一个工程，先让队员解释项目，再查看执行记录、文件和 Diff。
4. 完成变更后自行决定是否提交、推送或创建 PR。

更多安装、模型列表和旧数据说明见[Coaxi 使用说明](docs/guides/coaxi-workbench.md)。

## 本地开发

需要 Node.js 24、pnpm、仓库锁定的 Rust 工具链，以及对应平台的原生编译环境。

```sh
git switch develop/V1.0.0
pnpm install --frozen-lockfile
pnpm dev
```

开发实例必须使用独立数据目录、Skill Library 和 MCP 配置，不能占用日常安装版的数据。执行前阅读[本地隔离流程](docs/development/local-workflow.md)。类型检查、测试和打包的实际入口由 [package.json](package.json) 维护。

## 源码入口

| 目录 | 内容 |
|---|---|
| `apps/desktop` | Electron 主进程、界面与本机桥接 |
| `apps/web`、`crates/rovai-web` | Web 客户端与受控服务入口 |
| `crates/rovai-core`、`crates/rovai-host` | 数据、模型输入、任务执行与 Host |
| `apps/mobile-android`、`apps/mobile-ios` | 移动客户端 |
| `packages` | 类型合同、共享界面和主题 |
| `docs` | 用户说明、架构、合同与开发文档 |
| `integration` | 融合范围、来源锁定、移植规则与必要历史证据 |

## 名称与旧数据兼容

界面、菜单、通知和当前产品介绍统一使用 **Coaxi**。已有安装身份、数据库文件、数据目录、IPC 和受控资源引用继续兼容，避免旧数据失联。源码里的供体来源、第三方商标、许可证和历史记录按事实保留；它们不代表当前产品仍使用旧名称。详见[品牌与兼容边界](docs/development/coaxi-branding.md)。

## 来源与许可

Coaxi 融合 [Rovai AI](https://github.com/murray17/rovai-ai) 与 [Codeg](https://github.com/spacering-net/codeg) 的代码和交互设计。具体供体地址、冻结提交与许可证以 [sources.lock.json](integration/sources.lock.json) 和 [来源许可证](integration/licenses) 为准。

上游版权、MIT 与适用的 Apache-2.0 条款保持有效；移植文件保留其原始通知。完整说明见 [LICENSE](LICENSE) 与 [第三方通知](THIRD_PARTY_NOTICES.md)。Coaxi 不代表上游官方发行。
