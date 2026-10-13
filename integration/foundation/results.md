# I01 当前实测记录

截至2026-10-13，I01进行中，不声明完整融合或整阶段验收通过。

| 项目 | 结果 | 范围限制 |
|---|---|---|
| 两应用本地前端类型检查 | 通过 | 不等于运行和产品验收 |
| Codeg静态Web构建 | 通过 | 固定供体版本，未接Rovai Core |
| Rovai Web、Execution Web、Electron构建 | 通过 | Electron尚未运行UI |
| Rovai Windows原生Server CI | 全部成功 | 构建main提交9d1ad55；development-preview，不是正式发布 |
| 原生包ZIP与manifest | 通过 | SHA256一致，213个文件逐项校验 |
| 本机真实Host HTTP/SSE/SQLite | 12项通过 | 新隔离数据根，无真实AI执行；明细见windows-host-result.json |
| Codeg Windows原生Server | 重新构建中 | 初次CI工具版本路径冲突已修；不改供体packageManager |
| 本地Rust原生编译 | 未通过 | MSVC link.exe缺失，用户全局环境未修改 |
| 旧数据导入/回滚 | 6项设计夹具 | 尚未执行迁移，不以空库检查替代历史升级 |
| Mac/移动设备 | 设备实测暂挂 | 用户稍后提供ADB；没有使用旧设备 |
| 融合产品验收 | 尚未开始 | 81来源簇对应324场景仍not_run |

## 复验

在同版本的校验Windows包上，设置绝对ROVAI_HOST_BIN、ROVAI_WEB_UI以及专用FOUNDATION_EVIDENCE_ROOT，再运行tools/integration/smoke-foundation.mjs。脚本自行创建私有Windows数据根、技能库与MCP路径，测试后关闭本次Host，不需要模型凭据。

结果证据不保存管理员Token、业务Session或用户秘密；不上传SQLite及运行目录。源版本、Host协议4、Core合同v1.72和schema146分别核对。

## 自审结论

I00复核发现的固定ID校验和提交差异校验缺口已修，私有仓凭据获取方式已整改，完整CI通过后合并PR50。I01的源基线构建与协议检查有实际证据，仍存在原生供体CI、真实桌面运行、跨平台环境与历史数据夹具待办；不得因此降低一期终点。

资源逐项状态、负责人和最迟使用阶段见resource-status.json；基准日期是排期约束，客户未承诺设备到位日期，等待将顺延有关测试。
