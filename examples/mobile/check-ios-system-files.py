#!/usr/bin/env python3
"""Validate real UI logs and installed bytes on the two owned simulators.

This sends no UI events. Follow the README's actual-picker steps first.
"""
import argparse
import ast
import hashlib
import json
import pathlib
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--iphone-device', required=True)
parser.add_argument('--ipad-device', required=True)
parser.add_argument('--consumer-app', required=True, type=pathlib.Path)
parser.add_argument('--fixture-app', required=True, type=pathlib.Path)
parser.add_argument('--iphone-log', required=True, type=pathlib.Path)
parser.add_argument('--ipad-log', required=True, type=pathlib.Path)
parser.add_argument('--output', required=True, type=pathlib.Path)
args = parser.parse_args()
output = args.output.resolve()
assert output.is_relative_to(pathlib.Path('/Volumes/Data/Caches/glory'))
def sdk(*words):
    return subprocess.check_output(['xcrun', 'simctl', *words], text=True).strip()
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
expected = [
    (args.iphone_device, 'Glory owned acceptance', args.iphone_log),
    (args.ipad_device, 'Glory iPad scene acceptance', args.ipad_log),
]
inventory = json.loads(sdk('list', 'devices', 'booted', '-j'))
consumer_sha = sha(args.consumer_app / 'counter-ios')
fixture_sha = sha(args.fixture_app / 'GloryDocuments')
strings = subprocess.check_output(['strings', str(args.consumer_app / 'counter-ios')], text=True)
assert 'GloryIOProbe' not in strings and 'GLORY_IOS_IO_PROBE' not in strings
phases = []
for device, name, path in expected:
    assert any(d['udid'] == device and d['name'] == name
               for group in inventory['devices'].values() for d in group)
    log = path.read_text()
    assert 'Glory system files failure' not in log
    assert 'Glory system picker owner cancellation passed' in log
    assert 'Glory system picker selected count=2' in log
    assert re.search(r'Glory system files selected seq=\d+ count=2', log)
    assert re.search(r'Glory system files cancelled seq=\d+ preserved=2', log)
    assert log.count('Glory system files open: external=true scoped=true') >= 2
    closed = re.findall(r'Glory system files cleared seq=\d+ native=Ok\(\[0, 0, 0, 0, 0, 0, 1\]\) trace=(\[[^\n]+\])', log)
    assert closed
    trace = ast.literal_eval(closed[-1])
    assert len(trace) == 10 and all(isinstance(v, int) and v >= 0 for v in trace)
    assert trace[3] == trace[4] and trace[5] == trace[6]
    assert trace[3] >= 2 and trace[7] >= 2 and trace[8] == 0 and trace[9] >= 1
    installed = pathlib.Path(sdk('get_app_container', device, 'rs.glory.counter', 'app'))
    fixture = pathlib.Path(sdk('get_app_container', device, 'rs.glory.files.fixture', 'app'))
    assert sha(installed / 'counter-ios') == consumer_sha
    assert sha(fixture / 'GloryDocuments') == fixture_sha
    phases.append(dict(device=device, name=name, trace=trace,
                       consumer_sha256=consumer_sha, installed_consumer_sha256=consumer_sha,
                       fixture_sha256=fixture_sha, installed_fixture_sha256=fixture_sha,
                       log_sha256=sha(path), passed=True))
result = dict(scope='Actual system UI and built-in Local Storage File Provider, independent owned Documents app; no URL injection/shared group/cloud account',
              phases=phases, url_injection_probe_absent=True,
              limits=['Simulator API/grant/IO acceptance only; no real-device sandbox enforcement',
                      'No custom third-party provider, remote hydration/error recovery or reader acceptance'])
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(result, indent=2) + '\n')
print('Both owned simulators: real picker, external scoped reads/preview, cancel preservation, Owner dismissal and balanced close passed')
