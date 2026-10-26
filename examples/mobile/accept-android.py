"""Exercise the shared counter on the owned 1080x2400 Android emulator.

Pixel assertions cover visible updates and restoration. Full screenshots remain
for text review; this is not an Android accessibility or CJK composition test.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import time

from PIL import Image


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--serial", required=True)
parser.add_argument("--out", required=True, type=Path)
args = parser.parse_args()
if not args.serial.startswith("emulator-"):
    parser.error("use an explicitly selected owned emulator")
root = args.out.resolve()
if not str(root).replace("\\", "/").lower().startswith("d:/buildcache/"):
    parser.error("evidence and caches must stay under D:/BuildCache")
root.mkdir(parents=True, exist_ok=True)
os.environ["ANDROID_USER_HOME"] = str(root / "android-home")
os.environ["TEMP"] = os.environ["TMP"] = str(root / "tmp")
(root / "tmp").mkdir(exist_ok=True)
adb_path = Path(os.environ["ANDROID_HOME"]) / "platform-tools/adb.exe"
command = [str(adb_path), "-s", args.serial]


def adb(*values):
    return subprocess.check_output(command + list(values), stderr=subprocess.STDOUT, timeout=30).decode().strip()


def screen(name):
    pixels = subprocess.check_output(command + ["exec-out", "screencap", "-p"], timeout=15)
    (root / f"{name}.png").write_bytes(pixels)
    return Image.open(io.BytesIO(pixels)).convert("RGB")


def label(image):
    return hashlib.sha256(image.crop((50, 140, 350, 210)).tobytes()).hexdigest()


def package_badge(image):
    # The PNG owns this color; only the imported package stylesheet gives the
    # 1x1 image its 32-DIP box and top/right placement. Both must have loaded.
    x, y = image.width - 105, 105
    pixels = image.crop((x - 24, y - 24, x + 24, y + 24)).convert("RGB")
    histogram = pixels.getcolors(48 * 48) or []
    return sum(count for count, color in histogram if all(abs(a - b) <= 2 for a, b in zip(color, (20, 180, 90)))) > 2200


def tap(x, y):
    adb("shell", "input", "tap", str(x), str(y))
    time.sleep(0.5)


def wait_screen(name, condition):
    deadline = time.monotonic() + 15
    while True:
        image = screen(name)
        if condition(image):
            return image
        if time.monotonic() >= deadline:
            raise RuntimeError(f"native frame did not reach {name}")
        time.sleep(0.5)


def keyboard_top(image):
    # Gboard's light toolbar/background is tinted; the fixture's lower app
    # surface is white. Derive its actual top instead of assuming one keyboard
    # height (the default AVD and an Android Studio profile differ).
    for y in range(image.height // 2, image.height * 4 // 5):
        pixel = image.getpixel((5, y))
        if min(pixel) < 250 and max(pixel) - min(pixel) > 3:
            return y
    return None


package = "rs.glory.counter"
activity = f"{package}/android.app.NativeActivity"
rotation = adb("shell", "settings", "get", "system", "user_rotation")
automatic = adb("shell", "settings", "get", "system", "accelerometer_rotation")
hard_keyboard = adb("shell", "settings", "get", "secure", "show_ime_with_hard_keyboard")
try:
    adb("shell", "settings", "put", "secure", "show_ime_with_hard_keyboard", "1")
    adb("shell", "settings", "put", "system", "accelerometer_rotation", "0")
    adb("shell", "wm", "user-rotation", "lock", "0")
    wait_screen("portrait-ready", lambda image: image.size == (1080, 2400))
    adb("shell", "am", "force-stop", package)
    adb("shell", "am", "start", "-n", activity)
    deadline = time.monotonic() + 60
    while True:
        initial = screen("initial")
        if initial.size == (1080, 2400):
            dark = sum(initial.crop((50, 140, 350, 210)).convert("L").histogram()[:100])
            if dark > 300:
                break
        if time.monotonic() > deadline:
            raise RuntimeError("counter did not draw at the expected emulator dimensions")
        time.sleep(1)
    pid = adb("shell", "pidof", package)
    initial = wait_screen("package-assets", package_badge)
    zero = label(initial)
    tap(380, 400)  # +1
    one = wait_screen("plus", lambda image: label(image) != zero)
    tap(270, 400)  # -1
    wait_screen("minus", lambda image: label(image) == zero)
    tap(380, 400)
    tap(180, 270)  # editor
    ime = adb("shell", "dumpsys", "input_method")
    (root / "ime.txt").write_text(ime, encoding="utf-8")
    assert re.search(r"(?:mInputShown|isInputViewShown|inputShown)=true", ime), "system keyboard is not shown"
    adb("shell", "input", "text", "7")
    time.sleep(0.5)
    injected = label(screen("injected-key"))
    assert injected != label(one), "injected edit was not published"
    keyboard = wait_screen("keyboard-ready", lambda image: keyboard_top(image) is not None)
    key_x = 810  # 8, the eighth of ten equally spaced Gboard number keys
    key_y = keyboard_top(keyboard) + round(72 * 2.625)
    tap(key_x, key_y)
    edited = wait_screen("software-key", lambda image: label(image) != injected)
    adb("shell", "input", "keyevent", "4")  # hide the confirmed visible IME
    time.sleep(0.5)
    before = label(screen("before-home"))
    adb("shell", "input", "keyevent", "3")
    time.sleep(0.5)
    adb("shell", "am", "start", "-n", activity)
    time.sleep(1)
    assert adb("shell", "pidof", package) == pid, "backgrounding restarted the process"
    wait_screen("resumed", lambda image: label(image) == before and package_badge(image))
    adb("shell", "wm", "user-rotation", "lock", "1")
    time.sleep(1)
    wait_screen("landscape", lambda image: image.size == (2400, 1080) and label(image) == before and package_badge(image))
    adb("shell", "wm", "user-rotation", "lock", "0")
    time.sleep(1)
    wait_screen("portrait-restored", lambda image: image.size == (1080, 2400) and label(image) == before and package_badge(image))
    tap(130, 400)  # Clear after recreation
    assert label(screen("clear")) == zero, "input did not recover after surface recreation"
    # Finish the Activity, then recreate it without force-stopping the process.
    # This fails on the original winit once-per-process event-loop flag.
    adb("shell", "input", "keyevent", "4")
    time.sleep(1)
    adb("shell", "am", "start", "-n", activity)
    wait_screen("activity-recreated", lambda image: image.size == (1080, 2400) and label(image) == zero and package_badge(image))
    assert adb("shell", "pidof", package) == pid, "Activity test unexpectedly used a new process"
    tap(380, 400)
    assert label(screen("recreated-plus")) == label(one), "recreated Activity cannot receive input"
    logs = adb("logcat", "-d", f"--pid={pid}", "-v", "brief")
    (root / "application.log").write_text(logs, encoding="utf-8")
    assert "panicked at" not in logs and "FATAL EXCEPTION" not in logs, "application faulted"
    assert re.search(r"Glory mobile assets:.*completed: 2,.*closed: true", logs), "retired Activity did not complete and close its package loader"
    result = {
        "serial": args.serial, "pid": pid,
        "android": adb("shell", "getprop", "ro.build.version.release"),
        "abi": adb("shell", "getprop", "ro.product.cpu.abi"),
        "drawn": True, "touch_updates": True, "injected_edit": True,
        "system_keyboard_shown": True, "software_key_updates": True,
        "same_process_resume": True, "rotation_restoration": True,
        "post_resume_input": True,
        "same_process_activity_recreation": True,
        "package_css_and_png_pixels": True, "retired_package_loader_closed": True,
        "software_key_point": [key_x, key_y],
        "scope": "1080x2400 emulator, Gboard numeric key, pixel restoration; no CJK/AT/device claim",
    }
    (root / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps(result, indent=2))
finally:
    process = subprocess.run(command + ["shell", "pidof", package], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
    if process.returncode == 0:
        current = process.stdout.decode().strip()
        (root / "application.log").write_text(adb("logcat", "-d", f"--pid={current}", "-v", "brief"), encoding="utf-8")
    adb("shell", "settings", "put", "system", "user_rotation", rotation)
    adb("shell", "settings", "put", "system", "accelerometer_rotation", automatic)
    if hard_keyboard == "null":
        adb("shell", "settings", "delete", "secure", "show_ime_with_hard_keyboard")
    else:
        adb("shell", "settings", "put", "secure", "show_ime_with_hard_keyboard", hard_keyboard)
