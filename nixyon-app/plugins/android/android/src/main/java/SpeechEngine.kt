package dev.demo.nixyon.speech

import android.content.Context
import android.os.SystemClock
import android.util.Log
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

class EngineStatus(val source: String, val version: String, val loadMs: Long, val embeddingDim: Int)

// Эмбеддинг образца и имя файла модели голоса: по нему образец сверяется с моделью,
// которой считается запись.
class SampleResult(val embedding: FloatArray, val voiceModel: String)

// Запрос нейросегментации: куда писать отрезки и длина блока в секундах.
class NeuralRequest(val out: File, val blockSec: Double)

// Ленивая загрузка моделей: первый вызов грузит, остальные получают готовое.
// Mutex не даёт двум одновременным вызовам грузить модели дважды. Когда другой большой
// модели не хватает памяти, модели речи отпускаются (unload), следующий вызов грузит заново.
class SpeechEngine(private val context: Context) {
    private val mutex = Mutex()
    // Живой текст берёт модели без мьютекса из потока начала записи и держит до конца записи.
    private val slot = ModelSlot<SpeechModels> { it.release() }
    private var loadMs: Long = 0
    private val running = AtomicBoolean(false)
    @Volatile
    private var cancelled = AtomicBoolean(false)

    suspend fun status(): EngineStatus =
        mutex.withLock {
            val loaded = loadLocked()
            EngineStatus(
                source = loaded.located.source.label,
                version = loaded.located.manifest.version,
                loadMs = loadMs,
                embeddingDim = loaded.voice.dim,
            )
        }

    // Одна обработка за раз: модели общие, а две обработки поделили бы ядра пополам.
    // language: язык речи; null: определить по записи.
    // spans: нейросегментация вторым проходом (файл отрезков и длина блока), null: без неё.
    // overlaps: поиск перекрытий по модели сегментации, null: без него. Оба прохода идут в
    // фазе голосов и делят её полосу пополам.
    suspend fun analyze(
        wav: File,
        out: File,
        windows: WindowSpans?,
        hotwords: String,
        language: SpeechLanguage?,
        spans: NeuralRequest?,
        overlaps: OverlapRequest?,
        progress: (phase: String, doneSec: Double, totalSec: Double) -> Unit,
    ): AnalysisSummary = exclusive {
        val flag = AtomicBoolean(false)
        cancelled = flag
        val models = ensureLoaded()
        var phase = PhaseTiming(models.voice)
        val summary = SpeechAnalyzer(models, windows, hotwords, language).run(wav, out, flag) { done, total ->
            progress(PHASE_RECOGNIZE, done, total)
        }
        phase.log(PHASE_RECOGNIZE, summary.durationSec)
        val parts = listOfNotNull(spans, overlaps).size
        if (spans != null) {
            phase = PhaseTiming(models.voice)
            val share = SplitProgress(0, parts) { done, total -> progress(PHASE_VOICES, done, total) }
            val found = SpeechSegmenter(models, spans.blockSec).run(wav, summary.speech, flag, share::report)
            phase.log(PHASE_VOICES, summary.durationSec)
            SpansWriter.write(spans.out, found)
        }
        if (overlaps != null) {
            phase = PhaseTiming(models.voice)
            val share = SplitProgress(parts - 1, parts) { done, total -> progress(PHASE_VOICES, done, total) }
            val pass = AnalysisProgress(flag, share::report)
            models.powerset.scan(wav, overlaps, flag) { done -> pass.report(done, summary.durationSec) }
            pass.done(summary.durationSec)
            phase.log(PHASE_OVERLAPS, summary.durationSec)
        }
        summary
    }

    // Эмбеддинг образца голоса и модель, которой он посчитан. Занимает движок так же,
    // как разбор записи.
    suspend fun sampleEmbedding(wav: File, minSpeechSec: Double): SampleResult = exclusive {
        val models = ensureLoaded()
        val embedding = SampleEmbedder(models).run(wav, minSpeechSec)
        SampleResult(embedding, models.located.manifest.file(VoiceChoice.ROLE).path)
    }

    // Ребёнок или взрослый по кускам речи спикеров. null, если модели audEERING нет в наборе
    // или на неё не хватает памяти даже без моделей речи: метки считаются до сохранения
    // расшифровки, и падение по памяти здесь потеряло бы весь разбор, а без меток она
    // сохраняется как есть. Занимает движок так же, как разбор записи.
    // Отмена: флаг разбора, за которым идёт этот шаг. Со своим флагом отмена, нажатая
    // между концом разбора и началом шага, терялась, и модель грузилась впустую.
    suspend fun speakerAges(wav: File, speakers: List<AgeSpeaker>): List<AgeVerdict>? = exclusive {
        val flag = cancelled
        val located = ensureLoaded().located
        if (!located.manifest.hasAgeGender) return@exclusive null
        val modelBytes = located.manifest.file(ModelRole.AGE_GENDER).size
        val inApk = located.source is ModelSource.Apk
        val enough = { memory: MemorySnapshot -> AgeModel.isEnoughMemory(memory, modelBytes, inApk) }
        val (memory, freed) = DeviceMemory.check({ DeviceMemory.read(context) }, enough, ::unloadIdle)
        if (freed) Log.i(TAG, "для модели audEERING отпущены модели речи: свободно ${memory.availableBytes shr 20} МБ")
        if (!enough(memory)) {
            Log.w(TAG, "на модель audEERING не хватает памяти (свободно ${memory.availableBytes shr 20} МБ): без меток ребёнок или взрослый")
            return@exclusive null
        }
        SpeakerAges(located).run(wav, speakers, flag)
    }

    // Эмоция каждой реплики. null, если модели эмоций нет в наборе или на неё не хватает
    // памяти даже без моделей речи: как у меток возраста, без эмоций расшифровка
    // сохраняется как есть. Занимает движок и слушает отмену разбора так же.
    suspend fun turnEmotions(wav: File, turns: List<EmotionTurn>): List<EmotionVerdict>? = exclusive {
        val flag = cancelled
        val located = ensureLoaded().located
        if (!located.manifest.hasEmotion) return@exclusive null
        val modelBytes = located.manifest.file(ModelRole.EMOTION).size
        val inApk = located.source is ModelSource.Apk
        val enough = { memory: MemorySnapshot -> EmotionModel.isEnoughMemory(memory, modelBytes, inApk) }
        val (memory, freed) = DeviceMemory.check({ DeviceMemory.read(context) }, enough, ::unloadIdle)
        if (freed) Log.i(TAG, "для модели эмоций отпущены модели речи: свободно ${memory.availableBytes shr 20} МБ")
        if (!enough(memory)) {
            Log.w(TAG, "на модель эмоций не хватает памяти (свободно ${memory.availableBytes shr 20} МБ): без эмоций")
            return@exclusive null
        }
        TurnEmotions(located).run(wav, turns, flag)
    }

    // Живой текст на одну запись, если модели уже загружены. Не ждёт загрузки: запись
    // начинается сразу, а без готовых моделей просто идёт без живого текста. Пока поток
    // живого текста не закончился, модели речи не отпускаются.
    fun startLive(rate: Int, onLines: (List<String>) -> Unit): LiveTranscriber? {
        val hold = slot.hold() ?: return null
        val recognizer = hold.models.live
        if (recognizer == null) {
            hold.done()
            return null
        }
        return try {
            LiveTranscriber(recognizer, rate, hold::done, onLines)
        } catch (e: Throwable) {
            hold.done()
            throw e
        }
    }

    fun cancel() {
        cancelled.set(true)
    }

    // Отпускает модели речи, чтобы языковой модели хватило памяти. Только когда движок
    // свободен: ни разбора, ни загрузки, ни записи с живым текстом. true, если отпустил.
    fun unload(): Boolean {
        if (!running.compareAndSet(false, true)) return false
        try {
            return unloadIdle()
        } finally {
            running.set(false)
        }
    }

    // То же, когда движок уже занят тем, кто зовёт (running).
    private fun unloadIdle(): Boolean {
        if (!mutex.tryLock()) return false
        val released = try {
            slot.unload()
        } finally {
            mutex.unlock()
        }
        if (released) Log.i(TAG, "модели речи отпущены, следующий разбор загрузит их заново")
        return released
    }

    // Пока идёт block, второй разбор или образец получает busy.
    private inline fun <T> exclusive(block: () -> T): T {
        if (!running.compareAndSet(false, true)) {
            throw AnalysisException(AnalysisException.BUSY, "уже идёт обработка")
        }
        try {
            return block()
        } finally {
            running.set(false)
        }
    }

    private suspend fun ensureLoaded(): SpeechModels = mutex.withLock { loadLocked() }

    // Под mutex.
    private fun loadLocked(): SpeechModels =
        slot.current ?: run {
            val started = SystemClock.elapsedRealtime()
            val located = ModelLocator(context).locate()
            val loaded = SpeechModels.load(located)
            loadMs = SystemClock.elapsedRealtime() - started
            slot.put(loaded)
            loaded
        }

    companion object {
        private const val TAG = "NixyonSpeech"

        // Этапы прогресса; Rust переводит их в этапы интерфейса.
        const val PHASE_RECOGNIZE = "recognize"
        const val PHASE_VOICES = "voices"
        // Только для строки замера в логе: в интерфейсе это та же фаза голосов.
        private const val PHASE_OVERLAPS = "overlaps"
    }
}
