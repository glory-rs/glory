"""Check native backend dependency isolation without compiling or opening windows."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
GPU = {"anyrender_vello_hybrid", "wgpu", "vello_gpu"}
CPU = {"anyrender_vello_cpu", "softbuffer_window_renderer"}

# Normal/build edges exclude test-only dependencies. In particular, adding an
# offscreen CPU Raster must not switch the selected window renderer to CPU.
PROFILES = (
    ("", set(), GPU | CPU | {"glory-apace"}),
    ("apace", {"glory-apace"}, GPU | CPU | {"wingy"}),
    ("apace-raster", {"anyrender_vello_cpu"}, GPU | {"wingy", "softbuffer_window_renderer"}),
    ("apace-shell", GPU, CPU),
    ("apace-shell,apace-raster", GPU | {"anyrender_vello_cpu"}, {"softbuffer_window_renderer"}),
    ("apace-shell-cpu", CPU, GPU),
    ("apace-accesskit", GPU | {"apace_accesskit"}, CPU),
    ("apace-accesskit-cpu", CPU | {"apace_accesskit"}, GPU),
    ("apace-android-native-activity", GPU, CPU),
    ("apace-android-native-activity-cpu", CPU, GPU),
    ("apace-android-game-activity", GPU, CPU),
    ("apace-android-game-activity-cpu", CPU, GPU),
)


def main():
    for features, required, forbidden in PROFILES:
        command = ["cargo", "tree", "--locked", "-p", "glory", "--no-default-features",
                   "--edges", "normal,build", "--prefix", "none", "--format", "{p}"]
        if features:
            command += ["--features", features]
        tree = subprocess.check_output(command, cwd=ROOT, text=True, encoding="utf-8")
        packages = {line.split()[0] for line in tree.splitlines() if line.strip()}
        missing = required - packages
        leaked = forbidden & packages
        if missing or leaked:
            raise SystemExit(f"{features or 'default'}: missing={sorted(missing)}, leaked={sorted(leaked)}")
        print(f"Native dependency profile verified: {features or 'default'}")


if __name__ == "__main__":
    main()
