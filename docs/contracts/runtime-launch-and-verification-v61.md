---
document_type: contract
name: Runtime Launch and Verification
version: v61
status: accepted
source_version: v1.73
last_updated: 2026-10-23
---

# Runtime Launch and Verification v61

继承 [v60](runtime-launch-and-verification-v60.md) 的原生恢复、进程启动环境、Warm、权限、健康、Adapter 机器证据及输入幂等保护；本版仅增加已获确认的[可选会话模型工具 v1](conversation-model-tools-v1.md)。

## 可选服务的启动身份

新 Run 仅按已确认策略、真实成员、当前作用域和实际 Adapter 能力冻结 conversationTools。
未选择服务时保留旧字段省略、原目录和 MCP 投影2；选择时使用最多四项的独立 stdio 服务及 MCP 投影3。
命名、目录和权限身份来自固定冻结值；实际认证连接私有保存，不把令牌、原生调用 locator 或主机路径交给模型。
自有 MCP 服务一起准备，不能先发布一个再覆盖另一个；已有同名外部服务明确拒绝。
Pi/Cursor 等没有该 stdio 接线能力的运行方式不能虚报可用；其他程序也必须满足其真实协议和平台准入。

## 复用、恢复与停止

v60 的同类型历史恢复仍是通用路径。选择此可选服务的会话另须证明原工具目录、原调用身份和租约可恢复。
不能证明时保留原历史并使用新原生会话，不把旧工具回答写到新的调用，不按今天的开关扩大原 Run 权限。
当前可选stdio桥绑定确切Run/epoch，各Adapter尚无把旧MCP进程的每次调用证明归属到新Run的通道，因此选用四工具的Run使用新原生Session；未选用服务的Run沿v60原生恢复。新Session仍由原输入投递与幂等机制控制，不能借此重发已接受输入。
这个拒绝只适用于本服务的目录和原回答通道；不把一般配置差异重新设为所有旧 Session 的恢复门槛。
已 accepted、delivery_unknown 或结果未知的业务输入不得因新 Session 再发；原问题的真实 stdout flush 与回执分开记账。
停止或撤销只结束原可选租约与未发生动作，不能撤回已提交 Mission/自动化，不能代替 User 回答，也不能影响独立授权的原 Built-in 服务。
重启不猜测原流是否收到答案。确定写回、领取后未知、未领取取消与迟到原回执保存各自事实。

## 版本和当前资格

其余上下文、Bootstrap、Formatter、Charter、Built-in、Envelope 与 Output 版本轴保持原合同。
编号61承接现存合同60；确认 r1 的具体设计及字段不变，批准文档保持原始字节。版本号递进不等于已取得任何真实 Runtime 资格。
本期依用户执行顺序先编写所有源码，之后统一 Review 和修复，再开始测试点整理及后续工程验证。
