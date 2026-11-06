# Nixyon

Nixyon расшифровывает записи встреч на русском языке прямо на телефоне, без облака и без
интернета. Запись, распознавание, деление на спикеров, протокол встречи и экспорт работают
на устройстве; звук и текст никуда не отправляются.

Для кого: для тех, кто записывает рабочие встречи, интервью и созвоны и хочет получить
текст по репликам с именами участников, не выкладывая разговор во внешний сервис.

Приложение целиком работает на Android. На ноутбуке у приложения движка нет: там
расшифровку запускают утилиты `transcribe` и `eval` из этого репозитория, а десктопная
сборка приложения нужна для проверок (clippy, тесты) и работы над интерфейсом.

Лицензия проекта не выбрана.

## Содержание

- [Возможности](#возможности)
- [Как устроен конвейер](#как-устроен-конвейер)
- [Архитектура](#архитектура)
- [Хранение и шифрование](#хранение-и-шифрование)
- [Модели](#модели)
- [Требования](#требования)
- [Установка и сборка](#установка-и-сборка)
- [Docker](#docker)
- [Проверки](#проверки)
- [CI](#ci)
- [Замер качества](#замер-качества)
- [Локализация](#локализация)
- [Лицензии моделей и данных](#лицензии-моделей-и-данных)
- [Структура репозитория](#структура-репозитория)
- [Частые проблемы](#частые-проблемы)
- [Nixyon Hub](#nixyon-hub)

## Возможности

- **Запись с микрофона.** Запись идёт в фоновой службе с уведомлением, до 3 часов, на диск
  сразу пишется зашифрованный WAV. Нужно не меньше 200 МБ свободного места.
- **Импорт записи.** Системный выбор файла; Android раскодирует m4a, mp3, wav, ogg, opus
  и flac. Файл длиннее 3 часов не принимается.
- **Живой текст во время записи.** Черновой текст от потоковой модели появляется, пока идёт
  запись. Набор моделей без потоковой модели тоже допустим, тогда живого текста нет.
- **Подготовка звука.** Любой WAV переводится в 16 кГц моно с выравниванием громкости.
  Запись короче секунды и тишина не обрабатываются; о клиппинге приложение предупреждает,
  но запись обрабатывает.
- **VAD.** Silero VAD нарезает речь на куски.
- **Распознавание.** GigaAM v3 с пунктуацией сразу даёт знаки препинания, заглавные буквы и
  числа цифрами. Есть горячие слова: имена из библиотеки голосов и список из настроек.
- **Деление на спикеров.** Нейросегментация pyannote показывает, кто и когда говорит, и
  отмечает наложение голосов. Эмбеддинги голоса на телефоне считает ReDimNet2-B6. Число
  участников можно задать или оставить «не знаю».
- **Имена по образцам голоса.** Пользователь записывает образец голоса человека, и в новых
  встречах спикер получает его имя. После встречи голос спикера можно «запомнить»: центр его
  голоса добавляется к человеку в библиотеке. У образца и у центра хранится имя модели
  голоса, и в расшифровке участвуют только те, что посчитаны той же моделью.
- **Правка.** Переименовать спикера, слить двух спикеров в одного, исправить текст реплики.
- **Поиск** по расшифровке: без учёта регистра и разницы «е» и «ё», с основами слов и
  допуском опечаток.
- **Статистика встречи:** доля речи, слова в минуту, перебивания, монологи дольше 2 минут.
- **Протокол встречи.** Языковая модель на телефоне пишет краткое содержание, решения и
  задачи с исполнителями и сроками.
- **Экспорт:** TXT, Markdown, SRT, JSON, DOCX и PDF. Файл можно сохранить через системный
  диалог или отправить через «Поделиться».
- **Плеер:** исходная запись проигрывается с любого места.
- **Языки интерфейса:** русский и английский, переключатель «Авто / Русский / English».
  Темы: «Авто / Светлая / Тёмная».
- **Шифрование данных** на устройстве, ключ хранится в Android Keystore.

## Как устроен конвейер

На Android тяжёлую работу с моделями делает Kotlin-плагин через sherpa-onnx, остальное
делает Rust. Вход в конвейер: команда `transcribe` в
[`nixyon-app/src-tauri/src/transcription.rs`](nixyon-app/src-tauri/src/transcription.rs).

1. **Запись или импорт.** Запись пишет
   [`AudioRecorder.kt`](nixyon-app/plugins/android/android/src/main/java/AudioRecorder.kt) в
   зашифрованный WAV PCM16. Импорт раскодирует файл средствами Android:
   [`AudioImporter.kt`](nixyon-app/plugins/android/android/src/main/java/AudioImporter.kt).
2. **Проверка пути.** Обрабатываются только файлы из папок приложения (files, cache,
   external files), ссылки раскрываются:
   [`allowed_path.rs`](nixyon-app/src-tauri/src/transcription/allowed_path.rs).
3. **Перевод в 16 кГц моно.** `convert_reader_to_wav16k` в
   [`nixyon-audio/src/convert.rs`](nixyon-audio/src/convert.rs) читает запись потоком в два
   прохода. Первый проход меряет уровни по окнам, второй приводит громкость к -20 dBFS
   (усиление от -10 до +30 дБ). Заодно `QualityMeter` проверяет запись: короче 1 с, тише
   -50 dBFS, клиппинг.
4. **Горячие слова.** Имена голосов из библиотеки и список из настроек склеиваются в одну
   строку (`hotwords_line` в [`nixyon-core/src/hotwords_line.rs`](nixyon-core/src/hotwords_line.rs)).
5. **Анализ звука на устройстве.**
   [`SpeechAnalyzer.kt`](nixyon-app/plugins/android/android/src/main/java/SpeechAnalyzer.kt):
   - сначала Silero VAD режет речь на куски до 20 с с запасом 0,2 с с каждой стороны;
     запись до 20 с, где VAD речи не нашёл, идёт целиком;
   - каждый кусок распознаёт GigaAM (поиск по лучам с горячими словами);
   - эмбеддинг голоса считается по каждому куску. Модель голоса ReDimNet2-B6 работает в
     Rust (крейт [`nixyon-ort`](nixyon-ort/src/lib.rs)), Kotlin зовёт её через JNI
     ([`VoiceModel.kt`](nixyon-app/plugins/android/android/src/main/java/VoiceModel.kt),
     [`voice_model.rs`](nixyon-app/plugins/android/src/voice_model.rs)). Какая модель и с
     каким порогом слияния групп работает, задаёт
     [`VoiceChoice.kt`](nixyon-app/plugins/android/android/src/main/java/VoiceChoice.kt);
     CAM++ загружается только потому, что его требует диаризатор sherpa-onnx;
   - [`SpeechSegmenter.kt`](nixyon-app/plugins/android/android/src/main/java/SpeechSegmenter.kt)
     прогоняет нейросегментацию pyannote блоками по 60 с. Отрезки обрезаются по кускам VAD,
     номера говорящих sherpa отбрасываются, эмбеддинг считается по каждому отрезку;
   - [`PhaseTiming.kt`](nixyon-app/plugins/android/android/src/main/java/PhaseTiming.kt)
     пишет в лог (тег `NixyonTiming`) длительность фаз и время модели голоса, без звука и
     текста.

   Результаты пишутся во временные JSON-файлы и удаляются при любом исходе.
6. **Сборка расшифровки.** `build_transcript_with_centers` в
   [`nixyon-pipeline/src/build_transcript.rs`](nixyon-pipeline/src/build_transcript.rs):
   - слова и их время из токенов ([`words.rs`](nixyon-pipeline/src/words.rs));
   - спикеры по отрезкам нейросегментации и кластеризации эмбеддингов
     ([`neural_turns.rs`](nixyon-pipeline/src/neural_turns.rs),
     [`neural_labels.rs`](nixyon-pipeline/src/neural_labels.rs),
     [`word_speakers.rs`](nixyon-pipeline/src/word_speakers.rs)); говорящий слова берётся
     по середине слова, наложение голосов помечается флагом `overlap`;
   - куски с пустым текстом отбрасываются; если текста нет совсем, это ошибка;
   - кусок VAD, покрытый отрезками нейросегментации меньше чем наполовину, кластеризуется
     своим эмбеддингом; в кластеризацию AHC идут куски от 1,5 с, короткие присоединяются к
     ближайшему центру;
   - при неизвестном числе спикеров (не больше 8) оно оценивается по eigengap
     ([`nixyon-cluster/src/speaker_count.rs`](nixyon-cluster/src/speaker_count.rs)), группы с
     близкими центрами сливаются. Порог косинуса приходит с устройства вместе с моделью
     голоса: 0,7 для ReDimNet2-B6. На ноутбуке с CAM++ по умолчанию порог 0,6;
   - соседние реплики одного спикера с паузой до 1 с склеиваются
     ([`turns/glue.rs`](nixyon-pipeline/src/turns/glue.rs)); реплика с наложением голосов
     остаётся отдельной;
   - имена по образцам голоса и центрам прошлых встреч: сопоставление один к одному с
     наибольшей суммой косинусов, порог 0,55; человек с несколькими эмбеддингами
     сравнивается по наибольшему косинусу
     ([`sample_names.rs`](nixyon-pipeline/src/sample_names.rs));
   - статистика спикеров ([`stats.rs`](nixyon-pipeline/src/stats.rs)).
7. **Сохранение.** Расшифровка и центры голосов сохраняются в зашифрованные файлы, протокол
   прошлой встречи стирается, путь к исходной записи для плеера пишется в `last_audio.txt`.
8. **Дальше по запросу пользователя:** правка
   ([`nixyon-pipeline/src/edit/`](nixyon-pipeline/src/edit)), поиск
   ([`search.rs`](nixyon-pipeline/src/search.rs)), статистика
   ([`meeting_stats.rs`](nixyon-pipeline/src/meeting_stats.rs)), экспорт
   ([`nixyon-render`](nixyon-render/src/lib.rs); PDF верстает Kotlin в
   [`ProtocolPdf.kt`](nixyon-app/plugins/android/android/src/main/java/ProtocolPdf.kt) по
   блокам протокола из Rust), протокол встречи
   ([`summary.rs`](nixyon-app/src-tauri/src/summary.rs)).

Без нейросегментации у конвейера есть запасная ветка: спикеры по кускам VAD с
кластеризацией AHC и разрезом кусков по словам там, где окна эмбеддингов (1,5 с с шагом
0,5 с) внутри куска разных говорящих
([`speaker_labels.rs`](nixyon-pipeline/src/speaker_labels.rs),
[`word_cut.rs`](nixyon-pipeline/src/word_cut.rs)). Её включает флаг `--no-neural` у `eval`.
Пересегментация Витерби ([`resegment.rs`](nixyon-pipeline/src/resegment.rs)) выключена по
умолчанию и включается отдельным флагом `--viterbi`. В приложении окна не запрашиваются:
работает нейросегментация.

На ноутбуке те же шаги идут через крейт `nixyon-sherpa`: он реализует трейты движка
`SpeechEngine` и `VoiceSegmenter` из `nixyon-core` поверх Rust-обёртки sherpa-onnx. Отличия
от телефона:

- модель голоса по умолчанию CAM++; ReDimNet2 и другие модели из набора `models/ort/`
  выбираются флагом `--voice-model` у `eval` и `quality_eval`;
- есть фильтр музыки: модель CED tiny распознаёт классы звука AudioSet, и кусок
  отбрасывается, если музыка уверенная и сильнее речи
  ([`music_check.rs`](nixyon-sherpa/src/music_check.rs)). По умолчанию выключен,
  включается `quality_eval --drop-music`. На телефоне этого фильтра нет.

### Формат расшифровки

Источник истины: JSON `Transcript` из
[`nixyon-core/src/transcript.rs`](nixyon-core/src/transcript.rs), версия формата 1. Файл
другой версии не читается. Из него строятся все экспорты. Пример полей (сокращённо, по
[`nixyon-render/tests/golden/sample.json`](nixyon-render/tests/golden/sample.json)):

```json
{
  "schema_version": 1,
  "created_at": "2026-10-28T11:30:00Z",
  "audio": { "file": "meeting.wav", "duration_sec": 3725.4, "sample_rate": 16000 },
  "engine": { "vad": "silero_vad", "asr": "...", "embedding": "..." },
  "speakers": [
    { "id": 1, "name": null, "talk_time_sec": 14.0, "turns": 2, "words": 30 }
  ],
  "turns": [
    {
      "id": 1, "speaker": 1, "start": 5.2, "end": 7.0,
      "text": "Добрый день, давайте начнём.",
      "words": [{ "text": "Добрый", "start": 5.2, "end": 5.6 }],
      "flags": []
    }
  ]
}
```

- `words` бывает `null`, если движок не дал время слов;
- `flags` у реплики: `overlap` (наложение голосов) и `clipping`;
- `age` у спикера: `child` или `adult` по голосу (модель audEERING и классификатор из
  [`nixyon-ort/data`](nixyon-ort/data)); поля нет, если модели в наборе нет или она не
  уверена. При слиянии спикеров с разными метками метка пропадает;
- `gender` у спикера: `female` или `male` по оценке пола той же модели, у детей тоже (у
  мальчиков модель часто ошибается); при слиянии ведёт себя так же, как `age`;
- в `engine.embedding` приложение пишет имя файла модели голоса из манифеста, в `engine.asr`
  версию набора моделей.

В JSON хранится номер спикера. Подпись «Спикер N» или «Speaker N» подставляют рендеры
на языке интерфейса, если у спикера нет имени.

## Архитектура

Два Cargo-воркспейса. Корневой ([`Cargo.toml`](Cargo.toml)) содержит ядро и утилиты.
Приложение ([`nixyon-app/Cargo.toml`](nixyon-app/Cargo.toml)) исключено из корневого и
собирается отдельно, в том числе под Android. Edition 2024, MSRV 1.95, все версии
зависимостей заданы только в корневых `Cargo.toml` воркспейсов. `unsafe_code` запрещён
(`deny`), исключение одно: JNI-граница моделей в плагине Android
([`jni_support.rs`](nixyon-app/plugins/android/src/jni_support.rs),
[`voice_model/jni_exports.rs`](nixyon-app/plugins/android/src/voice_model/jni_exports.rs),
[`age_model/jni_exports.rs`](nixyon-app/plugins/android/src/age_model/jni_exports.rs)),
где каждый блок `unsafe` объяснён комментарием `SAFETY:`. Clippy запрещает `unsafe` без
такого комментария (`undocumented_unsafe_blocks`).

### Крейты

| Крейт | Назначение | Ключевые типы и функции | Зависит от |
|---|---|---|---|
| `nixyon-core` | Типы расшифровки, JSON, граница с движком и моделями | `Transcript`, `Speaker`, `Turn`, `Word`, `SegmentResult`, `VoiceSpan`, `VoiceSample`, `EngineOptions`, трейты `SpeechEngine`, `VoiceSegmenter`, `VoiceEmbedder` и `BlockSegmenter`, `Pool`, `par_map`, `to_json`, `from_json` | serde, serde_json, thiserror |
| `nixyon-audio` | WAV, ресемплинг, громкость, проверка качества, всё потоком | `convert_to_wav16k`, `convert_reader_to_wav16k`, `WavReader`, `WavWriter`, `Resampler16k`, `QualityMeter` | rubato, thiserror |
| `nixyon-cluster` | Математика спикеров | `ahc`, `estimate_speaker_count`, `centroids`, `nearest`, `viterbi`, `min_cost_assignment` (венгерский алгоритм), `cosine` | nalgebra |
| `nixyon-pipeline` | Сборка расшифровки из результатов движка, правка, поиск, статистика | `build_transcript`, `build_transcript_with_centers`, `PipelineOptions`, `SpeakerCount`, `rename_speaker`, `merge_speakers`, `edit_turn_text`, `search`, `meeting_stats` | core, cluster, rust-stemmers, thiserror |
| `nixyon-render` | Экспорт TXT, Markdown, SRT, JSON, DOCX и блоки протокола для PDF | `render_txt`, `render_markdown`, `render_srt`, `render_json`, `render_docx`, `protocol_blocks`, `Labels` | core, serde, zip |
| `nixyon-locales` | Словари интерфейса, их проверка, подписи экспорта, генератор JS-модуля ключей | `check`, `generate`, `used_keys`, `export_labels`, `Lang` | render, serde, serde_json, thiserror |
| `nixyon-sherpa` | Движок на sherpa-onnx для ноутбука, потоковая модель, шумоподавление GTCRN, фильтр музыки, утилиты | `SherpaEngine`, `SherpaModels`, `ModelManifest`, `Decoding`, `LiveText`, `StreamModel`, `analyze_default`, `denoise_wav`; бинарники `transcribe`, `make_test_dialogs`, `make_noisy_dialogs` | core, audio, pipeline, sherpa-onnx, serde, serde_json, thiserror |
| `nixyon-ort` | Модели, которые sherpa-onnx не загружает, на крейте ort: модель голоса ReDimNet2 и нейросегментация Sortformer. ONNX Runtime общий с sherpa-onnx | `Redimnet2` (реализует `VoiceEmbedder`), `Sortformer` (реализует `BlockSegmenter`), `Binarize`, `OrtManifest`, `OrtRole`, `OrtError` | core, ort, realfft, serde, serde_json, thiserror |
| `nixyon-metrics` | Метрики качества | `word_errors`, `word_errors_with_digits`, `word_errors_lenient`, `char_errors`, `cp_word_errors`, `diarization_errors`, `overlap_detection`, `naming_score`, `numbers_to_digits` | cluster, thiserror |
| `nixyon-eval` | Прогоны на наборах записей и отчёты | бинарники `eval`, `stream_eval`, `quality_eval` | core, audio, pipeline, sherpa, ort, metrics, sherpa-onnx, symphonia, serde, serde_json, thiserror |

Граф зависимостей между крейтами:

- `nixyon-core` и `nixyon-audio` ни от кого не зависят, `nixyon-cluster` только от nalgebra;
- `nixyon-pipeline` зависит от core и cluster;
- `nixyon-render` от core, `nixyon-locales` от render;
- `nixyon-metrics` от cluster;
- `nixyon-sherpa` от core, audio и pipeline;
- `nixyon-ort` от core;
- `nixyon-eval` от core, audio, pipeline, sherpa, ort и metrics;
- приложение берёт core, audio, pipeline, render и locales; плагин Android под Android
  берёт ещё `nixyon-ort` и `jni` для модели голоса. sherpa-onnx на устройстве вызывается
  из Kotlin.

### Приложение

[`nixyon-app`](nixyon-app) построено на Tauri 2 (`tauri` 2.12.5), идентификатор
`dev.demo.nixyon`, имя Nixyon.

- [`src-tauri`](nixyon-app/src-tauri/src): Rust-часть приложения. Команды Tauri
  регистрируются в [`lib.rs`](nixyon-app/src-tauri/src/lib.rs):
  - расшифровка: `transcribe`, `cancel_transcription`, `last_transcript`;
  - запись и импорт: `start_recording`, `stop_recording`, `import_audio`, `cancel_import`,
    `open_app_settings`;
  - правка: `rename_speaker`, `merge_speakers`, `edit_turn_text`;
  - статистика и поиск: `meeting_stats`, `search_transcript`;
  - плеер: `open_playback`, `play_audio`, `pause_audio`, `close_playback`;
  - голоса: `list_voices`, `add_voice`, `delete_voice`, `remember_voice`,
    `discard_voice_recording`;
  - настройки: `get_settings`, `set_language`, `set_theme`, `set_system_bars`, `set_hotwords`;
  - экспорт: `export_transcript`, `save_export`, `share_export`;
  - протокол: `summary_available`, `summarize_meeting`, `cancel_summary`, `last_summary`.
- [`plugins/android`](nixyon-app/plugins/android): плагин `nixyon-android`. Rust-обёртки в
  [`src/mobile.rs`](nixyon-app/plugins/android/src/mobile.rs) вызывают Kotlin-класс
  `dev.demo.nixyon.speech.SpeechPlugin`. На ноутбуке
  [`src/desktop.rs`](nixyon-app/plugins/android/src/desktop.rs) отвечает на всё ошибкой
  `Unsupported`. Типы ответов плагина и чтение временных JSON лежат в `models.rs`,
  `segments.rs`, `spans.rs`. [`src/voice_model.rs`](nixyon-app/plugins/android/src/voice_model.rs)
  и папка `voice_model/` дают Kotlin модель голоса через JNI: модель читается из файла или
  несжатой прямо из APK по дескриптору, смещению и длине. Kotlin-код в
  [`android/src/main/java`](nixyon-app/plugins/android/android/src/main/java):
  - запись и импорт: `AudioRecorder`, `RecordingService`, `RecordingTarget`, `RecordingsDir`,
    `SamplesDir`, `AudioImporter`, `LevelMeter`, `WavPcm16Writer`, `WavPcm16Reader`;
  - анализ: `SpeechEngine`, `SpeechModels`, `SpeechAnalyzer`, `SpeechSegmenter`,
    `LiveTranscriber`, `SampleEmbedder`, `PaddedBounds`, `WindowSpans`, `SegmentsWriter`,
    `SpansWriter`, `PhaseTiming`;
  - модель голоса: `VoiceModel`, `VoiceModelNative` (JNI к библиотеке `nixyon_lib`),
    `VoiceChoice`;
  - модели: `ModelLocator`, `ModelManifest`, `ModelRole`, `ModelSource`, `ModelVerifier`,
    `Sha256`;
  - языковая модель: `MeetingSummarizer`, `LlmLocator`, `LlmManifest`, `LlmMemory`,
    `LlmProgress`, `MeetingLlmNative` и JNI-обёртка над llama.cpp в
    [`cpp/meeting_llm.cpp`](nixyon-app/plugins/android/android/src/main/cpp/meeting_llm.cpp);
  - шифрование: `StorageKey`, `SealedFormat`, `SealedOutput`, `SealedMediaSource`;
  - PDF и плеер: `ProtocolPdf`, `ProtocolBlock`, `AudioPlayer`.
- [`ui`](nixyon-app/ui): интерфейс на HTML, CSS и JS без сборщика. Модули в
  [`ui/js`](nixyon-app/ui/js): экраны, главный экран с записью, обработка, живой текст,
  расшифровка, лента реплик, плеер, поиск, голоса, статистика, протокол, экспорт,
  настройки, тема, i18n. Словари в [`ui/locales`](nixyon-app/ui/locales).
- [`src-tauri/gen/android`](nixyon-app/src-tauri/gen/android): Gradle-проект Android.
  minSdk 24, targetSdk и compileSdk 37, только `arm64-v8a`, NDK 30.0.16248370.

Приложение на устройстве:

- разрешения плагина: `RECORD_AUDIO`, `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MICROPHONE`,
  `POST_NOTIFICATIONS`; в манифесте приложения есть ещё `INTERNET`;
- файлы экспорта отдаются наружу через `FileProvider` из манифеста приложения, открыта
  только папка `cache/exports`;
- приложение не участвует в резервном копировании (`allowBackup="false"`).

### Протокол встречи

- Модель: Qwen3.5-4B в квантовании Q4_K_M (GGUF, около 2,7 ГБ), движок llama.cpp тега
  `b12005`. Gradle собирает его через CMake в библиотеку `nixyon_llm` вместе с плагином.
- Модель загружается на один запрос и выгружается. Контекст 4096 токенов, не больше
  4 потоков, ответ до 1024 токенов.
- Перед запуском проверяется память: нужен запас 768 МБ сверх размера модели, и система не
  должна быть в состоянии нехватки памяти (`lowMemory`). Иначе модель не запускается.
- Где лежит модель:
  - в `llm/` внешней папки приложения, вместе с `llm.json`. Размер проверяется при каждом
    запуске, SHA-256 только когда меняются версия, путь, сумма, размер или время изменения
    файла;
  - или несжатой внутри APK; тогда проверяется только размер.
- Подсказка модели написана по-русски при любом языке интерфейса: только то, что есть в
  расшифровке, в виде «Краткое содержание», «Решения», «Задачи».
- Протокол привязан к своей расшифровке. Если расшифровку заменили, пока модель писала
  ответ, протокол не сохраняется.

## Хранение и шифрование

Хранилище описано в [`nixyon-app/src-tauri/src/vault`](nixyon-app/src-tauri/src/vault).

- **Шифр:** AES-256-GCM. Nonce 12 байт, новый на каждый кусок; тег 16 байт.
- **Формат файла** `NXSEAL01`:
  - заголовок 32 байта со случайным `file_id`, затем куски по 64 КиБ;
  - в AAD каждого куска входят заголовок, номер куска и признак последнего куска. Поэтому
    куски нельзя переставить, обрезать файл или перенести кусок в другой файл;
  - формат общий для Rust ([`format.rs`](nixyon-app/src-tauri/src/vault/format.rs)) и Kotlin
    ([`SealedFormat.kt`](nixyon-app/plugins/android/android/src/main/java/SealedFormat.kt)).
- **Ключ данных:** 32 байта, обёрнуты ключом Android Keystore (псевдоним `nixyon-storage`).
  Обёрнутый ключ лежит в `noBackupFilesDir/storage-key.bin`. Если ключ Keystore пропал,
  создаётся новый ключ данных, а файлы под старым ключом удаляются при запуске.
- **Что шифруется:** последняя расшифровка, протокол, центры голосов, библиотека голосов
  (`voices.json`), настройки (`settings.json`), записи в `files/recordings` и образцы голоса
  в `files/samples`. Плеер читает
  зашифрованную запись с любого места, не раскрывая её на диск. Запись файлов атомарная,
  через `.part` и переименование.
- **Что не шифруется:**
  - файлы экспорта в `cache/exports`: их стирает следующий экспорт и запуск приложения;
  - временные файлы расшифровки в `cache`: открытая копия записи в 16 кГц
    (`nixyon-<время>.wav`) и результаты анализа (`.segments.json`, `.spans.json`). Они
    удаляются после расшифровки при любом исходе и при запуске;
  - `last_audio.txt`: путь к последней записи;
  - копия выбранной темы и последний формат экспорта в `localStorage` WebView.
- **При запуске** приложение чистит временные файлы (`cache/nixyon-*`, `cache/exports`,
  `files/samples`, недописанные `.part` личных файлов и записей). Открытые файлы от прежних версий оно шифрует на
  месте.

Записи разговоров, расшифровки и образцы голоса в логи не пишутся. Поисковый запрос тоже
не пишется.

## Модели

Модели скачиваются скриптами, сверяются по SHA-256 и в git не попадают (`/models/` и
`/third_party/` в [`.gitignore`](.gitignore)). Скачанное хранится в `models/_download/`
(набор `models/ort/` сразу в своей папке) и повторно не качается.

### Распознавание: `scripts/fetch-models.sh`

[`scripts/fetch-models.sh`](scripts/fetch-models.sh) качает модели из релизов
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx/releases), с Hugging Face и из
приватного релиза GitHub (см. [Набор `models/ort/`](#набор-modelsort)) и раскладывает
основной набор в `models/` под постоянными именами:

| Роль | Файл в `models/` | Источник |
|---|---|---|
| `vad` | `vad.onnx` | `asr-models/silero_vad.onnx` |
| `asr_encoder`, `asr_decoder`, `asr_joiner`, `asr_tokens` | `asr-encoder.int8.onnx`, `asr-decoder.onnx`, `asr-joiner.onnx`, `asr-tokens.txt` | `asr-models/sherpa-onnx-nemo-transducer-punct-giga-am-v3-russian-2025-12-16.tar.bz2` |
| `embedding` | `embedding.onnx` | `speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx` |
| `segmentation` | `segmentation.onnx` | `speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2` (float32) |
| `asr_bpe_vocab` | `asr-bpe.vocab` | строится из токенизатора GigaAM v3 ([ai-sage/GigaAM-v3](https://huggingface.co/ai-sage/GigaAM-v3), ревизия закреплена) |
| `live_encoder`, `live_decoder`, `live_joiner`, `live_tokens` | `live-encoder.int8.onnx`, `live-decoder.onnx`, `live-joiner.int8.onnx`, `live-tokens.txt` | `asr-models/sherpa-onnx-streaming-zipformer-small-ru-vosk-int8-2025-08-16.tar.bz2` |
| `tagging_model`, `tagging_labels` | `tagging.int8.onnx`, `tagging-labels.csv` | `audio-tagging-models/sherpa-onnx-ced-tiny-audio-tagging-2024-04-19.tar.bz2` |
| `redimnet2` | `redimnet2-b6.onnx` | жёсткая ссылка (или копия) на `models/ort/redimnet2-b6.onnx` |
| `denoiser` | `denoiser.onnx` | `speech-enhancement-models/gtcrn_simple.onnx`: шумоподавление GTCRN для второго VAD, которым режутся куски VAD от 15 с |

- Словарь BPE строит [`scripts/spm-to-bpe-vocab.py`](scripts/spm-to-bpe-vocab.py) из
  `tokenizer.model` без пакета sentencepiece. Скрипт отказывается работать, если части словаря
  не совпадают с `tokens.txt` модели. По этому словарю sherpa-onnx разбивает горячие слова
  на токены.
- В `models/` кладётся и `example.wav` из архива GigaAM: он нужен для проверок, в манифест не
  входит.
- Скрипт пишет манифест `models/models.json`: версию набора и для каждой роли файл, размер и
  SHA-256. Если версия и суммы совпадают, большой архив заново не распаковывается.
- Роли на ноутбуке ([`nixyon-sherpa/src/manifest.rs`](nixyon-sherpa/src/manifest.rs)):
  обязательные `vad`, `asr_encoder`, `asr_decoder`, `asr_joiner`, `asr_tokens`,
  `embedding`; необязательные `segmentation` и `asr_bpe_vocab`; потоковая модель задаётся
  четырьмя ролями `live_*`, а фильтр музыки двумя ролями `tagging_*`, целиком или не
  задаётся вовсе. Роль `redimnet2` ноутбук не читает. Путь роли: простое имя файла без
  папок.
- Роли на телефоне ([`ModelRole.kt`](nixyon-app/plugins/android/android/src/main/java/ModelRole.kt)):
  обязательны те же шесть и ещё `redimnet2`, `segmentation` и `asr_bpe_vocab`;
  необязательны только четыре роли `live_*`, все вместе. Роли `tagging_*` телефон не
  читает.

Как приложение находит модели
([`ModelLocator.kt`](nixyon-app/plugins/android/android/src/main/java/ModelLocator.kt)):

- если во внешней папке приложения есть `models/models.json`, модели берутся оттуда. Размеры
  файлов сверяются при каждом запуске, SHA-256 только когда меняются версия набора, суммы,
  размеры или время изменения файлов. Тихо переключиться на модели из APK приложение не
  может;
- иначе модели берутся из APK, по манифесту внутри APK, без сверки файлов.

### Набор `models/ort/`

Модели, которые sherpa-onnx не загружает, работают на крейте `nixyon-ort`. `fetch-models.sh`
качает их в `models/ort/` и пишет там свой `models.json` со своей версией. Все роли набора
необязательные ([`nixyon-ort/src/manifest.rs`](nixyon-ort/src/manifest.rs)).

| Роль | Файл в `models/ort/` | Источник |
|---|---|---|
| `redimnet2` | `redimnet2-b6.onnx` | ReDimNet2-B6 в выгрузке [soniqo/ReDimNet2-B6-ONNX-FP32](https://huggingface.co/soniqo/ReDimNet2-B6-ONNX-FP32), вход ровно 6 с |
| `redimnet2_b0`, `redimnet2_b1`, `redimnet2_b2`, `redimnet2_b3`, `redimnet2_b3_vb2`, `redimnet2_b6_dyn` | `redimnet2-b0.onnx` и т. д. | выгрузки весов [PalabraAI/redimnet2](https://github.com/PalabraAI/redimnet2) v1.0.0 с входом любой длины, из релиза [`models-redimnet2-v1`](https://github.com/Andre-wb/Nixyon/releases/tag/models-redimnet2-v1) этого репозитория (MIT) |
| `sortformer_int8`, `sortformer_int8_data` | `sortformer.onnx`, `sortformer.data` | [DimQ1/sortformer-4spk-v2-onnx-int8-cpu](https://huggingface.co/DimQ1/sortformer-4spk-v2-onnx-int8-cpu) |
| `sortformer_fp32` | `sortformer-fp32.onnx` | [DimQ1/sortformer-4spk-v2-onnx-fp32-cpu](https://huggingface.co/DimQ1/sortformer-4spk-v2-onnx-fp32-cpu), 492 МБ, только при `NIXYON_SORTFORMER_FP32=1` |

- Ревизии на Hugging Face и релиз GitHub закреплены в скрипте вместе с SHA-256 каждого файла.
- В основной набор и в APK из `models/ort/` попадает только `redimnet2-b6.onnx`: это модель
  голоса приложения на Android. Остальное нужно для замеров (`eval`, `quality_eval`) и
  тестов с моделями.
- Выгрузки PalabraAI делает вручную [`scripts/export-redimnet2.py`](scripts/export-redimnet2.py)
  во временной среде `uv` (torch, onnx); в проект и в CI Python-пакеты не входят. Скрипт
  сверяет веса по SHA-256 и сравнивает выход ONNX Runtime с PyTorch.

### Языковая модель: `scripts/fetch-llm.sh`

[`scripts/fetch-llm.sh`](scripts/fetch-llm.sh) качает `Qwen3.5-4B-Q4_K_M.gguf` из
[unsloth/Qwen3.5-4B-GGUF](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF) с закреплённой
ревизией. Он кладёт модель в `models/llm/qwen3.5-4b-q4_k_m.gguf` (жёсткой ссылкой, если можно)
и пишет манифест `models/llm/llm.json`. В `models.json` эта модель не входит.

### Библиотеки

- [`scripts/fetch-sherpa.sh`](scripts/fetch-sherpa.sh) качает готовые библиотеки
  sherpa-onnx v2.0.1 в `third_party/sherpa-onnx/`:
  - `sherpa-onnx-2.0.1.aar` для Android;
  - статические библиотеки для `osx-arm64`, `osx-x64`, `linux-aarch64` и `linux-x64`.

  Крейт `sherpa-onnx` берёт их из этой папки и в сеть не ходит: путь задаёт
  `SHERPA_ONNX_ARCHIVE_DIR` в [`.cargo/config.toml`](.cargo/config.toml).

  Крейт `ort` линкуется с тем же ONNX Runtime, второй копии рантайма нет. Для этого скрипт
  достаёт:
  - `libonnxruntime.a` из статического архива своей платформы в
    `third_party/sherpa-onnx/ort-lib` (переменная `ORT_LIB_LOCATION` в `.cargo/config.toml`;
    заданная снаружи переменная важнее). На платформе без статических библиотек скрипт
    завершается с ошибкой;
  - `jni/arm64-v8a/libonnxruntime.so` из aar в `third_party/sherpa-onnx/ort-lib-android`
    для приложения на Android. На эту папку указывает
    [`nixyon-app/.cargo/config.toml`](nixyon-app/.cargo/config.toml) (`ORT_LIB_LOCATION` с
    `force` и `ORT_PREFER_DYNAMIC_LINK=1`): в APK попадает та же `.so`, что у sherpa-onnx.
- [`scripts/fetch-llama-cpp.sh`](scripts/fetch-llama-cpp.sh) качает исходники llama.cpp тега
  `b12005` в `third_party/llama.cpp/`. Из них Gradle собирает библиотеку языковой модели.

Суммы закреплены в самих скриптах: SHA-256, у шума DEMAND в `fetch-noise.sh` MD5 из записи
Zenodo. Если сумма не совпала, скрипт удаляет скачанное и завершается с ошибкой.

### Необязательные модели

Для отдельных замеров нужны модели, которые скрипты не качают. Их берут вручную:

- модель шумоподавления для `eval --denoise <модель>`; GTCRN для этого флага уже лежит в
  `models/_download/gtcrn_simple.onnx` после `fetch-models.sh`;
- потоковая модель T-one для `stream_eval ... t-one=<папка>`; в папке ожидаются
  `model.onnx` и `tokens.txt`;
- наборы моделей для сравнения в `models/variants/`; им можно не иметь необязательных ролей;
- `llama-server` из llama.cpp для [`scripts/llm-meeting-eval.sh`](scripts/llm-meeting-eval.sh).

## Требования

Общее:

- Rust 1.95 через rustup, с компонентами `rustfmt` и `clippy`;
- `curl`, `bzip2`, `tar`, `python3`;
- `sha256sum` или `shasum`;

Компилятор C++ нужен, потому что статическая sherpa-onnx линкуется с libstdc++.

### macOS

Поддерживается macOS на Apple Silicon и на Intel: `fetch-sherpa.sh` качает статические
библиотеки sherpa-onnx для `osx-arm64` и `osx-x64`.

### Linux

Ядро собирается на `x86_64` и `aarch64`. Пакеты для Debian, как в
[`docker/core/Dockerfile`](docker/core/Dockerfile):

```sh
sudo apt-get install g++ curl ca-certificates bzip2 python3-minimal
```

Для сборки и clippy приложения под хост нужны системные библиотеки Tauri, как в
[`docker/android/Dockerfile`](docker/android/Dockerfile):

```sh
sudo apt-get install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Android

Версии те же, что в [`docker/android/Dockerfile`](docker/android/Dockerfile):

- JDK 17;
- Android SDK с `platform-tools`, `platforms;android-36`, `platforms;android-37.0`,
  `build-tools;36.0.0`, `build-tools;37.0.0`, `ndk;30.0.16248370`, `cmake;4.1.2`;
- переменные `ANDROID_HOME` и `NDK_HOME` (`$ANDROID_HOME/ndk/30.0.16248370`);
- цель Rust: `rustup target add aarch64-linux-android`;
- tauri-cli 2.12.4: `cargo install tauri-cli --version 2.12.4 --locked`;
- `adb` для установки на телефон и `keytool` из JDK для отладочного ключа подписи.

## Установка и сборка

### Скачать библиотеки и модели

```sh
scripts/fetch-sherpa.sh       # библиотеки sherpa-onnx и ONNX Runtime, нужны для любой сборки
scripts/fetch-models.sh       # модели в models/ и models/ort/, нужен токен GitHub
scripts/fetch-test-audio.sh   # записи и диалоги для тестов с моделями (models/dialogs/)
scripts/fetch-llama-cpp.sh    # исходники llama.cpp, только для APK
scripts/fetch-llm.sh          # языковая модель, 2,7 ГБ, только для протокола встречи
scripts/fetch-noise.sh        # необязательно: зашумлённые копии диалогов
```

### Расшифровка на ноутбуке

```sh
cargo run --release -p nixyon-sherpa --bin transcribe -- models models/example.wav out.json
```

Синтаксис: `transcribe <папка моделей> <файл.wav> <расшифровка.json> [число спикеров]`.

- WAV любой поддерживаемой частоты сначала переводится в 16 кГц моно.
- Без числа спикеров оно оценивается само.
- Утилита пишет JSON расшифровки, печатает реплики и строку о качестве записи.

### Приложение на ноутбуке

```sh
cd nixyon-app
cargo build
cargo clippy --workspace --all-targets -- -D warnings
```

Эта сборка нужна для проверок и работы над интерфейсом: команды, которым нужен движок,
на ноутбуке отвечают ошибкой.

При сборке `build.rs` проверяет словари и создаёт `ui/generated/i18n.js` (см.
[Локализация](#локализация)).

### APK для разработки

```sh
cd nixyon-app
cargo tauri android build --debug --apk --target aarch64 --ci
```

Плагин под Android линкуется с `third_party/sherpa-onnx/ort-lib-android`, поэтому до сборки
нужен `scripts/fetch-sherpa.sh`. В такой APK модели не входят, их кладут на телефон
отдельно:

```sh
scripts/push-models.sh   # модели распознавания в /sdcard/Android/data/dev.demo.nixyon/files/models
scripts/push-llm.sh      # языковая модель в /sdcard/Android/data/dev.demo.nixyon/files/llm
```

`push-models.sh` закидывает все файлы из `models.json` и `example.wav`, набор `models/ort/`
на телефон не идёт. Манифест закидывается последним: пока его нет или он старый, приложение
не возьмёт недокачанный набор. Переустанавливать приложение нужно через `adb install -r`, потому что
удаление стирает эту папку вместе с моделями.

### APK с моделями внутри

```sh
scripts/fetch-sherpa.sh && scripts/fetch-models.sh
scripts/build-demo-apk.sh
```

[`scripts/build-demo-apk.sh`](scripts/build-demo-apk.sh) собирает release-APK для `aarch64`:

1. Докачивает llama.cpp и языковую модель (`fetch-llama-cpp.sh`, `fetch-llm.sh`).
2. Проверяет отладочный ключ `~/.android/debug.keystore`. Если ключа нет, создаёт его так
   же, как Android Studio.
3. Копирует модели строго по `models.json` и `llm.json`, сверяя размер и SHA-256 каждого
   файла. Берутся все роли манифеста, в том числе `tagging_*`, которые телефон не читает.
4. Собирает APK командой `cargo tauri android build --apk --target aarch64 --ci` со свойством
   Gradle `nixyonModelsAssets`, подпись отладочным ключом. После сборки удаляет
   промежуточные копии ресурсов Gradle: с языковой моделью они занимают гигабайты.
5. Заново выравнивает APK через `zipalign -P 16 64` и подписывает `apksigner`: llama.cpp
   отображает модель прямо из APK, и её веса должны лежать на смещении, кратном 32 байтам.
6. Проверяет выравнивание и подпись.

Готовый файл:
`nixyon-app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`.
Установка: `adb install -r <файл>`.

## Docker

Образы содержат только инструменты, исходники монтируются при запуске. Реестр cargo и
каталог сборки живут в томах, поэтому Linux-сборка не смешивается с `target/` на хосте.
Команды из [`compose.yaml`](compose.yaml):

```sh
NIXYON_UID=$(id -u) NIXYON_GID=$(id -g) docker compose build
docker compose run --rm fetch   # sherpa-onnx, llama.cpp, модели, языковая модель, записи для тестов
docker compose run --rm check          # fmt, clippy и тесты ядра
docker compose run --rm test-models    # тесты с моделями (нужны models/ и models/dialogs/)
docker compose run --rm transcribe models models/example.wav out.json [число спикеров]
docker compose run --rm app-check      # fmt и clippy приложения, в том числе под Android, и его тесты
docker compose run --rm apk            # APK с моделями внутри (arm64), нужны models/ из fetch
```

- **Образы.**
  - `nixyon-core:dev` из [`docker/core/Dockerfile`](docker/core/Dockerfile): Rust 1.95.0 на
    Debian trixie.
  - `nixyon-android:dev` из [`docker/android/Dockerfile`](docker/android/Dockerfile):
    `linux/amd64`, потому что Google выпускает Android SDK и NDK для Linux только под
    x86_64; на Apple Silicon образ работает через эмуляцию. Эмулятор Android в контейнере не
    запускается, APK ставится с хоста через `adb`.
- **ONNX Runtime для Linux.** Сервисы ядра задают `ORT_LIB_LOCATION=/cache/target/ort-lib`:
  библиотека для Linux лежит в томе сборки, а не в смонтированном `third_party/`, где
  библиотека хоста.
- **APK.** Сервис `apk` сам докачивает только llama.cpp и языковую модель, остальное
  (`third_party/sherpa-onnx`, `models/`) должен подготовить `fetch`.
- **Ограничения ресурсов:** `NIXYON_CPUS` (по умолчанию 8), `NIXYON_CORE_MEMORY` (8g),
  `NIXYON_ANDROID_MEMORY` (12g).
- **Ключ подписи.** Сервис `apk` монтирует `~/.android` с хоста, чтобы APK из контейнера и с
  машины разработчика были подписаны одним ключом и ставились поверх друг друга.

## Проверки

Все команды из корня репозитория, если не указано иное.

```sh
# ядро
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# тесты с моделями: нужны fetch-models.sh (с набором models/ort/) и fetch-test-audio.sh
MODELS_DIR=models cargo test --workspace --release -- --ignored

# приложение
cd nixyon-app
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --target aarch64-linux-android -- -D warnings
cargo test --workspace
cd ..

# тесты скриптов
python3 -I -B -m unittest discover -s scripts

# Kotlin-тесты плагина: проект Gradle готовит сборка APK
cd nixyon-app/src-tauri/gen/android
./gradlew :nixyon-android:testDebugUnitTest
```

- **Обычные тесты** лежат внутри модулей (`#[cfg(test)]`). Кроме них есть тесты в `tests/`:
  - эталонные файлы экспорта в
    [`nixyon-render/tests/golden`](nixyon-render/tests/golden): ru и en для txt, md, srt,
    протокола и XML документа DOCX. Эталоны обновляются командой
    `NIXYON_BLESS=1 cargo test -p nixyon-render`, после неё нужно просмотреть diff;
  - проверка ключей словаря в
    [`nixyon-locales/tests/usage.rs`](nixyon-locales/tests/usage.rs);
  - контраст цветов во всех режимах темы
    ([`nixyon-app/src-tauri/tests/contrast.rs`](nixyon-app/src-tauri/tests/contrast.rs)): текст
    от 4,5:1, элементы управления от 3:1;
  - полоса под строкой состояния
    ([`status_bar.rs`](nixyon-app/src-tauri/tests/status_bar.rs)).
- **Тесты с моделями** помечены `#[ignore]` и берут папку моделей из `MODELS_DIR`.
  Относительный путь считается от корня репозитория. Тесты лежат в
  [`nixyon-sherpa/tests`](nixyon-sherpa/tests), [`nixyon-eval/tests`](nixyon-eval/tests) и
  [`nixyon-ort/tests`](nixyon-ort/tests):
  - распознавание `example.wav`;
  - горячие слова;
  - образцы голоса;
  - нейросегментация и наложения;
  - число спикеров и точность на диалогах `ru2`, `ru4`, `ru6`, `real3`;
  - DER не больше 0,25;
  - живой текст: WER не больше 0,12;
  - края речи на клипах Common Voice из [`nixyon-eval/tests/data`](nixyon-eval/tests/data);
  - библиотека голосов по «прошлым встречам» `prior6a` и `prior6b`;
  - режим сочетаний `eval --combo` даёт ту же расшифровку, что отдельный прогон каждого
    сочетания, в том числе параллельно ([`combos.rs`](nixyon-eval/tests/combos.rs));
  - ReDimNet2: голоса одного человека ближе, чем разных, единичная норма эмбеддинга,
    загрузка из байтов, выгрузки с динамическим входом, пул сессий в параллели; Sortformer:
    реплики и наложения ([`nixyon-ort/tests/models.rs`](nixyon-ort/tests/models.rs)).

  Тесты одного крейта: `MODELS_DIR=models cargo test -p nixyon-eval --release -- --ignored`.
- **Kotlin-тесты** лежат в
  [`plugins/android/android/src/test/java`](nixyon-app/plugins/android/android/src/test/java):
  запас памяти для языковой модели, запас по краям кусков речи, куда пишутся запись встречи
  и образец голоса, выбор модели голоса.
  Модуль `:nixyon-android` подключает `tauri.settings.gradle`, а его создаёт
  `cargo tauri android build`. Поэтому перед первым запуском нужна сборка APK.
- **Тесты скриптов:**
  - [`scripts/test_build_dataset_index.py`](scripts/test_build_dataset_index.py): разбор
    длительности Ogg и другие части `build-dataset-index.py`;
  - [`scripts/test_fetch_sherpa.py`](scripts/test_fetch_sherpa.py): закреплённые архивы
    sherpa-onnx всех платформ, выбор архива под хост, библиотека ONNX Runtime для Android;
  - [`scripts/test_listen_page.py`](scripts/test_listen_page.py): `listen-page.py`.
- Для быстрой пересборки во внутреннем цикле есть профиль `fast`: на основе `dev`,
  `opt-level = 1`, без LTO, 16 единиц кодогенерации, инкрементально
  (`cargo test --profile fast`).

## CI

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) запускается на каждый push и
вручную.

Он идёт на своём раннере с метками `self-hosted, nixyon`, на обычных раннерах GitHub не
запустится. Каждый шаг обёрнут в `nixyon-ci`, это часть Nixyon Hub.

Библиотеки и модели живут в кэше раннера между прогонами. Скрипты сверяют SHA-256 и
ничего не качают заново, если файлы на месте.

Шаги по порядку:

1. Библиотеки и модели: `fetch-sherpa.sh`, `fetch-llama-cpp.sh`, `fetch-models.sh`,
   `fetch-test-audio.sh`.
2. Форматирование ядра и приложения.
3. Тесты скриптов.
4. Clippy ядра.
5. Тесты ядра.
6. Инструменты проверок: `cargo-nextest` 0.9.148.
7. Тесты с моделями: `cargo nextest run --workspace --release --run-ignored only`. nextest
   гоняет тестовые файлы параллельно, каждый в своём процессе.
8. Clippy приложения под хост.
9. Clippy приложения под `aarch64-linux-android`.
10. Тесты приложения.
11. Сборка отладочного APK: `cargo tauri android build --debug --apk --target aarch64 --ci`.
12. Тесты Kotlin.
13. Итог для хаба: `nixyon-ci finish`, выполняется всегда.

Новый push в ту же ветку отменяет идущий прогон, на весь прогон отведено 180 минут.

## Замер качества

### Проверочные диалоги

[`scripts/fetch-test-audio.sh`](scripts/fetch-test-audio.sh) качает живые записи и
собирает из них диалоги с разметкой в `models/dialogs/` (утилита `make_test_dialogs`):

| Диалог | Что внутри |
|---|---|
| `real3` | три человека из тестовых записей sherpa-onnx (китайская речь) |
| `ru2`, `ru4`, `ru6` | 2, 4 и 6 человек из VoxForge RU |
| `ru4_fast`, `ru4_overlap` | клипы `ru4` со сменой голоса без паузы и с наложением речи |
| `prior6a`, `prior6b` | «прошлые встречи» тех же шести человек для замера библиотеки голосов |

Рядом с каждым `<имя>.wav` лежит разметка `<имя>.truth.json`: `speakers` и `turns[]` с
полями `speaker`, `voice`, `start`, `end`, `source`. У VoxForge в разметке есть ещё `session`
и `text`, текст реплики берётся из `etc/PROMPTS` и служит эталоном для WER.

Границы речи клипов VoxForge берутся из
[`scripts/data/voxforge-speech-bounds.json`](scripts/data/voxforge-speech-bounds.json).
Опечатки PROMPTS правятся по
[`scripts/data/voxforge-text-fixes.json`](scripts/data/voxforge-text-fixes.json), и
неиспользованная поправка считается ошибкой. Состав фиксирован, каждый запуск даёт те же
файлы.

[`scripts/fetch-noise.sh`](scripts/fetch-noise.sh) качает шум помещений из набора DEMAND:
офис, столовую и ресторан. Утилита `make_noisy_dialogs` собирает из них
`ru2-snr<дБ>`, `ru4-snr<дБ>`, `ru6-snr<дБ>` при ОСШ 20, 10 и 5 дБ.

Образцы голоса для замера имён:
[`scripts/data/voice-samples.json`](scripts/data/voice-samples.json), по два клипа VoxForge
на диктора (`gav`, `kalan`, `polazhenko`, `snoop` и другие).

### `eval`: разделение на спикеров и имена

```sh
cargo run --release -p nixyon-eval --bin eval -- models models/dialogs
cargo run --release -p nixyon-eval --bin eval -- models models/dialogs --speakers truth ru4 ru6
cargo run --release -p nixyon-eval --bin eval -- models models/dialogs \
  --samples scripts/data/voice-samples.json --library prior6a,prior6b
```

Утилита прогоняет записи `<имя>.wav` с разметкой `<имя>.truth.json` и печатает таблицу
метрик. Без имён записей берутся все разметки папки. Флаги:

| Флаг | Что делает |
|---|---|
| `--speakers auto\|truth` | число спикеров: «не знаю» (по умолчанию) или из разметки |
| `--windows <окно>,<шаг>` | эмбеддинги по окнам, длина и шаг в секундах, по умолчанию 1,5 и 0,5 |
| `--viterbi <штраф>` | пересегментация Витерби с этим штрафом за смену говорящего, по умолчанию выключена |
| `--word-cut`, `--no-word-cut` | включить (по умолчанию) или выключить разрез куска по словам |
| `--no-neural` | без нейросегментации, разметка по кускам VAD |
| `--neural-block <с>` | длина блока нейросегментации, по умолчанию 60 с |
| `--samples <файл>` | образцы голоса; клипы ищутся в `<папка моделей>/_download/test-audio` |
| `--sample-threshold <косинус>` | наименьший косинус для имени, по умолчанию 0,55 |
| `--library <запись>,...` | запомнить голоса по этим записям как по прошлым встречам |
| `--voice-match max\|mean` | сравнение по наибольшему (по умолчанию) или среднему косинусу |
| `--same-speaker <косинус>` | при каком косинусе центров группы сливаются в одного человека, по умолчанию 0,6 |
| `--beam <прибавка>` | прибавка за горячие слова, по умолчанию 1,5 |
| `--beam-paths <число>` | число лучей, по умолчанию 8 |
| `--greedy` | жадный поиск, для наборов без словаря BPE |
| `--hotwords <файл>` | горячие слова, по одному слову или фразе в строке |
| `--dump <папка>` | сохранить расшифровку каждой записи в `<папка>/<имя>.json`, с `--combo` в `<папка>/<сочетание>/<имя>.json` |
| `--denoise <gtcrn.onnx>` | шумоподавление GTCRN до распознавания |
| `--no-vad-denoise` | без второго VAD по звуку после GTCRN (роль `denoiser` в наборе): куски VAD от 15 с не режутся; по умолчанию он работает, как в приложении |
| `--voice-model <имя>` | модель голоса: `campplus` (по умолчанию), `redimnet2`, `redimnet2-b0`, `redimnet2-b1`, `redimnet2-b2`, `redimnet2-b3`, `redimnet2-b3-vb2`, `redimnet2-b6-dyn` |
| `--segmenter <имя>` | нейросегментация: `pyannote` (по умолчанию), `sortformer-int8`, `sortformer-fp32` |
| `--ort-models <папка>` | набор ort, по умолчанию `<папка моделей>/ort` |
| `--no-truth` | записи без разметки: нужны имена записей и `--dump`, число спикеров только «не знаю», метрики не считаются; на запись печатается длина звука, время, RTF, число спикеров и спикеров от 5 с речи |
| `--combo <имя>=<голос>,<сегментация>,<порог>` | сочетание модели голоса, нейросегментации и порога слияния, флаг повторяется. Запись распознаётся один раз, эмбеддинги и отрезки считаются на каждое сочетание. Нельзя вместе с `--voice-model`, `--segmenter`, `--same-speaker`, `--library`, `--samples` |
| `--model-threads <1..64>` | потоки на вызов каждой модели |
| `--embed-jobs <1..64>` | сколько эмбеддингов считается параллельно, у каждого своя сессия модели голоса, по умолчанию 1 |
| `--asr-jobs <1..64>` | сколько кусков речи распознаётся параллельно, у каждого свой распознаватель, по умолчанию 1 |

Флаги `--sample-threshold`, `--voice-match` и `--same-speaker` принимают списки через
запятую. Каждая запись распознаётся один раз, а таблица выводится на каждое сочетание
значений. С `--combo` распознавание и нейросегментации сочетаний идут одновременно.

Дампы `eval --dump --combo` можно прослушать: [`scripts/listen-page.py`](scripts/listen-page.py)
строит страницу с плеером, дорожками сочетаний и текстом по ходу записи (нужен `ffmpeg`).

```sh
python3 -I scripts/listen-page.py <папка дампов> <папка звука> <папка страницы>
```

Страница содержит расшифровки и звук разговоров, поэтому папку страницы держат вне
репозитория.

### `stream_eval`: потоковые модели для живого текста

```sh
cargo run --release -p nixyon-eval --bin stream_eval -- [--threads N] \
  <папка записей> <имя,имя> <вид=папка> [<вид=папка>...]
```

- Вид модели: `zipformer` или `t-one`. `--threads` по умолчанию 1.
- Звук подаётся кусками по 0,1 с, как с микрофона.
- Таблица: WER, CER, число фраз, длина звука, время обработки и RTF по записям, плюс строка
  «всего» на модель.
- Эталон берётся из текста реплик разметки, поэтому подходят диалоги VoxForge.

### `quality_eval` и `scripts/quality.sh`: качество на наборах записей

`quality_eval` прогоняет каждый клип через весь конвейер с настройками приложения на Android
(модель голоса ReDimNet2-B6, порог слияния 0,7) и числом спикеров «не знаю». Поэтому ему
нужен и набор `models/ort/`.

```sh
scripts/quality.sh [--exclude <файл>] <корень наборов> <папка отчёта> <набор>=<минуты|all> [...]
```

Обёртка [`scripts/quality.sh`](scripts/quality.sh) делает четыре вещи:

- собирает `quality_eval` и берёт модели из `models/`;
- пишет отчёт в `<папка отчёта>.part` и переносит его на место целиком, когда он готов.
  Пока идёт прогон, прежний отчёт остаётся доступен;
- кладёт копию итогов в `<папка отчёта>-history`;
- записывает коммит, если репозиторий под git.

Прямой запуск:

```sh
cargo run --release --locked -p nixyon-eval --bin quality_eval -- \
  <папка моделей> <корень наборов> --out <папка отчёта> --take <набор>=<минуты|all> [--take ...] \
  [--jobs <число>] [--engine-threads <число>] [--commit <хэш>] [--history <папка>] [--exclude <файл>] \
  [--max-speech <с>] [--drop-music] [--voice-model <имя>] [--same-speaker <косинус>] [--ort-models <папка>]
```

- `--take`: сколько минут звука взять из набора, или `all`. Клипы выбираются перемешанными,
  но всегда одинаково.
- `--engine-threads`: потоки одного движка, по умолчанию 1. На коротких клипах много
  движков по одному потоку быстрее.
- `--jobs`: число движков. По умолчанию это число ядер, делённое на потоки движка.
- `--commit`: хэш коммита, 7-40 строчных шестнадцатеричных знаков.
- `--history <папка>`: копия `summary.json` ложится в
  `<папка>/<время>[-<коммит>]/summary.json`, время в секундах Unix, хранятся последние 100
  прогонов.
- `--exclude <файл>`: клипы с плохим эталоном не берутся в проверку. Это JSON со списком
  клипов по наборам, `{"<набор>": ["<id клипа>", ...]}`, он составляется вручную или хабом.
  Число исключённых клипов попадает в итоги и в отчёт.
- `--max-speech <с>`: наибольшая длина куска речи, от 2 до 60 с.
- `--drop-music`: не распознавать куски, где вместо речи музыка (модель CED tiny).
- `--voice-model <имя>`: модель голоса, имена как у `eval`, по умолчанию `redimnet2`.
- `--same-speaker <косинус>`: порог слияния групп, по умолчанию 0,7 для модели приложения и
  0,6 для остальных.
- `--ort-models <папка>`: набор ort, по умолчанию `<папка моделей>/ort`.
- Неизвестный флаг даёт ошибку.
- В stderr идут строки прогресса `@progress <звука готово, с> <звука всего, с> <клипов готово>
  <клипов всего> <прошло, с>`, не чаще раза в секунду. Каждые 5 % звука выводится строка для
  человека.

#### Формат набора

Набор лежит в `<корень>/<набор>/`: файлы `dataset.json` и `clips.jsonl`.

`dataset.json`:

| Поле | Значение |
|---|---|
| `id` | имя набора, совпадает с именем папки: строчные латинские буквы, цифры и `_` |
| `title` | название |
| `kind` | `single` (один голос в клипе), `dialog` (разговор) или `mixed` |
| `has_text` | у всех клипов есть эталонный текст |
| `reference` | кто писал тексты: `human`, `auto` (машина) или `unknown` |
| `license` | лицензия набора |
| `clips`, `hours` | число клипов и часов звука |
| `default_selected` | набор выбран для проверки по умолчанию |

`clips.jsonl`, одна строка на клип:

| Поле | Значение |
|---|---|
| `id` | имя клипа |
| `audio` | путь к звуку от корня наборов (wav, mp3, ogg, opus) |
| `text` | эталонный текст или `null` |
| `duration` | длительность в секундах |
| `speakers` | число спикеров, если известно |
| `truth` | путь к разметке спикеров `<имя>.truth.json` от корня, если она есть; по ней считается DER |

Пути `audio` и `truth` могут вести только вглубь корня наборов.

#### `scripts/build-dataset-index.py`

Скрипт строит индекс набора из папок с записями:

```sh
python3 -I scripts/build-dataset-index.py [--not-default] [--reference <кто>] \
  <источник> <корень наборов> <набор> <название> <папка> [...]
```

| Источник | Что ожидается в папке | Вид | Лицензия в индексе |
|---|---|---|---|
| `cv` | пары `<id>.mp3` и `<id>.json` (Common Voice) | single | CC0 |
| `open_stt` | пары `.wav` или `.opus` и `.txt` (open_stt), по всем подпапкам | single | CC BY-NC 4.0 |
| `wav` | `.wav` без текста | single | см. источник |
| `opus` | `.opus` без текста, полные части open_stt | single | CC BY-NC 4.0 |
| `podlodka` | parquet-файлы `bond005/podlodka_speech`, звук пишется в `<папка>/audio` | mixed | см. источник |
| `axondata` | пары `.mp3` и `.docx` (AxonData, русская расшифровка в docx) | dialog | CC BY-NC 4.0 |
| `dialogs` | пары `.wav` и `.truth.json` из `fetch-test-audio.sh` | dialog | см. `scripts/fetch-test-audio.sh` |

- `--not-default`: набор снят с выбора по умолчанию и включается только вручную. Пример:
  полные части open_stt без текста, прогон по ним идёт сутками.
- `--reference`: `human`, `auto` или `unknown` (по умолчанию). Отчёт показывает WER по
  наборам с текстом от человека отдельно.
- Длительность WAV, Ogg (Vorbis и Opus) и mp3 (у `axondata`) скрипт считает сам, без
  внешних библиотек; у `cv` она берётся из `<id>.json`. Для `podlodka` нужен `pyarrow`.
- У `dialogs` текст клипа есть, только если текст есть у каждой реплики, число спикеров
  берётся из разметки.
- Одинаковые по содержимому записи берутся один раз. Звук не копируется, кроме `podlodka`.
- Папки с записями должны лежать внутри корня наборов: пути в `clips.jsonl` пишутся от
  корня, и запись вне его скрипт не принимает.

Пример:

```sh
python3 -I scripts/build-dataset-index.py --reference human cv ~/datasets cv_ru "Common Voice RU" ~/datasets/cv
scripts/quality.sh ~/datasets ~/quality-report cv_ru=30
```

#### Отчёт

| Файл | Что внутри |
|---|---|
| `report.md` | общий WER, таблица по наборам и раздел на каждый набор. В разделе: метрики, группы по длине клипа (до 3 с, 3-5 с, 5-8 с, от 8 с), почему текст не получен, 20 частых замен слов, 50 худших клипов. Если настройки отличаются от приложения, есть строка «Настройки». Печатается и в конце прогона |
| `summary.json` | версия моделей, настройки прогона (`settings`: длина куска речи, фильтр музыки, модель голоса, порог слияния), коммит, время окончания, итоги WER по наборам с текстом от человека и по всем наборам с текстом, число движков, длина звука, время прогона, итоги каждого набора и число исключённых клипов |
| `clips/<набор>.jsonl` | строка на клип, худшие раньше. Поля: эталон, гипотеза, ошибки, `der`, метки `tags` (`errors`, `empty`, `failed`, `cut_start`, `cut_end`, `digits`); у наборов с текстом ещё выравнивание слов `edits` (`["=",w]`, `["~",эталон,гипотеза]`, `["-",w]`, `["+",w]`) и номер звука `audio` |
| `audio/<набор>/<номер>.wav` | звук первых 200 клипов с ошибками в каждом наборе, только у наборов с текстом |
| `<папка отчёта>-history/` | копии `summary.json` прошлых прогонов, последние 100 |

#### Метрики

Метрики считает [`nixyon-metrics`](nixyon-metrics/src/lib.rs). Ошибки по клипам
суммируются, а не усредняются.

- **WER:** слова нормализуются. Регистр нижний, «ё» считается как «е», всё, кроме букв и
  цифр, становится разделителем.
- **CER:** по тем же нормализованным словам; пробел тоже считается знаком.
- **WER со знаками и регистром:** слова, разделённые пробелами, сравниваются как есть
  (считает `nixyon-eval`).
- **Мягкий WER** (в отчёте «WER, дефисы и междометия не считаются»): слово через дефис
  склеивается («какие-то» и «какието» совпадают), а междометия-заминки убираются: слова
  только из «э», только из «м» или только из «ы», повторённые «а», «и», «у» («аа», «иии»),
  «э» или «х» с одними «м» после них («эм», «хмм»). Одиночные «а» и «и» считаются союзами и
  остаются, «ну», «угу» и «ага» тоже остаются как слова-ответы.
- **WER с цифрами:** числа с обеих сторон приводятся к цифрам (`numbers_to_digits`). Модель
  пишет числа цифрами, а эталоны вроде Common Voice пишут их словами. Разбираются числа
  словами в любом падеже и сокращения «тыс», «млн», «млрд», группы цифр («46 966 183»)
  склеиваются; дроби и деньги не разбираются. Отдельно считается WER без клипов, где в
  гипотезе есть цифры.
- **DER:** по разметке спикеров, если у клипа есть `truth`. Воротник 0,25 с. Состоит из трёх
  частей: пропуск речи, ложная речь и путаница спикеров. Номера спикеров сопоставляются
  венгерским алгоритмом. В `quality_eval` речь гипотезы берётся по словам расшифровки,
  слова одного спикера с паузой до 0,5 с сливаются.
- **В `eval` ещё:** cpWER (тексты по спикерам), точность и полнота пометок наложения, оценка
  имён (верно, неверно, пропущено).
- **Скорость:** RTF, то есть время обработки, делённое на длину звука.

### Замер языковой модели

```sh
scripts/llm-meeting-eval.sh <llama-server> <папка результатов> <модель.gguf>...
```

- Входные данные: синтетические тестовые встречи из
  [`scripts/data/llm-meetings`](scripts/data/llm-meetings). Тексты написаны для замера, это
  не записи настоящих встреч.
- Модель работает только на процессоре (`-ngl 0`), как на телефоне.
- На каждую встречу скрипт пишет ответ модели в `<папка>/<модель>/<встреча>.txt` и строку
  таблицы: время разбора текста, ответа и всего, память процесса, найдено задач из эталона.
  Лог сервера ложится в `<папка>/<модель>/server.log`, сервер слушает порт 8099.
- Подсказка совпадает с той, что в приложении; это проверяет тест в
  [`summary/prompt.rs`](nixyon-app/src-tauri/src/summary/prompt.rs).
- Нужны `curl` и `jq`.

## Локализация

Тексты интерфейса лежат только в словарях
[`nixyon-app/ui/locales/ru.json`](nixyon-app/ui/locales/ru.json) (эталон) и
[`en.json`](nixyon-app/ui/locales/en.json), не в коде.

Правила словарей проверяет [`nixyon-locales/src/check.rs`](nixyon-locales/src/check.rs):

- JSON валиден, ключи плоские вида `common.cancel`, без повторов, по алфавиту;
- значения не пустые;
- в `en.json` тот же набор ключей, что в `ru.json`;
- подстановки (`{n}`, `{value}` и другие) совпадают с русскими.

Код интерфейса берёт строки только через ключи:

- При сборке приложения [`build.rs`](nixyon-app/src-tauri/build.rs) строит из словарей модуль
  `ui/generated/i18n.js` с константами ключей `K` (`home.record` даёт `K.home_record`) и
  словарями. Этот модуль в git не попадает.
- Сборка падает, если словари не прошли проверку, если два ключа дают одно имя константы
  или если в `.js` и `.html` интерфейса встретился ключ (`K.имя`, `data-i18n`,
  `data-i18n-label`), которого нет в `ru.json`.
- Тест [`nixyon-locales/tests/usage.rs`](nixyon-locales/tests/usage.rs) проверяет, что
  ключи из кода и из словаря совпадают в обе стороны.

Выбор языка:

- переключатель «Авто / Русский / English», выбор хранится в зашифрованном файле настроек
  приложения;
- «Авто» берёт язык устройства: английский, если язык начинается с `en`, иначе русский;
- даты и время форматируются через `Intl` по языку интерфейса (`ru-RU` или `en-US`) в поясе
  устройства;
- подписи экспорта («Спикер N», заголовки) берутся из ключей `export.*` и идут на языке
  интерфейса;
- если строки нет в выбранном языке, берётся русская, а если нет и её, сам ключ.

## Лицензии моделей и данных

Лицензии сверены по страницам авторов, кроме строк, где сказано иное. Перед
распространением сборки с моделями их стоит проверить ещё раз.

| Компонент | Лицензия | Источник |
|---|---|---|
| sherpa-onnx | Apache-2.0 | [k2-fsa/sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) |
| GigaAM v3 | MIT | [ai-sage/GigaAM-v3](https://huggingface.co/ai-sage/GigaAM-v3) |
| Silero VAD | MIT | [snakers4/silero-vad](https://github.com/snakers4/silero-vad) |
| CAM++ (`speech_campplus_sv_zh_en_16k-common_advanced`) | Apache License 2.0 | [ModelScope iic/speech_campplus_sv_zh_en_16k-common_advanced](https://www.modelscope.cn/models/iic/speech_campplus_sv_zh_en_16k-common_advanced), [3D-Speaker](https://github.com/modelscope/3D-Speaker) |
| pyannote segmentation 3.0 | MIT (файл `LICENSE` в архиве модели) | [pyannote/segmentation-3.0](https://huggingface.co/pyannote/segmentation-3.0) |
| Потоковая zipformer `small-ru-vosk` | Apache-2.0 у исходной модели [alphacep/vosk-model-small-streaming-ru](https://huggingface.co/alphacep/vosk-model-small-streaming-ru): README в архиве sherpa-onnx говорит, что модели взяты оттуда. Файла лицензии в архиве нет | [релизы sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx/releases) |
| CED tiny audio tagging | Apache-2.0 по комментарию в `fetch-models.sh`; файла лицензии в архиве нет, по странице авторов не сверена | [релизы sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx/releases) |
| ReDimNet2-B6 (выгрузка soniqo), входит в APK | не сверена, в репозитории не указана | [soniqo/ReDimNet2-B6-ONNX-FP32](https://huggingface.co/soniqo/ReDimNet2-B6-ONNX-FP32) |
| ReDimNet2 B0-B6 (выгрузки весов PalabraAI) | не сверена, в репозитории не указана | [PalabraAI/redimnet2](https://github.com/PalabraAI/redimnet2) |
| Sortformer 4spk-v2 (выгрузки DimQ1) | не сверена, в репозитории не указана | [DimQ1/sortformer-4spk-v2-onnx-int8-cpu](https://huggingface.co/DimQ1/sortformer-4spk-v2-onnx-int8-cpu) |
| T-one (необязательная, для `stream_eval`) | Apache-2.0 (файл `LICENSE` в архиве) | [релизы sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx/releases) |
| GTCRN (`gtcrn_simple.onnx`), входит в APK: второй VAD для длинных кусков | MIT (файл `LICENSE` в репозитории авторов, 2026-11-04); что файл релиза sherpa-onnx выгружен из этой модели, по документации sherpa-onnx не сверено | [Xiaobin-Rong/gtcrn](https://github.com/Xiaobin-Rong/gtcrn), [релизы sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx/releases) |
| Qwen3.5-4B GGUF (unsloth) | Apache-2.0 | [unsloth/Qwen3.5-4B-GGUF](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF) |
| llama.cpp | MIT (файл `LICENSE` в исходниках) | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) |
| VoxForge RU | GPLv3 | [voxforge.org](http://www.repository.voxforge1.org/downloads/Russian/Trunk/Audio/Main/16kHz_16bit) |
| DEMAND | CC BY 4.0 | [Zenodo 1227121](https://zenodo.org/records/1227121) |
| Common Voice | CC0 | клипы в `nixyon-eval/tests/data` |
| open_stt, AxonData | CC BY-NC 4.0 | по `build-dataset-index.py` |

Что лежит в репозитории, а что нет:

- **Лежит в репозитории:** клипы Common Voice (CC0) в `nixyon-eval/tests/data`, синтетические
  тексты встреч в `scripts/data/llm-meetings`, границы речи и поправки текстов VoxForge в
  `scripts/data`. Это только числа и правки, не сам звук.
- **Не кладётся в репозиторий:** модели, библиотеки и исходники llama.cpp. Они качаются
  скриптами в `models/` и `third_party/`, обе папки в `.gitignore`. Выгрузки ReDimNet2 из
  весов PalabraAI делает вручную `scripts/export-redimnet2.py`, хранятся они в приватном
  релизе GitHub.
- **Только локально:** записи VoxForge под GPLv3 и всё, что из них собрано (`models/dialogs/`,
  в том числе зашумлённые копии). Наборы для `quality_eval` тоже лежат вне репозитория.

## Структура репозитория

```text
.
├── Cargo.toml                 воркспейс ядра
├── Cargo.lock
├── .cargo/config.toml         SHERPA_ONNX_ARCHIVE_DIR и ORT_LIB_LOCATION
├── compose.yaml               проверки и сборка в контейнерах
├── docker/
│   ├── core/Dockerfile        инструменты ядра
│   └── android/Dockerfile     JDK, Android SDK и NDK, tauri-cli
├── .github/workflows/ci.yml
├── nixyon-core/               типы расшифровки и граница с движком
├── nixyon-audio/              WAV, ресемплинг, громкость, качество записи
├── nixyon-cluster/            кластеризация и сопоставление спикеров
├── nixyon-pipeline/           сборка расшифровки, правка, поиск, статистика
│   └── src/{edit,search,meeting_stats,turns}/
├── nixyon-render/             экспорт TXT, MD, SRT, JSON, DOCX, блоки протокола
│   └── tests/golden/          эталонные файлы экспорта
├── nixyon-locales/            словари, проверка, генератор ключей
├── nixyon-sherpa/             движок sherpa-onnx для ноутбука
│   ├── src/bin/               transcribe, make_test_dialogs, make_noisy_dialogs
│   └── tests/                 тесты с моделями
├── nixyon-ort/                ReDimNet2 и Sortformer на крейте ort
│   └── tests/                 тесты с моделями
├── nixyon-metrics/            WER, CER, cpWER, DER, перекрытия, имена
├── nixyon-eval/               eval, stream_eval, quality_eval
│   └── tests/                 тесты с моделями и клипы Common Voice
├── nixyon-app/                приложение (отдельный воркспейс)
│   ├── Cargo.toml
│   ├── .cargo/config.toml     ONNX Runtime из aar для сборки под Android
│   ├── src-tauri/             Rust-часть, команды Tauri, хранилище
│   │   ├── src/{transcription,vault,voices,summary,settings,searching,statistics}/
│   │   ├── tests/             контраст и строка состояния
│   │   ├── capabilities/
│   │   ├── tauri.conf.json
│   │   └── gen/android/       Gradle-проект Android
│   ├── plugins/android/       плагин nixyon-android: Rust и Kotlin
│   │   ├── src/voice_model/   модель голоса через JNI
│   │   └── android/src/{main/java,main/cpp,test/java}/
│   └── ui/                    index.html, styles.css, js/, locales/
├── scripts/
│   ├── fetch-sherpa.sh        библиотеки sherpa-onnx
│   ├── fetch-models.sh        модели, models.json и набор models/ort/
│   ├── fetch-llm.sh           языковая модель и llm.json
│   ├── fetch-llama-cpp.sh     исходники llama.cpp
│   ├── fetch-test-audio.sh    записи и проверочные диалоги
│   ├── fetch-noise.sh         шум DEMAND и зашумлённые диалоги
│   ├── spm-to-bpe-vocab.py    словарь BPE для горячих слов
│   ├── export-redimnet2.py    выгрузка ReDimNet2 из весов PalabraAI в ONNX
│   ├── build-demo-apk.sh      APK с моделями внутри
│   ├── push-models.sh         модели на телефон через adb
│   ├── push-llm.sh            языковая модель на телефон через adb
│   ├── quality.sh             отчёт о качестве на наборах
│   ├── build-dataset-index.py индекс набора записей
│   ├── listen-page.py         страница прослушивания дампов eval
│   ├── listen-page.html       шаблон этой страницы
│   ├── test_*.py              тесты скриптов
│   ├── llm-meeting-eval.sh    замер языковой модели
│   └── data/                  образцы голоса, границы речи, поправки, тестовые встречи
├── models/                    скачанные модели (не в git)
└── third_party/               библиотеки и исходники (не в git)
```

## Частые проблемы

**`SHERPA_ONNX_ARCHIVE_DIR does not contain expected archive`.** Не запущен
`scripts/fetch-sherpa.sh`, либо сборка идёт на платформе, для которой скрипт не качает
библиотеку (например, Windows). На такой платформе сам скрипт падает с ошибкой
`для ... нет статических библиотек sherpa-onnx`.

**`нет роли redimnet2` в логе телефона.** На телефоне лежит набор моделей без модели голоса.
Запустите `scripts/fetch-models.sh` и `scripts/push-models.sh` заново.

**`ошибка: сумма ... не совпала`.** Скачанный файл отличается от закреплённого. Скрипт уже
удалил его, повторите запуск; если ошибка повторяется, файл на сервере изменился.

**`нет моделей в .../models: сначала scripts/fetch-models.sh`** или
**`нет .../models/models.json`.** Запустите `scripts/fetch-models.sh`. Для APK с моделями
внутри и для `push-llm.sh` нужен ещё `scripts/fetch-llm.sh`.

**`задайте MODELS_DIR: папку с models.json`.** Тесты с моделями запущены без переменной:
`MODELS_DIR=models cargo test --workspace --release -- --ignored`. Им же нужны
`models/dialogs/` и `models/_download/test-audio/` из `scripts/fetch-test-audio.sh`, а тестам
`nixyon-ort` и `combos.rs` ещё набор `models/ort/` из `scripts/fetch-models.sh`.

**Сборка приложения падает на ключах словаря.** В коде интерфейса есть ключ, которого нет в
`ru.json`, или словари не прошли проверку: порядок, повторы, пустые значения, разные
подстановки в `en.json`. Текст ошибки называет ключи.

**`... debug.keystore папка, а не ключ`.** Docker создал папку на месте файла ключа, когда
файла не было. Удалите папку и запустите сборку снова.

**`нет записи в ~/.android`** при сборке в контейнере на Linux. Создайте папку на хосте:
`mkdir -p ~/.android`.

**В образе Docker занят GID.** На Mac GID обычно 20 (staff), в Debian он уже занят; образ
тогда берёт существующую группу. Передавайте `NIXYON_UID` и `NIXYON_GID` при
`docker compose build`, чтобы созданные файлы принадлежали вам.

**`ошибка: не задан ANDROID_HOME`** или **`APK не выровнен`.** Нужен Android SDK с
build-tools: `zipalign` и `apksigner` берутся из самой новой версии build-tools.

**Kotlin-тесты не находят модуль `:nixyon-android`.** Модуль подключает
`tauri.settings.gradle`, его создаёт `cargo tauri android build`. Сначала соберите APK.

**Приложение не видит модели после переустановки.** Удаление приложения стирает его
внешнюю папку вместе с моделями. Ставьте поверх через `adb install -r` или закиньте модели
заново (`scripts/push-models.sh`, `scripts/push-llm.sh`).

**Сборка под Android падает на коде для ноутбука.** Модуль под `#[cfg(desktop)]` нельзя
вызывать из общего кода. Проверяйте clippy и под `--target aarch64-linux-android`.

**Протокол встречи не запускается.** На телефоне нет языковой модели (`push-llm.sh` или APK с
моделями внутри) либо мало свободной памяти: нужен запас 768 МБ сверх размера модели.

## Nixyon Hub

Nixyon Hub живёт в отдельном репозитории. Это сервер сборок: на нём идёт CI этого
репозитория, через него собираются APK и смотрятся результаты проверок и отчёты о качестве.
