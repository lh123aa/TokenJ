"""Test TokenJ MCP Server - all tools"""
import subprocess
import json
import time
import sys

def test():
    print("=" * 50)
    print("TokenJ MCP Server 测试")
    print("=" * 50)

    # Start MCP server
    proc = subprocess.Popen(
        ["python", "scripts/TokenJ_mcp_server.py"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd="e:/程序/tokenJ",
    )
    time.sleep(1)

    def send_request(method, params=None, id=1):
        req = json.dumps({
            "jsonrpc": "2.0", "id": id,
            "method": method,
            "params": params or {}
        })
        proc.stdin.write((req + "\n").encode())
        proc.stdin.flush()
        return json.loads(proc.stdout.readline())

    # 1. Initialize
    result = send_request("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "1.0"}
    }, id=1)
    print(f"\n[1/5] Initialize: {'OK' if result.get('id') == 1 else 'FAIL'}")

    # 2. List tools
    result = send_request("tools/list", {}, id=2)
    tools = result.get("result", {}).get("tools", [])
    print(f"\n[2/5] Tools list: {len(tools)} tools found")
    for t in tools:
        print(f"      [{t['name']}] {t['description'][:60]}")

    # 3. get_stats (no data)
    result = send_request("tools/call", {
        "name": "get_stats", "arguments": {"days": 30}
    }, id=3)
    if "result" in result:
        text = result["result"]["content"][0]["text"]
        data = json.loads(text)
        print(f"\n[3/5] get_stats: OK (0 records)")
        print(f"      requests={data['total']}, cost=${data.get('total_cost_dollars', 0)}")
    else:
        print(f"\n[3/5] get_stats: FAIL - {result}")

    # 4. estimate_savings
    result = send_request("tools/call", {
        "name": "estimate_savings",
        "arguments": {
            "provider": "anthropic",
            "model": "claude-sonnet-4-6",
            "daily_input_tokens": 100000,
            "daily_output_tokens": 20000,
            "cache_hit_rate": 70
        }
    }, id=4)
    if "result" in result:
        text = result["result"]["content"][0]["text"]
        data = json.loads(text)
        print(f"\n[4/5] estimate_savings: OK")
        print(f"      daily_saving=${data['daily_saving_dollars']}")
        print(f"      monthly_saving=${data['monthly_saving_dollars']}")
        print(f"      yearly_saving=${data['yearly_saving_dollars']}")
    else:
        print(f"\n[4/5] estimate_savings: FAIL - {result}")

    # 5. get_history
    result = send_request("tools/call", {
        "name": "get_history",
        "arguments": {"days": 7, "provider": ""}
    }, id=5)
    if "result" in result:
        text = result["result"]["content"][0]["text"]
        data = json.loads(text)
        print(f"\n[5/5] get_history: OK ({data['total_requests']} records)")
    else:
        print(f"\n[5/5] get_history: FAIL - {result}")

    proc.terminate()
    print("\n" + "=" * 50)
    print("所有测试完成")
    print("=" * 50)

if __name__ == "__main__":
    test()
