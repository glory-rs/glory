#!/usr/bin/env python3
"""Validate real UI logs and installed bytes on the two owned simulators.

This sends no UI events. Follow the README's actual-picker steps first.
"""
import argparse
import ast
import hashlib
import json
import pathlib
import plistlib
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--iphone-device', required=True)
parser.add_argument('--ipad-device', required=True)
parser.add_argument('--consumer-app', required=True, type=pathlib.Path)
parser.add_argument('--fixture-app', required=True, type=pathlib.Path)
parser.add_argument('--iphone-log', required=True, type=pathlib.Path)
parser.add_argument('--ipad-log', required=True, type=pathlib.Path)
parser.add_argument('--provider-kind', choices=['local-storage', 'enumerating'], default='local-storage')
parser.add_argument('--iphone-provider-log', type=pathlib.Path)
parser.add_argument('--ipad-provider-log', type=pathlib.Path)
parser.add_argument('--output', required=True, type=pathlib.Path)
args = parser.parse_args()
output = args.output.resolve()
assert output.is_relative_to(pathlib.Path('/Volumes/Data/Caches/glory'))
output.parent.mkdir(parents=True, exist_ok=True)
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
enumerating = args.provider_kind == 'enumerating'
fixture_bundle = 'rs.glory.hydration.enumerating' if enumerating else 'rs.glory.files.fixture'
fixture_executable = 'GloryHydrationHost' if enumerating else 'GloryDocuments'
fixture_sha = sha(args.fixture_app / fixture_executable)
def tree_sha(root):
    digest = hashlib.sha256()
    for path in sorted(root.rglob('*')):
        assert not path.is_symlink(), 'Unexpected bundle symlink'
        if path.is_file():
            digest.update(str(path.relative_to(root)).encode() + b'\0')
            digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()
fixture_tree = tree_sha(args.fixture_app)
if enumerating:
    assert args.iphone_provider_log and args.ipad_provider_log
    extension = args.fixture_app / 'PlugIns/GloryHydrationProvider.appex'
    info = plistlib.loads((extension / 'Info.plist').read_bytes())
    assert info['NSExtension']['NSExtensionPointIdentifier'] == 'com.apple.fileprovider-nonui'
    assert info['NSExtension']['NSExtensionPrincipalClass'] == 'GloryEnumeratingProvider'
    entitlements = subprocess.check_output(['codesign', '-d', '--entitlements', ':-', str(args.consumer_app)], stderr=subprocess.DEVNULL)
    assert not entitlements or not plistlib.loads(entitlements).get('com.apple.security.application-groups')
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
    assert trace[3] >= 2 and trace[7] >= 2 and trace[8] == 0
    # Owner retirement may occur after Clear; check its actual native completion
    # at that phase instead of requiring it in the earlier Clear snapshot.
    owners = re.findall(r'Glory system picker owner cancellation passed trace=(\[[^\n]+\])', log)
    assert owners
    owner_trace = ast.literal_eval(owners[-1])
    assert len(owner_trace) == 10 and owner_trace[8] == 0 and owner_trace[9] >= 1
    installed = pathlib.Path(sdk('get_app_container', device, 'rs.glory.counter', 'app'))
    fixture = pathlib.Path(sdk('get_app_container', device, fixture_bundle, 'app'))
    assert sha(installed / 'counter-ios') == consumer_sha
    assert sha(fixture / fixture_executable) == fixture_sha
    assert tree_sha(fixture) == fixture_tree
    provider_evidence = None
    provider_log_sha = None
    if enumerating:
        group = pathlib.Path(sdk('get_app_container', device, fixture_bundle, 'group.rs.glory.hydration.enumerating'))
        evidence_path = group / 'fetch-evidence.json'
        provider_evidence = json.loads(evidence_path.read_text())
        for filename in ['glory-provider-image.png', 'glory-provider-text.txt', 'glory-provider-retry.txt']:
            entry = provider_evidence[filename]
            assert entry['started'] >= 1 and entry['completed'] >= 1
            assert entry['completedAtMilliseconds'] - entry['startedAtMilliseconds'] >= 2900
        retry = provider_evidence['glory-provider-retry.txt']
        assert retry['failed'] == 1 and retry['started'] >= 2
        assert retry['failedAtMilliseconds'] <= retry['startedAtMilliseconds'] < retry['completedAtMilliseconds']
        provider_log = args.iphone_provider_log if device == args.iphone_device else args.ipad_provider_log
        provider_log_sha = sha(provider_log)
        events = provider_log.read_text()
        for event in ['started', 'failed', 'completed']:
            assert 'Glory hydration glory-provider-retry.txt ' + event in events
        # Persist the authoritative fixed counters alongside the result before fixture removal.
        (output.parent / (name.replace(' ', '-') + '-fetch-evidence.json')).write_bytes(evidence_path.read_bytes())
    phases.append(dict(device=device, name=name, trace=trace,
                       consumer_sha256=consumer_sha, installed_consumer_sha256=consumer_sha,
                       fixture_sha256=fixture_sha, installed_fixture_sha256=fixture_sha,
                       log_sha256=sha(path), fixture_tree_sha256=fixture_tree,
                       provider_evidence=provider_evidence, provider_log_sha256=provider_log_sha, passed=True))
result = dict(scope=('Actual system picker and independent enumerating File Provider extension; delayed content callbacks and fail-once recovery' if enumerating else 'Actual system UI and built-in Local Storage File Provider, independent owned Documents app; no URL injection/shared group/cloud account'),
              phases=phases, url_injection_probe_absent=True,
              consumer_has_no_provider_group=True if enumerating else None,
              limits=(['Simulator public API/grant/IO acceptance; no real-device sandbox enforcement',
                       'Replicated provider dataless materialization blocked by simulator POSIX EPERM',
                       'No cloud account, real network/offline recovery, hydration cancellation or reader acceptance'] if enumerating else
                      ['Simulator API/grant/IO acceptance only; no real-device sandbox enforcement',
                       'No custom third-party provider, remote hydration/error recovery or reader acceptance']))
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(result, indent=2) + '\n')
print(args.provider_kind + ': both owned simulators: real picker, external scoped reads/preview, cancel preservation, Owner dismissal and balanced close passed')
