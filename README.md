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
║         ║  LLM API Cache Optimizer            ║          ║
║         ║  Zero Config · Zero Code Change     ║          ║
║         ║  Save Up to 90% on API Costs       ║          ║
║         ╚════════════════════════════════════╝          ║
║                                                        ║
╚══════════════════════════════════════════════════════════╝
```

# TokenJ 🚀

**Zero‑config LLM API cache optimizer. Save up to 90%. No code changes needed.**

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.2.0-blue)](Cargo.toml)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-70%20passed-brightgreen)](https://github.com)
[![Build](https://img.shields.io/badge/build-release%20%7C%2010MB-orange)]()

---

</div>

## 💎 One‑Liner

> **TokenJ is a local proxy that intercepts your LLM API requests, automatically injects cache headers for each provider, and lets Anthropic / OpenAI / DeepSeek / Gemini's built-in caching discounts work for you — **without changing your code**.**

---

## ✨ Features

<table>
<tr>
<td width="33%">

### 🎯 Save Instantly
- Anthropic prompt caching → **90% off**
- OpenAI prompt caching → **50–75% off**
- DeepSeek prompt caching → **90% off**
- Gemini context caching → **75% off**
</td>
<td width="33%">

### 🔌 Zero Code Change
- **Mode A**: change one line of `base_url`
- **Mode B**: set `HTTPS_PROXY` env var
- LLM domains → automatic MITM + cache injection
- Non‑LLM domains → transparent passthrough
</td>
<td width="33%">

### 🛡️ Production‑Ready
- ✅ Pure Rust, zero `unsafe`
- ✅ 70 tests, zero warnings
- ✅ TLS MITM + dynamic certificate签发
- ✅ SQLite persistence + TUI dashboard
</td>
</tr>
</table>

---

## 📊 How Much Can You Save?

| Provider | Model | Without Cache | With Cache | Savings |
|:---------|:------|:-------------:|:----------:|:-------:|
| Anthropic | Claude Sonnet 4 | $3.00/MTok | **$0.30**/MTok | **90%** |
| Anthropic | Claude Opus 4 | $15.00/MTok | **$1.50**/MTok | **90%** |
| OpenAI | GPT-4o | $2.50/MTok | **$0.625**/MTok | **75%** |
| DeepSeek | V4 Pro | $1.74/MTok | **$0.145**/MTok | **92%** |
| DeepSeek | V4 Flash | $0.14/MTok | **$0.028**/MTok | **80%** |

> At 5M daily input tokens with 70% cache hit rate: **~$283/month, ~$3,449/year**.

---

## 🚀 Quick Start

### Mode A: Direct Mode (Recommended ✅)

```bash
# 1. Start the proxy
TokenJ proxy

# 2. Point your SDK's base_url to the proxy
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
# 3. Watch the savings
TokenJ dashboard
```

### Mode B: HTTPS_PROXY Mode (MITM ✅)

```bash
# No code changes — just set the env var
export HTTPS_PROXY=http://127.0.0.1:9100

# Start the proxy
TokenJ proxy

# LLM domains → auto TLS decrypt + cache injection ✅
# Other domains → transparent passthrough
```

> ⚠️ First‑time use of Mode B requires installing the CA certificate. Run `TokenJ proxy` and follow the printed instructions.

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Your Application                        │
│  (Claude Code / Cursor / OpenAI SDK / custom script)     │
└────────────────────┬────────────────────────────────────┘
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
   ┌──────────────┐    ┌──────────────────┐
   │  Mode A       │    │  Mode B           │
   │  base_url    │    │  HTTPS_PROXY     │
   │  HTTP direct │    │  CONNECT tunnel   │
   └──────┬───────┘    └────────┬─────────┘
          │                     │
          ▼                     ▼
   ┌──────────────────────────────────────┐
   │          TokenJ proxy                │
   │                                      │
   │  ┌──────────────────────────────┐    │
   │  │  ① Identify provider + model │    │
   │  │  ② Inject cache strategy:    │    │
   │  │    ├─ Anthropic → cache_ctl  │    │
   │  │    ├─ OpenAI    → cache_key  │    │
   │  │    ├─ DeepSeek  → monitor    │    │
   │  │    ├─ Gemini    → ContextCache│   │
   │  │    └─ GLM-5     → strip      │    │
   │  │  ③ Forward over HTTPS       │    │
   │  │  ④ Parse response → savings │    │
   │  │  ⑤ SQLite + TUI update      │    │
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

### MITM Flow

```
Client                         TokenJ                       Provider
  │                              │                            │
  │  CONNECT api.anthropic.com   │                            │
  │────────────────────────────▶│                            │
  │                              │                            │
  │  200 Connection Established  │                            │
  │◀────────────────────────────│                            │
  │                              │                            │
  │  TLS ClientHello             │                            │
  │────────────────────────────▶│                            │
  │                              │  Dynamic cert for domain   │
  │                              │  (rcgen + CA cache)        │
  │  TLS ServerHello + Cert ◀──│                            │
  │◀────────────────────────────│                            │
  │                              │                            │
  │  🔓 TLS decrypt → read HTTP  │                            │
  │  Inject cache_control       │                            │
  │                              │  Forward over HTTPS        │
  │                              │──────────────────────────▶│
  │                              │                            │
  │                              │  ◀────────────────────────│
  │                              │  Parse → calculate savings │
  │                              │  → SQLite write            │
  │                              │  → TUI dashboard update    │
  │  ◀── Response back ────────│                            │
```

---

## 📈 Performance

### Real Benchmark Data

Tested on Windows 11 x64, Rust 1.85, release build:

```
Startup (first run, CA generation)          1,180 ms
Startup (cold, certs exist)                   249 ms
Startup (hot, warmed up)                       78 ms

Cold request (first reqwest init)             423 ms
Hot request (proxy overhead, median)            9 ms
Hot request (proxy overhead, p95)              15 ms

Release build (incremental)                 20 sec
Release build (full)                        1m 28s

Single binary size                          10.2 MB
Runtime memory                               ~18 MB
```

### vs. Alternatives

| Metric | TokenJ | mitmproxy | LiteLLM |
|:-------|:------:|:---------:|:-------:|
| **Language** | Rust ✅ | Python | Python |
| **Binary** | **10 MB** single file | ~50 MB | ~200 MB |
| **Startup** | **~250ms** | ~2s | ~3s |
| **Request overhead** | **6–12ms** | ~50ms | ~100ms |
| **Concurrency** | tokio async | asyncio | asyncio |
| **Memory** | **~20MB** | ~100MB | ~300MB |
| **MITM** | ✅ built-in | ✅ | ❌ |
| **Cache injection** | ✅ **automatic** | ❌ | ⚠️ manual |

### Impact on Real LLM Calls

LLM APIs take **1–30 seconds** to respond. TokenJ adds only **6–12ms** — that's **< 0.1%** overhead.

```
┌──────────────────────────────────────────────────┐
│  Total = LLM processing (seconds) + proxy (ms)    │
│                                                    │
│  LLM:   █████████████████████████████ 3,000ms     │
│  Proxy: ▏                                     9ms │
│  Ratio: < 0.3%                                   │
└──────────────────────────────────────────────────┘
```

### Annual Savings Estimate

```python
# Based on real prices.json
provider = "anthropic"
model = "claude-sonnet-4-6"
daily_input = 5,000,000 tokens
daily_output = 500,000 tokens
cache_hit_rate = 70%

Result:
  Daily:  $9.45
  Monthly: $283.50
  Yearly:  $3,449.25
```

---

## 🧪 Test Suite

### All 70 Tests Pass ✅

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
Total           70 tests

🔒 unsafe code:  0 lines
🔧 warnings:     0
✅ zero unwrap() in production code
```

### Execution Time

| Scenario | Duration |
|:---------|:--------:|
| Full unit test suite | **0.19 sec** |
| Incremental compile check | **0.66 sec** |
| Release build | **20 sec** |
| MCP end‑to‑end verification | **<1 sec** |

---

## 🎮 Demo Mode

Run `TokenJ demo` to see the live TUI dashboard:

```
┌──────────────────────────────────────────────────────────────┐
│  TokenJ │ Savings $0.04 Cost $0.04 │ Hit Rate 85.7% Reqs 31 │
├───────────────┬────────────────┬───────────────┬──────────────┤
│ 💰 Savings    │ 📊 Requests    │ 🎯 Cache      │ 📈 Models    │
│               │                │               │              │
│ Today $0.04   │ Total 31       │ Cached 89700  │ claude 20%   │
│ Cost $0.04    │ Input Tokens   │ Writes 15000  │ gpt-4o 20%   │
│ Rate 47.5%    │ 115500         │ Hit Rate      │ opus 20%     │
│               │ Output 6090    │ 85.7%        │ deepseek 10% │
├───────────────┴────────────────┴───────────────┴──────────────┤
│ 📡 Live Requests                                            │
│ ✅ claude-sonnet-4-6 in:5000  cached:4500  $0.0027 (90%)   │
│ ✅ gpt-4o          in:3000  cached:2800  $0.0002 (85%)     │
│ ✅ deepseek-v4-flash in:1000 cached:900   $0.0000 (90%)    │
│ 📝 gpt-4o          in:3000  cached:0     $0.0000 (0%)      │
│    (📝 = cache write, ✅ = cache hit)                      │
├──────────────────────────────────────────────────────────────┤
│ 📋 Event Log                                               │
│ [HIT]  anthropic | claude-sonnet-4-6 | in:5000  save:90%  │
│ [HIT]  openai    | gpt-4o          | in:3000  save:85%    │
│ [WRITE] anthropic | claude-haiku-4-5 | in:2000  cache:new │
└──────────────────────────────────────────────────────────────┘
```

---

## 🔍 MCP Server Verification

TokenJ ships with an MCP Server for IDE integration (Trae / VS Code / Cursor):

```
=== get_stats ===
  Requests: 10
  Cost: $0.01
  Savings: $0.01
  Hit Rate: 85.7%

=== get_repeats ===
  Groups: 7
    anthropic    claude-opus-4-7      2x
    anthropic    claude-sonnet-4-6    2x
    openai       gpt-4o               2x
    ...

=== estimate_savings ===
  Daily:  $9.45
  Monthly: $283.5
  Yearly:  $3,449.25

=== verify prices.json ===
  prices.json: 8 entries (exported from Rust)
    anthropic:opus-4-7
    anthropic:sonnet-4-6
    openai:gpt-4o
    deepseek:v4-pro
    ...
```

---

## 📦 Installation

### From Source

```bash
git clone https://github.com/lh123aa/TokenJ.git
cd TokenJ
cargo build --release
./target/release/TokenJ --version
```

### Prerequisites

- Rust 1.85+
- Windows / macOS / Linux

---

## 🎮 Commands

| Command | Description |
|:--------|:------------|
| `TokenJ proxy` | Start the proxy (default 127.0.0.1:9100) |
| `TokenJ dashboard` | Open the live TUI dashboard |
| `tokenj demo` | Demo mode (built‑in sample data) |
| `TokenJ --help` | Show help |

---

## ⚙️ Tech Stack

```
┌──────────────────────────────────────────┐
│               TokenJ                      │
├──────────────────────────────────────────┤
│  Runtime     │  tokio (async)             │
│  HTTP        │  hyper 1.x                 │
│  TLS         │  rustls + tokio-rustls     │
│  TUI         │  ratatui + crossterm       │
│  Database    │  SQLite + rusqlite         │
│  Certificate │  rcgen (dynamic CA + per‑domain) │
│  Testing     │  70 tests · 0 unsafe       │
│  MCP Server  │  Python FastMCP (IDE)      │
└──────────────┴───────────────────────────┘
```

---

## 🤝 Contributing

```bash
# Run tests
cargo test

# Build release
cargo build --release

# Verify end‑to‑end
python scripts/verify.py
```

---

## 📄 License

MIT © 2026 TokenJ

---

<div align="center">

**Zero config · Zero code change · Save up to 90%**

<sub>Built with ❤️ and 🦀 Rust</sub>

</div>
