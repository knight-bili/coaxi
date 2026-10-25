# Coaxi v0.4.8

<!-- lang:zh-CN -->

修复右侧浏览器在打开菜单时画面消失、网页材料投递与共享异常，并修复使命变更树滚动。

### 更新内容

- 打开遮挡网页的菜单时保留已绘制的网页画面，关闭后恢复网页交互；不相交的菜单保持原生网页显示。
- 页面文字、元素、截图和标注以附件加入对话，保留原有输入；元素选中后直接加入，去掉重复确认。
- 共享菜单可选当前全部队员或指定队员，并切换查看、操作和停止共享；工具未开启时给出明确入口。
- 共享请求异常、跨站或页面关闭时收回权限，未知结果不会因关闭菜单而被清除。
- 恢复使命累计变更和差异弹窗的独立滚动，保留动态编号避免多个目录树冲突。

### 验证范围

本地 Windows 隔离客户端和自动化回归已检查。真实模型账号、第三方仓库/渠道写入及移动原生平台不作为本次已验证项目。

<!-- lang:en -->

Fix browser visibility while menus are open, page attachments, sharing recovery, and mission change-tree scrolling.

### Changes

- Preserve a painted browser frame under overlapping menus and restore native interaction when they close. Non-overlapping menus keep the native page visible.
- Add page text, selected elements, screenshots, and annotations as attachments while preserving the existing composer text. Element selection adds its materials directly.
- Choose all current teammates or specific teammates, then allow viewing, control, or stop sharing. Explain when browser tools are disabled.
- Revoke uncertain grants after errors, navigation, or page closure; reopening a menu does not clear an unknown outcome.
- Restore independent scrolling in mission change trees and diff dialogs without duplicate element IDs.

### Verification scope

Checked with an isolated Windows desktop and automated regression tests. Live model accounts, third-party repository/channel writes, and native mobile platforms are outside the verified scope of this update.
