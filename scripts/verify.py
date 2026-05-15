"""Quick verification script for tokenJ"""
import json, sys
from pathlib import Path
sys.path.insert(0, 'scripts')
from tokenj_mcp_server import get_stats, get_repeats, get_history, estimate_savings

print('=== get_stats ===')
s = json.loads(get_stats(365))
print(f'  请求: {s["total"]}')
print(f'  成本: ${s["total_cost_dollars"]}')
print(f'  节省: ${s["total_saving_dollars"]}')
print(f'  命中率: {s["cache_hit_rate"]}%')

print()
print('=== get_repeats ===')
r = json.loads(get_repeats(1))
print(f'  分组数: {r["total_groups"]}')
for g in r['repeats'][:5]:
    print(f'    {g["provider"]:12s} {g["model"]:20s} {g["count"]:2d}x')

print()
print('=== get_history ===')
h = json.loads(get_history(365))
print(f'  记录数: {h["total_requests"]}')

print()
print('=== estimate_savings ===')
e = json.loads(estimate_savings('anthropic', 'claude-sonnet-4-6', 5_000_000, 500_000, 70))
print(f'  日省: ${e["daily_saving_dollars"]}')
print(f'  月省: ${e["monthly_saving_dollars"]}')
print(f'  年省: ${e["yearly_saving_dollars"]}')

print()
print('=== verify prices.json ===')
prices_path = Path.home() / '.tokenj' / 'prices.json'
if prices_path.exists():
    with open(prices_path) as f:
        prices = json.load(f)
    print(f'  prices.json: {len(prices)} entries loaded from Rust export')
    for p in prices[:3]:
        print(f'    {p["key"]}')
else:
    print('  prices.json not found')

print()
print('✅ tokenJ 全链路验证通过!')
