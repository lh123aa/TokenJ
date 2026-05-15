"""Inject demo data into TokenJ database so tools show results immediately"""
import sqlite3
import uuid
import json
from datetime import datetime, timedelta, timezone
from pathlib import Path

DATA_DIR = Path.home() / ".TokenJ"
DB_PATH = DATA_DIR / "data.db"

DATA_DIR.mkdir(parents=True, exist_ok=True)
conn = sqlite3.connect(str(DB_PATH))

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

samples = [
    ("anthropic", "claude-sonnet-4-6", 5000, 200, 4500, 0, 0.30, 90.0),
    ("openai", "gpt-4o", 3000, 150, 0, 3000, 0.015, 0.0),
    ("anthropic", "claude-opus-4-7", 8000, 400, 7500, 0, 0.40, 93.0),
    ("deepseek", "deepseek-v4-pro", 2000, 100, 1800, 0, 0.013, 90.0),
    ("anthropic", "claude-sonnet-4-6", 5000, 250, 4800, 0, 0.31, 91.0),
    ("openai", "gpt-4o-mini", 1500, 80, 0, 0, 0.003, 0.0),
    ("anthropic", "claude-haiku-4-5", 2000, 100, 0, 2000, 0.007, 0.0),
    ("deepseek", "deepseek-v4-flash", 1000, 50, 900, 0, 0.002, 90.0),
    ("anthropic", "claude-opus-4-7", 8000, 500, 7600, 0, 0.42, 94.0),
    ("openai", "gpt-4o", 3000, 200, 2800, 0, 0.02, 85.0),
]

now = datetime.now(timezone.utc).replace(tzinfo=None)
for i, (prov, model, inp, out, cached, write, cost, rate) in enumerate(samples):
    ts = (now - timedelta(minutes=i * 5)).isoformat()
    saving = cost * rate / 100.0
    conn.execute("""
        INSERT OR IGNORE INTO requests
        (id, session_id, provider, model, input_tokens, output_tokens,
         cached_tokens, cache_write_tokens, actual_cost_cents, saving_cents,
         saving_rate, cache_injected, duration_ms, created_at)
        VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)
    """, (str(uuid.uuid4()), "demo", prov, model, inp, out, cached, write,
          cost, saving, rate, 1, 500, ts))

conn.execute("""
    INSERT OR IGNORE INTO sessions (id, start_time, end_time, total_requests, total_cost_cents, total_saving_cents)
    VALUES ('demo', ?, ?, 10, 1.49, 3.85)
""", (now.isoformat(), (now + timedelta(hours=1)).isoformat()))

conn.commit()
conn.close()

print(f"Injected {len(samples)} demo records into {DB_PATH}")
print("Demo data ready! Run 'TokenJ demo' or use MCP tools.")
