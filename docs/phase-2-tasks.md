# 第二阶段开发任务：能力契约与发现

## 1. 阶段目标

第二阶段只交付同步、进程内的能力元数据、权威注册和只读发现：

```text
validated capability name/category
        ↓
immutable descriptor (description, input/output contracts)
        ↓
single-owner registry with atomic conflict handling
        ↓
owned, deterministic discovery snapshots
```

这里的“无状态”只描述未来的能力实现；注册表本身拥有元数据状态。本阶段不调用
宿主代码，不定义能力实现 trait，不引入 `Arc`、锁、Task、Resource、Observer、
Command 或 Adapter。注册表保存描述符的权威副本，而不是将来的可执行能力实例。

能力发现先作为 `CapabilityRegistry` 的只读行为交付。等 Resource 与 Task 也存在且
确有聚合需求后，再引入独立 `discovery` façade；本阶段不为它预置 trait 或空模块。

## 2. 进入阶段前的硬性门槛

### P2-000：收口第一阶段公开契约

**目标**：先消除会被第二阶段放大的基础 API 缺口，确保 Phase 2 建立在真实不变量
而非文档约定上。

**依赖**：P1-001 至 P1-010。

**范围**：

- 封闭 `TypeContract` 的组合表示；crate 外部不得通过公开 enum variant 直接构造
  含空字段名或重复字段的 object contract，所有可公开获得的 contract 都必须有效；
- `Error` 对外只保留观察接口；错误由拥有规则的模块构造，路径、详情和类别不能由
  调用方任意拼装；
- 删除当前没有任何语义来源的预留 `ErrorKind`，新增类别只随实际失败行为交付；
- 在加入 `CapabilityName` 前，将过于宽泛的根级 `MAX_LEN` 收口为领域明确的名称，
  避免两个标识类型共享含义不清的公共常量；
- 所有公开构造错误类型实现 `Display` 和 `std::error::Error`；
- 保留并验证当前未提交修订中 `CorrelationId::as_ref()` 与 root-path `Error::Display`
  的回归测试，同时移除对应 mutation exclusion。

**验收**：

- crate 外部不存在绕过 `TypeContract` 校验的构造路径；
- 公开调用方不能制造相互矛盾的 Axiom `Error`；
- `ErrorKind` 中每个类别都能由当前公开操作实际产生并有行为测试；
- 第一阶段文档、rustdoc、根导出和实现一致。

**测试与验证**：增加黑盒测试或 compile-fail doctest 保护构造边界；运行聚焦测试、
`cargo make fast` 和 `cargo make full`。

**非目标**：借此引入 Phase 2 类型、通用错误 builder 或未来错误码。

P2-000 未完成时，不开始 capability 模块实现。

## 3. 已冻结的阶段决策

以下决策在编码前固定，若产品需求要求改变，应先修改本文档及相应验收条件：

1. `CapabilityName` 是独立值对象，不复用 `CorrelationId`；允许 ASCII 字母、数字、
   `.`、`_`、`-`，非空，最大 128 字节，保留原始大小写且不 trim、不规范化；
2. 名称相等、哈希和排序均按保存后的字节序，因而名称是大小写敏感的；
3. category 使用独立的受验证标签值；规则与名称相同但最大 64 字节，不预置业务
   枚举；
4. description 必填，拒绝空字符串或纯空白字符串，但保留调用方提供的原文；本阶段
   不引入任意的最大长度；
5. descriptor 拥有 name、description、category、input contract 和 output contract，
   构造后不可变；
6. registry 取得 descriptor 所有权，同名注册返回冲突且保留原值，不提供替换、移除
   或热更新；
7. 单项查询以 `Option` 表示不存在；“未知能力”在只读发现中不是运行失败，因此本
   阶段不提前增加 `UnknownCapability` 错误类别；
8. 单项查询和列表都返回 owned snapshot；列表按 `CapabilityName` 升序，与注册顺序
   和内部容器无关；
9. registry 使用 `&mut self` 注册、`&self` 发现，不内置锁或共享所有权；需要重叠
   注册/发现的并发语义推迟到 Runtime 拥有关系明确之后；
10. snapshot 的隔离边界是一次方法调用：返回后不受后续 registry 修改影响。

## 4. 有序任务清单

### P2-001：建立 capability 模块边界与注册冲突类别

**目标**：建立只依赖 `foundation` 与 `contract` 的 capability 模块，并为同名注册
增加最小、稳定的失败类别。

**依赖**：P2-000。

**范围**：更新 crate 文档、根导出和 `architecture.toml`；增加 duplicate capability
类别及固定详情字段 `capability`。错误构造仍归 capability 注册规则所有。

**验收**：

- capability 不依赖 `execution` 或未来 invocation 语义；
- 根模块仍是便利导出的统一入口，模块路径不形成另一套不一致 API；
- duplicate capability 无需解析消息即可识别，详情携带冲突名称；
- 未创建空的 `discovery`、`runtime` 或 adapter 模块。

**测试与验证**：错误类别/详情单元测试，`cargo make architecture-check`、
`cargo make fast`。

**非目标**：未知能力错误、调用入口、实现 trait、同步原语。

### P2-002：定义受验证的 CapabilityName 与 Category

**目标**：提供不会和任意字符串、Correlation ID 或彼此混用的元数据值对象。

**依赖**：P2-001。

**范围**：按第 3 节冻结的字符、长度、大小写和规范化规则实现两个类型及各自的类型化
构造错误；可在模块内部复用校验函数，但不公开含义模糊的通用 label 类型。

**验收**：

- 合法、空、最大长度、超长、非法字符均有边界测试；
- 错误暴露结构化原因，并正确实现标准错误边界；
- 相等、排序、哈希、`AsRef<str>` 和 `Display` 与保存文本一致；
- 类型不暴露可变内部字符串或 unchecked 公共构造路径。

**测试与验证**：表驱动单元测试和 crate 外黑盒测试，`cargo make fast`。

**非目标**：命名空间语义、别名、版本号、自动生成或 Unicode 规范化。

### P2-003：定义不可变 CapabilityDescriptor

**目标**：用运行时可读取的数据描述一个能力的名称、说明、分类及 I/O 契约。

**依赖**：P2-002、P1-007。

**范围**：descriptor 拥有所有字段并通过一个受验证构造边界创建；输入与输出直接
复用 `TypeContract`。仅提供只读 accessor，以及快照所需的 `Clone`、`Debug`、
`Eq`/`PartialEq`；不为 descriptor 定义与发现顺序无关的 `Ord`。

**验收**：

- 空或纯空白 description 在构造边界被拒绝；
- 合法元数据可无损读取，调用方不能在构造后修改；
- input/output contract 保留原有字段声明顺序和严格验证语义；
- 不复制 `TypeContract::validate`，不暴露内部容器或未来实现对象。

**测试与验证**：合法/非法构造、字段读取、clone 隔离和契约复用测试，
`cargo make fast`。

**非目标**：默认参数、权限、标签集合、搜索、i18n、版本协商。

### P2-004：实现 CapabilityRegistry 的所有权与冲突原子性

**目标**：由单一 owner 管理 descriptor 注册，并保证失败不会改变权威状态。

**依赖**：P2-001、P2-003。

**范围**：registry 初始为空；`register(&mut self, descriptor)` 成功后取得所有权；
名称是唯一键。内部可选择 `BTreeMap` 或其他结构，但不得让容器类型进入公共契约。

**验收**：

- 首次注册成功；同名注册返回稳定冲突错误和名称详情；
- 冲突后原 descriptor 原样保留，registry 数量不变；
- 大小写不同的名称按既定名称语义分别注册；
- 无 partial write、silent replacement 或“最后写入获胜”。

**测试与验证**：成功、冲突、冲突原子性和大小写测试，`cargo make fast`。

**非目标**：移除、替换、批量事务、持久化、跨进程注册、锁或权限决策。

### P2-005：提供 owned、确定性的 Discovery 快照

**目标**：公开单项 descriptor 查询和完整的确定性列表，两者均为无副作用读取。

**依赖**：P2-004。

**范围**：未知名称返回 `None`；已知名称返回 owned descriptor snapshot；列表返回
按 capability name 升序排列的 owned snapshot。排序契约不得依赖哈希或注册顺序。

**验收**：

- 空列表、查询命中/未命中、多个名称排序和重复读取均有黑盒测试；
- 取得 snapshot 后继续注册，不会改变旧 snapshot；
- 调用方修改或丢弃自己的集合不影响 registry；
- 发现路径不触发宿主代码、日志回调或其他副作用。

**测试与验证**：单元测试与 crate 外 integration test，`cargo make fast`。

**非目标**：筛选、分页、订阅、事件、借用视图或 Command 形式的 discover 请求。

### P2-006：公共 API 组合测试与文档示例

**目标**：从 crate 外部证明元数据注册、发现和 Phase 1 契约验证可以组合使用。

**依赖**：P2-005。

**范围**：新增 integration test 和可编译 rustdoc 示例：构造并注册两个 descriptor、
读取有序快照、使用发现到的输入契约验证 `Value`、检查 duplicate 的结构化错误和
未知查询的 `None`。

**验收**：

- 测试只使用公开 API；
- descriptor 不暴露 registry 内部容器、生命周期或同步实现；
- 发现所得 input/output contract 与注册时语义一致；
- 不出现第二套 Value 或 contract 验证规则。

**测试与验证**：`cargo test --doc`、对应 integration test、`cargo make fast`。

**非目标**：实际调用能力、mock 宿主实现或断言内部调用次数。

### P2-007：阶段收口与质量验证

**目标**：冻结进入 Invocation 阶段前的 capability metadata 契约。

**依赖**：P2-006。

**范围**：同步 README、本文档和 `docs/development-plan.md`；审查根导出、rustdoc、
架构依赖、错误详情键和 mutation exclusions。

**验收**：

- 第 3 节全部决策已有测试保护；
- 实际 API、非目标和文档一致；
- 没有能力实现 trait、宿主回调、锁、Task、Resource 或 Adapter 泄漏进本阶段；
- mutation exclusion 只保留有书面等价性理由的项目。

**测试与验证**：运行 `cargo make full`。本阶段按设计不需要 `hardening`；若实现偏离
计划而引入 unsafe 或同步，则必须先说明必要性，再增加 `cargo make hardening`。

## 5. 依赖与建议交付批次

```text
P2-000
   ↓
P2-001 → P2-002 → P2-003 → P2-004 → P2-005 → P2-006 → P2-007
```

建议每项独立形成一个可评审增量；P2-001 与 P2-002 可在同一短分支交付，但不要把
registry、快照和公共示例压缩为一个大提交。

## 6. 推迟到后续阶段的决策

以下问题当前没有足够调用语义支撑，不在 Phase 2 预先回答：

1. 能力实现 trait、同步/异步调用形态和业务错误类型；
2. Runtime 是否以 `Arc`/锁共享 registry，以及并发注册是否存在；
3. 未知能力在 invocation/command 边界映射为何种 `ErrorKind`；
4. descriptor 替换、注销、版本迁移和热重载；
5. 跨 Capability、Resource、Task 的统一 Discovery façade；
6. 序列化字段名、协议版本、分页、筛选和权限。

## 7. 完成定义

第二阶段完成时，宿主可以可靠地登记并发现能力的静态描述和 I/O 契约；所有返回
结果都是确定、隔离、无副作用的 owned snapshot，但宿主仍不能调用能力。任何
“注册即执行”“发现即调用”、注册表内置宿主实现或为了未来并发提前暴露锁/共享
所有权的设计，都应视为范围越界。

公开表面由 `capability` 模块拥有，并通过 crate 根再导出：`CapabilityName`、
`CapabilityCategory`、`CapabilityDescriptor`、`CapabilityRegistry`，以及
`ErrorKind::DuplicateCapability`（详情键 `capability`）。本阶段没有独立的
`discovery`、`runtime` 或 adapter 模块。
