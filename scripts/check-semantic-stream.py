"""Transport acceptance for the explicitly running local stream_fixture binary.

This script consumes NDJSON for arrival order, pending-request disconnect and
slow-reader backpressure. The same fixture's HTML/CSP/adoption path is tested
by examples/semantic-web/stream-acceptance.js.
"""
import sys
if sys.platform == "win32":
    sys.pycache_prefix = "D:/BuildCache/glory-semantic-ui/pycache"
import http.client
import argparse
import json
import socket
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--port', type=int, default=8789)
port = parser.parse_args().port


def stats():
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    try:
        connection.request("GET", "/stats")
        return json.loads(connection.getresponse().read())
    finally:
        connection.close()


def wait_for(predicate, timeout=3):
    end = time.monotonic() + timeout
    while True:
        current = stats()
        if predicate(current):
            return current
        assert time.monotonic() < end, current
        time.sleep(0.01)


before = stats()
connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
started = time.perf_counter()
connection.request("GET", "/stream")
response = connection.getresponse()
assert response.status == 200
frames = []
while line := response.readline():
    frame = json.loads(line)
    frames.append((frame, (time.perf_counter() - started) * 1000))
response.close()
connection.close()
assert [frame["kind"] for frame, _ in frames] == ["shell", "patch", "patch", "end"]
assert [frame["sequence"] for frame, _ in frames] == list(range(4))
assert "waiting first" in frames[0][0]["body"]
assert frames[0][0]["body"].startswith("<!--gly:stream:2-->")
assert "ready first" in frames[1][0]["body"]
assert "ready second" in frames[2][0]["body"]
assert frames[1][1] - frames[0][1] >= 60, frames
assert frames[2][1] - frames[0][1] >= 350, frames
terminal = json.loads(frames[-1][0]["body"])
assert terminal["html_version"] == 4
assert len(terminal["resources"]["records"]) == 2
wait_for(lambda s: s["active"] == 0 and s["timers"] == 0)

for _ in range(5):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    connection.request("GET", "/stream")
    response = connection.getresponse()
    assert json.loads(response.readline())["kind"] == "shell"
    wait_for(lambda s: s["timers"] > 0)
    response.close()
    connection.close()
    wait_for(lambda s: s["active"] == 0 and s["timers"] == 0)

before_pressure = stats()
slow = socket.socket()
slow.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4096)
slow.settimeout(3)
slow.connect(("127.0.0.1", port))
slow.sendall(b"GET /pressure HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
reader = slow.makefile("rb")
while reader.readline().strip():
    pass
assert json.loads(reader.readline())["kind"] == "shell"
time.sleep(0.2)
paused = stats()
assert 0 < paused["produced"] - before_pressure["produced"] < 24, paused
assert paused["active"] == 1, paused
time.sleep(0.2)
stable = stats()
assert stable["produced"] == paused["produced"], (paused, stable)
reader.close()
slow.close()
wait_for(lambda s: s["active"] == 0 and s["timers"] == 0)

after = stats()
assert after["completed"] - before["completed"] == 1, after
assert after["cancelled"] - before["cancelled"] == 6, after
print(json.dumps({
    "arrival_ms": [{"kind": frame["kind"], "ms": round(ms, 3)} for frame, ms in frames],
    "resource_records": len(terminal["resources"]["records"]),
    "before": before, "after": after,
    "slow_consumer": {"before": before_pressure, "paused": paused, "stable": stable},
}, indent=2))
