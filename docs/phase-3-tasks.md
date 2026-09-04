# 第三阶段开发任务：Invocation 与 Runtime 调度

## 1. 阶段目标

第三阶段在已完成的能力元数据之上，交付同步、进程内、可重入的动态调用路径：

```text
executable capability registration
        ↓
resolve descriptor + implementation
        ↓
validate input exactly once
        ↓
release runtime synchronization
        ↓
invoke host behavior with ExecutionContext
        ↓
validate output and preserve structured failure
```

本阶段只回答一次 Invocation 的语义，不把 Invocation 等同于 Future 或持续存在的
Task。Runtime 负责解析、校验和调度，但不重新实现能力业务逻辑，也不引入 Resource、
Observer、Command、Adapter、持久化或热更新。

## 2. 当前基线与进入门槛

截至 2026-09-04，Phase 1 与 Phase 2 已实现：

- `foundation` 已提供 `Value`、`Path`、结构化 `Error` 与 `CorrelationId`；
- `contract` 已提供封闭的 `TypeContract` 和严格、带路径的输入验证；
- `execution` 已提供不可变 `ExecutionContext` 及父子关联；
- `capability` 已提供不可变 descriptor、冲突原子的 registry 和 owned discovery；
- 当前分支 `feat/phase-2-capability-discovery` 工作区干净；
- `cargo make full` 已通过：74 个测试通过，行覆盖率 96.0%，区域覆盖率 96.7%，
  架构检查无违规。

### P3-000：交付并冻结 Phase 2 基线

**目标**：确保 Phase 3 从已评审、可回退的 Phase 2 基线开始。

**依赖**：P2-007。

**范围**：确认当前 Phase 2 分支及收口文档进入目标开发基线；复核公开导出、错误类别、
架构边界和完整质量门结果。

**验收**：Phase 2 变更已合入 Phase 3 的起始分支；工作区无非预期修改；
`cargo make full` 通过。

**非目标**：借收口修改 Invocation API。

## 3. 编码前必须冻结的决策

P3-001 必须以契约测试或文档明确以下问题，不能让实现偶然决定公开语义：

1. 本阶段调用是同步的；不返回 Future，不创建 Task；
2. descriptor 与实现必须作为一个完整注册项提交，失败不得留下只有元数据或只有实现的
   半注册状态；发现结果继续来自同一份 descriptor 权威状态；
3. Runtime 的调用入口接收 `CapabilityName`、owned `Value` 和 `ExecutionContext`，返回
   owned `Value` 或结构化 Axiom 错误；
4. 宿主实现通过一个窄 trait 接收输入与只读执行上下文；trait 不暴露 registry、锁、
   executor 或传输类型；
5. 若 Runtime 支持跨线程并发调用，则实现边界明确要求 `Send + Sync`，且 Runtime 只在
   解析并捕获实现期间持有内部同步；
6. 输入由 Runtime 按 descriptor 校验一次；能力实现不得被要求重复验证同一动态契约；
7. 成功返回值必须按 output contract 校验；实现返回不合约值属于实现/内部契约违例，
   不能伪装成调用方输入错误或业务失败；
8. 未知能力、调用方输入无效、能力业务失败、输出契约违例必须可按 `ErrorKind` 区分；
9. 业务失败由能力实现显式返回，保留人类可读说明和机器可读详情，不允许调用方直接构造
   任意 Axiom `Error`；
10. Runtime 不捕获或改写 Rust panic，除非先形成明确的 panic 隔离需求与边界；本阶段不把
    `catch_unwind` 当成通用内部错误机制；
11. 传入的 `ExecutionContext` 原样对宿主实现可见；创建 child context 仍是显式操作，
    Runtime 不暗中生成 correlation id；
12. 注册后替换、移除和热重载继续推迟；发现本身不得执行宿主代码。

## 4. 有序任务清单

### P3-001：冻结 Invocation、失败与并发契约

**目标**：先确定最小公共边界和失败分类，再编写 Runtime 实现。

**依赖**：P3-000。

**范围**：用设计说明和最小 compile-level 测试固定 capability implementation trait、
业务失败值、调用签名、所有权与线程安全边界；更新 `architecture.toml` 的目标依赖方向。

**验收**：第 3 节每项决策都有明确结论；公开 API 不泄漏同步容器或第三方类型；
`capability` 不反向依赖 `runtime`。

**测试与验证**：API 构造/trait 使用测试，`cargo make architecture-check`、
`cargo make fast`。

**非目标**：完成实际调度、Task、async trait、取消或超时。

### P3-002：定义可执行能力边界与结构化业务失败

**目标**：允许宿主把强类型业务适配为统一的 `Value` 调用，同时保留可操作失败信息。

**依赖**：P3-001。

**范围**：在 `capability` 中加入最小实现 trait 与业务失败类型；实现只接收 owned 输入和
只读 `ExecutionContext`，只返回 owned 结果；业务失败提供受控构造与只读观察接口。

**验收**：成功值、业务失败 message/details、标准错误边界和 context 可见性有测试；
能力实现不能构造相互矛盾的 Axiom `Error`；不引入泛型框架或 async。

**测试与验证**：单元测试与 crate 外 compile/use 测试，`cargo make fast`。

**非目标**：Runtime 查找、参数验证、panic 隔离、重试策略。

### P3-003：建立 Runtime 所有权与原子注册

**目标**：让 Runtime 持有可执行注册项，并保证 descriptor 与实现不可分离。

**依赖**：P3-002、P2-004。

**范围**：增加 `runtime` 模块及最小构造/注册边界；复用 `CapabilityName`、descriptor 和
既有 duplicate 语义；发现仍返回 owned、确定性 descriptor snapshot。

**验收**：注册成功后可发现且可解析实现；重复注册不替换原项、不留下 partial write；
大小写和排序继续遵守 Phase 2；内部实现句柄、容器和同步策略不进入公开 API。

**测试与验证**：成功、冲突原子性、发现一致性测试，`cargo make architecture-check`、
`cargo make fast`。

**非目标**：移除、替换、批量事务、动态加载或跨进程注册。

### P3-004：实现统一同步 Invocation 路径

**目标**：交付从名称解析到宿主能力执行的唯一动态调用入口。

**依赖**：P3-003。

**范围**：Runtime 解析注册项、按 input contract 严格验证 owned `Value`、调用实现并返回
owned 结果；未知名称新增稳定错误类别及固定结构化详情键。

**验收**：已知能力成功调用；未知能力与 discovery 的 `None` 语义明确区分；缺失字段、
多余字段和类型错误保留原始 `ErrorKind` 与精确 `Path`；输入非法时宿主代码不执行；
每次 Invocation 只经过一次 Runtime 输入校验。

**测试与验证**：表驱动单元测试、crate 外黑盒测试，`cargo make fast`。

**非目标**：Command 请求解析、隐式转换、默认参数、重试、超时。

### P3-005：保真映射业务失败并验证输出契约

**目标**：完整区分调用方错误、能力业务失败和实现违反输出契约。

**依赖**：P3-004。

**范围**：把能力显式返回的失败映射为稳定 Axiom 错误；成功值通过 descriptor 的 output
contract；为输出违例增加独立错误类别和能力名称等必要详情。

**验收**：业务失败的说明与详情不被改写为通用字符串；输出违例可与输入类型错误、业务
失败区分；失败不产生伪成功值；每个新增 `ErrorKind` 都有公开操作能够产生并有测试。

**测试与验证**：成功、业务失败、根/嵌套输出违例黑盒测试，`cargo make fast`。

**非目标**：捕获 panic、日志、指标、重试或错误序列化格式。

### P3-006：证明锁外执行、重入与并发查询语义

**目标**：用行为测试证明 Runtime 基础设施不会在宿主代码周围形成隐藏串行化或死锁。

**依赖**：P3-005。

**范围**：实现并测试 `resolve → capture → release → execute`；同一 Runtime 支持并发发现和
并发调用；能力回调能够重入 Runtime 调用另一个能力或执行发现。

**验收**：重入测试在确定性超时边界内完成；一个阻塞能力不会阻止只读发现或独立能力
开始执行；并发测试不依赖 sleep 猜测顺序；宿主代码执行期间不持有 Runtime 核心锁。

**测试与验证**：基于 barrier/channel 的确定性并发测试，`cargo make fast`，并运行
`cargo make hardening`，因为本任务稳定共享所有权与同步边界。

**非目标**：公平调度、线程池、优先级、取消或跨进程并发。

### P3-007：验证 Execution Correlation 传播

**目标**：保证调用关联信息从 Runtime 边界无损到达能力实现及其显式子操作。

**依赖**：P3-004、P3-006。

**范围**：覆盖 root/child context 调用、重入调用和失败路径；文档明确 Runtime 不充当
全局依赖容器，也不隐式生成或替换 correlation id。

**验收**：实现观察到的 correlation/parent 与调用方传入值一致；父 context 不被修改；
业务成功、业务失败与输出违例均不丢失调用时的 context 传播语义。

**测试与验证**：crate 外集成测试，`cargo make fast`。

**非目标**：日志、trace exporter、审计存储或 Task correlation。

### P3-008：公共 API 示例与阶段收口

**目标**：冻结进入 Resource 阶段前的可执行能力契约。

**依赖**：P3-007。

**范围**：增加只使用 crate 公共 API 的注册、发现、成功调用和结构化失败示例；同步
README、crate rustdoc、整体开发规划、架构规则和 mutation exclusions。

**验收**：宿主可从公开 API 完成 descriptor + implementation 注册并同步调用；第 3 节
决策均有契约测试；没有 Task、Resource、Observer、Command 或 Adapter 泄漏；文档与实际
所有权、并发和错误行为一致。

**测试与验证**：`cargo test --doc`、`cargo make full`、`cargo make hardening`。

**非目标**：开始 Phase 4 实现或为了未来阶段增加扩展点。

## 5. 依赖与建议交付批次

```text
P3-000
   ↓
P3-001 → P3-002 → P3-003 → P3-004 → P3-005 → P3-006 → P3-007 → P3-008
```

建议每个任务形成一个可独立评审的增量。P3-001 是首要任务：如果 trait、失败模型、
注册所有权或并发承诺尚未冻结，不应先写 Runtime 调度代码。P3-006 是最高风险验证点，
应在公共 API 收口前完成，而不是留到阶段末补并发测试。

## 6. 推迟到后续阶段的决策

1. Resource 身份、解析和生命周期；
2. Invocation 创建 Tracked Task 的返回模型；
3. async 能力、executor、取消、超时、重试和优先级；
4. descriptor/implementation 的替换、注销和热重载；
5. panic 隔离、进程隔离或插件崩溃恢复；
6. 跨 Capability、Resource、Task 的统一 Discovery façade；
7. Command、JSON、serde、CLI、MCP、RPC 或其他 Adapter；
8. 权限、认证、配额、持久化和分布式调度。

## 7. 完成定义

第三阶段完成时，宿主能够原子注册能力描述与实现，通过唯一同步动态入口严格校验输入、
执行宿主逻辑、校验输出，并按机器可读类别区分未知能力、输入错误、业务失败和实现契约
违例。调用必须传播 `ExecutionContext`，支持并发发现/调用与回调重入，且 Runtime 不在
执行宿主代码时持有核心同步。本阶段仍不产生持续存在的 Task，也不定义任何传输协议。
