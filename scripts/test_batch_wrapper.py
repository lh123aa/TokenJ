"""Test batch wrapper for MCP"""
import subprocess, json, time, sys

batch_path = r"e:\程序\tokenJ\.trae\tokenj_mcp.bat"

proc = subprocess.Popen(
    ["cmd.exe", "/c", batch_path],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)
time.sleep(0.5)

init = json.dumps({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {"protocolVersion": "2024-11-05", "capabilities": {},
               "clientInfo": {"name": "test", "version": "1.0"}}
})
proc.stdin.write((init + "\n").encode())
proc.stdin.flush()

line = proc.stdout.readline()
result = json.loads(line)
print(f"Init OK: id={result['id']}")

req = json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
proc.stdin.write((req + "\n").encode())
proc.stdin.flush()
tools = json.loads(proc.stdout.readline())
print(f"Tools: {len(tools['result']['tools'])} found")

proc.terminate()
print("Batch wrapper test PASSED")
