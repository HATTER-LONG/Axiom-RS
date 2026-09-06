# Axiom 整体设计

## 1. 项目定位

Axiom 的目标是提供一套**语言无关、协议无关、领域无关的应用能力运行时模型**。

它不是一个业务框架，也不是某种 RPC、插件、AI 或脚本系统。

Axiom 解决的核心问题是：

> 如何让一个复杂应用内部原本分散、只对本地代码可见的能力，变成一套可以被程序、脚本、自动化系统和 AI Agent 稳定发现、调用、观察和控制的运行时能力。

首个产品验证应聚焦一个真实宿主：同一批业务能力经过一次接入，能够同时被本地程序
和一个外部入口发现、调用和诊断。协议无关要求核心不依赖 Adapter，并不要求把首个
Adapter 推迟到全部 Runtime 子系统完成之后。具体交付顺序见
[整体开发规划](development-plan.md)；资源、持续任务和观察系统按宿主需求逐步加入。
当前规划将这些完整目标分别落实为
P4 资源、P5 持续任务、P6 观察，再以 P7 Agent 入口、P8 Rust 接入与发布、P9 跨实现
规范验证收口；这表示后续交付范围，不表示这些子系统已经实现。

典型应用内部往往已经存在大量功能：

```text
打开工程
保存工程
生成网格
查询几何
执行计算
导入文件
运行后台任务
访问当前模型
```

传统实现中，这些能力通常散落在：

```text
UI callback
service object
manager
controller
singleton
application state
```

外部系统很难知道：

```text
系统能做什么？
如何调用？
参数是什么？
返回什么？
有哪些对象？
当前有哪些任务？
调用失败在哪里？
某个任务是谁创建的？
某条日志属于哪次请求？
```

Axiom 的设计目标就是为这些问题建立统一语义。

---

## 2. 核心设计思想

Axiom 不试图重新定义应用业务，而是在业务系统之上建立一个稳定的 **Capability Runtime Boundary**。

整体思想可以概括成：

```text
Application Business Logic
            |
            v
      Axiom Capability Model
            |
            v
     Dynamic Runtime Boundary
            |
      +-----+-----+-----+
      |           |     |
   Python       Agent  RPC
```

应用仍然使用自己最自然的语言和强类型模型实现业务。

Axiom 负责把这些业务能力转换成：

```text
可描述
可发现
可调用
可观察
可控制
可诊断
```

的运行时能力。

---

# 3. 强类型内部，动态边界

Axiom 不要求应用内部使用动态类型。

相反，业务内部应该继续使用最适合自身领域的强类型模型。

例如：

```text
Mesh
Project
Document
Geometry
SimulationOptions
```

Axiom 只在需要跨越运行时边界时建立统一动态表达。

```text
Strongly Typed Application
            |
            v
      Dynamic Boundary
            |
            v
External / Generic Caller
```

这个动态边界需要能够表达：

- 输入参数；
- 返回结果；
- 描述信息；
- 查询结果；
- 错误；
- Runtime 状态。

这样 Python、CLI、Agent、RPC 等调用者不需要理解具体语言的类型系统。

---

# 4. 单一语义来源

Axiom 最重要的设计原则之一是：

> 一个业务事实只能有一个权威来源。

例如：

```text
能力如何执行
    -> 业务运行时决定

参数是否合法
    -> 能力本身的契约决定

对象是否存在
    -> 对象管理系统决定

任务当前状态
    -> Task 状态源决定
```

更高层只能：

```text
发现
协调
转换
路由
```

不能重新实现一份业务规则。

这避免形成：

```text
C++ 一套语义
Python 一套语义
Agent 一套语义
RPC 又一套语义
```

最终产生行为漂移。

---

# 5. 能力必须天然可发现

Axiom 不接受“只有知道 API 的程序员才能使用系统能力”。

系统中的可调用能力需要拥有机器可读取的描述。

调用者应该能够在运行时知道：

```text
有哪些能力
能力叫什么
能力做什么
需要什么输入
产生什么输出
属于什么类别
是否存在相关元信息
```

因此 Discovery 是系统语义的一部分，而不是额外生成的文档，也不是必须单独存在的聚合入口。

目标是：

```text
Runtime Definition
        |
        +--> Native API
        +--> CLI Help
        +--> Python API
        +--> Agent Tool Definition
        +--> Documentation
```

这些能力都来自同一份运行时描述。

### 5.1 能力元数据、注册与只读发现

在能够调用能力之前，运行时必须先拥有一份权威、机器可读的能力描述。该描述至少包含：

```text
name
description
category
input contract
output contract
```

约束如下。

- **名称**是独立标识，不能与任意字符串或执行关联标识混用。允许字符、长度和大小写规则确定；保存原文，不 trim、不大小写折叠。名称比较按保存后的字节序，因此大小写敏感。
- **类别**是受验证的标签，不是预置业务枚举。规则与名称同类，但长度上限可以更短。
- **说明**必填，拒绝空串或纯空白，但保留调用方提供的原文。
- **输入/输出契约**复用同一套类型契约与严格验证语义，不另建第二套校验规则。
- **描述符**在构造后不可变。注册表取得描述符所有权，保存权威副本，而不是将来的可执行实现。

注册是控制面，发现是只读面：

```text
register descriptor
        ↓
authoritative metadata store
        ↓
owned, side-effect-free snapshots
```

同名再次注册必须失败且不改变原值：没有静默替换、没有最后写入获胜。单项查询在只读发现中用“不存在”表示未知能力，这不是调用失败；未知能力如何映射为错误类别，属于后续 invocation/command 边界。列表必须确定排序，且与注册顺序、内部容器无关。一次查询返回的快照在返回后不受后续注册影响。

“无状态”只描述未来的能力实现可以不持有业务会话；注册表本身拥有元数据状态。发现不得调用宿主代码，也不得把注册变成执行。

跨 Capability、Resource 与 Task 的统一 Discovery 入口，要等对应权威状态都存在且确有聚合需求后再建立。在此之前，能力发现由拥有注册表的能力层直接提供。

---

# 6. 调用与异步工作必须分离

Axiom 区分：

```text
一次能力调用
```

和：

```text
一个持续存在的工作
```

二者不是同一个概念。

一次调用表达：

> 请求系统执行一个能力，并返回结果。

而长时间工作需要额外具备：

```text
身份
状态
进度
取消
结果
错误
观察
```

因此长任务不能通过把所有调用都变成异步 Future 来解决。

Axiom 要求：

```text
Invocation
```

和：

```text
Tracked Work
```

保持独立。

这样既可以支持简单同步能力，也可以支持真正的长运行操作。

---

# 7. 对象身份与数据必须区分

Axiom 需要明确区分：

```text
Data
```

与：

```text
Application Object
```

普通数据可以复制、传输和序列化。

但应用对象通常具有：

```text
identity
ownership
lifetime
mutable state
```

例如一个打开的 Document 或一个 Mesh 对象，不能简单等价于一段动态数据。

因此 Axiom 需要支持：

```text
Value-oriented data
```

以及：

```text
Identity-oriented resources
```

两种不同语义。

这一点对于大型 native application 尤其重要。

---

# 8. 外部调用必须经过统一动态入口

不同外部系统不应该分别直接接触内部 Runtime。

理想结构是：

```text
Python
CLI
Agent
MCP
RPC
Other Language
        |
        v
Unified Dynamic Command Boundary
        |
        v
Application Capabilities
```

这个统一边界负责：

```text
识别请求
验证请求结构
定位目标能力
路由请求
返回统一结果
```

但不负责重新实现业务逻辑。

这样可以保证：

```text
Python 调用
Agent 调用
CLI 调用
Remote 调用
```

最终执行的是同一份应用能力。

---

# 9. 动态协议必须严格

Axiom 面向 Agent 和自动化系统时，一个重要要求是：

> Dynamic 不意味着宽松。

动态调用必须严格区分：

```text
未知命令
缺少参数
多余参数
参数类型错误
目标不存在
业务失败
内部失败
```

这些类别只在对应失败真正出现时进入公开错误模型。只读发现中的“未知能力”先表示为不存在，不提前占用调用失败类别。

不能依靠模糊 fallback 或静默类型转换。类型契约一旦公开，就必须保证可观察实例有效：调用方不能绕过校验拼出矛盾的对象形状，也不能任意拼装相互矛盾的错误。
这一不变量覆盖直接构造、对已有实例的可变访问以及嵌套组合；仅阻止直接构造不足以
建立封闭边界。发现可以读取契约，但不能暴露可破坏不变量的内部表示。

契约应提供调用方理解实际参数所需的说明。枚举、范围或单位等按真实用例加入；已声明
约束的描述与验证共享同一来源，宿主继续拥有依赖业务状态的规则。强类型参数与动态值
的转换应通过接入测试约束，不能让每个 Adapter 各自维护一份规则。

例如 Agent 生成错误请求时，系统应该返回明确诊断：

```text
哪里错了
为什么错
期望什么
```

而不是：

```text
operation failed
```

严格的动态协议本身就是 AI-first 的重要组成部分。

---

# 10. 错误必须机器可理解

Axiom 的错误不仅面向人。

它还需要被：

```text
程序
测试
Agent
RPC client
automation
```

读取。

因此错误必须是结构化信息，而不是仅有字符串。

至少要能够表达：

```text
错误类别
错误说明
错误发生的位置
必要的结构化细节
```

尤其对于嵌套输入，应能够定位到具体路径。

例如：

```text
options.mesh.faces[3].size
```

这种诊断对于 Agent 自动修复请求非常重要。

---

# 11. 运行时状态必须可观察

一个 AI-first Runtime 不能只回答：

```text
我能调用什么？
```

还需要回答：

```text
当前系统里有什么？
现在有什么工作正在执行？
它执行到哪里了？
为什么失败？
```

因此 Axiom 不只建模“能力”，也需要提供运行时状态的只读发现能力。

Discovery 必须：

```text
read-only
side-effect free
snapshot isolated from later mutation
```

调用查询不能改变系统业务状态。能力元数据的发现先返回描述符快照；资源与 Task 的发现在各自权威状态出现后再加入，不要求一开始就有统一 façade。

---

# 12. 观察与控制必须分离

Axiom 中：

```text
查询
```

与：

```text
修改
```

应具有清晰边界。

例如：

```text
查询任务
```

不能隐式触发工作。

```text
读取结果
```

不能重新执行任务。

```text
发现对象
```

不能改变对象生命周期。

这个原则可以让 Agent 或诊断工具安全地大量查询 Runtime，而不用担心产生副作用。

---

# 13. Execution Correlation 是基础能力

复杂应用中一次用户或 Agent 请求往往产生：

```text
Request
    |
    v
Capability Invocation
    |
    +--> Sub-operation
    |
    +--> Background Task
    |
    +--> Logs
```

如果这些数据彼此没有关联，后续诊断会非常困难。

因此 Axiom 要求所有运行时操作都能够携带并传播执行关联信息。

目标是能够回答：

```text
这个 Task 是谁启动的？
这条 Log 属于哪次请求？
这个错误对应哪个 Action？
这个 Agent request 后续创建了哪些工作？
```

Correlation 是：

```text
diagnostics
observability
audit
agent reasoning
```

的基础。

但 correlation metadata 不能变成全局依赖容器。

---

# 14. Observability 不能影响业务语义

Logging、Events、Tracing 等观察系统必须与业务结果解耦。

原则是：

> 诊断失败不能改变业务成功或失败。

例如：

```text
日志输出失败
observer 抛错
trace exporter 不可用
```

不应该让一个本来成功的业务能力失败。

Observability 是辅助基础设施，而不是业务控制流的一部分。

---

# 15. 状态是权威，事件只是通知

对于任何可观察运行时对象：

```text
真实状态
```

必须有权威存储。

事件只是告诉观察者：

```text
状态发生变化了
```

而不能依赖事件流本身还原权威状态。

因此：

```text
State is authoritative.
Events announce changes.
```

这允许：

- late subscriber；
- reconnect；
- snapshot；
- polling；
- event-based observation；

同时存在。

---

# 16. 并发必须属于公开语义

Axiom 面向 runtime infrastructure，因此 concurrency 不能只是实现细节。

每一种能力都应该明确：

```text
是否支持并发调用
是否支持并发查询
是否允许重复调用重叠
销毁时需要什么同步
callback 是否可以重入
```

能力是否能跨线程执行，必须结合宿主线程亲和要求确定；同步调用不意味着任意线程可
调用，也不意味着必须支持并发注册。首个实现选择满足实际用例的执行模型，不能强迫
主线程对象通过不安全的线程声明进入通用调用接口。

一个尤其重要的原则是：

> Runtime 自己的同步不能长时间包围用户业务代码。

基础设施应该：

```text
resolve
capture required state
release infrastructure synchronization
execute business logic
```

而不是在执行任意用户代码时持有核心全局锁。

---

# 17. Ownership 必须明确

Axiom 不应该演变成一个：

```text
Global God Object
```

去自动拥有整个应用。

能力、对象、Task、Logging、执行器等可以拥有不同生命周期。

Axiom 的职责是：

```text
建立明确关系
定义访问契约
阻止悬空语义
```

而不是隐藏真实 ownership。

这一原则对于：

```text
C++
Rust
plugins
Python
remote runtime
```

都非常重要。

---

# 18. Adapter 不属于核心语义

Python、CLI、MCP、HTTP、RPC、WebSocket 等都只是 Adapter。

其作用是：

```text
External Representation
          |
          v
Axiom Semantic Boundary
```

Adapter 可以改变：

```text
编码格式
协议格式
语言体验
调用方式
```

但不能改变：

```text
能力含义
任务含义
错误含义
对象身份
调用结果
```

---

# 19. AI-first 的含义

Axiom 的 AI-first 不意味着：

```text
在 Core 内调用 LLM
```

也不意味着为某一家模型设计 API。

真正的 AI-first 是：

> 一个 AI Agent 可以像普通程序一样，通过稳定机器接口理解并操作应用。

Agent 应该能够回答：

```text
我能做什么？
需要什么参数？
现在有哪些对象？
当前有哪些工作？
如何执行某个能力？
失败原因是什么？
之前发生了什么？
```

而不需要：

```text
分析源码
猜测 UI
模拟鼠标
理解应用内部类结构
```

---

# 20. Axiom 不应该承担的职责

Axiom 应避免过早吸收高级领域。

例如：

```text
LLM orchestration
Prompt management
Authentication protocol
Network transport
Workflow engine
Distributed consensus
Database persistence
UI framework
Domain business model
```

这些能力未来可以建立在 Axiom 之上。

但基础 Runtime 应保持简单。

原则是：

> 先提供稳定 primitive，再构建高级系统。

---

# 21. 语言无关设计

Axiom 的设计本身不属于 C++，也不属于 Rust。

正确关系应该是：

```text
                 Axiom Design
                  /         \
                 /           \
          C++ Runtime      Rust Runtime
```

两种实现可以完全不同。

例如：

```text
ownership model
concurrency primitives
async runtime
generic mechanism
error representation
internal storage
API style
```

都可以使用各自语言最自然的设计。

不应该要求 Rust 复制 C++：

```text
class
builder
template
PImpl
shared ownership
registry implementation
```

---

# 22. 多实现一致性的核心

不同语言实现真正需要保持一致的是**可观察语义**。

例如：

```text
能力如何被描述
名称与类别如何校验
重复注册如何失败
调用输入如何理解
调用失败如何表达
只读发现返回 snapshot 还是“不存在”
对象身份意味着什么
Task 生命周期意味着什么
Command 如何验证请求
Correlation 如何传播
```

实现内部则完全自由。

因此未来可以拥有：

```text
Axiom Specification

Axiom C++
    implementation optimized for C++ applications

Axiom-rs
    implementation idiomatic for Rust
```

而不是：

```text
Axiom C++
    ↓
source translation
    ↓
Axiom-rs
```

---

# 23. 最终运行时模型

Axiom 最终希望把一个应用从：

```text
A collection of internal services
```

转化成：

```text
A discoverable capability runtime
```

也就是：

```text
                    Application
                         |
                         v
                 Business Capabilities
                         |
                         v
                  Axiom Runtime Model
                  /       |        \
                 /        |         \
            Discovery   Control   Observation
                 \        |         /
                  \       |        /
                   Machine Boundary
                         |
          +--------------+--------------+
          |              |              |
        Human          Program         Agent
```

---

# 24. 核心设计原则总结

Axiom 应始终保持：

```text
Strong types internally,
dynamic boundary externally.

Single source of truth.

Capabilities are discoverable as metadata
before they are invocable.

Invocation and long-running work are separate.

Data and object identity are separate.

Observation is side-effect free.

Commands coordinate, business systems decide.

Errors are structured and actionable.

Execution is correlatable.

Observability never owns business semantics.

Ownership and concurrency are explicit.

Adapters translate, not redefine.

AI uses the runtime;
AI does not define the runtime.

Language implementations follow semantics,
not each other's internal architecture.
```

---

# 25. 一句话定义

> Axiom 是一套语言无关的应用 Capability Runtime 设计，用统一、严格、可观察的运行时语义，把应用内部能力转化为机器可以发现、调用、控制和理解的能力系统，并为自动化和 AI Agent 提供稳定基础。
