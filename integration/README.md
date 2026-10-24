# Coaxi 融合项目

Coaxi一期目标是完整融合两份固定供体的现有能力，包括原I00–I12计划内的整改、兼容与修复。第二期才规划自研新方向；不能因排期方便把一期能力删掉。

## 当前开发方式

唯一开发分支为`develop/V1.0.0`。小功能或单个问题独立中文commit，阶段集中验证后push。main冻结，必须在工程验证、客户实际使用验收后由客户明确指示合入。具体见[分支约定](branch-policy.md)和[当前执行路径](execution/phase-one-execution.md)。

## 状态边界

I04–I12源码已集中归并；一次全范围静态审查确认的25个Issue已通过PR #141归并开发分支。当前Windows本机试用包为0.4.8，目标版本为V1.0.0；正式Coaxi稳定安装包尚未发布。当前进入Windows本机使用反馈与整改，不能把源码齐备或该轮局部检查等同全部324场景、全部平台和客户验收通过。

2026-10-24当前工作：Coaxi品牌遗漏、内置队员名称/形象、仓库入口整理及独立CodexCLI模型目录兼容。用户已明确取消启动最大化改动。

## 范围、来源与规则

- [一期执行路径](execution/phase-one-execution.md)
- [固定源码和UI来源](inventory/canvas-ui-reference.md)
- [全部页面原应用UI规则](ui-source-policy.md)
- [单分支与main边界](branch-policy.md)
- [批量验证与Actions成本](ci-cost-policy.md)
- [许可证与来源通知](licenses/)
- [Coaxi用户说明](../docs/guides/coaxi-workbench.md)
- [完整技术文档导航](../docs/README.md)

此前逐步执行日志保留在[旧版项目入口的固定Git提交](https://github.com/knight-bili/coaxi/blob/1e3b455efa8bce4725112aaf7bf6b8036ef99b0e/integration/README.md)。它是历史记录，不作为当前进度页。
