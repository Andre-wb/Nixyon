package dev.demo.nixyon.speech

import android.Manifest
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.webkit.WebView
import android.util.Log
import androidx.activity.result.ActivityResult
import androidx.appcompat.app.AppCompatActivity
import androidx.biometric.BiometricManager
import androidx.fragment.app.FragmentActivity
import androidx.core.content.FileProvider
import androidx.core.view.WindowCompat
import app.tauri.PermissionState
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import javax.crypto.SecretKey

@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.RECORD_AUDIO], alias = SpeechPlugin.MICROPHONE),
        Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = SpeechPlugin.NOTIFICATIONS),
    ],
)
class SpeechPlugin(private val activity: Activity) : Plugin(activity) {
    // Команды Tauri приходят в главном потоке; загрузка моделей идёт в фоне,
    // иначе Android закроет приложение по ANR.
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val engine = SpeechEngine(activity.applicationContext)
    private val recordings = RecordingsDir(activity.filesDir)
    private val keys = StorageKey(activity.applicationContext)
    private val recorder = AudioRecorder(activity.applicationContext, recordings, SamplesDir(activity.filesDir), keys)
    private val importer = AudioImporter(activity.applicationContext, recordings, keys)
    private val player = AudioPlayer(activity.applicationContext, keys)
    // Если языковой модели не хватает памяти, движок отпускает модели речи.
    private val summarizer = MeetingSummarizer(activity.applicationContext, engine::unload)
    private val importing = AtomicBoolean(false)
    private var webView: WebView? = null
    @Volatile
    private var importCancelled = AtomicBoolean(false)
    private val installing = AtomicBoolean(false)
    @Volatile
    private var installCancelled = AtomicBoolean(false)

    @Command
    fun engineStatus(invoke: Invoke) {
        scope.launch {
            try {
                val status = engine.status()
                Log.i(TAG, "модели: ${status.source} (${status.version}), загрузка ${status.loadMs} мс")
                invoke.resolve(
                    jsObject(
                        "source" to status.source,
                        "version" to status.version,
                        "loadMs" to status.loadMs,
                        "embeddingDim" to status.embeddingDim,
                    ),
                )
            } catch (e: ModelsException) {
                Log.e(TAG, "модели не загружены: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "модели не загружены", e)
                invoke.reject("internal error", ModelsException.LOAD_FAILED)
            }
        }
    }

    @Command
    fun analyze(invoke: Invoke) {
        val args = invoke.parseArgs(AnalyzeArgs::class.java)
        scope.launch {
            try {
                val windows = WindowSpans.fromRequest(args.windowSec, args.stepSec, SpeechModels.SAMPLE_RATE)
                val neural = neuralRequest(args.spansPath, args.neuralBlockSec)
                val summary = engine.analyze(File(args.wavPath), File(args.outPath), windows, args.hotwords, neural) { phase, done, total ->
                    args.onProgress?.send(jsObject("phase" to phase, "doneSec" to done, "totalSec" to total))
                }
                Log.i(TAG, "разбор: кусков речи ${summary.segments}, запись ${summary.durationSec} с")
                invoke.resolve(
                    jsObject(
                        "segments" to summary.segments,
                        "durationSec" to summary.durationSec,
                        "vadModel" to summary.vadModel,
                        "asrModels" to summary.asrModels,
                        "embeddingModel" to summary.embeddingModel,
                        "sameSpeakerCosine" to summary.sameSpeakerCosine,
                    ),
                )
            } catch (e: AnalysisException) {
                if (e.code == AnalysisException.CANCELLED) Log.i(TAG, "разбор отменён") else Log.e(TAG, "разбор не удался: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: ModelsException) {
                Log.e(TAG, "модели не загружены: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "разбор не удался", e)
                invoke.reject("internal error", AnalysisException.ENGINE_FAILED)
            }
        }
    }

    // 0 и без файла: нейросегментация не нужна. Отрицательный или бесконечный блок
    // и блок без файла отклоняются.
    private fun neuralRequest(path: String?, blockSec: Double): NeuralRequest? {
        if (blockSec == 0.0 && path == null) return null
        if (path == null || !blockSec.isFinite() || blockSec <= 0.0) {
            throw AnalysisException(AnalysisException.ENGINE_FAILED, "нейросегментация: блок $blockSec с, файл $path")
        }
        return NeuralRequest(File(path), blockSec)
    }

    // Эмбеддинг образца голоса по WAV 16 кГц. Имя и эмбеддинг в лог не пишутся.
    @Command
    fun sampleEmbedding(invoke: Invoke) {
        val args = invoke.parseArgs(SampleArgs::class.java)
        scope.launch {
            try {
                val sample = engine.sampleEmbedding(File(args.wavPath), args.minSpeechSec)
                invoke.resolve(jsObject("embedding" to sample.embedding.toJsonArray(), "voiceModel" to sample.voiceModel))
            } catch (e: AnalysisException) {
                Log.i(TAG, "образец голоса не принят: ${e.code}")
                invoke.reject(e.message, e.code)
            } catch (e: ModelsException) {
                Log.e(TAG, "модели не загружены: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "образец голоса не обработан", e)
                invoke.reject("internal error", AnalysisException.ENGINE_FAILED)
            }
        }
    }

    // Ребёнок или взрослый по голосу спикеров. Звук и ответы в лог не пишутся, только число
    // спикеров.
    @Command
    fun speakerAges(invoke: Invoke) {
        val args = invoke.parseArgs(SpeakerAgesArgs::class.java)
        scope.launch {
            try {
                val speakers = args.speakers.map { s -> AgeSpeaker(s.id, s.pieces.map { AgePiece(it.startSec, it.endSec) }) }
                val started = System.nanoTime()
                val verdicts = engine.speakerAges(File(args.wavPath), speakers)
                if (verdicts == null) {
                    invoke.resolve(jsObject("available" to false, "speakers" to JSONArray()))
                    return@launch
                }
                Log.i(TAG, "ребёнок или взрослый: спикеров ${verdicts.size}, ${(System.nanoTime() - started) / 1_000_000} мс")
                val out = JSONArray()
                for (v in verdicts) {
                    out.put(jsObject("id" to v.id, "child" to (v.child ?: JSONObject.NULL), "female" to (v.female ?: JSONObject.NULL)))
                }
                invoke.resolve(jsObject("available" to true, "speakers" to out))
            } catch (e: AnalysisException) {
                if (e.code == AnalysisException.CANCELLED) Log.i(TAG, "разбор голоса отменён") else Log.e(TAG, "ребёнок или взрослый: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: ModelsException) {
                Log.e(TAG, "модель audEERING не загружена: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "ребёнок или взрослый не определён", e)
                invoke.reject("internal error", AnalysisException.ENGINE_FAILED)
            }
        }
    }

    // Эмоция каждой реплики по голосу. Звук и ответы в лог не пишутся, только число реплик.
    @Command
    fun turnEmotions(invoke: Invoke) {
        val args = invoke.parseArgs(TurnEmotionsArgs::class.java)
        scope.launch {
            try {
                val turns = args.turns.map { EmotionTurn(it.id, it.startSec, it.endSec) }
                val started = System.nanoTime()
                val verdicts = engine.turnEmotions(File(args.wavPath), turns)
                if (verdicts == null) {
                    invoke.resolve(jsObject("available" to false, "turns" to JSONArray()))
                    return@launch
                }
                Log.i(TAG, "эмоции: реплик ${verdicts.size}, ${(System.nanoTime() - started) / 1_000_000} мс")
                val out = JSONArray()
                for (v in verdicts) {
                    out.put(jsObject("id" to v.id, "emotion" to (v.emotion ?: JSONObject.NULL)))
                }
                invoke.resolve(jsObject("available" to true, "turns" to out))
            } catch (e: AnalysisException) {
                if (e.code == AnalysisException.CANCELLED) Log.i(TAG, "эмоции отменены") else Log.e(TAG, "эмоции: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: ModelsException) {
                Log.e(TAG, "модель эмоций не загружена: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "эмоции не определены", e)
                invoke.reject("internal error", AnalysisException.ENGINE_FAILED)
            }
        }
    }

    // Значки строки состояния и навигации под тему интерфейса: светлые на тёмной теме,
    // тёмные на светлой. Без этого при ручной теме они следуют системной.
    @Command
    fun setSystemBars(invoke: Invoke) {
        val args = invoke.parseArgs(SystemBarsArgs::class.java)
        activity.runOnUiThread {
            val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
            controller.isAppearanceLightStatusBars = !args.dark
            controller.isAppearanceLightNavigationBars = !args.dark
            invoke.resolve()
        }
    }

    @Command
    fun cancelAnalysis(invoke: Invoke) {
        engine.cancel()
        invoke.resolve()
    }

    // Есть ли на устройстве языковая модель для протокола встречи.
    @Command
    fun summaryStatus(invoke: Invoke) {
        invoke.resolve(jsObject("available" to summarizer.isAvailable()))
    }

    // Протокол встречи языковой моделью. Текст встречи и ответ в лог не пишутся.
    @Command
    fun summarize(invoke: Invoke) {
        val args = invoke.parseArgs(SummaryArgs::class.java)
        scope.launch {
            try {
                val started = System.nanoTime()
                val summary = summarizer.summarizeMeeting(args.system, args.user, args.maxTokens, progressTo(args.onProgress))
                Log.i(TAG, "протокол встречи: ${(System.nanoTime() - started) / 1_000_000} мс, частей ${summary.parts}, сведений ${summary.combines}")
                invoke.resolve(jsObject("text" to summary.text))
            } catch (e: LlmException) {
                if (e.code == LlmException.CANCELLED) Log.i(TAG, "протокол отменён") else Log.e(TAG, "протокол не составлен: ${e.code}: ${e.message}")
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "протокол не составлен", e)
                invoke.reject("internal error", LlmException.FAILED)
            }
        }
    }

    // Подсказки имён участников из самого разговора той же языковой моделью (SpeakerNamer).
    // Если в разговоре нет слов, похожих на имена, модель не грузится и ответ пустой.
    // Реплики, ответ модели и имена в лог не пишутся. Отменяет cancelSummary.
    @Command
    fun suggestSpeakerNames(invoke: Invoke) {
        val args = invoke.parseArgs(SpeakerNamesArgs::class.java)
        scope.launch {
            try {
                val started = System.nanoTime()
                val progress = progressTo(args.onProgress)
                val namer = SpeakerNamer { system, user, maxTokens -> summarizer.summarize(system, user, maxTokens, progress) }
                val found = namer.suggest(
                    args.speakers.map { NamingSpeaker(it.id, it.name) },
                    args.turns.map { NamingTurn(it.id, it.speaker, it.text) },
                )
                Log.i(TAG, "имена участников: подсказок ${found.size}, ${(System.nanoTime() - started) / 1_000_000} мс")
                val list = JSONArray()
                for (suggestion in found) {
                    list.put(jsObject("speaker" to suggestion.speaker, "name" to suggestion.name, "turn" to suggestion.turn))
                }
                invoke.resolve(jsObject("suggestions" to list))
            } catch (e: LlmException) {
                if (e.code == LlmException.CANCELLED) Log.i(TAG, "подсказка имён отменена") else Log.e(TAG, "имена не подсказаны: ${e.code}: ${e.message}")
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "имена не подсказаны", e)
                invoke.reject("internal error", LlmException.FAILED)
            }
        }
    }

    @Command
    fun cancelSummary(invoke: Invoke) {
        summarizer.cancel()
        invoke.resolve()
    }

    // Ход работы модели в канал интерфейса: читает запрос или пишет ответ.
    private fun progressTo(channel: Channel?) = LlmProgress { phase, done, total ->
        val name = if (phase == LlmProgress.READING) "reading" else "writing"
        channel?.send(jsObject("phase" to name, "done" to done, "total" to total))
    }

    // Папки приложения: только из них ядро соглашается брать записи на обработку.
    // Ключ данных для ядра: шифрование расшифровки, голосов, настроек и чтение записи.
    // Команд для JS у плагина нет, интерфейс эту команду вызвать не может. В лог не пишется.
    // Ядро спрашивает ключ один раз за запуск, до подготовки хранилища, которая удаляет
    // недописанные файлы: поэтому записи, оборванные падением, восстанавливаются здесь.
    // Пока ключ закрыт и вход по отпечатку включён, спрашивает палец; при отмене отвечает
    // locked.
    @Command
    fun storageKey(invoke: Invoke) {
        val host = activity as? FragmentActivity
        if (host == null) {
            invoke.reject("storage key failed", "storage_failed")
            return
        }
        keys.unlock(host) { result ->
            scope.launch {
                try {
                    val key = result.getOrThrow()
                    recoverOnce(key)
                    val raw = key.encoded
                    val ret = jsObject("key" to JSONArray(raw.map { it.toInt() and 0xff }))
                    raw.fill(0)
                    invoke.resolve(ret)
                } catch (e: StorageLockedException) {
                    invoke.reject("storage locked", e.code)
                } catch (e: Throwable) {
                    Log.e(TAG, "ключ данных недоступен", e)
                    invoke.reject("storage key failed", "storage_failed")
                }
            }
        }
    }

    // Удаляет ключ, который система стёрла, чтобы начать с чистого хранилища. Плагин
    // соглашается, только если последнее открытие нашло ключ стёртым.
    @Command
    fun resetStorageKey(invoke: Invoke) {
        try {
            keys.reset()
            invoke.resolve()
        } catch (e: StorageLockedException) {
            invoke.reject("storage key is not lost", e.code)
        } catch (e: Throwable) {
            Log.e(TAG, "стёртый ключ данных не удалён", e)
            invoke.reject("storage key reset failed", "storage_failed")
        }
    }

    // Включён ли вход по отпечатку и есть ли на телефоне отпечаток.
    @Command
    fun biometricStatus(invoke: Invoke) {
        try {
            val status = keys.biometricStatus()
            invoke.resolve(jsObject("enabled" to status.enabled, "available" to status.available))
        } catch (e: Throwable) {
            Log.e(TAG, "состояние входа по отпечатку не прочитано", e)
            invoke.reject("biometric status failed", "storage_failed")
        }
    }

    // Включает или выключает вход по отпечатку. Система спрашивает палец в обе стороны.
    @Command
    fun setBiometric(invoke: Invoke) {
        val args = invoke.parseArgs(BiometricArgs::class.java)
        val host = activity as? FragmentActivity
        if (host == null) {
            invoke.reject("biometric switch failed", "storage_failed")
            return
        }
        keys.setBiometric(host, args.enabled) { result ->
            result.fold(
                onSuccess = { invoke.resolve() },
                onFailure = { e ->
                    if (e is StorageLockedException) {
                        invoke.reject("biometric switch refused", e.code)
                    } else {
                        Log.e(TAG, "вход по отпечатку не переключён", e)
                        invoke.reject("biometric switch failed", "storage_failed")
                    }
                },
            )
        }
    }

    // Добавление отпечатка в настройках Android. Прямой экран добавления есть с Android 11;
    // раньше и там, где его нет, открываются настройки безопасности.
    @Command
    fun openBiometricSettings(invoke: Invoke) {
        val enroll = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Intent(Settings.ACTION_BIOMETRIC_ENROLL)
                .putExtra(Settings.EXTRA_BIOMETRIC_AUTHENTICATORS_ALLOWED, BiometricManager.Authenticators.BIOMETRIC_STRONG)
        } else {
            null
        }
        for (intent in listOfNotNull(enroll, Intent(Settings.ACTION_SECURITY_SETTINGS))) {
            try {
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                activity.startActivity(intent)
                invoke.resolve()
                return
            } catch (e: ActivityNotFoundException) {
                Log.w(TAG, "экран настроек отпечатка не найден: ${intent.action}")
            } catch (e: Throwable) {
                Log.e(TAG, "настройки отпечатка не открылись", e)
                break
            }
        }
        invoke.reject("settings failed", "storage_failed")
    }

    // Один раз за процесс. Новая запись и импорт начинаются только после подготовки
    // хранилища ядром (media.rs), поэтому ни один `.part` здесь сейчас не пишется.
    private fun recoverOnce(key: SecretKey) {
        if (!recoveryDone.compareAndSet(false, true)) return
        try {
            val found = recordings.recoverParts(key)
            for (r in found) Log.i(TAG, "восстановлена запись, оборванная падением приложения: ${r.durationSec} с")
            found.lastOrNull()?.let { recovered.set(it) }
        } catch (e: Throwable) {
            Log.e(TAG, "оборванные записи не восстановлены", e)
        }
    }

    // Запись, восстановленная при этом запуске после падения приложения: путь, длительность
    // и причина "crashed"; если восстанавливать было нечего, путь null. Отдаётся один раз,
    // чтобы интерфейс предложил её обработать только однажды. Ядро должно звать после
    // подготовки хранилища, иначе восстановление ещё не прошло.
    @Command
    fun recoveredRecording(invoke: Invoke) {
        val found = recovered.getAndSet(null) ?: RecordingResult(null, 0.0, RecordingsDir.CRASHED)
        invoke.resolve(recordingJson(found))
    }

    @Command
    fun storagePaths(invoke: Invoke) {
        invoke.resolve(
            jsObject(
                "files" to activity.filesDir.absolutePath,
                "cache" to activity.cacheDir.absolutePath,
                "externalFiles" to activity.getExternalFilesDir(null)?.absolutePath,
            ),
        )
    }

    // Разрешение на микрофон спрашивается в момент нужды: при первой записи.
    @Command
    fun startRecording(invoke: Invoke) {
        // Уведомления спрашиваются вместе с микрофоном и только пока не отвечено: отказ
        // запись не блокирует, уведомление тогда просто не видно в шторке.
        val aliases = mutableListOf<String>()
        if (getPermissionState(MICROPHONE) != PermissionState.GRANTED) aliases.add(MICROPHONE)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && notificationsUnanswered()) aliases.add(NOTIFICATIONS)
        if (aliases.isNotEmpty()) {
            requestPermissionForAliases(aliases.toTypedArray(), invoke, "afterMicrophonePermission")
            return
        }
        beginRecording(invoke)
    }

    private fun notificationsUnanswered(): Boolean {
        val state = getPermissionState(NOTIFICATIONS)
        return state == PermissionState.PROMPT || state == PermissionState.PROMPT_WITH_RATIONALE
    }

    @PermissionCallback
    fun afterMicrophonePermission(invoke: Invoke) {
        if (getPermissionState(MICROPHONE) == PermissionState.GRANTED) {
            beginRecording(invoke)
        } else {
            Log.i(TAG, "в доступе к микрофону отказано")
            invoke.reject("microphone permission denied", MediaException.MIC_DENIED)
        }
    }

    private fun beginRecording(invoke: Invoke) {
        val args = invoke.parseArgs(RecordArgs::class.java)
        scope.launch {
            try {
                var liveText = false
                val rate = recorder.start(
                    sample = args.sample,
                    onLevel = { dbfs ->
                        args.onEvent?.send(jsObject("type" to "level", "dbfs" to dbfs.toDouble()))
                    },
                    onStopped = { result ->
                        RecordingService.stop(activity.applicationContext)
                        Log.i(TAG, "запись остановлена: ${result.reason}, ${result.durationSec} с")
                        args.onEvent?.send(recordingJson(result).also { it.put("type", "stopped") })
                    },
                    live = { rate ->
                        engine.startLive(rate) { lines ->
                            args.onEvent?.send(jsObject("type" to "live", "lines" to JSONArray(lines)))
                        }.also { liveText = it != null }
                    },
                )
                if (!RecordingService.start(activity.applicationContext, args.notificationTitle, args.notificationChannel)) {
                    Log.w(TAG, "запись без foreground service: при сворачивании остановится")
                }
                Log.i(TAG, "запись идёт, $rate Гц, живой текст: ${if (liveText) "есть" else "нет"}")
                invoke.resolve(jsObject("sampleRate" to rate))
            } catch (e: MediaException) {
                Log.e(TAG, "запись не началась: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "запись не началась", e)
                invoke.reject("internal error", MediaException.MIC_UNAVAILABLE)
            }
        }
    }

    @Command
    fun stopRecording(invoke: Invoke) {
        scope.launch {
            try {
                val result = try {
                    recorder.stop("user")
                } finally {
                    RecordingService.stop(activity.applicationContext)
                }
                Log.i(TAG, "запись сохранена: ${result.durationSec} с")
                invoke.resolve(recordingJson(result))
            } catch (e: MediaException) {
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "запись не сохранилась", e)
                invoke.reject("internal error", MediaException.MIC_UNAVAILABLE)
            }
        }
    }

    @Command
    fun pauseRecording(invoke: Invoke) = setPaused(invoke, true)

    @Command
    fun resumeRecording(invoke: Invoke) = setPaused(invoke, false)

    private fun setPaused(invoke: Invoke, paused: Boolean) {
        val args = invoke.parseArgs(PauseArgs::class.java)
        try {
            val recorded = if (paused) recorder.pause() else recorder.resume()
            RecordingService.update(activity.applicationContext, args.notificationTitle, recorded, paused)
            invoke.resolve()
        } catch (e: MediaException) {
            invoke.reject(e.message, e.code)
        }
    }

    // Свёрнутому приложению без foreground service Android даёт вместо звука тишину,
    // поэтому, если сервис не запустился, запись останавливается и сохраняется; интерфейс
    // узнаёт причину из события. С сервисом запись продолжается в фоне.
    override fun onStop(activity: AppCompatActivity) {
        // Проигрывание в фоне не нужно: свёрнутое приложение встаёт на паузу.
        if (player.isOpen) player.pause()
        if (recorder.isRecording && !RecordingService.active.get()) {
            scope.launch {
                try {
                    recorder.stop("background")
                } catch (_: MediaException) {
                }
            }
        }
    }

    // Системный выбор файла: доступ только к выбранному файлу, без разрешения на память.
    @Command
    fun importAudio(invoke: Invoke) {
        if (!importing.compareAndSet(false, true)) {
            invoke.reject("import already running", MediaException.BUSY)
            return
        }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("audio/*")
        try {
            startActivityForResult(invoke, intent, "afterPick")
        } catch (e: Throwable) {
            importing.set(false)
            Log.e(TAG, "выбор файла не открылся", e)
            invoke.reject("picker failed", MediaException.IMPORT_FAILED)
        }
    }

    @ActivityCallback
    fun afterPick(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            importing.set(false)
            invoke.resolve(jsObject("cancelled" to true))
            return
        }
        val args = invoke.parseArgs(ImportArgs::class.java)
        val flag = AtomicBoolean(false)
        importCancelled = flag
        scope.launch {
            try {
                val imported = importer.import(uri, flag) { done, total ->
                    args.onProgress?.send(jsObject("doneSec" to done, "totalSec" to total))
                }
                Log.i(TAG, "импорт: ${imported.durationSec} с")
                invoke.resolve(
                    jsObject(
                        "cancelled" to false,
                        "path" to imported.path,
                        "durationSec" to imported.durationSec,
                        "name" to imported.name,
                    ),
                )
            } catch (e: MediaException) {
                if (e.code == MediaException.CANCELLED) Log.i(TAG, "импорт отменён") else Log.e(TAG, "импорт не удался: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "импорт не удался", e)
                invoke.reject("internal error", MediaException.IMPORT_FAILED)
            } finally {
                importing.set(false)
            }
        }
    }

    // WebView сам не следует системному размеру шрифта: переносим его в масштаб текста,
    // чтобы крупный шрифт из настроек Android работал и в приложении.
    override fun load(webView: WebView) {
        this.webView = webView
        applyFontScale(activity.resources.configuration)
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        applyFontScale(newConfig)
    }

    private fun applyFontScale(config: Configuration) {
        webView?.settings?.textZoom = (config.fontScale * 100).toInt()
    }

    // Страница приложения в настройках Android: там пользователь включает микрофон,
    // если раньше запретил доступ.
    @Command
    fun openAppSettings(invoke: Invoke) {
        try {
            val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", activity.packageName, null))
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            activity.startActivity(intent)
            invoke.resolve()
        } catch (e: Throwable) {
            Log.e(TAG, "настройки приложения не открылись", e)
            invoke.reject("settings failed", MediaException.MIC_UNAVAILABLE)
        }
    }

    // «Сохранить»: системный диалог выбора места, доступ только к выбранному файлу.
    @Command
    fun saveFile(invoke: Invoke) {
        val args = invoke.parseArgs(FileArgs::class.java)
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType(args.mime)
            .putExtra(Intent.EXTRA_TITLE, args.name)
        try {
            startActivityForResult(invoke, intent, "afterSaveFile")
        } catch (e: Throwable) {
            Log.e(TAG, "диалог сохранения не открылся", e)
            invoke.reject("save dialog failed", "save_failed")
        }
    }

    @ActivityCallback
    fun afterSaveFile(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(jsObject("saved" to false))
            return
        }
        val args = invoke.parseArgs(FileArgs::class.java)
        scope.launch {
            try {
                val out = activity.contentResolver.openOutputStream(uri, "wt")
                    ?: throw IllegalStateException("место для файла не открылось")
                out.use { stream -> File(args.path).inputStream().use { it.copyTo(stream) } }
                Log.i(TAG, "экспорт сохранён")
                invoke.resolve(jsObject("saved" to true))
            } catch (e: Throwable) {
                Log.e(TAG, "экспорт не сохранился", e)
                invoke.reject("save failed", "save_failed")
            }
        }
    }

    // «Поделиться»: файл из папки экспорта уходит через FileProvider с правом только на чтение.
    @Command
    fun shareFile(invoke: Invoke) {
        val args = invoke.parseArgs(FileArgs::class.java)
        try {
            val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", File(args.path))
            val send = Intent(Intent.ACTION_SEND)
                .setType(args.mime)
                .putExtra(Intent.EXTRA_STREAM, uri)
                .putExtra(Intent.EXTRA_TITLE, args.name)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            activity.startActivity(Intent.createChooser(send, args.name))
            invoke.resolve()
        } catch (e: Throwable) {
            Log.e(TAG, "меню отправки не открылось", e)
            invoke.reject("share failed", "share_failed")
        }
    }

    // Протокол в PDF: путь в папке экспорта выбирает Rust, блоки уже посчитаны там же.
    @Command
    fun renderPdf(invoke: Invoke) {
        val args = invoke.parseArgs(PdfArgs::class.java)
        scope.launch {
            try {
                ProtocolPdf(ProtocolBlock.parseAll(args.blocks)).writeTo(File(args.path))
                invoke.resolve()
            } catch (e: Throwable) {
                Log.e(TAG, "PDF не собрался", e)
                invoke.reject("pdf failed", "export_failed")
            }
        }
    }

    // Плеер работает в главном потоке: туда приходят команды и колбэки MediaPlayer.
    @Command
    fun playerOpen(invoke: Invoke) {
        val args = invoke.parseArgs(PlayerOpenArgs::class.java)
        try {
            val duration = player.open(args.path) { event ->
                args.onEvent?.send(jsObject("type" to event.type, "sec" to event.sec))
            }
            invoke.resolve(jsObject("durationSec" to duration))
        } catch (e: MediaException) {
            Log.e(TAG, "плеер: ${e.code}: ${e.message}", e)
            invoke.reject(e.message, e.code)
        } catch (e: Throwable) {
            Log.e(TAG, "плеер не открылся", e)
            invoke.reject("internal error", MediaException.PLAYBACK_FAILED)
        }
    }

    @Command
    fun playerPlay(invoke: Invoke) {
        val args = invoke.parseArgs(PlayArgs::class.java)
        try {
            player.play(args.fromSec)
            invoke.resolve()
        } catch (e: MediaException) {
            Log.w(TAG, "плеер: ${e.code}: ${e.message}")
            invoke.reject(e.message, e.code)
        } catch (e: Throwable) {
            Log.e(TAG, "проигрывание не началось", e)
            invoke.reject("internal error", MediaException.PLAYBACK_FAILED)
        }
    }

    @Command
    fun playerPause(invoke: Invoke) {
        player.pause()
        invoke.resolve()
    }

    @Command
    fun playerClose(invoke: Invoke) {
        player.close()
        invoke.resolve()
    }

    override fun onDestroy(activity: AppCompatActivity) {
        player.close()
    }

    // Какие модели есть в каталоге, какие уже скачаны и не идёт ли загрузка, начатая
    // до перезапуска приложения.
    @Command
    fun modelsStatus(invoke: Invoke) {
        scope.launch {
            try {
                val catalog = ModelCatalog.read(activity.assets)
                val folders = ModelFolders.of(activity)
                val local = InstalledModels.read(folders)
                val installed = AvailableModels.packs(local, local.ready(folders), BundledModels.read(activity.assets))
                val pending = journal().read()
                val packs = JSONArray()
                for (pack in catalog.packs) {
                    packs.put(
                        jsObject(
                            "key" to pack.key,
                            "required" to pack.required,
                            "size" to catalog.size(pack),
                            "installed" to (pack in installed),
                        ),
                    )
                }
                invoke.resolve(
                    jsObject(
                        "packs" to packs,
                        "downloading" to (pending != null),
                        "pending" to JSONArray(pending?.packs?.map { it.key }.orEmpty()),
                        "pendingAllowMetered" to (pending?.allowMetered ?: false),
                        "disabled" to JSONArray(DisabledModels.of(activity).read().map { it.key }),
                        "metered" to NetworkState.isMetered(activity),
                    ),
                )
            } catch (e: ModelsException) {
                Log.e(TAG, "состояние моделей: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "состояние моделей не прочиталось", e)
                invoke.reject("internal error", ModelsException.DOWNLOAD_FAILED)
            }
        }
    }

    // Загрузка выбранных моделей с зеркала. Ход загрузки идёт в канал; после загрузки
    // движок отпускает старые модели, следующая запись загрузит новый набор.
    @Command
    fun downloadModels(invoke: Invoke) {
        val args = invoke.parseArgs(DownloadModelsArgs::class.java)
        val packs = args.packs.map { key -> ModelPack.fromKey(key) }
        if (packs.any { it == null }) {
            invoke.reject("unknown model pack", ModelsException.BAD_MANIFEST)
            return
        }
        if (!installing.compareAndSet(false, true)) {
            invoke.reject("download already running", ModelsException.BUSY)
            return
        }
        val flag = AtomicBoolean(false)
        installCancelled = flag
        scope.launch {
            try {
                val context = activity.applicationContext
                val folders = ModelFolders.of(activity)
                val installer = ModelInstaller(
                    ModelCatalog.read(activity.assets),
                    folders,
                    ParallelFetcher(HttpRangeSource(), ModelInstaller.CONNECTIONS),
                    journal(),
                    ModelInstaller.freeBytesOf(folders.base),
                )
                val started = System.nanoTime()
                ModelDownloadService.start(context, args.notificationTitle)
                val canUseNetwork = { args.allowMetered || !NetworkState.isMetered(context) }
                try {
                    installer.install(packs.filterNotNull().toSet(), args.allowMetered, canUseNetwork, flag) { p ->
                        args.onProgress?.send(jsObject("phase" to p.phase, "done" to p.done, "total" to p.total))
                        ModelDownloadService.update(context, p.done, p.total)
                    }
                } finally {
                    ModelDownloadService.stop(context)
                }
                engine.unload()
                Log.i(TAG, "модели скачаны: ${(System.nanoTime() - started) / 1_000_000} мс")
                invoke.resolve()
            } catch (e: ModelsException) {
                if (e.code == ModelsException.CANCELLED) Log.i(TAG, "загрузка моделей отменена") else Log.e(TAG, "загрузка моделей: ${e.code}: ${e.message}", e)
                invoke.reject(e.message, e.code)
            } catch (e: Throwable) {
                Log.e(TAG, "загрузка моделей не удалась", e)
                invoke.reject("internal error", ModelsException.DOWNLOAD_FAILED)
            } finally {
                installing.set(false)
            }
        }
    }

    // Выключает необязательные модели или включает их обратно. Движок отпускает модели речи,
    // следующая запись загрузит набор уже без выключенных.
    @Command
    fun setModelsDisabled(invoke: Invoke) {
        val args = invoke.parseArgs(ModelsDisabledArgs::class.java)
        val packs = args.packs.map { ModelPack.fromKey(it) }
        if (packs.any { it == null || it.required }) {
            invoke.reject("bad model pack", ModelsException.BAD_MANIFEST)
            return
        }
        scope.launch {
            try {
                DisabledModels.of(activity).write(packs.filterNotNull().toSet())
                if (!engine.unload()) Log.i(TAG, "движок занят, выключенные модели уйдут со следующей загрузки")
                invoke.resolve()
            } catch (e: Exception) {
                Log.e(TAG, "выключенные модели не сохранились", e)
                invoke.reject("internal error", ModelsException.DOWNLOAD_FAILED)
            }
        }
    }

    @Command
    fun cancelModelDownload(invoke: Invoke) {
        installCancelled.set(true)
        invoke.resolve()
    }

    private fun journal() = DownloadJournal(File(activity.noBackupFilesDir, DOWNLOAD_JOURNAL))

    @Command
    fun cancelImport(invoke: Invoke) {
        importCancelled.set(true)
        invoke.resolve()
    }

    private fun recordingJson(result: RecordingResult): JSObject =
        jsObject("path" to result.path, "durationSec" to result.durationSec, "reason" to result.reason)

    companion object {
        private const val TAG = "NixyonSpeech"

        // На процесс, а не на экземпляр плагина: ядро спрашивает ключ один раз за процесс.
        private val recoveryDone = AtomicBoolean(false)
        private val recovered = AtomicReference<RecordingResult?>(null)
        // Журнал начатой загрузки моделей; в резервную копию Android не попадает.
        private const val DOWNLOAD_JOURNAL = "model-downloads.json"
        const val MICROPHONE = "microphone"
        const val NOTIFICATIONS = "notifications"
    }
}

// Ответ или событие для интерфейса из пар ключ-значение. Значение null убирает ключ, как
// JSObject.put.
private fun jsObject(vararg fields: Pair<String, Any?>): JSObject {
    val json = JSObject()
    for ((key, value) in fields) json.put(key, value)
    return json
}
