# I01 基线构建与融合契约

I00经客户授权再次自审，修复范围校验和私有仓CI后完整门禁通过，PR50已合入main。
当前I01继续核实源应用、Host协议、数据迁移边界和资源；不能把源基线构建当成融合产品完成。

- [Core与宿主契约](contracts.md)、[固定源协议快照](source-protocol.json)。
- [真实Host协议夹具](protocol-cases.json)、[旧数据导入及回滚夹具](migration-fixtures.json)。
- [资源与兼容问题](resources.md)。实际执行结果见[当前实测记录](results.md)和[Windows真实Host证据](windows-host-result.json)。

两应用Windows前端类型检查和静态Web构建已通过。Windows本地缺MSVC链接器；Rovai原生Server CI及本机12项Host检查通过，Codeg原生Server CI构建和隔离认证健康检查通过。设备实测按客户回复暂停，等待后续ADB，Mac资源尚无。[资源台账](resource-status.json)保留负责人、使用阶段和基准日期。

- [关键源存储与映射设计](storage-map.json)：8项Codeg实体、7张隔离Core实际表的列元数据；不代表已部署迁移。

- [Codeg源Server实测证据](codeg-server-result.json)。
