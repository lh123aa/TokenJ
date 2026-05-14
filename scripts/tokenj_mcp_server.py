"""
tokenJ MCP Server - 直接在 Trae IDE 中使用 tokenJ 的 Token 分析功能

安装: pip install mcp
运行: python scripts/tokenj_mcp_server.py
配置: 在 Trae 设置中添加 MCP Server:
      "tokenj": { "command": "python", "args": ["scripts/tokenj_mcp_server.py"] }
"""
import json
import os
import sqlite3
from datetime import datetime, timedelta, timezone
from pathlib import Path
from mcp.server.fastmcp import FastMCP

DATA_DIR = Path(os.path.expanduser("~/.tokenj"))
DB_PATH = DATA_DIR / "data.db"

mcp = FastMCP("tokenj", instructions="""
tokenJ - LLM API 缓存优化引擎

自动分析 LLM API 调用的 Token 消耗、缓存命中率、节省金额。
通过在 LLM 请求中自动注入 cache_control 标记，让 API 提供商启用缓存折扣。

支持的 Provider: Anthropic Claude / OpenAI / DeepSeek / Google Gemini
""")


def get_db() -> sqlite3.Connection:
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(DB_PATH))
    conn.row_factory = sqlite3.Row
    return conn


def ensure_tables():
    conn = get_db()
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS requests (
            id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
            provider TEXT NOT NULL, model TEXT NOT NULL,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cached_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0,
            actual_cost_cents REAL NOT NULL DEFAULT 0,
            saving_cents REAL NOT NULL DEFAULT 0,
            saving_rate REAL NOT NULL DEFAULT 0,
            cache_injected INTEGER NOT NULL DEFAULT 0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY, start_time TEXT NOT NULL, end_time TEXT,
            total_requests INTEGER NOT NULL DEFAULT 0,
            total_cost_cents REAL NOT NULL DEFAULT 0,
            total_saving_cents REAL NOT NULL DEFAULT 0
        );
    """)
    conn.close()


def calculate_estimated_saving(provider: str, model: str, input_tokens: int, output_tokens: int, cached_tokens: int) -> dict:
    prices = {
        "anthropic:sonnet": {"input": 3.0, "output": 15.0, "cache_read": 0.30},
        "anthropic:opus": {"input": 5.0, "output": 25.0, "cache_read": 0.50},
        "anthropic:haiku": {"input": 1.0, "output": 5.0, "cache_read": 0.10},
        "openai:gpt-4o": {"input": 2.50, "output": 10.0, "cache_read": 0.625},
        "openai:gpt-4o-mini": {"input": 0.15, "output": 0.60, "cache_read": 0.0375},
        "deepseek:v4-pro": {"input": 1.74, "output": 3.48, "cache_read": 0.145},
        "deepseek:v4-flash": {"input": 0.14, "output": 0.28, "cache_read": 0.028},
    }
    key = f"{provider}:"
    matched = None
    for k, p in prices.items():
        if k.startswith(key) and (model.lower().split("-")[0] in k or any(w in model.lower() for w in k.split(":")[1].split("-"))):
            matched = p
            break
    if not matched:
        for k, p in prices.items():
            if k.startswith(key):
                matched = p
                break
    if not matched:
        matched = {"input": 2.0, "output": 8.0, "cache_read": 2.0}

    no_cache = (input_tokens / 1_000_000 * matched["input"] + output_tokens / 1_000_000 * matched["output"]) * 100
    uncached = input_tokens - cached_tokens
    with_cache = (uncached / 1_000_000 * matched["input"] + cached_tokens / 1_000_000 * matched["cache_read"] + output_tokens / 1_000_000 * matched["output"]) * 100
    saving = no_cache - with_cache
    rate = (saving / no_cache * 100) if no_cache > 0 else 0

    return {"no_cache_cost_cents": round(no_cache, 4), "with_cache_cost_cents": round(with_cache, 4),
            "saving_cents": round(saving, 4), "saving_rate": round(rate, 1)}


@mcp.tool(description="获取 Token 使用统计概览：总请求数、总成本、节省金额、缓存命中率")
def get_stats(days: int = 7) -> str:
    ensure_tables()
    conn = get_db()
    since = datetime.now(timezone.utc).replace(tzinfo=None) - timedelta(days=days)
    since = since.isoformat()
    row = conn.execute("""
        SELECT COUNT(*) as total, COALESCE(SUM(actual_cost_cents),0) as cost,
               COALESCE(SUM(saving_cents),0) as saving,
               COALESCE(SUM(cached_tokens),0) as cached,
               COALESCE(SUM(input_tokens),0) as input_tokens,
               COALESCE(SUM(cache_write_tokens),0) as writes,
               COALESCE(SUM(output_tokens),0) as output_tokens
        FROM requests WHERE created_at >= ?
    """, (since,)).fetchone()

    result = dict(row)
    result["days"] = days
    total_cache = result["cached"] + result["writes"]
    result["cache_hit_rate"] = round(result["cached"] / total_cache * 100, 1) if total_cache > 0 else 0
    result["total_cost_dollars"] = round(result["cost"] / 100, 2)
    result["total_saving_dollars"] = round(result["saving"] / 100, 2)
    conn.close()
    return json.dumps(result, ensure_ascii=False, indent=2)


@mcp.tool(description="列出重复发送次数最多的内容 — 找出哪些 Token 被浪费了")
def get_repeats(min_count: int = 3) -> str:
    ensure_tables()
    conn = get_db()
    rows = conn.execute("""
        SELECT provider, model, COUNT(*) as count,
               SUM(input_tokens) as total_input,
               SUM(cached_tokens) as total_cached,
               SUM(saving_cents) as total_saving
        FROM requests GROUP BY provider, model
        HAVING count >= ? ORDER BY total_input DESC LIMIT 20
    """, (min_count,)).fetchall()

    results = []
    for row in rows:
        d = dict(row)
        est = calculate_estimated_saving(d["provider"], d["model"], d["total_input"], 0, d["total_input"])
        d["estimated_waste_without_cache_cents"] = est["no_cache_cost_cents"]
        d["estimated_saving_with_cache_cents"] = est["with_cache_cost_cents"]
        results.append(d)

    summary = {"total_groups": len(results), "repeats": results}
    conn.close()
    return json.dumps(summary, ensure_ascii=False, indent=2)


@mcp.tool(description="查询请求历史记录，可按时间范围和 Provider 过滤")
def get_history(days: int = 7, provider: str = "") -> str:
    ensure_tables()
    conn = get_db()
    since = datetime.now(timezone.utc).replace(tzinfo=None) - timedelta(days=days)
    since = since.isoformat()
    if provider:
        rows = conn.execute("""
            SELECT * FROM requests WHERE created_at >= ? AND provider = ?
            ORDER BY created_at DESC LIMIT 50
        """, (since, provider)).fetchall()
    else:
        rows = conn.execute("""
            SELECT * FROM requests WHERE created_at >= ?
            ORDER BY created_at DESC LIMIT 50
        """, (since,)).fetchall()

    results = [dict(r) for r in rows]
    for r in results:
        r["total_cost_dollars"] = round(r["actual_cost_cents"] / 100, 4)
        r["saving_dollars"] = round(r["saving_cents"] / 100, 4)
        r["cache_hit"] = r["cached_tokens"] > 0

    summary = {"total_requests": len(results), "requests": results}
    conn.close()
    return json.dumps(summary, ensure_ascii=False, indent=2)


@mcp.tool(description="预估在不同提供商和模型上使用缓存能节省多少钱")
def estimate_savings(provider: str, model: str, daily_input_tokens: int, daily_output_tokens: int, cache_hit_rate: float = 70.0) -> str:
    cached = int(daily_input_tokens * cache_hit_rate / 100)
    uncached = daily_input_tokens - cached
    est = calculate_estimated_saving(provider, model, daily_input_tokens, daily_output_tokens, cached)

    result = {
        "provider": provider, "model": model,
        "daily_input_tokens": daily_input_tokens,
        "daily_output_tokens": daily_output_tokens,
        "cache_hit_rate": cache_hit_rate,
        "daily_saving_cents": est["saving_cents"],
        "daily_saving_dollars": round(est["saving_cents"] / 100, 2),
        "monthly_saving_dollars": round(est["saving_cents"] / 100 * 30, 2),
        "yearly_saving_dollars": round(est["saving_cents"] / 100 * 365, 2),
        "note": "预估仅供参考。实际节省取决于缓存命中率和模型实际价格。"
    }
    return json.dumps(result, ensure_ascii=False, indent=2)


if __name__ == "__main__":
    ensure_tables()
    mcp.run(transport="stdio")
