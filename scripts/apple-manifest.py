#!/usr/bin/env python3
"""Record the exact signed Apple package, including all resource/signature files."""
import hashlib
import json
import os
import pathlib
import sys

root = pathlib.Path(sys.argv[1]).resolve()
files = []
for path in sorted(root.rglob('*')):
    if path.is_file() and path.name != 'build.json':
        data = path.read_bytes()
        files.append(dict(path=path.relative_to(root).as_posix(), bytes=len(data),
                          sha256=hashlib.sha256(data).hexdigest()))
(root / 'build.json').write_text(json.dumps(dict(format=1, target=sys.argv[2],
    profile=sys.argv[3], features=os.environ.get("GLORY_IOS_FEATURES", "").split(), files=files), indent=2) + '\n')
