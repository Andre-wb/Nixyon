package dev.demo.nixyon.speech

import com.k2fsa.sherpa.onnx.FastClusteringConfig
import com.k2fsa.sherpa.onnx.FeatureConfig
import com.k2fsa.sherpa.onnx.OfflineModelConfig
import com.k2fsa.sherpa.onnx.OfflineRecognizer
import com.k2fsa.sherpa.onnx.OfflineRecognizerConfig
import com.k2fsa.sherpa.onnx.OfflineSpeakerDiarization
import com.k2fsa.sherpa.onnx.OfflineSpeakerDiarizationConfig
import com.k2fsa.sherpa.onnx.OfflineSpeakerSegmentationModelConfig
import com.k2fsa.sherpa.onnx.OfflineSpeakerSegmentationPyannoteModelConfig
import com.k2fsa.sherpa.onnx.OfflineSpeechDenoiserGtcrnModelConfig
import com.k2fsa.sherpa.onnx.OfflineSpeechDenoiserModelConfig
import com.k2fsa.sherpa.onnx.OfflineTransducerModelConfig
import com.k2fsa.sherpa.onnx.OnlineModelConfig
import com.k2fsa.sherpa.onnx.OnlineRecognizer
import com.k2fsa.sherpa.onnx.OnlineRecognizerConfig
import com.k2fsa.sherpa.onnx.OnlineSpeechDenoiser
import com.k2fsa.sherpa.onnx.OnlineSpeechDenoiserConfig
import com.k2fsa.sherpa.onnx.OnlineTransducerModelConfig
import com.k2fsa.sherpa.onnx.SileroVadModelConfig
import com.k2fsa.sherpa.onnx.SpeakerEmbeddingExtractorConfig
import com.k2fsa.sherpa.onnx.Vad
import com.k2fsa.sherpa.onnx.VadModelConfig

// Загруженные модели. Создаются при первом обращении и живут, пока другой большой модели
// хватает памяти: перед языковой моделью или моделью audEERING их можно отпустить
// (SpeechEngine.unload), следующий разбор загрузит их заново.
// Vad и шумоподавитель хранят состояние потока, поэтому здесь только их конфиги: на каждую
// обработку создаётся свой экземпляр. live: потоковая модель для живого текста, null,
// если её нет в наборе. denoiserConfig: шумоподавление для VAD, null, если его нет в наборе.
class SpeechModels private constructor(
    val located: LocatedModels,
    val vadConfig: VadModelConfig,
    val recognizer: OfflineRecognizer,
    val voice: VoiceModel,
    val diarizer: OfflineSpeakerDiarization,
    val live: OnlineRecognizer?,
    private val denoiserConfig: OfflineSpeechDenoiserModelConfig?,
) {
    // Новый поток шумоподавления на одну обработку; null без шумоподавителя в наборе.
    fun newDenoiser(): OnlineSpeechDenoiser? =
        denoiserConfig?.let { OnlineSpeechDenoiser(located.source.assetManager, OnlineSpeechDenoiserConfig(it)) }

    // Отпускает модели в порядке, обратном загрузке. Зовётся, только когда ими никто не
    // пользуется (ModelSlot); после этого ни одну из них трогать нельзя.
    fun release() {
        live?.let { runCatching { it.release() } }
        runCatching { diarizer.release() }
        runCatching { voice.close() }
        runCatching { recognizer.release() }
    }

    companion object {
        const val SAMPLE_RATE = 16000
        // У GigaAM признаки задаёт сама модель (метаданные is_giga_am), 80 будет переопределено.
        private const val FEATURE_DIM = 80
        private const val GIGAAM_MODEL_TYPE = "nemo_transducer"
        private const val MAX_SPEECH_SECONDS = 20f

        // Порог низкий, как в движке на ноутбуке: при 0,5 VAD пропускал тихую речь и
        // короткие слова целиком, а ложный кусок без речи распознаётся пустым текстом.
        private const val VAD_THRESHOLD = 0.2f
        // Поиск по лучам всегда, чтобы горячие слова работали без второй копии модели.
        // Значения те же, что у движка на ноутбуке (nixyon-sherpa: config::recognizer,
        // HOTWORD_SCORE, BEAM_PATHS); слова разбиваются на токены словарём BPE той же модели.
        private const val DECODING_METHOD = "modified_beam_search"
        private const val MODELING_UNIT = "bpe"
        private const val HOTWORD_SCORE = 1.5f
        private const val BEAM_PATHS = 8
        // Нейросегментация: значения те же, что у движка на ноутбуке (nixyon-sherpa,
        // config::diarization берёт значения по умолчанию Rust-крейта). Номера говорящих
        // sherpa конвейер не берёт, нужны только отрезки.
        private const val WINDOW_SHIFT_RATIO = 0.1f
        private const val CLUSTERS_UNKNOWN = -1
        private const val CLUSTER_THRESHOLD = 0.5f
        private const val MIN_DURATION_ON = 0.3f
        private const val MIN_DURATION_OFF = 0.5f
        // Живой текст: значения те же, что у движка на ноутбуке (nixyon-sherpa,
        // StreamModel::config): жадный поиск, один поток (телефон в это время пишет звук),
        // границы фраз по тишине с правилами sherpa-onnx по умолчанию.
        private const val LIVE_DECODING_METHOD = "greedy_search"
        private const val LIVE_THREADS = 1
        // GTCRN считает кадр за кадром, потоки ему почти не помогают.
        private const val DENOISER_THREADS = 1

        fun load(located: LocatedModels): SpeechModels {
            val source = located.source
            val manifest = located.manifest
            val threads = CpuCores.computeThreads
            val vadConfig = vadConfig(located)
            val denoiserConfig = if (manifest.hasDenoiser) denoiserConfig(located) else null
            val recognizerConfig = recognizerConfig(located, threads)
            val diarizationConfig = diarizationConfig(located, threads)
            // Что уже загружено: если следующая модель не загрузится, оно отпускается в
            // обратном порядке.
            val loaded = ArrayList<() -> Unit>()
            try {
                // Пробная загрузка VAD и шумоподавителя: ошибка в модели видна сразу, а не
                // при первой записи.
                Vad(source.assetManager, vadConfig).release()
                denoiserConfig?.let { OnlineSpeechDenoiser(source.assetManager, OnlineSpeechDenoiserConfig(it)).release() }
                val recognizer = OfflineRecognizer(source.assetManager, recognizerConfig)
                    .also { loaded.add(it::release) }
                val voice = VoiceModel.open(source, manifest.file(VoiceChoice.ROLE), threads)
                    .also { loaded.add(it::close) }
                val diarizer = OfflineSpeakerDiarization(source.assetManager, diarizationConfig)
                    .also { loaded.add(it::release) }
                val live = if (manifest.hasLiveModel) OnlineRecognizer(source.assetManager, liveConfig(located)) else null
                return SpeechModels(located, vadConfig, recognizer, voice, diarizer, live, denoiserConfig)
            } catch (e: Throwable) {
                loaded.asReversed().forEach { release -> runCatching(release) }
                if (e is ModelsException) throw e
                throw ModelsException(ModelsException.LOAD_FAILED, "sherpa-onnx не загрузил модели: ${e.message}", e)
            }
        }

        private fun vadConfig(located: LocatedModels) =
            VadModelConfig(
                sileroVadModelConfig = SileroVadModelConfig(
                    model = located.source.pathOf(located.manifest.file(ModelRole.VAD)),
                    threshold = VAD_THRESHOLD,
                    maxSpeechDuration = MAX_SPEECH_SECONDS,
                ),
                sampleRate = SAMPLE_RATE,
                numThreads = 1,
            )

        private fun denoiserConfig(located: LocatedModels) =
            OfflineSpeechDenoiserModelConfig(
                gtcrn = OfflineSpeechDenoiserGtcrnModelConfig(model = located.source.pathOf(located.manifest.file(ModelRole.DENOISER))),
                numThreads = DENOISER_THREADS,
            )

        private fun recognizerConfig(located: LocatedModels, threads: Int): OfflineRecognizerConfig {
            val source = located.source
            val manifest = located.manifest
            return OfflineRecognizerConfig(
                featConfig = FeatureConfig(sampleRate = SAMPLE_RATE, featureDim = FEATURE_DIM),
                modelConfig = OfflineModelConfig(
                    transducer = OfflineTransducerModelConfig(
                        encoder = source.pathOf(manifest.file(ModelRole.ASR_ENCODER)),
                        decoder = source.pathOf(manifest.file(ModelRole.ASR_DECODER)),
                        joiner = source.pathOf(manifest.file(ModelRole.ASR_JOINER)),
                    ),
                    tokens = source.pathOf(manifest.file(ModelRole.ASR_TOKENS)),
                    modelType = GIGAAM_MODEL_TYPE,
                    numThreads = threads,
                    modelingUnit = MODELING_UNIT,
                    bpeVocab = source.pathOf(manifest.file(ModelRole.ASR_BPE_VOCAB)),
                ),
                decodingMethod = DECODING_METHOD,
                maxActivePaths = BEAM_PATHS,
                hotwordsScore = HOTWORD_SCORE,
            )
        }

        // CAM++ нужен только диаризатору: без модели голоса он не создаётся. Его номера
        // говорящих конвейер не берёт, эмбеддинги отрезков считает VoiceModel.
        private fun diarizationConfig(located: LocatedModels, threads: Int): OfflineSpeakerDiarizationConfig {
            val source = located.source
            val manifest = located.manifest
            return OfflineSpeakerDiarizationConfig(
                segmentation = OfflineSpeakerSegmentationModelConfig(
                    pyannote = OfflineSpeakerSegmentationPyannoteModelConfig(
                        model = source.pathOf(manifest.file(ModelRole.SEGMENTATION)),
                        windowShiftRatio = WINDOW_SHIFT_RATIO,
                    ),
                    numThreads = threads,
                    debug = false,
                    provider = "cpu",
                ),
                embedding = SpeakerEmbeddingExtractorConfig(
                    model = source.pathOf(manifest.file(ModelRole.EMBEDDING)),
                    numThreads = threads,
                ),
                clustering = FastClusteringConfig(
                    numClusters = CLUSTERS_UNKNOWN,
                    threshold = CLUSTER_THRESHOLD,
                    computeConfidence = false,
                ),
                minDurationOn = MIN_DURATION_ON,
                minDurationOff = MIN_DURATION_OFF,
            )
        }

        private fun liveConfig(located: LocatedModels): OnlineRecognizerConfig {
            val source = located.source
            val manifest = located.manifest
            return OnlineRecognizerConfig(
                featConfig = FeatureConfig(sampleRate = SAMPLE_RATE, featureDim = FEATURE_DIM),
                modelConfig = OnlineModelConfig(
                    transducer = OnlineTransducerModelConfig(
                        encoder = source.pathOf(manifest.file(ModelRole.LIVE_ENCODER)),
                        decoder = source.pathOf(manifest.file(ModelRole.LIVE_DECODER)),
                        joiner = source.pathOf(manifest.file(ModelRole.LIVE_JOINER)),
                    ),
                    tokens = source.pathOf(manifest.file(ModelRole.LIVE_TOKENS)),
                    numThreads = LIVE_THREADS,
                ),
                enableEndpoint = true,
                decodingMethod = LIVE_DECODING_METHOD,
            )
        }
    }
}
