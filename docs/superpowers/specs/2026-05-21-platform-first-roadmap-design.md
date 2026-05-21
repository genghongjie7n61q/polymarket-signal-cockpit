# Polymarket Signal Cockpit 平台优先路线设计

日期：2026-05-21

## 共同认可的方向

本项目采用“完整平台优先”路线。先建设可审计、可回放、可配置、可观测的交易研究平台，再把策略模型作为插件接入。

策略模型不是平台的中心，它是平台上的插件和消费者。平台必须先稳定提供数据、实时状态、模型运行契约、回测、通知、驾驶舱和 dev-2 运维能力。

## 硬边界

- 不在本机 Mac 安装运行环境、数据库、虚拟环境或容器测试环境。
- 运行测试、容器验证、部署验证统一在 `dev-2`。
- `dev-2` 健康检查使用 `192.168.103.157`，不能只测 `127.0.0.1`。
- Linear 是架构、计划、里程碑、依赖、验收和交接记录的协调源。
- 多会话或多智能体并行开发必须使用独立 `git worktree`。
- 每个开发任务必须有单元测试和 dev-2 容器/运行验证。
- 临时测试容器验证后要清理，除非明确保留为运行环境。
- 当前平台只做研究、信号、回测、告警和纸面交易；不做真实下单、私钥管理或绕地域交易。

## Linear 标题语言约定

Linear 中面向项目管理的标题使用中文，包括：

- 里程碑标题。
- issue 标题。
- 项目路线文档标题。

产品名 `Polymarket Signal Cockpit` 可以保留英文作为项目名或品牌名。

## 中文里程碑

### M0 治理与交接

确认 `AGENT.md`、Linear 任务锁、worktree、多智能体规则、dev-2 验收、共同认可后再开发。

验收重点：

- Linear 中有平台优先路线和任务领取规则。
- `AGENT.md` 记录多会话、subagent、dev-2、验证、禁令和风险清单。
- 后续主会话可在明确任务内自动推进，只有触发 stop 条件才询问用户。

### M1 数据与存储底座

对应现有 `WEB-6`，标题建议改为：`M1 数据与存储底座`。

目标：

建立平台的数据与存储底座，让实时链路、模型插件、回测、通知和 Web Cockpit 都有统一、可审计、可回放的数据基础。

重点：

- PostgreSQL schema 和 migrations。
- raw events、normalized events、ticks、candles、Polymarket snapshots。
- models、model_versions、model_assignments。
- signals、paper_orders、notification_channels、notification_deliveries。
- runtime_events。
- bounded async storage writer。
- 幂等键和去重键。

验收：

- migrations 从空库应用成功且重复运行安全。
- storage writer 批量写 ticks/signals/notification deliveries，不阻塞调用方。
- DB outage 有 bounded backpressure 和 runtime_events。
- 单元测试和 dev-2 容器/PostgreSQL 验证通过。

### M2 实时采集与事件总线

对应现有 `WEB-7`，标题建议改为：`M2 实时采集与事件总线`。

目标：

建立稳定的实时市场数据链路，先保证平台能可靠地产生 BTC 5m 和 ETH 15m 当前状态，不以策略收益为目标。

重点：

- Coinbase BTC-USD / ETH-USD WebSocket collectors。
- Polymarket BTC 5m / ETH 15m market/window discovery 和 snapshot refresh。
- canonical event normalization。
- state owner task 独占 live state mutation。
- bounded fanout 到 storage、model runtime、websocket broadcaster。
- reconnect、heartbeat、stale-source detection。

验收：

- dev-2 上实时链路可持续运行。
- tick 到达后能先生成策略/模型输入，不等待 DB 写入。
- 源断开、过期、队列溢出可通过 health/status/runtime_events 观察。
- 测试覆盖 normalization、window rollover、candle aggregation、reconnect state、bounded queue overflow。

### M3 后端平台 API 与配置面

建议新增 Linear issue，标题：`M3 后端平台 API 与配置面`。

目标：

在完整 Web UI 之前先稳定后端平台 API，避免 Web Cockpit 直接耦合内部模块。

重点：

- REST API：markets、current state、recent candles、signals、runtime health。
- 配置 API：model assignments、notification channels、webhook config。
- WebSocket API：live ticks/candles/live_signal/actionable_alert。
- API DTO 与内部 domain types 分离。
- 权限暂以 dev-2 内网/配置保护为边界，不引入复杂账号系统。

验收：

- dev-2 上可通过 API 读取 BTC 5m / ETH 15m 当前状态和最近历史。
- 修改模型分配和 webhook 配置可持久化，并被后端行为读取。
- WebSocket 客户端断线重连可恢复最新状态。
- API 测试覆盖 DTO 序列化、配置写入、状态读取、WebSocket 基础推送。

### M4 模型插件运行时契约与基线模型

对应现有 `WEB-8`，标题建议改为：`M4 模型插件运行时契约与基线模型`。

目标：

定义平台可运行模型插件的稳定契约。策略优化不是本阶段目标，基线模型只用于验证平台链路。

重点：

- 小而稳定的 ModelContext / MarketTick / ModelDecision / BacktestResult 接口。
- 模型注册、版本、参数和 feature snapshot。
- 内置 BTC 5m / ETH 15m 基线模型。
- 通用 sizing engine 可以包含 fractional Kelly、上限、最小值、冷却等规则，但必须作为平台能力。
- realtime 模型执行禁止网络 I/O，输入相同输出必须确定。

验收：

- 每个市场可选择模型 assignment。
- 模型输出包含 replay/backtest 所需的完整 input snapshot、features、model version。
- 单元测试覆盖 NO_TRADE、CANDIDATE、TTL、limit price、suggested size、重复执行确定性。
- 接口不依赖 Feishu、HTTP handler 或数据库表。

### M5 回放与回测平台

对应现有 `WEB-9`，标题建议改为：`M5 回放与回测平台`。

目标：

建立判断模型是否可进入生产告警的可信数据基础。

重点：

- 从持久化 ticks、candles、Polymarket snapshots、model assignments、market windows 回放。
- walk-forward backtesting。
- first actionable alert freeze 语义。
- win rate、Wilson lower bound、coverage、expected value、drawdown、consecutive losses、price paid、calibration buckets。
- backtest_runs 和 per-signal replay outputs 持久化。

验收：

- 没有近期合格回测的模型不能标记为 production-eligible。
- 回测使用历史首个 frozen actionable alert，而不是事后更优 live_signal。
- 测试覆盖 replay determinism、first-alert freeze、Wilson lower bound、drawdown、BTC/ETH 独立参数。
- Web Cockpit 可查询回测结果。

### M6 通知平台

对应现有 `WEB-10`，标题建议改为：`M6 飞书通知平台`。

目标：

让可执行建议能被快速看见，同时通知绝不阻塞实时关键路径。

重点：

- 每个 market 支持多个 Feishu webhook。
- 通知队列 worker、retry、timeout、exponential backoff、dedupe。
- 卡片第一屏显示方向、limit price、suggested size、market/window、TTL、confidence。
- 次要分析弱化展示。
- notification_deliveries 审计。
- dry-run 测试接口或 CLI。

验收：

- 飞书消息不是 title-only，关键内容第一时间可见。
- 通知失败不阻塞模型评估或 storage writer。
- 同一个 frozen actionable signal 不重复刷屏。
- 测试覆盖卡片 payload、多 webhook、retry、dedupe、delivery persistence。

### M7 Web Cockpit

对应现有 `WEB-11`，标题建议改为：`M7 Web Cockpit 驾驶舱`。

目标：

构建操作型驾驶舱，用于监控 BTC 5m / ETH 15m、查看 K 线和信号、配置模型与 webhook。

重点：

- React + Vite 或项目选择的前端栈。
- BTC 5m / ETH 15m cards。
- countdown、Polymarket Up/Down prices、spread、active model、latest live signal、latest actionable alert、notification status。
- lightweight-charts recent candles with markers。
- model assignment 和 webhook config controls。
- alert history、backtest summary。

验收：

- 打开 Web 即可看到两个市场状态，不依赖日志。
- 改模型 assignment 或 webhook config 能持久化并影响后端行为。
- WebSocket 更新无需刷新，断线重连可恢复。
- 桌面和移动端验证：图表渲染、无文本重叠、信号细节可见。

### M8 dev-2 运维硬化

对应现有 `WEB-12`，标题建议改为：`M8 dev-2 部署与可观测性硬化`。

目标：

让平台在 dev-2 上可部署、可诊断、可重启恢复。

重点：

- production compose/podman-compose。
- backend、web、PostgreSQL/TimescaleDB、volumes、logs、env。
- 部署 runbook：first boot、migration、restart、rollback、log inspection。
- metrics/logs：tick latency、strategy latency、WS reconnects、queue depth、dropped events、DB write lag、notification latency、alert counts。
- readiness/liveness。
- 禁用通知或禁用市场的 emergency switches。

验收：

- fresh dev-2 deployment 可按文档启动。
- runtime health 明确展示 collectors、DB writer、model runtime、notifier、WebSocket broadcaster 状态。
- “没有新信号”可以通过日志/指标定位。
- backend 重启不丢历史，并可从 DB/外部源恢复当前状态。

### M9 社区插件生态

对应现有 `WEB-13`，标题建议改为：`M9 WASM 社区插件沙箱`。

目标：

在核心平台稳定后，支持第三方模型插件，但不能牺牲实时稳定性和安全边界。

重点：

- WASM plugin loading。
- plugin package metadata。
- schema version、permissions、resource limits。
- CPU/memory/determinism/network/file access sandbox。
- plugin validation command。
- author documentation。

验收：

- sample WASM model 可加载、验证、replay。
- 超 CPU/memory 或试图访问禁用能力的插件被安全拒绝。
- schema version mismatch 有明确错误。
- 文档包含最小接口和可运行示例。

## 建议的 Linear 调整

### 需要更新的现有 issue 标题

- `WEB-5`：`M0 Rust 后端基线与容器基础`
- `WEB-6`：`M1 数据与存储底座`
- `WEB-7`：`M2 实时采集与事件总线`
- `WEB-8`：`M4 模型插件运行时契约与基线模型`
- `WEB-9`：`M5 回放与回测平台`
- `WEB-10`：`M6 飞书通知平台`
- `WEB-11`：`M7 Web Cockpit 驾驶舱`
- `WEB-12`：`M8 dev-2 部署与可观测性硬化`
- `WEB-13`：`M9 WASM 社区插件沙箱`

### 建议新增 issue

标题：`M3 后端平台 API 与配置面`

依赖：`WEB-6` 和 `WEB-7`。

位置：在实时采集与事件总线之后、模型插件运行时和 Web Cockpit 之前。

原因：

- Web Cockpit 不应直接依赖内部 Rust 模块。
- 模型 assignment、webhook config、runtime health、recent history 都需要稳定 API。
- 将 API 层单独成 issue 可以降低 `WEB-11` 的范围。

## 需要更新的 Linear 文档

现有文档 `Execution Order and Dependency Map` 建议改名为：`平台优先路线与依赖地图`。

文档内容应改为：

- 平台优先，不以策略优化为第一阶段目标。
- DoD 使用 dev-2 验证，不写 local container。
- 明确主控可自动推进，但共同认可前不开始开发。
- 明确 Linear 标题用中文。
- 明确每个 issue 都要记录测试、dev-2 验证、临时容器清理、风险。

## 当前写入状态

2026-05-21 本会话尝试写入 Linear milestone 和 issue 更新时，Linear 写操作被自动审批超时拦截。读取 Linear 正常。

因此本设计文档作为待回填 Linear 的审定草案。Linear 写入恢复后，按本文内容回填。
