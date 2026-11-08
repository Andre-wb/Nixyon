# Local design preview

This folder is not part of the Tauri bundle. It loads the real `nixyon-app/ui/styles.css`
and JS modules with a copy of the screen markup and a mocked Tauri API, so the complete
design can be reviewed in a desktop browser, and builds the design-preview APK.

```powershell
# from the repository root
python -m http.server 8123 --bind 127.0.0.1
# open http://127.0.0.1:8123/preview/index.html
```

## Screens

- `preview/index.html` — первый запуск: мини-обучение и «Продолжить», как в APK;
- `preview/index.html?screen=onboarding` — тот же экран принудительно;
- `preview/index.html?screen=home` — главный экран с архивом встреч;
- `preview/index.html?screen=search` — поиск по всем встречам с подсветкой;
- `preview/index.html?screen=transcript` — расшифровка: участники, таймлайн, реплики;
- `preview/index.html?screen=player` — плеер: скорость, участник, пропуск тишины;
- `preview/index.html?screen=voices`, `settings`, `models`, `export`, `stats`,
  `speaker`, `summary`, `labels`, `fillers`, `emotion`, `turn`, `check`, `processing`;
- `preview/index.html?lang=en&screen=home` — English copy pass;
- `preview/icon.html` — 1024 px источник знака и иконки;
- `preview/board.html` — статический дизайн-борд по `artifacts/`.

## Tools

- `python preview/build_preview.py` — `preview/index.html` из `nixyon-app/ui/index.html`;
- `python preview/build_i18n.py` — `ui/generated/i18n.js` из `ui/locales`; `--check`
  сверяет файл и проверяет ключи из кода (то же делает `src-tauri/build.rs`);
- `python preview/make_icons.py` — иконки Android (mipmap, adaptive) и набор Tauri;
- `node preview/capture-icon.mjs` — рендер слоёв иконки;
- `node preview/snap.mjs <url> <out.png> [w] [h] [full]` — скриншот любой страницы;
- `node preview/audit.mjs [width]` — все экраны на нескольких ширинах: ошибки консоли,
  горизонтальный скролл, маленькие цели нажатия;
- `node preview/smoke.mjs` — функциональный прогон: 17 сценариев от онбординга до
  загрузки моделей.

Screenshot and audit helpers expect a Chromium with the DevTools protocol. Start it with
`--remote-debugging-port=9223` (or set `CDP_PORT`) and leave `BASE` default. They are
development tools only and are not referenced by the app.
