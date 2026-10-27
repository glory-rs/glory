"""Run the owned clipboard-probe APK without injecting UI input or recording clipboard bodies."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adb", required=True, type=Path)
    parser.add_argument("--apk", required=True, type=Path)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args()
    output = options.output.resolve()
    if output == Path("/Volumes/Data/Caches/glory") or not output.is_relative_to("/Volumes/Data/Caches/glory"):
        parser.error("output must stay in the approved data-volume cache")
    output.mkdir(parents=True, exist_ok=True)

    def adb(*args):
        return subprocess.check_output(
            [str(options.adb), "-s", options.serial, *args],
            text=True, stderr=subprocess.STDOUT, timeout=30,
        ).strip()

    name = adb("emu", "avd", "name").splitlines()[0].strip()
    if name != "Glory_Input":
        parser.error("this acceptance script requires the owned Glory_Input AVD")
    package = "rs.glory.counter"
    adb("shell", "am", "force-stop", package)
    (output / "install.log").write_text(adb("install", "-r", str(options.apk.resolve())))
    (output / "launch.log").write_text(adb("shell", "am", "start", "-W", "-n", package + "/.GloryGameActivity"))
    deadline = time.monotonic() + 25
    pid, logs = "", ""
    while time.monotonic() < deadline:
        try:
            pid = adb("shell", "pidof", package).split()[0]
        except (subprocess.CalledProcessError, IndexError):
            time.sleep(0.25)
            continue
        logs = adb("logcat", "--pid", pid, "-d", "-v", "threadtime")
        if "Glory Android clipboard acceptance:" in logs:
            break
        time.sleep(0.3)
    # App diagnostics contain no returned clipboard Strings. Avoid screenshots:
    # the OS could show the restored original clipboard in a system overlay.
    (output / "app.log").write_text(logs)
    installed = adb("shell", "pm", "path", package).removeprefix("package:")
    remote = adb("shell", "sha256sum", installed).split()[0]
    local = hashlib.sha256(options.apk.read_bytes()).hexdigest()
    passed = remote == local and "Glory Android clipboard acceptance: Ok(())" in logs
    result = {
        "serial": options.serial, "avd": name, "pid": pid,
        "apk_sha256": local, "installed_sha256": remote, "passed": passed,
        "scope": "Foreground OS write/read, UTF-8 budgets, semantic CopyText and original ClipData restoration; no UI input injection",
        "limits": "ARM64 emulator; no real-device, native clipboard menu, keyboard or screen-reader acceptance",
    }
    (output / "sdk-result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"pid": pid, "passed": passed, "installed_hash_matches": remote == local}))
    if not passed:
        raise SystemExit("Clipboard acceptance failed; see body-free app.log")


if __name__ == "__main__":
    main()
