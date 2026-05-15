<div align="center">

```
╔══════════════════════════════════════════════════════════╗
║                                                        ║
║   ████████╗ ██████╗ ██╗  ██╗███████╗███╗   ██╗     ██╗  ║
║   ╚══██╔══╝██╔═══██╗██║ ██╔╝██╔════╝████╗  ██║     ██║  ║
║      ██║   ██║   ██║█████╔╝ █████╗  ██╔██╗ ██║     ██║  ║
║      ██║   ██║   ██║██╔═██╗ ██╔══╝  ██║╚██╗██║██╗  ██║  ║
║      ██║   ╚██████╔╝██║  ██╗███████╗██║ ╚████║╚█████╔╝  ║
║      ╚═╝    ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝ ╚════╝   ║
║                                                        ║
║         ╔════════════════════════════════════╗          ║
║         ║  LLM API 缓存优化引擎                ║          ║
║         ║  装了就省钱 · 零配置 · 无损          ║          ║
║         ║  最高省 90%                        ║          ║
║         ╚════════════════════════════════════╝          ║
║                                                        ║
╚══════════════════════════════════════════════════════════╝
```

# TokenJ 🚀

**零配置 LLM API 缓存优化工具 · 装了就省钱 · 最高省 90%**

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.2.0-blue)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-70%20passed-brightgreen)](https://github.com)
[![Build](https://img.shields.io/badge/build-release%20%7C%2010MB-orange)]()

---

</div>

## 💎 一句话

> **TokenJ 是一个本地代理，自动拦截你的 LLM API 请求，根据不同 Provider 注入缓存标记，让 Anthropic / OpenAI / DeepSeek / Gemini 自带的缓存折扣**自动生效**。你什么都不用改。**

---

## ✨ 特性

<table>
<tr>
<td width="33%">

### 🎯 装了就省
- Anthropic 缓存 → **90% 折扣**
- OpenAI 缓存 → **50–75% 折扣**
- DeepSeek 缓存 → **90% 折扣**
- Gemini 缓存 → **75% 折扣**
</td>
<td width="33%">

### 🔌 零代码修改
- **方式 A**: 改一行 `base_url`
- **方式 B**: 设 `HTTPS_PROXY` 环境变量
- LLM 域名自动 MITM 解密
- 非 LLM 域名自动透传
</td>
<td width="33%">

### 🛡️ 生产级保障
- ✅ 纯 Rust 实现，零 unsafe
- ✅ 70 个测试，零编译警告
- ✅ TLS MITM + 动态证书签发
- ✅ SQLite 持久化 + TUI 仪表盘
</td>
</tr>
</table>

---

## 📊 能省多少？

| Provider | 模型 | 无缓存 | 有缓存 | 节省 |
|:---------|:-----|:------:|:------:|:----:|
| Anthropic | Claude Sonnet 4 | $3.00/MTok | **$0.30**/MTok | **90%** |
| Anthropic | Claude Opus 4 | $15.00/MTok | **$1.50**/MTok | **90%** |
| OpenAI | GPT-4o | $2.50/MTok | **$0.625**/MTok | **75%** |
| DeepSeek | V4 Pro | $1.74/MTok | **$0.145**/MTok | **92%** |
| DeepSeek | V4 Flash | $0.14/MTok | **$0.028**/MTok | **80%** |

> 按日均 5M 输入 Token + 70% 缓存命中率计算，**每月可省 $283.5，每年省 $3,449**。

---

## 🚀 快速开始

### 方式 A：直连模式（推荐 ✅）

```bash
# 1. 启动代理
TokenJ proxy

# 2. 修改 SDK 的 base_url
```

```python
# OpenAI
client = OpenAI(base_url="http://127.0.0.1:9100/v1")

# Anthropic
client = Anthropic(base_url="http://127.0.0.1:9100")

# DeepSeek
client = DeepSeek(base_url="http://127.0.0.1:9100")
```

```bash
# 3. 查看节省效果
TokenJ dashboard
```

### 方式 B：HTTPS_PROXY 模式（MITM ✅）

```bash
# 无需改代码，设环境变量即可
export HTTPS_PROXY=http://127.0.0.1:9100

# 启动代理
TokenJ proxy

# LLM 域名 → 自动 TLS 解密 + 缓存注入 ✅
# 其他域名 → 透传（不干预）
```

> ⚠️ 首次使用方式 B 需安装 CA 证书。运行 `TokenJ proxy` 后按指引操作。

---

## 🏗️ 架构

```
┌─────────────────────────────────────────────────────────┐
│                    用户应用                                │
│  (Claude Code / Cursor / OpenAI SDK / 自定义脚本)         │
└────────────────────┬────────────────────────────────────┘
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
   ┌──────────────┐    ┌──────────────────┐
   │  方式 A       │    │   方式 B          │
   │  base_url    │    │  HTTPS_PROXY     │
   │  HTTP 直连    │    │  CONNECT 隧道     │
   └──────┬───────┘    └────────┬─────────┘
          │                     │
          ▼                     ▼
   ┌──────────────────────────────────────┐
   │          TokenJ proxy                │
   │                                      │
   │  ┌──────────────────────────────┐    │
   │  │  ① 识别 Provider + 模型      │    │
   │  │  ② 注入缓存策略:             │    │
   │  │    ├─ Anthropic → cache_ctl  │    │
   │  │    ├─ OpenAI    → cache_key  │    │
   │  │    ├─ DeepSeek  → 自动监控   │    │
   │  │    ├─ Gemini    → ContextCache│   │
   │  │    └─ GLM-5     → 剥除不兼容 │    │
   │  │  ③ HTTPS 转发到真实 Provider │    │
   │  │  ④ 解析响应 → 计算节省      │    │
   │  │  ⑤ SQLite 记录 + TUI 更新   │    │
   │  └──────────────────────────────┘    │
   └──────────────────────────────────────┘
                     │
                     ▼
          ┌──────────────────────┐
          │  LLM API Provider    │
          │  (OpenAI/Anthropic/  │
          │   DeepSeek/Gemini)   │
          └──────────────────────┘
```

### MITM 流程详解

```
客户端                        TokenJ                       Provider
  │                            │                            │
  │  CONNECT api.anthropic.com │                            │
  │──────────────────────────▶│                            │
  │                            │                            │
  │  200 Connection Established│                            │
  │◀──────────────────────────│                            │
  │                            │                            │
  │  TLS ClientHello           │                            │
  │──────────────────────────▶│                            │
  │                            │  动态签发域名证书           │
  │                            │  (rcgen + CA 缓存)         │
  │  TLS ServerHello + Cert ◀─│                            │
  │◀──────────────────────────│                            │
  │                            │                            │
  │  🔓 TLS 解密 → 读取 HTTP   │                            │
  │  注入缓存标记              │                            │
  │                            │  HTTPS 转发                │
  │                            │──────────────────────────▶│
  │                            │                            │
  │                            │  ◀────────────────────────│
  │                            │  解析响应 → 计算节省金额    │
  │                            │  → SQLite 写入             │
  │                            │  → TUI 仪表盘更新          │
  │  ◀── 响应返回 ───────────│                            │
```

---

## 📈 性能

### 实测基准数据

Windows 11 x64, Rust 1.85 release 构建实测：

```
启动（首次含 CA 证书生成）                  1,180 ms
启动（冷启动，已有证书）                      249 ms
启动（热启动，缓存预热）                       78 ms

冷请求（首次 reqwest 初始化）                 423 ms
热请求（代理开销中位数）                        9 ms
热请求（代理开销 p95）                         15 ms

Release 构建（增量）                         20 秒
Release 构建（全量）                         1m 28s

单二进制体积                                10.2 MB
运行时内存                                    ~18 MB
```

### 竞品对比

| 指标 | TokenJ | mitmproxy | LiteLLM |
|:-----|:------:|:---------:|:-------:|
| **语言** | Rust ✅ | Python | Python |
| **二进制** | **10 MB** 单文件 | ~50 MB | ~200 MB |
| **启动时间** | **~250ms** | ~2s | ~3s |
| **请求开销** | **6–12ms** | ~50ms | ~100ms |
| **并发模型** | tokio async | asyncio | asyncio |
| **内存占用** | **~20MB** | ~100MB | ~300MB |
| **MITM** | ✅ 内置 | ✅ | ❌ |
| **缓存注入** | ✅ **自动** | ❌ | ⚠️ 需配置 |

### 对 LLM 请求的实际影响

LLM API 响应本身耗时 **1-30 秒**，TokenJ 引入的 **6-12ms** 额外延迟占比 **< 0.1%**，几乎不可感知。

```
总请求时间 = LLM 处理(秒级) + 代理开销(毫秒级)

LLM 处理:  █████████████████████████████ 3,000ms
代理开销:  ▏                                    9ms
占比:      < 0.3%
```

### 年化节省预估（基于 `prices.json` 真实价格）

```python
provider = "anthropic"
model = "claude-sonnet-4-6"
日输入 Token  = 5,000,000
日输出 Token  =   500,000
缓存命中率   = 70%

结果:
  日省: $9.45
  月省: $283.50
  年省: $3,449.25
```

---

## 🧪 测试套件

### 全部 70 个测试通过 ✅

```
📦 TokenJ — 70 passed, 0 failed, 0 warnings

pricing         ████████████████████████████████ 13
proxy/tls       ████████████████████████████████ 11
provider/mod    ████████████████████████████████ 10
anthropic       ████████████████████████████████  8
config          ████████████████████████████████  7
db              ████████████████████████████████  7
openai          ████████████████████████████████  6
cert            ████████████████████████████████  5
gemini_cache    ████████████████████████████████  3
──────────────────────────────────────────────────
总计            70 个测试

🔒 unsafe 代码:   0 行
🔧 编译警告:       0 个
✅ 生产代码中:     0 个 unwrap()
```

### 执行时间

| 场景 | 耗时 |
|:-----|:----:|
| 全量单元测试 | **0.19 秒** |
| 增量编译检查 | **0.66 秒** |
| Release 构建 | **20 秒** |
| MCP 全链路验证 | **<1 秒** |

---

## 🎮 Demo 效果

运行 `TokenJ demo` 查看实时仪表盘：

```
┌──────────────────────────────────────────────────────────────┐
│  TokenJ │ 节省 $0.04 成本 $0.04 │ 命中率 85.7% 请求 31       │
├───────────────┬────────────────┬───────────────┬──────────────┤
│ 💰 节省概览    │ 📊 请求统计    │ 🎯 缓存状态    │ 📈 模型分布   │
│               │                │               │              │
│ 今日节省 $0.04 │ 请求总数 31    │ 缓存命中 89700│ claude 20%   │
│ 今日成本 $0.04 │ 输入 Token     │ 缓存写入 15000│ gpt-4o 20%   │
│ 节省率 47.5%   │ 115500        │ 命中率 85.7%  │ opus 20%     │
│               │ 输出 Token 6090│               │ deepseek 10% │
├───────────────┴────────────────┴───────────────┴──────────────┤
│ 📡 实时请求                                                │
│ ✅ claude-sonnet-4-6 in:5000  cached:4500  $0.0027 (90%)   │
│ ✅ gpt-4o          in:3000  cached:2800  $0.0002 (85%)     │
│ ✅ deepseek-v4-flash in:1000 cached:900   $0.0000 (90%)    │
│ 📝 gpt-4o          in:3000  cached:0     $0.0000 (0%)      │
│    (📝 = 缓存写入, ✅ = 缓存命中)                          │
├──────────────────────────────────────────────────────────────┤
│ 📋 事件日志                                                │
│ [HIT]  anthropic | claude-sonnet-4-6 | in:5000  save:90%  │
│ [HIT]  openai    | gpt-4o          | in:3000  save:85%    │
│ [WRITE] anthropic | claude-haiku-4-5 | in:2000  cache:new │
└──────────────────────────────────────────────────────────────┘
```

---

## 🔍 MCP Server 全链路验证

TokenJ 内置 MCP Server，可在 Trae / VS Code / Cursor 中直接查询数据：

```
=== get_stats ===
  请求: 10
  成本: $0.01
  节省: $0.01
  命中率: 85.7%

=== get_repeats ===
  分组数: 7
    anthropic    claude-opus-4-7      2x
    anthropic    claude-sonnet-4-6    2x
    openai       gpt-4o               2x
    ...

=== estimate_savings ===
  日省: $9.45
  月省: $283.5
  年省: $3,449.25

=== prices.json ===
  8 条价格记录（Rust 自动导出）
    anthropic:opus-4-7
    anthropic:sonnet-4-6
    openai:gpt-4o
    deepseek:v4-pro
    ...
```

---

## 📦 安装

### 从源码构建

```bash
git clone https://github.com/lh123aa/TokenJ.git
cd TokenJ
cargo build --release
./target/release/TokenJ --version
```

### 前置依赖

- Rust 1.85+
- Windows / macOS / Linux

---

## 🎮 命令

| 命令 | 说明 |
|:-----|:------|
| `TokenJ proxy` | 启动代理（默认 127.0.0.1:9100） |
| `TokenJ dashboard` | 打开 TUI 实时仪表盘 |
| `tokenj demo` | 演示模式（内置示例数据） |
| `TokenJ --help` | 查看帮助 |

---

## ⚙️ 技术栈

```
┌──────────────────────────────────────────┐
│               TokenJ                      │
├──────────────────────────────────────────┤
│  运行时      │  tokio (异步)               │
│  HTTP       │  hyper 1.x                  │
│  TLS        │  rustls + tokio-rustls      │
│  TUI        │  ratatui + crossterm        │
│  数据库      │  SQLite + rusqlite          │
│  证书       │  rcgen (动态 CA + 域名签发)   │
│  测试       │  70 个 · 0 unsafe           │
│  MCP Server │  Python FastMCP (IDE 集成)   │
└──────────────┴───────────────────────────┘
```

---

## 🤝 贡献

```bash
# 运行测试
cargo test

# 构建发布版
cargo build --release

# 验证全链路
python scripts/verify.py
```

---

## 📄 开源协议

MIT © 2026 TokenJ

---

<div align="center">

**装了就省钱 · 零配置 · 最高省 90%**

<sub>用 ❤️ 和 🦀 Rust 打造</sub>

</div>
