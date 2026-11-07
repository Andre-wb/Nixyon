# Модели распознавания эмоций по голосу для русской речи (SER), офлайн на Android

Дата проверки: 2026-11-07. Все цифры взяты из карточек моделей, кода репозиториев и API Hugging Face (размеры файлов из `?blobs=true`). Метрики везде заявлены авторами самих моделей, независимо никто их не перепроверял. Разделитель между фактом и ссылкой здесь двоеточие (длинное тире в проекте запрещено).

## 1. GigaAM-Emo (salute-developers/GigaAM): жива ли, классы, метрики, размер, ONNX, лицензия

### Takeaway
GigaAM-Emo есть в текущем репозитории (`main`, последний коммит 2026-08-17). Модель одна, `emo`, она построена на энкодере линейки v1. Отдельной Emo для v2 и v3 нет. Классов четыре: angry, sad, neutral, positive. На Dusha это лучшая из найденных моделей (crowd macro F1 0.84, podcast 0.67). Экспорт в ONNX официальный и покрыт тестом. Вход ONNX: лог-мел признаки, а не сырая волна. Лицензия MIT. Минус для телефона: это полный Conformer, ONNX fp32 весит около 953 MiB, а официальной int8 нет.

### Cited Findings
- В текущем README перечислены версии: v1 (`v1_ssl`, `emo`, `v1_ctc`, `v1_rnnt`), v2 (`v2_ssl`, `v2_ctc`, `v2_rnnt`), v3 (`v3_ssl`, `v3_ctc`, `v3_rnnt`, `v3_e2e_ctc`, `v3_e2e_rnnt`), multilingual (`multilingual_ssl`, `multilingual_large_ssl`, `multilingual_ctc`, `multilingual_large_ctc`). Модель `emo` есть только в строке v1: [GigaAM README](https://github.com/salute-developers/GigaAM)
- В README_ru строка v1 выглядит так: «Wav2vec 2.0 | 50 000 | 2 000 | `v1_ssl`, `emo`, `v1_ctc`, `v1_rnnt`», то есть предобучение v1 по схеме wav2vec 2.0 на 50 000 часах. Релиз GigaAM-Emo датирован 2024/04: [README_ru.md](https://github.com/salute-developers/GigaAM/blob/main/README_ru.md)
- В README сказано: «Our emotion recognition model `GigaAM-Emo` outperforms existing models by 15% Macro F1-Score». Описание семейства: Conformer, 220M–600M параметров. Энкодер v1 в старой версии README описан как «220-240M parameters»: [GigaAM README](https://github.com/salute-developers/GigaAM)
- Классы взяты из эталонного теста `tests/test_loading.py`: `"emo": {"angry": ..., "sad": ..., "neutral": 0.923..., "positive": 0.074...}`. Значит, четыре класса в таком порядке. Класса other в выходе нет: [tests/test_loading.py](https://github.com/salute-developers/GigaAM/blob/main/tests/test_loading.py)
- Метрики на Dusha из evaluation.md (UA / WA / Macro F1):
  - Crowd: baseline Dusha (MobileNetV2 + Self-Attention) 0.83 / 0.76 / 0.77; АБК (TIM-Net) 0.84 / 0.77 / 0.78; GigaAM-Emo 0.90 / 0.87 / 0.84.
  - Podcast: baseline 0.89 / 0.53 / 0.54; АБК 0.90 / 0.50 / 0.55; GigaAM-Emo 0.90 / 0.76 / 0.67.

  Источник: [evaluation.md](https://github.com/salute-developers/GigaAM/blob/main/evaluation.md)
- Что делает модель: `GigaAMEmo.get_probs` пропускает признаки через энкодер, усредняет выход по времени (avg pool), затем голова и softmax. Для экспорта у класса есть отдельный `forward_for_export(features, feature_lengths)`, и `_to_onnx` пишет граф с входами `["features", "feature_lengths"]` и выходом `["probs"]`. Оси batch и seq_len динамические: [gigaam/model.py](https://github.com/salute-developers/GigaAM/blob/main/gigaam/model.py)
- `load_onnx` в описании: «Supports any family of models (ASR, Emo, SSL)». Для emo `infer_onnx` сначала считает признаки препроцессором на Python, затем запускает одну сессию `{model_version}.onnx`: [gigaam/onnx_utils.py](https://github.com/salute-developers/GigaAM/blob/main/gigaam/onnx_utils.py)
- `tests/test_onnx.py` проверяет экспорт `("emo", torch.float32)` и сравнивает ONNX-вероятности с эталоном («ONNX emo probs failed»): [tests/test_onnx.py](https://github.com/salute-developers/GigaAM/blob/main/tests/test_onnx.py)
- В colab-примере вывод выглядит так: «Succesfully ported onnx emo to onnx/emo.onnx», ONNX возвращает `[[7.7e-05 2.2e-03 9.23e-01 7.44e-02]]`: [colab_example.ipynb](https://github.com/salute-developers/GigaAM/blob/main/colab_example.ipynb)
- Препроцессор `FeatureExtractor` строит лог-мел через `torchaudio.transforms.MelSpectrogram`. Параметры по умолчанию: hop = sr/100, win = n_fft = sr/40 (при 16 кГц это 160 и 400), `center=True`, затем `log(clamp(x, 1e-9, 1e9))`. Число мел-полос берётся из конфига модели. Звук 16 кГц моно: [gigaam/preprocess.py](https://github.com/salute-developers/GigaAM/blob/main/gigaam/preprocess.py)
- `to_onnx` по умолчанию экспортирует fp32, для GPU можно передать `dtype=torch.float16`. Про int8 в README ничего не сказано: [GigaAM README](https://github.com/salute-developers/GigaAM)
- Лицензия MIT, в новости 2024/12 упомянуто «MIT License»: [GigaAM README](https://github.com/salute-developers/GigaAM)
- Чекпойнт скачивается с `https://cdn.chatwm.opensmodel.sberdevices.ru/GigaAM/emo.ckpt`, контрольная сумма md5 `7ce76f9535cb254488985057c0d33006`: [gigaam/__init__.py](https://github.com/salute-developers/GigaAM/blob/main/gigaam/__init__.py)
- Сторонний (неофициальный) экспорт `checkpoints/emo/v1_emo.onnx` весит 952.9 MiB, рядом лежит `checkpoints/preprocessor.onnx` на 0.1 MiB. Обновлён 2025-11-19, лицензия в репозитории не указана: [benzlokzik/gigaam_models_onnx](https://huggingface.co/benzlokzik/gigaam_models_onnx)
- На Hugging Face у ai-sage есть только `ai-sage/GigaAM-v3` (ветки ssl, ctc, rnnt, e2e_ctc, e2e_rnnt) и `ai-sage/GigaAM-Multilingual`. Отдельного репозитория Emo нет: [HF API ai-sage](https://huggingface.co/ai-sage), [GigaAM-v3 refs](https://huggingface.co/api/models/ai-sage/GigaAM-v3/refs)
- Последние коммиты: 2026-08-17 («Fx: time axis in encoder onnx dynamic_axes», PR #88) и 2026-07-14 (GigaAM Multilingual): [GitHub commits API](https://api.github.com/repos/salute-developers/GigaAM/commits)

### Inferences
- Модель emo осталась в репозитории со времён v1 (2024/04) и не переобучалась на энкодерах v2 или v3. Код, тесты ONNX и загрузка её поддерживают.
- Размер fp32 около 953 MiB подходит к энкодеру примерно на 240M параметров. Динамическая int8-квантизация в onnxruntime (`quantize_dynamic`) теоретически дала бы около 240–260 MiB. Это не проверено: официальной int8 нет, точность после квантизации неизвестна.
- На телефоне придётся повторить лог-мел препроцессор (в стороннем репозитории он уже есть отдельным `preprocessor.onnx`). Приложение уже считает похожие признаки для GigaAM v3 ASR, но число мел-полос у emo (v1) нужно взять из её конфига, оно может отличаться от v3.
- Классы Dusha (angry / sad / neutral / positive) хорошо подходят для разговоров на совещаниях. Podcast-часть Dusha (спонтанная речь) ближе всего к реальному применению, и там GigaAM-Emo заметно впереди: macro F1 0.67 против 0.54–0.55 у остальных.

### Gaps
- Точный размер `emo.ckpt` и число параметров emo не проверены. CDN `cdn.chatwm.opensmodel.sberdevices.ru` на 2026-11-07 отдавал TLS-ошибку «certificate has expired» (это мой curl; обходить проверку сертификата не стал). Тот же CDN раздаёт Dusha. Если это не локальный сбой, скачать чекпойнт будет трудно.
- Число мел-полос у emo в конфиге чекпойнта не проверено: без скачивания чекпойнта его не узнать.
- Метрик GigaAM-Emo на RESD не нашёл.

## 2. Модели Aniemore (Hugging Face, org Aniemore)

### Takeaway
В Aniemore 36 моделей. В августе 2026 их обновили: перезалили карточки, добавили единые метрики, квантованные версии и новую линейку `*-v1-crosslingual`. У всех аудиомоделей 7 классов RESD. Модели, обученные только на RESD, дают на RESD macro F1 0.70–0.80, а на спонтанной речи (Dusha podcast) проваливаются до 0.09–0.15. Crosslingual-версии на Dusha podcast дают 0.30–0.49. Все модели крупные, 300M+ параметров, fp32 около 1.2 GB. Квантованные версии сделаны в формате `compressed-tensors` (для transformers), а не в ONNX. Лицензии MIT или Apache-2.0.

### Cited Findings
- В организации 36 моделей и 7 датасетов (resd, resd_annotated, resd_annotated_multi, resd_ctc16000, REPV, REPV-S, cedr-m7). Даты обновления: Aug 1 – Aug 24 (2026, по `lastModified` в API): [Aniemore на HF](https://huggingface.co/Aniemore), [список моделей](https://huggingface.co/models?author=Aniemore&sort=modified)
- Классы у всех аудиомоделей: anger, disgust, enthusiasm, fear, happiness, neutral, sadness: [wavlm-emotion-russian-resd-quantized](https://huggingface.co/Aniemore/wavlm-emotion-russian-resd-quantized)
- Метрики из model-index карточек (macro F1 / UA / accuracy). Тестовые наборы: RESD test (280 клипов), Dusha podcast test, CAMEO test:
  - `wavlm-emotion-russian-resd` (316.5M, MIT, база jonatasgrosman/exp_w2v2t_ru_wavlm_s363): RESD 0.8026 / 0.8058 / 0.8071; Dusha podcast 0.1116 / 0.3259 / 0.1043; CAMEO 0.2104: [карточка](https://huggingface.co/Aniemore/wavlm-emotion-russian-resd)
  - `hubert-emotion-russian-resd` (316.2M, MIT): RESD 0.7522 / 0.7494 / 0.75; Dusha podcast 0.1477: [карточка](https://huggingface.co/Aniemore/hubert-emotion-russian-resd)
  - `wav2vec2-emotion-russian-resd` (316.5M, MIT, база jonatasgrosman/wav2vec2-large-xlsr-53-russian): RESD 0.7079; Dusha podcast 0.126: [карточка](https://huggingface.co/Aniemore/wav2vec2-emotion-russian-resd)
  - `unispeech-sat-emotion-russian-resd` (MIT, обновлена 2026-08-24): RESD 0.7027; Dusha podcast 0.10: [карточка](https://huggingface.co/Aniemore/unispeech-sat-emotion-russian-resd)
  - `wav2vec2-xlsr-53-russian-emotion-recognition` (316.5M, MIT, создана 2022-05-22): RESD 0.7206; Dusha podcast 0.0954: [карточка](https://huggingface.co/Aniemore/wav2vec2-xlsr-53-russian-emotion-recognition)
  - `wavlm-bert-fusion-s-emotion-russian-resd` (0.5B, MIT): RESD 0.7941; Dusha podcast 0.092: [карточка](https://huggingface.co/Aniemore/wavlm-bert-fusion-s-emotion-russian-resd)
  - `wavlm-bert-base-s-emotion-russian-resd` (0.5B, MIT): RESD 0.7368: [карточка](https://huggingface.co/Aniemore/wavlm-bert-base-s-emotion-russian-resd)
  - `wav2vec2-bert-base-s-emotion-russian-resd` (0.5B, MIT): RESD 0.7182: [карточка](https://huggingface.co/Aniemore/wav2vec2-bert-base-s-emotion-russian-resd)
  - `wav2vec2-bert-tiny2-s-emotion-russian-resd` (345M, 1318 MiB fp32, MIT): RESD 0.7228; Dusha podcast 0.1361; RESD без транскрипта 0.5064: [карточка](https://huggingface.co/Aniemore/wav2vec2-bert-tiny2-s-emotion-russian-resd)
- Модели `*-bert-*` мультимодальные. Карточка wav2vec2-bert-tiny2: «Input: 16 kHz mono waveform and the utterance transcript», текстовая ветка `cointegrated/rubert-tiny2`. Авторы пишут: «the gap between 0.723 with one and 0.506 without is what a transcript is worth here»: [карточка](https://huggingface.co/Aniemore/wav2vec2-bert-tiny2-s-emotion-russian-resd)
- Линейка crosslingual v1 (создана 2026-08-03, обновлена 2026-08-04) обучена на 27 939 клипах из 8 языков. Состав: RESD 948, Dusha crowd 6 800, Dusha podcast 6 060, CAMEO 6 800 (без русского), IEMOCAP 4 735, ASVP-ESD 2 596: [wavlm-emotion-v1-crosslingual](https://huggingface.co/Aniemore/wavlm-emotion-v1-crosslingual)
- Метрики crosslingual (macro F1 / UA):
  - `wavlm-emotion-v1-crosslingual` (MIT): RESD 0.6084 / 0.6479; Dusha podcast 0.3633 / 0.6466; CAMEO 0.5048.
  - `hubert-emotion-v1-crosslingual` (Apache-2.0): RESD 0.6167; Dusha podcast 0.3147.
  - `wav2vec2-emotion-v1-crosslingual` (Apache-2.0): RESD 0.5344; Dusha podcast 0.3454.
  - `unispeech-sat-emotion-v1-crosslingual` (MIT): RESD 0.5697; Dusha podcast 0.3705 / 0.6992.
  - `wav2vec2-xlsr-53-emotion-v1-crosslingual` (Apache-2.0): RESD 0.665; Dusha podcast 0.2982.
  - `wav2vec2-bert-tiny2-s-emotion-v1-crosslingual` (Apache-2.0, нужен текст): RESD 0.6226; Dusha podcast 0.4033.
  - `wavlm-bert-fusion-s-emotion-v1-crosslingual` (MIT, нужен текст): RESD 0.6553; Dusha podcast 0.4869 / 0.511.

  Источник: HF API cardData, например [hubert-emotion-v1-crosslingual](https://huggingface.co/Aniemore/hubert-emotion-v1-crosslingual)
- Противоречие в лицензии `wavlm-emotion-v1-crosslingual`: в заголовке стоит тег MIT, а в Limitations написано «the licence follows the base model» (microsoft/wavlm-large): [карточка](https://huggingface.co/Aniemore/wavlm-emotion-v1-crosslingual)
- Квантованные версии хранятся в формате `compressed-tensors` («transformers loads it directly»). Размеры для wavlm: fp32 1207 MiB; int8 W8A16 352 MiB (macro F1 0.8026); fp8 344 MiB (0.7984); int4 W4A16 210 MiB (0.7966). Вход: «16 kHz mono, clips capped at 12 s»: [wavlm-emotion-russian-resd-quantized](https://huggingface.co/Aniemore/wavlm-emotion-russian-resd-quantized)
- У crosslingual те же подпапки: int8/model.safetensors 352.5 MiB, fp8 344.3 MiB, int4 209.6 MiB (wavlm). У wav2vec2-bert-tiny2 crosslingual fp8 весит 445.1 MiB: [HF API wavlm-emotion-v1-crosslingual](https://huggingface.co/api/models/Aniemore/wavlm-emotion-v1-crosslingual?blobs=true)
- Сторонний ONNX-экспорт старой версии unispeech-sat-emotion-russian-resd: один файл на 1206.9 MiB (fp32), обновлён 2024-05-28, лицензия не указана: [steveway/unispeech-sat-emotion-russian-resd_onnx](https://huggingface.co/steveway/unispeech-sat-emotion-russian-resd_onnx)
- Предупреждение из карточки RESD: «Acted emotion is not spontaneous emotion... a model that scores well on this test set can still miss most of the neutral speech it meets in production»: [Aniemore/resd](https://huggingface.co/datasets/Aniemore/resd)

### Inferences
- Аудиомодели Aniemore (WavLM, HuBERT, wav2vec2, UniSpeech-SAT) это стандартные `*ForSequenceClassification` из transformers на сыром waveform 16 кГц. Через optimum или `torch.onnx` они экспортируются без особых сложностей, а дальше их можно квантовать в int8 средствами onnxruntime. Так же устроена уже встроенная в приложение модель audEERING. Сам экспорт я не проверял.
- Для совещаний модели «только RESD» не годятся: на спонтанной речи macro F1 0.09–0.15. Из Aniemore разумно брать только crosslingual, лучше без текстовой ветки (wavlm, hubert, unispeech-sat). Но даже они на Dusha podcast (0.31–0.37 macro F1) хуже GigaAM-Emo (0.67). Оговорка: у Aniemore 7 классов, у GigaAM-Emo 4, поэтому прямое сравнение некорректно.
- Мультимодальным моделям `*-bert-*` нужен транскрипт. В приложении он есть (GigaAM ASR), но модели тяжелее (345–500M) и зависят от качества распознавания.

### Gaps
- Как Aniemore сводит 7 своих классов к меткам Dusha (angry / sad / neutral / positive / other) при подсчёте метрик на Dusha podcast, в просмотренных фрагментах карточек не описано.
- Год в датах «Aug 1 – Aug 24» на страницах не указан. 2026 взят из поля `lastModified` API (2026-08-01 и т. д.).

## 3. Модели, обученные на Dusha: официальный baseline и дообучения сообщества

### Takeaway
Официальный baseline Dusha (MobileNetV2 + self-attention на мел-спектрограммах) существует только как код для обучения: готовых весов я не нашёл. Сообщество выложило несколько дообучений на 5 классов Dusha (neutral, angry, positive, sad, other). Самая скачиваемая: xbgoose HuBERT-large (1.2 GB, macro F1 0.81). Самая лёгкая с метриками: waveletdeboshir whisper-base-ser-dusha (20.7M параметров, 79 MiB, macro F1 0.843 на crowd).

### Cited Findings
- Код baseline: класс `ConvSelfAttentionMobileNet` на блоках `InvertedResidual` из torchvision MobileNetV2, `last_channel=128`, `in_channels=1`. Каналы в конфиге: `[1,16,1,1], [2,32,2,2], [2,64,6,2], [2,128,6,2]`. Датасет `MelEmotionsDataset`. Конфиги: crowd_small, crowd_medium, crowd_large, crowd_unconfident, dusha_large, podcast_large, podcast_tune: [dusha/experiments](https://github.com/salute-developers/golos/tree/master/dusha/experiments), [core/model.py](https://github.com/salute-developers/golos/blob/master/dusha/experiments/core/model.py), [configs/base.config](https://github.com/salute-developers/golos/blob/master/dusha/experiments/configs/base.config)
- README экспериментов предлагает обучать модели самому (`train.py`) и считать метрики (`inf.py`). Ссылок на готовые веса там нет. Архив `paper_setups.tgz` (16 MB) описан как набор для воспроизведения: [dusha/experiments/README.md](https://github.com/salute-developers/golos/blob/master/dusha/experiments/README.md), [dusha README](https://github.com/salute-developers/golos/tree/master/dusha)
- Метрики baseline на Dusha: crowd UA / WA / macro F1 0.83 / 0.76 / 0.77, podcast 0.89 / 0.53 / 0.54: [GigaAM evaluation.md](https://github.com/salute-developers/GigaAM/blob/main/evaluation.md)
- `xbgoose/hubert-large-speech-emotion-recognition-russian-dusha-finetuned`: HubertForSequenceClassification, 315.7M параметров, model.safetensors 1204.4 MiB, Apache-2.0, база facebook/hubert-large-ls960-ft. Обучена на половине train Dusha. Заявлено: «accuracy = 0.86, balanced = 0.76, macro f1 score = 0.81 on test set» (какой тест, crowd или podcast, не уточнено). Классы: `{0: 'neutral', 1: 'angry', 2: 'positive', 3: 'sad', 4: 'other'}`. Вход: сырая волна 16 кГц через Wav2Vec2FeatureExtractor, max 10 с. Обновлена 2024-04-26, около 207 тыс. скачиваний: [карточка](https://huggingface.co/xbgoose/hubert-large-speech-emotion-recognition-russian-dusha-finetuned)
- `xbgoose/hubert-base-...-dusha-finetuned` (pytorch_model.bin 360.8 MiB, 2023-06-11), `xbgoose/wavlm-base-...` (360.9 MiB, 2023-06-12), `xbgoose/wavlm-large-...` (1204.5 MiB, 2023-06-18). Тега лицензии нет: [HF API](https://huggingface.co/xbgoose)
- `waveletdeboshir/whisper-base-ser-dusha`: WhisperForAudioClassification (энкодер whisper-base и голова классификации), 20 723 205 параметров, model.safetensors 79.1 MiB. Метрики на Dusha crowd test: weighted accuracy 0.8364, F1 macro 0.8429, recall macro 0.83, precision macro 0.85. Классы: angry 0, sad 1, neutral 2, positive 3, other 4. Обучена на полном Dusha-crowd с аугментациями и WeightedRandomSampler. Вход: лог-мел признаки WhisperFeatureExtractor (80 полос, окно 30 с с паддингом, это стандарт whisper-base, по карточке не проверял). Создана 2025-07-07, обновлена 2025-07-08, около 4.5 тыс. скачиваний. Тега лицензии нет (у базовой openai/whisper-base лицензия Apache-2.0, это не проверено в этой сессии): [карточка](https://huggingface.co/waveletdeboshir/whisper-base-ser-dusha)
- `nixiieee/gigaam-rnnt-emotion-classifier-dusha`: энкодер GigaAM RNNT (база waveletdeboshir/gigaam-rnnt) и 3-слойная голова с LayerNorm, 232 957 061 параметров, 888.7 MiB. Обучена на `nixiieee/dusha_balanced`, 5 классов (neutral, angry, positive, sad, other), нужен trust_remote_code. Метрик в карточке нет, лицензии нет. Обновлена 2025-06-19: [карточка](https://huggingface.co/nixiieee/gigaam-rnnt-emotion-classifier-dusha)
- У того же автора есть whisper-small и whisper-large-v3 классификаторы на Dusha (`nixiieee/whisper-small-emotion-classifier-dusha`, `nixiieee/whisper-large-v3-emotion-classifier-dusha`). Их карточки я не читал: [HF поиск dusha](https://huggingface.co/models?search=dusha)
- `KELONMYOSA/wav2vec2-xls-r-300m-emotion-ru`: дообучение facebook/wav2vec2-xls-r-300m на Dusha, Apache-2.0, pytorch_model.bin 1207.5 MiB, классы `['neutral', 'positive', 'angry', 'sad', 'other']`, нужен trust_remote_code. Обновлена 2023-06-05: [карточка](https://huggingface.co/KELONMYOSA/wav2vec2-xls-r-300m-emotion-ru)
- `LessThanThree/whisper-tiny-finetuned-dusha`: WhisperForAudioClassification, 8 308 229 параметров, model.safetensors 31.7 MiB. id2label: neutral, positive, sad, angry, other. README нет («Entry not found»), значит нет ни метрик, ни лицензии. 2024-09-01. Есть также `LessThanThree/whisper-small-finetuned-dusha` и `LessThanThree/wav2vec2-xls-r-300m-finetuned-dusha`: [HF API](https://huggingface.co/LessThanThree/whisper-tiny-finetuned-dusha)

### Inferences
- waveletdeboshir whisper-base-ser-dusha по метрике на crowd (0.843 macro F1) сравнима с GigaAM-Emo (0.84), при этом она примерно в 12 раз меньше. Но она обучена только на crowd, а метрик на podcast (спонтанная речь) у неё нет. Энкодер whisper экспортируется в ONNX штатно (sherpa-onnx и optimum его поддерживают для ASR). Вход: лог-мел whisper, 80 полос. Энкодер Whisper рассчитан на фиксированное окно 30 с, поэтому каждый короткий сегмент придётся паддить до 3000 кадров. Это лишние вычисления на телефоне, и это нужно проверить.
- Класс other у дообучений Dusha есть только в этих моделях. В GigaAM-Emo его нет.

### Gaps
- Готовых весов официального baseline Dusha не нашёл. Число параметров baseline не посчитано: по конфигу это маленькая MobileNetV2 до 128 каналов, предположительно меньше 1M параметров, но я это не проверял.
- Для xbgoose не указано, на каком тесте (crowd или podcast) получены метрики.
- Метрики nixiieee, LessThanThree и KELONMYOSA не найдены.

## 4. Лёгкие (< 50 MB) модели эмоций для русской речи

### Takeaway
Готовая модель меньше 50 MB с опубликованными метриками на русском не найдена. Единственная модель меньше 50 MB: LessThanThree/whisper-tiny-finetuned-dusha (31.7 MiB, 8.3M параметров), но у неё нет ни карточки, ни метрик, ни лицензии. Ближайший вариант с метриками: whisper-base-ser-dusha (79 MiB fp32). Её int8 теоретически уложится примерно в 20–25 MB, но это не проверено.

### Cited Findings
- whisper-tiny-finetuned-dusha: 31.7 MiB, 8 308 229 параметров, без README: [HF](https://huggingface.co/LessThanThree/whisper-tiny-finetuned-dusha)
- whisper-base-ser-dusha: 79.1 MiB, 20.7M параметров, macro F1 0.8429 на Dusha crowd: [HF](https://huggingface.co/waveletdeboshir/whisper-base-ser-dusha)
- Квантованные модели Aniemore начинаются от 209.6 MiB (int4): [HF](https://huggingface.co/Aniemore/wavlm-emotion-russian-resd-quantized)
- Baseline Dusha (маленькая MobileNetV2 на мел-спектрограммах) доступен только как код для обучения: [dusha/experiments](https://github.com/salute-developers/golos/tree/master/dusha/experiments)
- На поисковые запросы «emotion russian», «dusha», «gigaam emo» и т. п. API Hugging Face других русских SER-моделей меньше 50 MB не вернул. Результаты с rubert-tiny относятся к эмоциям по тексту, а не по голосу: [HF API поиск](https://huggingface.co/api/models?search=emotion%20russian)

### Inferences
- Варианты меньше 50 MB: (а) квантовать whisper-base-ser-dusha в int8; (б) обучить baseline Dusha самому (код открыт, но датасет crowd весит 28 GB); (в) whisper-tiny без метрик. Каждый вариант нужно проверять на своих данных.
- В Aniemore есть и текстовые модели (`rubert-tiny2-russian-emotion-detection` и квантованные rubert-tiny на CEDR). Они размечают эмоцию по тексту расшифровки, а не по голосу. Это отдельный и очень лёгкий путь, но к SER он не относится.

### Gaps
- Размер whisper-base-ser-dusha после int8 и её качество после квантизации не измерены.
- Лицензия whisper-tiny-finetuned-dusha и whisper-base-ser-dusha явно не указана.

## 5. Русские датасеты эмоциональной речи и их метки

### Takeaway
Основные датасеты: Dusha (SberDevices, около 350 ч, 4 эмоции плюс other в разметке), RESD (Aniemore, 2.3 ч, 7 эмоций, игровая речь) и REPV / REPV-S (Aniemore, около 2 ч, 5 эмоций, краудсорсинг). Для спонтанной речи совещаний ближе всего podcast-часть Dusha.

### Cited Findings
- Dusha: «about 300 000 audio recordings with Russian speech, their transcripts and emotional labels... approximately 350 hours». Эмоции: «Happiness (Positive), Sadness, Anger and Neutral emotion». Crowd: 201 850 файлов, 255.7 ч, 2068 дикторов. Podcast: 102 113 файлов, 90.9 ч, 6240 дикторов. Аудио подкастов не раздаётся из-за лицензии, вместо него даны признаки и ссылки. Архивы: crowd.tar 28 GB, podcast.tar 360 MB, features.tar 30 GB. Лицензия: отдельные PDF (en_us.pdf, ru.pdf) в репозитории golos: [dusha README](https://github.com/salute-developers/golos/tree/master/dusha)
- Статья Dusha: «Large Raw Emotional Dataset with Aggregation Mechanism», arXiv 2212.12266 (v1, 23.12.2022, ICASSP 2023), разметка краудсорсингом: [arXiv](https://arxiv.org/abs/2212.12266)
- Метка other используется в дообучениях на Dusha (5 классов: neutral, angry, positive, sad, other): [xbgoose config](https://huggingface.co/xbgoose/hubert-large-speech-emotion-recognition-russian-dusha-finetuned), [waveletdeboshir](https://huggingface.co/waveletdeboshir/whisper-base-ser-dusha)
- RESD (Russian Emotional Speech Dialogs): студийная запись, 20 актёров, импровизированные диалоги без сценария, 7 эмоций. Train 1116 клипов (1.88 ч), test 280 (0.46 ч), средний клип около 6 с. Частота дискретизации неоднородна: в train 565 клипов 16 кГц и 551 клип 44.1 кГц. Лицензия MIT: [Aniemore/resd](https://huggingface.co/datasets/Aniemore/resd)
- REPV (Russian Emotional Phonetic Voices): краудсорсинг, около 200 дикторов (у REPV-S около 50). Train 1628 (1.65 ч), test 407 (0.40 ч), средний клип 3.6 с. Пять меток: `anger`, `enthusiasm`, `happiness`, `sadness`, `tiredness` (neutral, fear и disgust нет). Лицензия MIT: [Aniemore/REPV](https://huggingface.co/datasets/Aniemore/REPV), [REPV-S](https://huggingface.co/datasets/Aniemore/REPV-S)
- Зеркала Dusha на HF: `xbgoose/dusha` (без карточки) и `KELONMYOSA/dusha_emotion_audio` (около 125 000 записей, лицензия как у оригинала): [xbgoose/dusha](https://huggingface.co/datasets/xbgoose/dusha), [KELONMYOSA/dusha_emotion_audio](https://huggingface.co/datasets/KELONMYOSA/dusha_emotion_audio)
- В Aniemore есть также CEDR-M7 (`Aniemore/cedr-m7`): это эмоции в тексте, а не в речи: [Aniemore](https://huggingface.co/Aniemore)

### Inferences
- Лицензию Dusha нужно прочитать по PDF перед использованием моделей, обученных на Dusha. Для некоммерческого хакатона это, скорее всего, допустимо, но я это не проверял.

### Gaps
- Текст лицензии Dusha (PDF golos) не читал.
- Другие русские корпуса (например RUSLANA, упоминаемые в литературе) не искал и не проверял: ни в одном просмотренном первоисточнике они не встретились.

## Сводная таблица кандидатов (по фактам выше)

| Модель | Архитектура | Параметры | Размер fp32 | int8 | Классы | Обучение | Метрика (macro F1) | Лицензия | ONNX | Вход | Обновлена |
|---|---|---|---|---|---|---|---|---|---|---|---|
| GigaAM `emo` | Conformer (GigaAM v1) | около 240M (не проверено) | ONNX 952.9 MiB (сторонний) | нет официальной | angry, sad, neutral, positive | Dusha | crowd 0.84, podcast 0.67 | MIT | официальный `to_onnx`, есть тест | лог-мел | репозиторий 2026-08-17 |
| waveletdeboshir/whisper-base-ser-dusha | энкодер whisper-base и голова | 20.7M | 79.1 MiB | нет | angry, sad, neutral, positive, other | Dusha crowd | crowd 0.843 | не указана | нет готового, энкодер whisper экспортируется | лог-мел whisper | 2025-07-08 |
| xbgoose/hubert-large-...-dusha | HuBERT-large | 315.7M | 1204.4 MiB | нет | neutral, angry, positive, sad, other | половина Dusha train | 0.81 (тест не уточнён) | Apache-2.0 | нет готового | волна 16 кГц | 2024-04-26 |
| nixiieee/gigaam-rnnt-emotion-classifier-dusha | энкодер GigaAM RNNT и 3-слойная голова | 233.0M | 888.7 MiB | нет | 5 классов Dusha | dusha_balanced | нет | не указана | нет | признаки GigaAM | 2025-06-19 |
| Aniemore/wavlm-emotion-russian-resd | WavLM-large | 316.5M | 1207 MiB | 352 MiB (compressed-tensors) | 7 классов RESD | RESD | RESD 0.80, podcast 0.11 | MIT | нет официального | волна 16 кГц | 2026-08-01 |
| Aniemore/unispeech-sat-emotion-v1-crosslingual | UniSpeech-SAT | около 0.3B | около 1.2 GB | int8 в подпапке (compressed-tensors) | 7 классов | 8 языков, включая Dusha и RESD | RESD 0.57, podcast 0.37 | MIT | нет | волна 16 кГц | 2026-08-04 |
| Aniemore/wavlm-emotion-v1-crosslingual | WavLM-large | 316.5M | 1207.4 MiB | 352.5 MiB | 7 классов | то же | RESD 0.61, podcast 0.36 | MIT (противоречие в карточке) | нет | волна 16 кГц | 2026-08-04 |
| Aniemore/wavlm-bert-fusion-s-emotion-v1-crosslingual | WavLM и BERT, нужен текст | 0.5B | нет данных | есть | 7 классов | то же | RESD 0.66, podcast 0.49 | MIT | нет | волна и транскрипт | 2026-08-04 |
| KELONMYOSA/wav2vec2-xls-r-300m-emotion-ru | wav2vec2 XLS-R 300M | около 0.3B | 1207.5 MiB | нет | 5 классов Dusha | Dusha | нет данных | Apache-2.0 | нет | волна | 2023-06-05 |
| LessThanThree/whisper-tiny-finetuned-dusha | энкодер whisper-tiny и голова | 8.3M | 31.7 MiB | нет | 5 классов Dusha | Dusha (без карточки) | нет | не указана | нет | лог-мел whisper | 2024-09-01 |
