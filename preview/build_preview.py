"""preview/index.html из рабочей разметки интерфейса.

Страница дизайн-превью — это та же разметка, что и в приложении, с двумя правками:
стили и модули грузятся из `nixyon-app/ui`, а перед `app.js` подключается `mock.js`
с подменой Tauri-команд. Так превью не расходится с приложением.

    python preview/build_preview.py
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
UI = ROOT / "nixyon-app" / "ui"

CSS = '<link rel="stylesheet" href="styles.css">'
CSS_PREVIEW = '<link rel="stylesheet" href="../nixyon-app/ui/styles.css">'
BOOT = '<script src="js/boot.js"></script>'
BOOT_PREVIEW = '<script src="../nixyon-app/ui/js/boot.js"></script>'
APP = '<script src="js/app.js" type="module"></script>'
APP_PREVIEW = (
    '<script src="./mock.js"></script>\n'
    '  <script src="../nixyon-app/ui/js/app.js" type="module"></script>'
)


def main() -> int:
    source = (UI / "index.html").read_text(encoding="utf-8")
    for marker in (CSS, BOOT, APP):
        if marker not in source:
            print(f"в ui/index.html нет {marker}", file=sys.stderr)
            return 1
    source = source.replace(CSS, CSS_PREVIEW).replace(BOOT, BOOT_PREVIEW).replace(APP, APP_PREVIEW)
    (ROOT / "preview" / "index.html").write_text(source, encoding="utf-8", newline="\n")
    print("preview/index.html обновлён из ui/index.html")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
