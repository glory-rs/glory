"""Verify the installed APK and capture actual AssetManager-rendered pixels.

Read-only ADB calls; no input injection. Uses Pillow. Run after launching an
owned Glory emulator APK built with android-assets-probe.
"""
import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import re
import subprocess
import zipfile

from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--adb', type=Path, required=True)
parser.add_argument('--serial', required=True)
parser.add_argument('--package', type=Path, required=True, help='directory containing build.json and glory-counter.apk')
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--phase', default='probe')
parser.add_argument('--scale', type=float, default=2.625)
args = parser.parse_args()
root = args.out.resolve()
allowed = root.is_relative_to('/Volumes/Data/Caches/glory') or str(root).replace('\\', '/').lower().startswith('d:/buildcache/')
if not allowed or not args.serial.startswith('emulator-') or not re.fullmatch(r'[a-z0-9-]+', args.phase):
    parser.error('use an owned emulator, approved data-volume output and simple phase name')
if not args.adb.is_absolute() or not math.isfinite(args.scale) or not 0.5 <= args.scale <= 8:
    parser.error('use an absolute ADB executable and finite scale in 0.5..8')
root.mkdir(parents=True, exist_ok=True)
command = [str(args.adb), '-s', args.serial]


def adb(*parts):
    return subprocess.check_output(command + list(parts), timeout=30)


avd = adb('emu', 'avd', 'name').decode().splitlines()[0].strip()
assert avd.startswith('Glory_'), f'not an owned Glory AVD: {avd}'
manifest = json.loads((args.package / 'build.json').read_text())
assert 'android-assets-probe' in manifest['features']
apk = args.package / 'glory-counter.apk'
assert hashlib.sha256(apk.read_bytes()).hexdigest() == manifest['sha256']
installed = adb('shell', 'pm', 'path', 'rs.glory.counter').decode().strip()
match = re.fullmatch(r'package:(/[A-Za-z0-9_+/=.~-]+\.apk)', installed)
assert match, f'unexpected installed APK path: {installed}'
assert adb('shell', 'sha256sum', match[1]).decode().split()[0] == manifest['sha256'], 'installed APK differs from build'
compression = {}
with zipfile.ZipFile(apk) as archive:
    for entry in archive.infolist():
        # aapt2 stores UTF-8 names without always setting ZIP's language flag.
        name = entry.filename if entry.flag_bits & 0x800 else entry.filename.encode('cp437').decode('utf-8')
        if name.startswith('assets/glory/'):
            compression[name] = entry.compress_type
assert compression['assets/glory/theme.css'] == zipfile.ZIP_DEFLATED
assert compression['assets/glory/中文/组合.txt'] == zipfile.ZIP_DEFLATED
pid = adb('shell', 'pidof', 'rs.glory.counter').decode().strip()
assert re.fullmatch(r'[0-9]+', pid)
log = adb('logcat', '-d', '--pid=' + pid, '-s', 'RustStdoutStderr').decode()
(root / f'{args.phase}-logcat.txt').write_text(log)
assert 'Glory APK assets probe: CSS/PNG/Unicode, misses/policy, body limit, prepare and close passed' in log
assert 'APK AssetManager prefix=glory, embedded entries=0' in log
pixels = adb('exec-out', 'screencap', '-p')
(root / f'{args.phase}.png').write_bytes(pixels)
image = Image.open(io.BytesIO(pixels)).convert('RGB')
mask = Image.new('L', image.size)
mask.putdata([255 if all(abs(a-b) <= 2 for a, b in zip(pixel, (20, 180, 90))) else 0
              for pixel in (image.get_flattened_data() if hasattr(image, 'get_flattened_data') else image.getdata())])
badge = mask.getbbox()
assert badge, 'APK PNG missing'
x0, y0, x1, y1 = badge
scale = args.scale
assert abs(x1-x0-32*scale) <= 1 and abs(y1-y0-32*scale) <= 1, f'APK CSS size: {badge}'
# Native layout rounds logical edges before fractional-DPI rasterization;
# the fully opaque color excludes its antialiased outer pixel.
expected_right = round(image.width / scale - 24) * scale
assert abs(x1-expected_right) <= 1, f'APK CSS right offset: {badge}'
assert abs(y0-24*scale) <= 1, f'APK CSS top offset: {badge}'
result = dict(format=1, serial=args.serial, avd=avd, pid=int(pid), phase=args.phase,
              apk_sha256=manifest['sha256'], features=manifest['features'], compression=compression,
              png_sha256=hashlib.sha256(pixels).hexdigest(), dimensions=image.size,
              scale=scale, badge=badge, embedded_entries=0, asset_probe_passed=True, package_pixels=True,
              limits=['emulator only', 'no network or permissions', 'no input injection or interaction acceptance'])
(root / f'{args.phase}.json').write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n')
print(json.dumps(result, ensure_ascii=False))
