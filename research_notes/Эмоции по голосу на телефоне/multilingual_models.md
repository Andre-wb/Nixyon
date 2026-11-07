# Мультиязычные и языконезависимые модели распознавания эмоций по голосу (SER) и их работа на русском

Дата сбора: 2026-11-07. Источники: карточки моделей Hugging Face, GitHub, arXiv. Всё, что не подтверждено первичным источником в этой сессии, помечено «не проверено».

## emotion2vec и emotion2vec+ (base, seed, large)

### Takeaway
emotion2vec+ (9 классов, 16 кГц, base около 90M, large около 300M параметров) сейчас самый сильный открытый «универсальный» классификатор эмоций; на русском RESD исходный emotion2vec лучший среди SSL-моделей в своей статье (WA 64,75). Официального ONNX нет, но есть сторонние экспорты в ONNX на Hugging Face, проверенные против PyTorch.

### Cited Findings
- Классы emotion2vec+ (seed, base, large) и emotion2vec_base_finetuned: angry, disgusted, fearful, happy, neutral, other, sad, surprised, unknown (индексы 0..8). Сам emotion2vec_base только даёт признаки, меток не выдаёт: [GitHub ddlBoJack/emotion2vec](https://github.com/ddlBoJack/emotion2vec); [HF emotion2vec_plus_large](https://huggingface.co/emotion2vec/emotion2vec_plus_large)
- Размеры и данные дообучения: seed, 201 ч академических данных из EmoBox; base, около 90M параметров, 4 788 ч отфильтрованных псевдоразмеченных данных; large, около 300M параметров, 42 526 ч («40k из 160k часов»). Подробности сбора данных авторы обещали опубликовать позже: [HF emotion2vec_plus_large](https://huggingface.co/emotion2vec/emotion2vec_plus_large)
- Ссылки на веса: ModelScope `iic/emotion2vec_plus_seed|base|large`, Hugging Face `emotion2vec/emotion2vec_plus_*`; запуск через FunASR: [GitHub](https://github.com/ddlBoJack/emotion2vec)
- Вход: 16 кГц; уровни «utterance» (вектор на фрагмент) или «frame» (признаки 50 Гц): [HF emotion2vec_plus_large](https://huggingface.co/emotion2vec/emotion2vec_plus_large)
- Лицензия: в README репозитория значок MIT (код); на HF у весов указано «model-license» без расшифровки: [GitHub](https://github.com/ddlBoJack/emotion2vec); [HF](https://huggingface.co/emotion2vec/emotion2vec_plus_large). Сторонние ONNX-экспорты указывают FunASR Model Open Source License, унаследованную от оригинала; автор экспорта base пишет, что она допускает коммерческое использование с указанием авторства: [HF pankotaro/emotion2vec-plus-base-onnx](https://huggingface.co/pankotaro/emotion2vec-plus-base-onnx)
- Числовых результатов emotion2vec+ в тексте карточки и README нет, только картинки (радар, EmoBox 4 класса); «detailed figures will be announced later». Новостей в README за 2025–2026 нет, последняя от октября 2024: [GitHub](https://github.com/ddlBoJack/emotion2vec)
- Архитектура исходного emotion2vec: инициализация от data2vec / data2vec 2.0, 7-слойный 1-D CNN (шаг 320, 50 Гц), 12 слоёв Transformer, d=768, 12 голов; около 93,8M параметров плюс линейная голова около 0,2M. Предобучение на 262 ч английских эмоциональных данных (IEMOCAP, MELD, MEAD, CMU-MOSEI, MSP-Podcast): [arXiv 2312.15185](https://arxiv.org/html/2312.15185)
- IEMOCAP (4 класса, замороженные признаки, линейная голова, leave-one-session-out): WA 71,79, UA 72,69, WF1 71,80. На MSP-Podcast результатов нет, он только в предобучении: [arXiv 2312.15185](https://arxiv.org/html/2312.15185)
- Мультиязычный эксперимент (9 языков, вне домена): RESD (русский) WA 64,75, UA 65,04, WF1 64,53; лучший базовый вариант data2vec 2.0: 64,08 / 64,33 / 64,17. Для сравнения CaFE 74,52 WA, EmoDB 84,34 WA, SUBESCO 90,91 WA. Авторы: emotion2vec лучше всех SSL-базовых моделей на всех 9 языках. Порядок столбцов WA/UA/WF1 восстановлен по шаблону, заголовки таблицы в HTML искажены: [arXiv 2312.15185](https://arxiv.org/html/2312.15185)
- ONNX: в официальном репозитории не упоминается; в FunASR обсуждение #2151 «Emotion2Vec onnx?» без ответа: [GitHub](https://github.com/ddlBoJack/emotion2vec); [FunASR discussion #2151 (зеркало)](https://scriptagc.wasmer.app/http_github_com/modelscope/FunASR/discussions/2151)
- Сторонний ONNX emotion2vec+ large (Voldorn): один файл, вход `audio` (1, N) float32 моно 16 кГц в [-1, 1], выход `scores` (1, 9) вероятности, метки в метаданных `labels`; в граф включены нормализация, пулинг, проекция и softmax; изменён только расчёт ALiBi-смещения, максимальная разница вероятностей с PyTorch 2,9e-6 на длительностях 0,5..30 с. Размер файла, opset и версия ONNX Runtime не указаны, квантованных вариантов нет: [HF Voldorn/emotion2vec-plus-large-onnx](https://huggingface.co/Voldorn/emotion2vec-plus-large-onnx)
- Сторонний ONNX emotion2vec+ base (pankotaro): бэкбон отдельно (выход 768 на кадр), голова 9×768 в JSON; пулинг и softmax делаются в коде. В дереве модели есть два квантованных варианта base: [HF pankotaro/emotion2vec-plus-base-onnx](https://huggingface.co/pankotaro/emotion2vec-plus-base-onnx); [HF Voldorn (model tree)](https://huggingface.co/Voldorn/emotion2vec-plus-large-onnx)
- Поддержки emotion2vec в sherpa-onnx не найдено (поиск без результатов): [поиск, см. выше]
- Emotion2Vec-S (arXiv 2502.18186, февраль 2025): полуобучаемый вариант emotion2vec, по словам авторов лучше других моделей на большинстве наборов, обучен в основном на китайском и английском: [arXiv 2502.18186](https://arxiv.org/pdf/2502.18186)

### Inferences
- Размер fp32 оценочно: base около 90M × 4 байта, примерно 360 МБ; large около 300M × 4, примерно 1,2 ГБ; int8 примерно в 4 раза меньше (base около 90–100 МБ). Это расчёт, не измерение.
- emotion2vec+ base по размеру сопоставим с уже встроенной моделью возраста и пола (129 МБ int8) и проще всего встраивается: голова линейная, пулинг среднее по кадрам.
- Результат RESD 64,75 WA получен с обучением линейной головы на RESD (в домене русского), а не при переносе «как есть». Готовый emotion2vec+ на русском без дообучения в источниках не измерен.
- Классы «other» и «unknown» на практике нужно сводить к «нейтрально / не определено».

### Gaps
- Нет опубликованных чисел emotion2vec+ (seed/base/large) ни на одном наборе, включая русские; только картинки.
- Точная лицензия весов emotion2vec+ на HF/ModelScope не прочитана (только «model-license» и пересказ в сторонних экспортах).
- Размер файлов ONNX-экспортов и совместимость с ONNX Runtime 1.28 на Android не проверены.
- Список языков 40k часов псевдоразметки не раскрыт; доля русского неизвестна.

## SenseVoice-Small: эмоции через sherpa-onnx

### Takeaway
sherpa-onnx выдаёт поля `lang`, `emotion`, `event` в результате распознавания, включая Kotlin API. Но выпущенный SenseVoice-Small официально поддерживает только китайский, кантонский, английский, японский и корейский, а SER оценивался только на китайских и английских данных; данных о качестве эмоций на русском нет.

### Cited Findings
- В C++ структуре `OfflineRecognitionResult` есть `std::string lang;`, `std::string emotion;` (комментарий «emotion target of the audio.»), `std::string event;` («event target of the audio.»); допустимые значения не описаны: [sherpa-onnx offline-stream.h](https://github.com/k2-fsa/sherpa-onnx/blob/master/sherpa-onnx/csrc/offline-stream.h)
- Kotlin `OfflineRecognizerResult` содержит поля text, tokens, timestamps, lang, emotion (`val emotion: String`), event, durations, words: [sherpa-onnx OfflineRecognizer.kt](https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/master/sherpa-onnx/kotlin-api/OfflineRecognizer.kt)
- Поддержка эмоций и событий SenseVoice добавлена в sherpa-onnx 1.10.22 (PR #1257), в Flutter и Kotlin API в 1.10.23 (PR #1268, #1275); в Node API поля описаны как «значение, найденное SenseVoice»: [pub.dev changelog sherpa_onnx 1.10.23](https://pub.dev/packages/sherpa_onnx/versions/1.10.23/changelog); [sherpa-onnx JS API OfflineRecognizerResult](https://k2-fsa.github.io/sherpa/onnx/javascript-api/html/interfaces/non-streaming-asr.OfflineRecognizerResult.html)
- Выпущенный чекпойнт SenseVoiceSmall: мандарин, кантонский, английский, японский, корейский (коды zh, en, yue, ja, ko, nospeech); «более 50 языков» относится к исследовательскому охвату, не к этому чекпойнту; карточка HF противоречива («6 languages» в теге и «50+» в Highlights): [GitHub FunAudioLLM/SenseVoice](https://github.com/FunAudioLLM/SenseVoice); [HF FunAudioLLM/SenseVoiceSmall](https://huggingface.co/FunAudioLLM/SenseVoiceSmall)
- Эмоции в данных обучения: HAPPY, SAD, ANGRY, NEUTRAL, FEARFUL, DISGUSTED, SURPRISED; события: BGM, Speech, Applause, Laughter, Cry, Sneeze, Breath, Cough: [GitHub SenseVoice](https://github.com/FunAudioLLM/SenseVoice)
- Тестовые наборы SER: китайские и английские данные (выступления, фильмы, разговоры), результаты только картинками; SenseVoice-Large лучше почти везде, но выложен только Small: [GitHub SenseVoice](https://github.com/FunAudioLLM/SenseVoice)
- Параметры: «примерно как Whisper-Small»: [HF SenseVoiceSmall](https://huggingface.co/FunAudioLLM/SenseVoiceSmall)
- Лицензия: код MIT, веса по FunASR Model Open Source License Agreement: [GitHub SenseVoice](https://github.com/FunAudioLLM/SenseVoice)
- Экспорт в ONNX: через `funasr_onnx`, с опцией `quantize=True`: [GitHub SenseVoice](https://github.com/FunAudioLLM/SenseVoice)
- GitHub-обсуждений о качестве эмоций SenseVoice на русском не найдено: [поиск](https://huggingface.co/FunAudioLLM/SenseVoiceSmall)

### Inferences
- Чтобы получить эмоцию от SenseVoice, пришлось бы гонять вторую ASR-модель параллельно с GigaAM только ради тега; на русской речи (вне языков обучения) тег, скорее всего, в основном будет NEUTRAL или случайным. Это вывод, не измерение.
- Если всё же пробовать, поле `emotion` уже доступно в Kotlin без своего кода разбора токенов.

### Gaps
- Точное число параметров SenseVoice-Small и размер int8-модели sherpa-onnx в этой сессии не проверены (по памяти около 234M и около 230 МБ, не проверено).
- Числа SER SenseVoice (наборы, UA/WA) в тексте не опубликованы.
- Поведение тега эмоции на русском нигде не измерено.

## audEERING: wav2vec2-large-robust-12-ft-emotion-msp-dim и другие

### Takeaway
Модель audEERING даёт три измерения (возбуждение, доминирование, валентность, примерно 0..1), обучена на MSP-Podcast 1.7, около 0,2B параметров, лицензия CC BY-NC-SA 4.0 (только исследования), официальный ONNX есть на Zenodo. Категориальной открытой модели audEERING в этой сессии не найдено.

### Cited Findings
- Основа: wav2vec2-large-robust, Transformer обрезан с 24 до 12 слоёв, регрессионная голова (dense, tanh, dropout, проекция); около 0,2B параметров, F32 safetensors: [HF audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)
- Выход: arousal, dominance, valence примерно в диапазоне 0..1, плюс эмбеддинг (пулинг последнего слоя): [HF](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)
- Данные: MSP-Podcast v1.7: [HF](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)
- Лицензия: CC BY-NC-SA 4.0, «for research purpose only»; коммерческая модель на большем объёме данных у audEERING за плату: [HF](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)
- ONNX: официальный экспорт, doi:10.5281/zenodo.6221127: [HF](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)
- Вход: сырой сигнал 16 кГц, нормализация как в Wav2Vec2Processor; для 1 с тишины выход A 0,5461, D 0,6062, V 0,4043. Метрик качества на карточке нет: [HF](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)

### Inferences
- Модель архитектурно родственна уже встроенной модели возраста и пола (тот же wav2vec2-large-robust, но 12 слоёв против 6), поэтому её путь подготовки входа можно переиспользовать; размер int8 оценочно около 200 МБ (расчёт, не измерение).
- Лицензия NC-SA совместима с некоммерческим проектом, но требует указания авторства и той же лицензии для производных.

### Gaps
- Метрики CCC на MSP-Podcast из статьи audEERING (Wagner и др., «Dawn of the transformer era in SER») в этой сессии не прочитаны.
- Результатов на русском для этой модели не найдено.
- Размер ONNX на Zenodo не проверен.

## Бенчмарки: EmoBox, IS25 SER Challenge, новые модели 2025

### Takeaway
В EmoBox (Interspeech 2024, 32 набора, 14 языков) единственный русский набор RESD; на нём лучшая модель WavLM large (UA 55,87), вторая Whisper large v3. Dusha в EmoBox нет. Публичной модели 2025–2026, которая явно заявлена лучше emotion2vec+ на многоязычных данных и пригодна для телефона, не найдено.

### Cited Findings
- EmoBox: 10 предобученных моделей, 32 набора, 14 языков; кросс-корпусная оценка на 4 наборах: [arXiv 2406.07162](https://arxiv.org/html/2406.07162v1)
- RESD в EmoBox: актёрский русский корпус, 7 эмоций, 200 дикторов, 1 396 фраз, около 2,3 ч: [arXiv 2406.07162](https://arxiv.org/html/2406.07162v1)
- RESD, UA / WA / F1: wav2vec 2.0 base 52,82 / 53,39 / 52,90; HuBERT base 51,56 / 52,34 / 51,65; HuBERT large 50,82 / 51,51 / 50,74; WavLM base 44,81 / 45,09 / 44,97; WavLM large 55,87 / 56,47 / 55,82; data2vec base 37,09 / 37,90 / 36,86; data2vec large 30,78 / 31,57 / 30,81; data2vec 2.0 base 43,54 / 44,03 / 43,00; data2vec 2.0 large 44,08 / 44,64 / 44,25; Whisper large v3 54,98 / 55,54 / 54,99. Dusha в статье не упоминается: [arXiv 2406.07162](https://arxiv.org/html/2406.07162v1)
- Interspeech 2025 SER in Naturalistic Conditions Challenge (MSP-Podcast): 93 команды; организаторы отмечают эффективность объединения аудио- и текстовых фундаментальных моделей: [ISCA Archive, Naini et al.](https://www.isca-archive.org/interspeech_2025/naini25_interspeech.html)
- Задача 2 (атрибуты A/V/D): команда USC (Lertpetchpun и др.) сообщает первое и второе место, лучший результат у ансамбля из двух систем: [ISCA Archive](https://www.isca-archive.org/interspeech_2025/lertpetchpun25_interspeech.html)
- Задача 1 (категории): команда MEDUSA сообщает первое место, Macro-F1 0,4316. Источник вторичный, не проверено по официальной таблице: [themoonlight.io](https://www.themoonlight.io/review/enhancing-speech-emotion-recognition-with-graph-based-multimodal-fusion-and-prosodic-features-for-the-speech-emotion-recognition-in-naturalistic-conditions-challenge-at-interspeech-2025)
- MERaLiON-SER (arXiv 2511.04914, ноябрь 2025): категории плюс A/V/D (кросс-энтропия плюс CCC), английский и языки Юго-Восточной Азии (английский, китайский, малайский, тамильский); заявлено превосходство над открытыми энкодерами и Audio-LLM; веса на HF `MERaLiON/MERaLiON-SER-v1`; бэкбон и лицензия модели в аннотации не указаны; сравнения с emotion2vec в аннотации нет: [arXiv 2511.04914](https://arxiv.org/abs/2511.04914)

### Inferences
- Числа RESD в EmoBox (WavLM large 56,47 WA) и в статье emotion2vec (64,75 WA) не сравнимы напрямую: разные разбиения и протоколы.
- Победители челленджей 2025 это ансамбли крупных моделей с текстом, для офлайна на телефоне непригодны.
- RESD маленький и актёрский (2,3 ч), поэтому результат на нём плохо предсказывает поведение на живых совещаниях.

### Gaps
- Результаты на Dusha для мультиязычных моделей (emotion2vec+, WavLM, Whisper) без дообучения не найдены.
- Модели SpeechBrain (например, wav2vec2-IEMOCAP), Vesper и Whisper-SER отдельно не проверялись в этой сессии из-за лимита запросов; данных о них в этих заметках нет.
- Лицензия и размер MERaLiON-SER-v1 не проверены.

## Перенос между языками: как модели, обученные на английском и китайском, работают на русском

### Takeaway
Прямых измерений «английская или китайская модель как есть на Dusha/RESD» в первичных источниках почти нет. Есть исследование 2025 года (Лемаев, Лукашевич): HuBERT, обученный на польском, китайском, японском (и, по другому описанию, английском), проверяли на Dusha; главный вклад даёт просодия, различия синтаксиса почти не влияют.

### Cited Findings
- Lemaev и Loukashevitch (Вестник МГУ, сер. 9 Филология, 2025): HuBERT обучали на nEMO (польский), ESD (китайский), JVNV (японский), тестировали на Dusha; основной вклад в качество дают просодические признаки, синтаксические различия обычно не влияют: [Вестник МГУ 2025 №4](https://vestnik.philol.msu.ru/issues/VMU_9_Philol__2025_04_1.pdf)
- В другом описании той же работы (Automatic Documentation and Mathematical Linguistics, т. 59, 2025, «Cross-Lingual Transfer for Russian Speech Emotion Automatic Recognition: Data and Trends») среди исходных языков указан английский (IEMOCAP), тест на сокращённой выборке Dusha; числа не получены, требуется проверка по полному тексту: [каталог UTB](https://vufind-test.katalog.k.utb.cz/SummonRecord/FETCH-LOGICAL-c113X-3038d8dc554c5ad7eff854e7d18ac159693b22e96c584a5e154f895c286d24153)
- Dusha: около 350 ч, более 300 000 записей русской речи, 4 эмоции (злость, радость, нейтрально, грусть), актёрская часть и подкасты; авторы предлагают его для кросс-корпусных исследований: [Kondratenko et al., Interspeech 2023](https://www.isca-archive.org/interspeech_2023/kondratenko23_interspeech.html); [arXiv 2212.12266](https://arxiv.org/pdf/2212.12266)
- Русская библиотека Aniemore заявляет точность голосовых моделей примерно 72–83% по собственным оценкам (несопоставимо с бенчмарками выше): [результаты поиска, вторичный источник](https://huggingface.co/Natlis/cnn-emotion-classification-ru/discussions)

### Inferences
- Раз эмоции по голосу в основном передаются просодией, языконезависимые модели (emotion2vec+, audEERING A/V/D) должны частично переноситься на русский, но без измерения на Dusha это гипотеза. Практичный путь: прогнать выбранную модель на тестовой части Dusha (4 класса пересекаются с классами emotion2vec+: angry, happy, neutral, sad) до встраивания.
- Для совещаний (естественная, слабо выраженная речь) ожидаемое качество заметно ниже, чем на актёрском RESD; MSP-Podcast-челлендж 2025 даже у победителей даёт Macro-F1 около 0,43 (вторичный источник).

### Gaps
- Числа переноса из работы Лемаева и Лукашевич не извлечены (PDF выпуска не разобран).
- Нет измерений emotion2vec+ или SenseVoice на Dusha в открытых первичных источниках, найденных в этой сессии.
