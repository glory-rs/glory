#!/usr/bin/env python3
"""Prepare owned files or validate real DocumentsUI evidence; sends no UI input."""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import re
import subprocess
import zipfile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--adb', required=True, type=Path)
    parser.add_argument('--serial', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--cleanup', action='store_true')
    parser.add_argument('--apk', type=Path)
    parser.add_argument('--aapt2', type=Path)
    parser.add_argument('--pending-log', type=Path)
    parser.add_argument('--foreground', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    assert output != Path('/Volumes/Data/Caches/glory') and output.is_relative_to('/Volumes/Data/Caches/glory')
    output.mkdir(parents=True, exist_ok=True)

    def adb(*words):
        return subprocess.check_output([str(args.adb), '-s', args.serial, *words],
                                       text=True, stderr=subprocess.STDOUT, timeout=30).strip()

    assert adb('emu', 'avd', 'name').splitlines()[0] == 'Glory_Input', 'Use the owned acceptance AVD'
    directory = '/sdcard/Download/glory-owned-system-files'
    fixtures = {
        'glory-system-text.txt': 'Glory Android DocumentsUI: 中文😀\n'.encode(),
        'glory-system-image.png': (Path(__file__).parent / 'assets/badge.png').read_bytes(),
        'glory-system-extra.txt': b'Owned count-refusal fixture\n',
        'rejected.bin': bytes([1, 2, 3, 4]),
    }
    hashes = {name: sha(body) for name, body in fixtures.items()}

    def check_owned(name):
        path = directory + '/' + name
        exists = subprocess.run([str(args.adb), '-s', args.serial, 'shell', 'test', '-e', path],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        assert exists.returncode in (0, 1), 'Cannot inspect owned fixture path'
        if exists.returncode == 0:
            assert adb('shell', 'sha256sum', path).split()[0] == hashes[name], 'Refuse to overwrite/remove unknown bytes'
        return exists.returncode == 0

    assert not (args.prepare and args.cleanup)
    if args.prepare:
        adb('shell', 'mkdir', '-p', directory)
        local = output / 'fixtures/glory-owned-system-files'
        local.mkdir(parents=True, exist_ok=True)
        for name, body in fixtures.items():
            check_owned(name)
            path = local / name
            path.write_bytes(body)
            adb('push', str(path), directory + '/' + name)
            assert check_owned(name)
        (output / 'fixture.json').write_text(json.dumps(dict(directory=directory, files_sha256=hashes), indent=2) + '\n')
        print('Four owned fixture files ready; navigate the actual picker through Internal Storage → Download.')
        return
    if args.cleanup:
        for name in fixtures:
            if check_owned(name):
                adb('shell', 'rm', directory + '/' + name)
        # Never remove unrelated files or recurse into this public directory.
        if not adb('shell', 'ls', '-A', directory):
            adb('shell', 'rmdir', directory)
        print('Owned fixture bytes removed; unrelated storage preserved.')
        return

    assert all([args.apk, args.aapt2, args.pending_log, args.foreground]), 'Verification requires APK/tools and paused-picker evidence'
    package = 'rs.glory.counter'
    pid = adb('shell', 'pidof', package).split()[0]
    logs = adb('logcat', '--pid', pid, '-d', '-v', 'threadtime')
    (output / 'app.log').write_text(logs + '\n')
    assert 'Glory DocumentsUI failure' not in logs and 'FATAL EXCEPTION' not in logs
    phases = [('selected', 'count=1'), ('cancelled', 'preserved=1'),
              ('selected', 'count=2'), ('cancelled', 'preserved=2'),
              ('count refused', 'preserved=2'), ('cleared', '')]
    traces = []
    cursor = 0
    for phase, detail in phases:
        match = re.search(r'Glory DocumentsUI ' + phase + r' seq=\d+ ' + detail + r' ?trace=(\[[^\n]+\])', logs[cursor:])
        assert match, (phase, detail)
        cursor += match.end()
        trace = ast.literal_eval(match[1])
        assert len(trace) == 8 and all(type(n) is int and n >= 0 for n in trace)
        traces.append(trace)
    clear = traces[-1]
    assert clear[0] >= 5 and clear[1] >= 3 and clear[2] >= 2
    assert clear[3] == clear[4] and clear[3] >= 3 and clear[5] >= 3 and clear[6] >= 9
    pending = args.pending_log.read_text()
    marker = 'Glory DocumentsUI retired pending: permit retained, second picker busy'
    assert marker in pending and f' {pid} ' in pending
    assert 'Glory DocumentsUI retired passed' not in pending
    foreground = args.foreground.read_text()
    assert 'com.google.android.documentsui/com.android.documentsui.picker.PickActivity' in foreground
    assert 'Glory DocumentsUI retired passed trace=' in logs and 'ACTION_OPEN_DOCUMENT launched explicit=false' in logs
    assert 'ACTION_OPEN_DOCUMENT launched explicit=true' not in logs
    retired = ast.literal_eval(re.findall(r'Glory DocumentsUI retired passed trace=(\[[^\n]+\])', logs)[-1])
    assert len(retired) == 8 and retired[7] >= 1
    installed = adb('shell', 'pm', 'path', package).removeprefix('package:')
    local_hash = sha(args.apk.read_bytes())
    assert adb('shell', 'sha256sum', installed).split()[0] == local_hash
    with zipfile.ZipFile(args.apk) as apk:
        dex = b''.join(apk.read(n) for n in apk.namelist() if re.fullmatch(r'classes\d*\.dex', n))
        assert b'GlorySystemFilesTrace' in dex
        assert b'GloryFilesSelectionProbe' not in dex and b'GloryFilesProbeProvider' not in dex
    permissions = subprocess.check_output([str(args.aapt2), 'dump', 'permissions', str(args.apk)], text=True)
    assert not re.search(r'android.permission.(?:.*STORAGE|READ_MEDIA_|MANAGE_)', permissions)
    for name in fixtures:
        assert check_owned(name)
    result = dict(passed=True, pid=pid, serial=args.serial, apk_sha256=local_hash,
                  log_sha256=sha(logs.encode()), paused_log_sha256=sha(args.pending_log.read_bytes()),
                  foreground_sha256=sha(args.foreground.read_bytes()), phase_traces=traces,
                  retired_trace=retired, files_sha256=hashes, permissions=permissions.splitlines(),
                  scope='Actual DocumentsUI input, system-provider grants, bounded exact reads/private PNG preview, Cancel/count preservation, clear/revocation, paused Owner retirement and ignored late result',
                  limits='Owned ARM64 emulator; no real-device sandbox, cloud provider/hydration, TalkBack, camera or arbitrary file acceptance')
    (output / 'system-result.json').write_text(json.dumps(result, indent=2) + '\n')
    print('Real DocumentsUI: ordered selection/cancel/count/clear phases, paused Owner retirement, installed hash and isolation passed.')


if __name__ == '__main__':
    main()
