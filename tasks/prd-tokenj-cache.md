# PRD: tokenJ — 自动缓存优化引擎

## 一句话定位

> **装了就省钱。零配置，最高省 90%。**

## Introduction

### 问题

开发者每天都在为 LLM API 付费，但绝大多数人不知道一件事：**提供商本来就给了巨额折扣，只是没人去用。**

- Anthropic 提供**90% 折扣**的缓存读取价格（$3 → $0.30/MTok）
- DeepSeek 自动给**90% 折扣**的缓存命中价格
- OpenAI 给**50-75% 折扣**的缓存读取价格
- Gemini 给**75% 折扣**的上下文缓存价格

但问题是：**每个提供商的缓存配置方式完全不同。**

| Provider | 缓存方式 | 用户需要做什么 |
|----------|---------|--------------|
| Anthropic | 手动标记 | 在 messages 中添加 `cache_control` 字段 |
| OpenAI | 自动 | 但需要 prompt 超过 1024 tokens 且结构正确 |
| DeepSeek | 自动 | 无需配置，但用户不知道有没有命中 |
| Gemini | 手动开启 | 需要额外 API 调用创建缓存 |
| GLM-5 | **不支持** | 什么都不能做 |

用户不是不想省，是**太麻烦了**。需要一个工具来统一处理这些事情。

### tokenJ 的解法

tokenJ 是一个本地 MITM 代理（中间人代理）。用户安装后，把 LLM SDK 的 base_url 指向 tokenJ，tokenJ 会自动：

1. **检测 Provider 和模型** — 识别请求发往哪个 API
2. **自动注入缓存策略** — 根据不同 Provider 的规则，自动添加缓存标记
3. **监控缓存命中率** — 实时显示省了多少钱
4. **容错处理** — 遇到不支持的 Provider（如 GLM-5），自动去掉不兼容字段

**用户什么都不用改。装好、设好代理、开始工作。tokenJ 在中间默默帮你省钱。**

### 为什么是现在

2025-2026 年，三大条件同时成熟：

1. **主流 Provider 都推出了缓存功能** — OpenAI、Anthropic、DeepSeek、Gemini 全部支持
2. **折扣力度前所未有** — 全部在 50-90% 之间
3. **但配置分散且不统一** — 每个 Provider 的配置方式完全不同

这个窗口期就是现在。再晚半年，可能各 SDK 就自动做了——所以现在动手刚刚好。

## Goals

- 提供一个 MITM 代理，拦截 LLM API 请求和响应
- 自动识别请求的目标 Provider 和模型
- 根据 Provider 规则，自动注入缓存优化策略（无损）
- 解析响应中的缓存命中数据，计算节省金额
- 通过终端 UI 展示实时节省效果
- 以单二进制分发（Rust），用户零依赖

## Non-Goals

- **不做客户端缓存** — 不缓存响应内容，只优化请求中的缓存标记
- **不做请求修改** — 不修改 prompt 内容、模型选择、任何业务数据
- **不做分析报告** — 不分析 Token 结构，只看缓存命中率和节省金额
- **不做 CLI 历史查询** — 仪表盘专注实时数据

## User Stories

### US-001: MITM 代理启动

**Description:** As a user, 我想通过一行命令启动 tokenJ 代理，这样我能快速开始。

**Acceptance Criteria:**
- [ ] 提供 `tokenJ proxy` 命令启动 MITM 代理
- [ ] 代理默认监听 `127.0.0.1:9100`
- [ ] 支持 `--port` 参数自定义端口
- [ ] 支持 `--daemon` 后台运行模式
- [ ] 首次启动时自动生成自签名 CA 证书，输出证书安装指引
- [ ] 支持 `--cert-dir` 指定证书存储路径
- [ ] 代理启动后输出：监听地址、CA 证书路径、状态

### US-002: Provider 自动识别与缓存注入

**Description:** As a user, 我想 tokenJ 自动适配不同 Provider 的缓存规则，不用我手动配置。

**Acceptance Criteria:**
- [ ] 拦截到请求后，自动识别 Provider（基于 URL/域名）
- [ ] 识别到 Anthropic 请求（api.anthropic.com）时：
  - [ ] 检查 system prompt 或 messages 是否超过 1024 tokens
  - [ ] 如果是，自动在最后一个 cacheable block 上注入 `cache_control: {type: "ephemeral"}`
  - [ ] 如果已存在用户手动设置的 cache_control，不覆盖
- [ ] 识别到 OpenAI 请求（api.openai.com）时：
  - [ ] 检查 prompt 总长度是否超过 1024 tokens
  - [ ] 如果超过，自动添加 `prompt_cache_key` 参数（基于内容 hash 前 8 位）
  - [ ] 不修改已存在的 `prompt_cache_key`
- [ ] 识别到 DeepSeek 请求（api.deepseek.com）时：
  - [ ] DeepSeek 缓存是自动的，无需注入
  - [ ] 但标记该请求"可缓存"，在仪表盘中展示预估节省
- [ ] 识别到 Gemini 请求时：
  - [ ] 检查是否超过缓存最低阈值（32K tokens）
  - [ ] 如果超过，自动启用上下文缓存
- [ ] 识别到 GLM-5 等不支持缓存的 Provider 时：
  - [ ] 自动剥除请求中可能存在的 `cache_control` 字段（避免 400 错误）
  - [ ] 在仪表盘中标记"该 Provider 不支持缓存"

### US-003: 响应解析与节省计算

**Description:** As a user, 我想 tokenJ 自动解析响应中的缓存数据，告诉我每次请求省了多少钱。

**Acceptance Criteria:**
- [ ] 对 Anthropic 响应：解析 `usage.cache_creation_input_tokens` 和 `usage.cache_read_input_tokens`
- [ ] 对 OpenAI 响应：解析 `usage.prompt_tokens_details.cached_tokens`
- [ ] 对 DeepSeek 响应：解析 `usage.prompt_tokens_details.cached_tokens`
- [ ] 根据模型名称和缓存活跃类型，自动查找对应的价格表
- [ ] 计算本次请求的节省金额（美分精度）
- [ ] 计算累计节省金额（会话级和全局）
- [ ] 所有数据异步写入 SQLite（不阻塞请求转发）

### US-004: 实时仪表盘（TUI）

**Description:** As a user, 我想在终端中实时看到 tokenJ 帮我省了多少钱。

**Acceptance Criteria:**
- [ ] 提供 `tokenJ dashboard` 命令启动 TUI
- [ ] 仪表盘包含 4 个面板：

```
┌─────────────────────────────────────────────────┐
│ tokenJ 实时节省面板                   累计: $12.45│
├──────────┬──────────┬──────────┬──────────────────┤
│ 实时请求   │ 今日统计   │ 命中排行   │ 模型分布          │
│           │          │          │                  │
│ GPT-4o    │ 请求: 87 │ 1. Claude│ ● Claude  45%   │
│ 省 $0.03  │ 节省: $12│  系统提示  │ ● GPT-4o  30%   │
│ Claude    │ 命中率:   │ 2. GPT-4o│ ● DeepSeek 25%  │
│ 省 $0.08  │ 68%      │  System  │                  │
│ DeepSeek  │ 活跃会话: │ 3. Claude│                  │
│ 省 $0.001 │ 3        │  工具定义 │                  │
├──────────┴──────────┴──────────┴──────────────────┤
│ 日志: [14:23:01] Claude 缓存命中 +$0.08          │
│      [14:22:58] GPT-4o 缓存写入 +$0.01 (首次)    │
└─────────────────────────────────────────────────┘
```

- [ ] 实时请求面板：最新的几条请求记录（模型、节省金额、命中/未命中标记）
- [ ] 今日统计面板：请求总数、节省总额、缓存命中率、活跃会话数
- [ ] 命中排行面板：按节省金额排序的"哪些内容被缓存最多"列表
- [ ] 模型分布面板：各模型的 Token 消耗占比（使用 Sparkline 或 BarChart）
- [ ] 底部日志面板：滚动的事件日志
- [ ] 默认每 1 秒刷新一次
- [ ] 支持快捷键 `q` 退出、`r` 重置统计

### US-005: 首个启动体验（Onboarding）

**Description:** As a new user, 我想在安装后 60 秒内看到 tokenJ 帮我省了第一笔钱。

**Acceptance Criteria:**
- [ ] 首次运行 `tokenJ proxy` 时输出：

```
$ tokenJ proxy

  ╔══════════════════════════════════════════════╗
  ║        tokenJ  — 自动缓存优化引擎             ║
  ║        装了就省，零配置                       ║
  ╚══════════════════════════════════════════════╝

  [1/3] 生成 CA 证书... ✅
  [2/3] 启动代理: 127.0.0.1:9100 ... ✅
  [3/3] 等待请求...

  你需要做的事情：
    1. 安装 CA 证书（只需一次）：
       Windows:  双击 ca.crt → 安装到"受信任的根证书颁发机构"
       macOS:    sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ca.crt
       Linux:    sudo cp ca.crt /usr/local/share/ca-certificates/ && sudo update-ca-certificates

    2. 设置环境变量：
       export HTTP_PROXY=http://127.0.0.1:9100
       export HTTPS_PROXY=http://127.0.0.1:9100

    3. 开始工作！任何 LLM 请求都会自动优化。

   仪表盘: tokenJ dashboard
```

- [ ] 提供 `tokenJ demo` 命令：启动演示模式
- [ ] 演示模式内置一组模拟的历史请求数据
- [ ] 演示模式展示 TUI 并标注"这是演示数据"
- [ ] 演示数据展示"安装前 vs 安装后"的对比效果

### US-006: 配置管理

**Description:** As a user, 我想自定义 tokenJ 的配置，比如端口、价格表、排除规则。

**Acceptance Criteria:**
- [ ] 支持 `~/.tokenj/config.json` 配置文件
- [ ] CLI 参数优先级高于配置文件
- [ ] 可配置项：`port`、`cert_dir`、`prices`（自定义价格表）、`exclude_hosts`（排除不做缓存的域名）
- [ ] 提供 `tokenJ config init` 生成默认配置
- [ ] 提供 `tokenJ config show` 显示当前配置
- [ ] 提供 `tokenJ config set <key> <value>` 修改配置项

### US-007: 兼容性保障

**Description:** As a user, 我不想因为用了 tokenJ 导致请求出错。

**Acceptance Criteria:**
- [ ] 对于所有请求，tokenJ 不修改请求 body 中除了缓存相关字段以外的任何内容
- [ ] 对于不支持缓存的 Provider，自动去掉 cache_control 等不兼容字段
- [ ] 如果注入缓存字段导致 Provider 报错，自动降级：不再对该 Provider 做任何修改
- [ ] 提供 `--safe-mode` 参数：只监控不注入，用户手动验证后再开启自动注入
- [ ] 所有修改操作都记录日志

## Functional Requirements

- **FR-1**: 代理基于 Rust 实现，使用 `hyper` 和 `tokio` 构建异步 HTTP 服务器
- **FR-2**: 代理支持 HTTP CONNECT 隧道 + MITM TLS 解密
- **FR-3**: 首次启动自动生成自签名 CA 根证书（RSA 2048 位）
- **FR-4**: 证书有效期 10 年，存储在 `~/.tokenj/certs/` 目录
- **FR-5**: Provider 识别基于请求 URL 的域名匹配（精确匹配 + 通配符）
- **FR-6**: 价格表内置，覆盖主流模型；支持通过配置文件自定义
- **FR-7**: 价格表按季度更新，提供 `tokenJ update-prices` 命令
- **FR-8**: 数据库使用 SQLite，存储在 `~/.tokenj/data.db`
- **FR-9**: 数据库保留策略：默认保留 7 天数据
- **FR-10**: 支持 `tokenJ dashboard --no-tui` 模式：在终端输出纯文本摘要（适合没有 TTY 的环境）
- **FR-11**: 支持 `tokenJ stats` 命令：打印纯文本统计摘要

## Architecture

```
用户应用                             LLM API
(Claude Code /                      (OpenAI / Anthropic /
 Cursor / 自定义脚本)                  DeepSeek / Gemini)
       │                                  ▲
       │  HTTPS CONNECT                    │
       │  + TLS handshake                  │
       ▼                                  │
┌───────────────────────────────────────────┐
│            tokenJ MITM Proxy               │
│                                            │
│  ① 拦截 CONNECT 请求 → TLS 解密            │
│  ② 解析 HTTP 请求 → 识别 Provider/模型     │
│  ③ 注入缓存策略:                           │
│     ├─ Anthropic → cache_control 注入      │
│     ├─ OpenAI → prompt_cache_key 注入      │
│     ├─ DeepSeek → 无需操作                │
│     ├─ Gemini → 上下文缓存启用             │
│     └─ GLM-5 → 剥除不兼容字段              │
│  ④ 转发请求到真实 Provider                 │
│  ⑤ 拦截响应 → 解析缓存数据 → 计算节省       │
│  ⑥ 写入 SQLite → 更新 TUI 仪表盘          │
│  ⑦ 响应原样返回给用户                      │
└───────────────────────────────────────────┘
                    │
                    ▼
          SQLite (统计 + 配置)
                    │
                    ▼
          TUI 仪表盘 (实时显示)
```

### 核心流程（以 Anthropic 为例）

```
用户请求:
  POST https://api.anthropic.com/v1/messages
  {
    "model": "claude-opus-4-7",
    "system": "You are... (5000 tokens 的 system prompt)",
    "messages": [
      {"role": "user", "content": "你好"}
    ]
  }

tokenJ 拦截后修改:
  POST https://api.anthropic.com/v1/messages
  {
    "model": "claude-opus-4-7",
    "system": [
      {
        "type": "text",
        "text": "You are... (5000 tokens)",
        "cache_control": {"type": "ephemeral"}  ← tokenJ 自动注入
      }
    ],
    "messages": [
      {"role": "user", "content": "你好"}
    ]
  }

Provider 响应:
  {
    "usage": {
      "input_tokens": 5020,
      "output_tokens": 150,
      "cache_creation_input_tokens": 5000,  ← 首次写缓存
      "cache_read_input_tokens": 0
    }
  }

第二次请求（同一 system prompt）:
  → tokenJ 注入 cache_control → Provider 命中缓存
  → 响应: "cache_read_input_tokens": 5000
  → 成本: 5000 tokens × $0.50/MTok = $0.0025（而不是 $0.025）
  → 节省: 90%
```

## Data Model

```sql
-- 核心表：请求记录
CREATE TABLE requests (
    id TEXT PRIMARY KEY,               -- UUID v4
    provider TEXT NOT NULL,             -- 'openai' | 'anthropic' | 'deepseek' | 'gemini' | 'other'
    model TEXT NOT NULL,                -- 模型名
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cached_tokens INTEGER DEFAULT 0,    -- 缓存命中的 token 数
    cache_write_tokens INTEGER DEFAULT 0, -- 缓存写入的 token 数
    estimated_cost REAL NOT NULL,       -- 实际成本（美分）
    estimated_saving REAL DEFAULT 0,    -- 节省金额（美分）
    saving_rate REAL DEFAULT 0,         -- 节省百分比
    cache_injected BOOLEAN DEFAULT 0,   -- tokenJ 是否注入了缓存标记
    duration_ms INTEGER,                -- 请求耗时
    created_at TEXT NOT NULL
);

-- 会话表
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    start_time TEXT NOT NULL,
    end_time TEXT,
    total_requests INTEGER DEFAULT 0,
    total_cost REAL DEFAULT 0,
    total_saving REAL DEFAULT 0,
    avg_saving_rate REAL DEFAULT 0
);
```

### TUI 布局详细设计

```
┌─────────────────────────────────────────────────────────┐
│  tokenJ                    累计节省: $12.45    缓存命中率: 68% │
├───────────────┬─────────────────┬────────────────────────┤
│ 📡 实时请求流    │ 📊 今日统计       │ 🏆 缓存命中排行        │
│               │                  │                        │
│ 14:23:01      │ 请求总数: 87     │ 1. Claude 系统提示     │
│ ✅ Claude     │ 今日节省: $12.45 │   命中 23 次, 省 $1.84 │
│   省 $0.08    │ 缓存命中率: 72%  │   总 Token: 115,000    │
│               │ 今日写入: $1.83  │                        │
│ 14:22:58      │ 活跃会话: 3     │ 2. GPT-4o System       │
│ ⏳ GPT-4o     │ 代理运行: 2h13m │   命中 15 次, 省 $0.52 │
│   缓存写入     │                  │                        │
│ 省 $0.00*     │ 💰 模型节省排行   │ 3. Claude 工具定义      │
│  *首次请求     │ 1. Claude  $8.20 │   命中 8 次, 省 $0.64  │
│               │ 2. GPT-4o $2.85  │                        │
│ 14:22:55      │ 3. DeepSeek $1.40│                        │
│ ✅ DeepSeek   │                  │                        │
│   省 $0.001   │                  │                        │
├───────────────┴─────────────────┴────────────────────────┤
│ 💡 提示: Claude 连续请求缓存命中率达 72%，表现良好。     │
│          检测到同一 system prompt 被重复发送 23 次。     │
└─────────────────────────────────────────────────────────┘
```

## Technical Considerations

### 技术栈

| 维度 | 选择 | 理由 |
|------|------|------|
| 语言 | Rust | 单二进制分发、性能关键路径、TLS/HTTP 生态成熟 |
| TLS | `rustls` | 纯 Rust TLS 实现，不需要 OpenSSL 依赖 |
| HTTP | `hyper` + `tokio` | Rust 最成熟的异步 HTTP 栈 |
| TUI | `ratatui` + `crossterm` | 生产就绪的 Rust TUI 框架 |
| SQLite | `rusqlite` | 本地存储，零配置 |
| 证书生成 | `rcgen` | Rust 原生 CA 证书生成 |
| JSON 解析 | `serde_json` | Rust 标准 JSON 库 |

### HTTPS MITM 详细流程

```
1. 启动时生成 CA 根证书（rcgen）
2. 用户安装 CA 证书到系统信任存储
3. 客户端配置 HTTP_PROXY/HTTPS_PROXY
4. 客户端发送 CONNECT 请求建立隧道
5. tokenJ 用 CA 证书动态签发对应域名证书
6. 完成 TLS 握手 → 解密请求内容
7. 处理 → 重新加密 → 转发到真实服务器
8. 反向同样处理响应
```

### 性能目标

| 指标 | 目标 | 说明 |
|------|------|------|
| 额外延迟 | <10ms | MITM TLS 解密 + 加密的开销 |
| 并发连接 | 1000+ | tokio 异步处理 |
| 内存占用 | <50MB | 不缓存响应 body，仅透传 |
| TUI 刷新 | 1 秒 | 不影响代理性能 |

### 安全约束

- 自签名 CA 证书只存储在本地，不传输
- 证书有效期为 10 年，不自动续期
- 仅解密 LLM API 域名的请求（白名单模式）
- 白名单外的域名直接透传，不做 MITM
- 数据库文件默认权限 600
- 不记录任何请求/响应的 body 内容（只记录 Token 统计数据）

### 白名单域名（默认）

```
api.openai.com
api.anthropic.com
api.deepseek.com
generativelanguage.googleapis.com  (Gemini)
open.bigmodel.cn                   (GLM-5, 透传但跳过缓存)
```

## 落地可行性审计

### 审计 1: Rust 技术栈就绪度

| 依赖 | 方案 | 状态 |
|------|------|------|
| TLS | `rustls` + `rcgen` | ✅ 生产就绪 |
| HTTP 代理 | `hyper` + `tokio` | ✅ 生产就绪 |
| TUI | `ratatui` | ✅ 生产就绪 |
| SQLite | `rusqlite` | ✅ 生产就绪 |
| JSON | `serde_json` | ✅ 生产就绪 |

### 审计 2: MITM 实现复杂度

MITM 代理的核心逻辑约 1500-2000 行 Rust，包括：
- CA 证书生成：100 行（rcgen）
- TLS 拦截逻辑：400 行（rustls + hyper）
- Provider 识别与缓存注入：400 行（按 Provider 分支处理）
- 响应解析与节省计算：200 行
- SQLite 写入：200 行
- TUI 仪表盘：600 行（ratatui 布局 + 4 个面板）

**风险点**：首次实现 MITM TLS 代理需要仔细处理证书链和 TLS 握手。`rustls` 的 API 比 OpenSSL 更安全但文档较少。需要阅读 `hyper` 的 proxy 示例。

### 审计 3: Anthropic cache_control 注入的最佳位置

Anthropic 的缓存规则：
- `cache_control` 可以放在 `system` 数组的 block 上，或 `messages` 数组的 content block 上
- 每个请求最多 4 个缓存断点
- 缓存的是**完整前缀**（system + tools + messages 按顺序）
- 最小缓存粒度：1024 tokens

**tokenJ 的策略**：
1. 优先在 `system` 上注入（最稳定的缓存目标）
2. 如果 system 长度不足 1024 tokens，检查 `messages` 前面的 user/assistant 对
3. 如果仍不足 1024，不做注入（Mark 为"不可缓存"）
4. 如果用户已手动设置了 cache_control，不覆盖

### 审计 4: OpenAI prompt_cache_key 策略

OpenAI 的缓存规则：
- 自动触发，不需要手动标记
- 最低 1024 tokens
- 缓存粒度：128 tokens 增量
- `prompt_cache_key` 参数可以提升路由一致性

**tokenJ 的策略**：
1. 检查 prompt 是否超过 1024 tokens
2. 如果超过，自动生成 `prompt_cache_key`（基于 system prompt 的 SHA256 前 8 位）
3. 这样同一份 system prompt 的请求会被路由到同一台服务器
4. 如果用户已设置 `prompt_cache_key`，不覆盖

### 审计 5: 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 自签名 CA 证书安装门槛 | 用户可能不会/不愿装 | 提供各平台的安装脚本；提供纯文本统计模式作为降级方案 |
| MITM 被安全软件拦截 | 代理无法工作 | 白名单域名；提供 `--safe-mode` 只监控不注入 |
| Provider API 变更导致缓存字段不兼容 | 请求失败 | 自动降级机制；先小规模验证 |
| 用户手动设置了缓存但被覆盖 | 用户不满 | 检测已有缓存字段时不覆盖 |
| 性能开销 | 请求变慢 | 目标 <10ms；非关键路径 async 写入 |

## Success Metrics

| 指标 | 目标 | 测量方式 |
|------|------|---------|
| 缓存命中率 | >50% 用户可观察 | 仪表盘显示 |
| 用户平均节省 | 节省金额 > 开通成本的 10 倍 | 仪表盘累计 |
| 代理延迟 | <10ms p95 | 内置性能统计 |
| 兼容性 | 不导致任何请求失败 | 自动化测试 |
| 安装成功率 | >90% 用户成功完成证书安装 | 遥测（可选） |

## Open Questions

1. **证书安装自动化**：能否在 Windows/macOS/Linux 上自动安装 CA 证书而不需要用户手动操作？（macOS 需要 sudo，Windows 需要管理员权限）
2. **Claude Code 集成**：Claude Code 是否支持 `HTTPS_PROXY` 环境变量？还是需要在 `settings.json` 中单独配置？
3. **价格表维护**：模型价格变化快，更新策略是内置 + 手动更新，还是从远程拉取最新价格表？
4. **遥测**：是否收集匿名的缓存命中率统计数据（不包含任何请求内容，仅元数据）用于改进产品？
5. **共存**：用户已经使用了 Token Optimizer MCP 等其他优化工具，tokenJ 是否会冲突？

## 竞品分析

| 产品 | 用途 | 和 tokenJ 的关系 |
|------|------|----------------|
| **OpenAI/Anthropic 原生缓存** | Provider 功能 | tokenJ 是"自动帮你启用这些功能的工具" |
| **Helicone / Langfuse** | LLM 可观测性平台 | tokenJ 不是可观测性工具，是省钱工具 |
| **Token Optimizer MCP** | 客户端缓存+压缩 | tokenJ 是服务端缓存优化，可互补 |
| **tokencost** | Token 成本监控 | tokenJ 不监控，直接省钱 |
| **Snip / RTK** | 请求内容裁剪 | tokenJ 不修改内容，只加缓存标记 |

**核心差异**：所有现有工具要么是"看花了多少"，要么是"改内容来省"。tokenJ 是唯一一个**利用 Provider 现有折扣来省钱的工具**。

## 发布策略

```
Day -14: 内部开发完成，自用验证
Day -7:  开源 GitHub，写 README + 安装文档
Day 0:   发布 v0.1.0，Hacker News / V2EX / 即刻
         标题示例:
         "装了就省钱：我用 Rust 写了个自动缓存代理，Claude API 费用打 1 折"

Day 7:   根据反馈快速迭代
Day 30:  v0.2.0 — 增加 Gemini 支持 + 改进 TUI
Day 60:  v0.3.0 — 增加更多 Provider + 配置界面完善
```

**关键营销点**：
- "不需要改代码" — 设个环境变量就行
- "不需要学配置" — 自动检测一切
- "不损伤质量" — 和压缩工具不同，这是无损的
- "省多少实时看得见" — TUI 仪表盘

## 用户使用全流程

```
STEP 0: 安装
  macOS/Linux:  curl -fsSL https://tokenj.dev/install.sh | sh
  Windows:      winget install tokenJ
  或:           cargo install tokenJ

STEP 1: 启动代理
  $ tokenJ proxy

STEP 2: 安装 CA 证书（按终端输出指引，一次性的）
  Windows:  双击 ca.crt → 安装到"受信任的根证书颁发机构"
  macOS:    sudo security add-trusted-cert -d ...

STEP 3: 设置环境变量
  export HTTPS_PROXY=http://127.0.0.1:9100

STEP 4: 正常使用 LLM（Claude Code / Cursor / OpenAI SDK ...）
  $ claude      ← tokenJ 自动拦截，注入缓存
  $ python app.py  ← 同上

STEP 5: 看效果
  $ tokenJ dashboard
  → 实时显示: 今日省了 $12.45，命中率 72%

从安装到看到第一笔节省: < 3 分钟
```
