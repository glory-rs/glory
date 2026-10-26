"""Capture/verify actual simulator pixels after a named native UI interaction.

Uses Pillow. This does not synthesize input, infer IME composition, or claim
VoiceOver/device acceptance. Retains raw frames for independent visual review.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--device', required=True)
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--phase', required=True)
parser.add_argument('--scale', type=int, default=3)
parser.add_argument('--safe-left', type=int, default=0)
parser.add_argument('--safe-right', type=int, default=0)
parser.add_argument('--same-label', type=Path)
args = parser.parse_args()
root = args.out.resolve()
if not root.is_relative_to('/Volumes/Data/Caches/glory') or not re.fullmatch(r'[a-z0-9-]+', args.phase):
    parser.error('use an Apple data-volume output directory and a simple phase name')
root.mkdir(parents=True, exist_ok=True)
os.environ.setdefault('DEVELOPER_DIR', '/Applications/Xcode.app/Contents/Developer')

def sim(*arguments):
    return subprocess.check_output(['xcrun', 'simctl', *arguments], timeout=30)

inventory = json.loads(sim('list', 'devices', '-j'))
device = next(d for group in inventory['devices'].values() for d in group if d['udid'] == args.device)
if not device['name'].startswith('Glory ') or device['state'] != 'Booted':
    parser.error('select an explicitly owned, booted Glory simulator')
# CoreSimulator's service may deny direct data-volume writes. Its own device
# tmp is OS-managed; copy only our frame to the evidence root after capture.
temporary = Path(device['dataPath']) / 'tmp' / f'glory-{args.phase}.png'
sim('io', args.device, 'screenshot', str(temporary))
frame = root / f'{args.phase}.png'
frame.write_bytes(temporary.read_bytes())
temporary.unlink()
image = Image.open(frame).convert('RGB')
mask = Image.new('L', image.size)
mask.putdata([255 if all(abs(a-b) <= 2 for a,b in zip(pixel,(20,180,90))) else 0 for pixel in (image.get_flattened_data() if hasattr(image, 'get_flattened_data') else image.getdata())])
badge = mask.getbbox()
scale = args.scale
assert badge is not None, 'embedded PNG missing'
x0,y0,x1,y1 = badge
assert (x1-x0,y1-y0) == (32*scale,32*scale), f'package CSS size: {badge}'
assert image.width-x1 == (24+args.safe_right)*scale, f'package CSS placement: {badge}'
safe_top = y0-24*scale
assert 0 <= safe_top < image.height//3, 'invalid safe-area-relative image placement'
label = image.crop(((24+args.safe_left)*scale,safe_top+50*scale,(240+args.safe_left)*scale,safe_top+76*scale))
label_hash = hashlib.sha256(label.tobytes()).hexdigest()
if args.same_label:
    assert json.loads(args.same_label.read_text())['label_sha256'] == label_hash, 'label changed across lifecycle/rotation'
result = dict(device=args.device,phase=args.phase,dimensions=image.size,scale=scale,
    png_sha256=hashlib.sha256(frame.read_bytes()).hexdigest(),badge=badge,safe_top=safe_top,
    label_sha256=label_hash,package_pixels=True)
(root/f'{args.phase}.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
