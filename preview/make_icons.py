import pathlib
import shutil

from PIL import Image, ImageDraw, ImageOps

ROOT = pathlib.Path(__file__).resolve().parent.parent
ART = ROOT / "artifacts"
ANDROID = ROOT / ".android-build" / "app" / "res"
TAURI = ROOT / "nixyon-app" / "src-tauri" / "icons"

full = Image.open(ART / "app-icon-1024.png").convert("RGBA")
background = Image.open(ROOT / ".android-build" / "icon-background.png").convert("RGBA")


def rounded(image, radius_ratio=0.225):
    size = image.size[0]
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, size - 1, size - 1), radius=int(size * radius_ratio), fill=255
    )
    result = image.copy()
    result.putalpha(mask)
    return result


def circular(image):
    size = image.size[0]
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, size - 1, size - 1), fill=255)
    result = image.copy()
    result.putalpha(mask)
    return result


def save_resized(image, size, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    image.resize((size, size), Image.LANCZOS).save(path, optimize=True)


# Adaptive icon: 108 dp layers at xxxhdpi.
layer_size = 432
save_resized(background, layer_size, ANDROID / "drawable-xxxhdpi" / "ic_launcher_background.png")
foreground_png = ANDROID / "drawable-xxxhdpi" / "ic_launcher_foreground.png"
if foreground_png.exists():
    foreground_png.unlink()
vector = ANDROID / "drawable" / "ic_launcher_foreground.xml"
vector.parent.mkdir(parents=True, exist_ok=True)
shutil.copyfile(ROOT / "preview" / "ic_launcher_foreground.xml", vector)

for density, size in {
    "mdpi": 48,
    "hdpi": 72,
    "xhdpi": 96,
    "xxhdpi": 144,
    "xxxhdpi": 192,
}.items():
    save_resized(rounded(full), size, ANDROID / f"mipmap-{density}" / "ic_launcher.png")
    save_resized(circular(full), size, ANDROID / f"mipmap-{density}" / "ic_launcher_round.png")

adaptive = """<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@drawable/ic_launcher_background"/>
    <foreground android:drawable="@drawable/ic_launcher_foreground"/>
</adaptive-icon>
"""
(ANDROID / "mipmap-anydpi-v26").mkdir(parents=True, exist_ok=True)
(ANDROID / "mipmap-anydpi-v26" / "ic_launcher.xml").write_text(adaptive, encoding="utf-8")
(ANDROID / "mipmap-anydpi-v26" / "ic_launcher_round.xml").write_text(adaptive, encoding="utf-8")

# Rounded preview and circular avatar.
rounded_full = rounded(full)
rounded_full.save(ART / "app-icon-rounded-1024.png", optimize=True)
circular(full).resize((512, 512), Image.LANCZOS).save(ART / "avatar-feather-512.png", optimize=True)
save_resized(full, 192, ART / "app-icon-192.png")
save_resized(full, 48, ART / "app-icon-48.png")

# Tauri desktop icon set.
TAURI.mkdir(parents=True, exist_ok=True)
full.save(TAURI / "icon.png", optimize=True)
for name, size in {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "Square30x30Logo.png": 30,
    "Square44x44Logo.png": 44,
    "Square71x71Logo.png": 71,
    "Square89x89Logo.png": 89,
    "Square107x107Logo.png": 107,
    "Square142x142Logo.png": 142,
    "Square150x150Logo.png": 150,
    "Square284x284Logo.png": 284,
    "Square310x310Logo.png": 310,
    "StoreLogo.png": 50,
}.items():
    save_resized(full, size, TAURI / name)

full.save(
    TAURI / "icon.ico",
    format="ICO",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
try:
    full.save(TAURI / "icon.icns", format="ICNS")
except Exception as error:
    print("ICNS skipped:", error)

print("icons generated")
