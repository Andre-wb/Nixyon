# Исходный код

Платформа: **mobile (android)**.

Код лежит в корне репозитория, а не в этой папке: Rust собирается двумя Cargo-воркспейсами,
и их пути заданы в CI, Docker и скриптах.

| Папка | Что там |
|---|---|
| [`nixyon-app`](../nixyon-app) | приложение на Tauri 2: интерфейс `ui`, ядро на Rust `src-tauri/src`, плагин Android на Kotlin и C++ `plugins/android`, проект Gradle `src-tauri/gen/android` |
| [`nixyon-core`](../nixyon-core), [`nixyon-audio`](../nixyon-audio), [`nixyon-cluster`](../nixyon-cluster), [`nixyon-pipeline`](../nixyon-pipeline), [`nixyon-render`](../nixyon-render), [`nixyon-locales`](../nixyon-locales), [`nixyon-ort`](../nixyon-ort) | крейты конвейера, которые входят в приложение |
| [`nixyon-sherpa`](../nixyon-sherpa), [`nixyon-metrics`](../nixyon-metrics), [`nixyon-eval`](../nixyon-eval) | тот же конвейер на компьютере, метрики и замеры |
| [`scripts`](../scripts) | загрузка моделей и библиотек, сборка APK, замеры |
| [`docker`](../docker) | образы для сборки ядра и APK |

Устройство решения: [docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md).

## Установка на телефон

Готовые APK последней сборки: [nixyon.onyxar.org/apk](https://nixyon.onyxar.org/apk), без
входа.

- **Полная** (около 3,7 ГБ): все модели внутри, работает без интернета с первого запуска.
  Для проверки без сети нужна она.
- **Лёгкая** (около 51 МБ): модели скачиваются при первом запуске, дальше работает без
  интернета.

Установка:

1. Скачайте APK на телефон. Android спросит разрешение на установку из браузера или
   файлового менеджера: разрешите.
2. Откройте файл и нажмите «Установить». Для полной сборки на время установки нужно
   свободного места примерно вдвое больше размера APK.
3. С компьютера то же самое: `adb install -r nixyon.apk`.

Требования: Android 7.0 и новее, процессор arm64.

## Как пользоваться

1. **Запись или файл.** На главном экране «Записать» пишет разговор с микрофона, выбор
   файла импортирует запись из памяти телефона (m4a, mp3, wav, ogg, opus, flac).
2. **Участники.** Перед разбором можно указать число участников или оставить «не знаю».
3. **Разбор.** Приложение показывает ход обработки, разбор можно отменить. Всё считается на
   телефоне.
4. **Расшифровка.** Реплики по участникам со временем начала. Нажатие на время реплики или на
   слово включает запись с этого места. Участника можно переименовать или слить с другим, текст реплики
   исправить, реплику удалить или распознать заново.
5. **Экспорт.** Кнопка экспорта сохраняет расшифровку в JSON (формат задания), TXT,
   Markdown, SRT, DOCX или PDF через системный диалог или отправляет через «Поделиться».
   Формат JSON описан в [docs/PROTOCOL.md](../docs/PROTOCOL.md).
6. **Настройки.** Язык речи (Авто, Русский, English), язык интерфейса, тема, горячие слова,
   необязательные модели, вход по отпечатку.

Для русских записей можно выбрать «Язык речи: Русский», тогда язык не определяется по
записи.

## Сборка из исходников

Нужны Rust 1.95, JDK 17, Android SDK с NDK 30.0.16248370 и CMake 4.1.2, цель
`aarch64-linux-android` и tauri-cli 2.12.4. Полный список: раздел «Требования» корневого
[README](../README.md).

```sh
scripts/fetch-sherpa.sh       # библиотеки sherpa-onnx и ONNX Runtime
scripts/fetch-models.sh       # модели
scripts/fetch-llama-cpp.sh    # исходники llama.cpp
scripts/build-demo-apk.sh     # полная и лёгкая APK
```

Полная APK: `nixyon-app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`.
Лёгкая: `nixyon-app/src-tauri/gen/android/app/build/outputs/nixyon-lite.apk`.

Переменные окружения скриптов: [config/example.env](../config/example.env).

Расшифровка той же цепочкой на компьютере, без телефона:

```sh
cargo run --release -p nixyon-sherpa --bin transcribe -- models models/example.wav out.json
```
