"""Resolve project-local verification paths without a shared cache root."""
import functools
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]

@functools.lru_cache(maxsize=1)
def cargo_target():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, text=True))
    return Path(metadata["target_directory"]).resolve()

def local_build_path(suffix=""):
    parts = Path(suffix).parts
    return str(cargo_target().joinpath("verification", *parts))
