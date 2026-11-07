package dev.demo.nixyon.speech

import android.annotation.SuppressLint
import android.annotation.TargetApi
import android.content.Context
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioRecordingConfiguration
import android.media.MediaRecorder
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.os.SystemClock
import android.util.Log
import java.io.File
import java.util.Locale
import java.util.concurrent.atomic.AtomicReference

class RecordingResult(val path: String?, val durationSec: Double, val reason: String)

// Запись с микрофона в WAV PCM 16 бит моно. Источник VOICE_RECOGNITION: без шумодава и
// автоусиления, звук для распознавания. Частота 16 кГц, если устройство её даёт; иначе
// 48 или 44,1 кГц, и ядро пересчитает в 16 кГц.
//
// Чтобы звук не пропадал: буфер микрофона на 2 с (MicBuffer), поток записи с приоритетом
// звука, отвалившийся микрофон открывается заново (MicGate). Звонок или другое приложение,
// заглушившие микрофон, запись останавливают, записанное сохраняется.
class AudioRecorder(
    private val context: Context,
    private val recordings: RecordingsDir,
    private val samples: SamplesDir,
    private val keys: StorageKey,
) {
    private val lock = Any()
    private var session: Session? = null

    private class Session(
        // Меняется, когда микрофон открыт заново; читает и поток записи, и главный поток.
        @Volatile var record: AudioRecord,
        val target: RecordingTarget,
        val part: File,
        val writer: WavPcm16Writer,
        val rate: Int,
        val live: LiveTranscriber?,
    ) {
        // Первая причина остановки остаётся: пользователь, лимит, место или обрыв.
        val stopReason = AtomicReference<String?>(null)
        // Пока пауза, микрофон открыт, но звук не пишется и не идёт в живой текст.
        @Volatile var paused = false
        val gate = MicGate(REOPEN_MS)
        val startedAt = SystemClock.elapsedRealtime()
        var result: RecordingResult? = null
        lateinit var thread: Thread
        var callback: AudioManager.AudioRecordingCallback? = null

        val running: Boolean
            get() = stopReason.get() == null

        fun end(reason: String) {
            stopReason.compareAndSet(null, reason)
        }
    }

    val isRecording: Boolean
        get() = synchronized(lock) { session != null }

    // Разрешение на микрофон проверяет плагин до вызова. live: живой текст для частоты
    // микрофона, null, если живой модели нет. sample: образец голоса, он пишется в
    // SamplesDir и не стирает запись встречи.
    @SuppressLint("MissingPermission")
    fun start(
        sample: Boolean,
        onLevel: (Float) -> Unit,
        onStopped: (RecordingResult) -> Unit,
        live: (Int) -> LiveTranscriber?,
    ): Int {
        synchronized(lock) {
            if (session != null) throw MediaException(MediaException.ALREADY_RECORDING, "запись уже идёт")
            val target: RecordingTarget = if (sample) samples else recordings
            if (!target.hasRoom()) throw MediaException(MediaException.STORAGE_LOW, "мало места для записи")
            val (record, rate) = openMicrophone(RATES)
                ?: throw MediaException(MediaException.MIC_UNAVAILABLE, "микрофон не открывается ни на одной частоте")
            val part = target.newPart(if (sample) "smp" else RecordingsDir.RECORDING_PREFIX)
            val writer = try {
                WavPcm16Writer(part, keys.get(), rate, 1)
            } catch (e: Exception) {
                record.release()
                throw MediaException(MediaException.STORAGE_LOW, "файл записи не создаётся: ${e.message}", e)
            }
            if (!begin(record)) {
                writer.close()
                part.delete()
                throw MediaException(MediaException.MIC_UNAVAILABLE, "микрофон занят другим приложением")
            }
            val s = Session(record, target, part, writer, rate, live(rate))
            s.callback = watchSilencing(s)
            s.thread = Thread({ loop(s, onLevel, onStopped) }, "nixyon-recorder")
            session = s
            s.thread.start()
            return rate
        }
    }

    // Пауза и продолжение: та же запись, тот же файл. Время на паузе в длительность не входит.
    fun pause() = setPaused(true)

    fun resume() = setPaused(false)

    private fun setPaused(paused: Boolean) {
        val s = synchronized(lock) { session } ?: throw MediaException(MediaException.NOT_RECORDING, "запись не идёт")
        s.paused = paused
    }

    // Останавливает запись и ждёт, пока файл будет дописан и сохранён.
    fun stop(reason: String): RecordingResult {
        val s = synchronized(lock) { session } ?: throw MediaException(MediaException.NOT_RECORDING, "запись не идёт")
        s.end(reason)
        s.thread.join()
        return s.result ?: RecordingResult(null, 0.0, reason)
    }

    private fun loop(s: Session, onLevel: (Float) -> Unit, onStopped: (RecordingResult) -> Unit) {
        raisePriority()
        val buffer = ShortArray(s.rate / 10)
        val bytes = ByteArray(buffer.size * 2)
        val meter = LevelMeter(s.rate / 10, onLevel)
        var sinceRoomCheck = 0
        try {
            while (s.running) {
                val n = s.record.read(buffer, 0, buffer.size)
                if (n < 0) {
                    if (!reopen(s)) s.end(INTERRUPTED)
                    continue
                }
                meter.add(buffer, n)
                when (s.gate.chunk()) {
                    MicGate.Chunk.WRITE -> Unit
                    MicGate.Chunk.STOP -> {
                        s.end(INTERRUPTED)
                        continue
                    }
                }
                if (s.paused) continue
                for (i in 0 until n) Pcm16.put(bytes, 2 * i, buffer[i].toInt())
                s.writer.write(bytes, n * 2)
                s.live?.offer(buffer, n)
                if (s.writer.durationSec >= RecordingsDir.MAX_SECONDS) s.end("limit")
                sinceRoomCheck += n
                if (sinceRoomCheck >= s.rate * ROOM_CHECK_SECONDS) {
                    sinceRoomCheck = 0
                    if (!s.target.hasRoom()) s.end("storage")
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "запись оборвалась", e)
            s.end(INTERRUPTED)
        } finally {
            s.result = finish(s)
            synchronized(lock) { session = null }
        }
        val result = s.result ?: return
        if (result.reason != "user") onStopped(result)
    }

    // Поток записи с приоритетом звука: при нагрузке (живой текст, интерфейс, обработка) он
    // успевает забирать звук из буфера микрофона. Если система не даёт, остаётся обычный.
    private fun raisePriority() {
        for (priority in intArrayOf(Process.THREAD_PRIORITY_URGENT_AUDIO, Process.THREAD_PRIORITY_AUDIO)) {
            try {
                Process.setThreadPriority(priority)
                return
            } catch (_: RuntimeException) {
            }
        }
        Log.w(TAG, "приоритет звука потоку записи не дали")
    }

    // Чтение вернуло ошибку: микрофон открывается заново на той же частоте, раз в
    // REOPEN_RETRY_MS, пока MicGate разрешает. Звук за это время теряется, запись идёт
    // дальше в тот же файл. false: открыть не удалось или запись уже останавливают.
    private fun reopen(s: Session): Boolean {
        s.record.release()
        while (s.running && s.gate.lost(SystemClock.elapsedRealtime())) {
            val record = openMicrophone(intArrayOf(s.rate))?.first
            if (record != null && begin(record)) {
                s.record = record
                s.gate.reopened()
                Log.w(TAG, "микрофон отвалился и открыт заново")
                return true
            }
            Thread.sleep(REOPEN_RETRY_MS)
        }
        Log.w(TAG, "микрофон отвалился и не открылся заново")
        return false
    }

    // Запускает микрофон; если он не пошёл, отпускает его.
    private fun begin(record: AudioRecord): Boolean {
        try {
            record.startRecording()
        } catch (_: IllegalStateException) {
        }
        if (record.recordingState == AudioRecord.RECORDSTATE_RECORDING) return true
        record.release()
        return false
    }

    private fun finish(s: Session): RecordingResult {
        val reason = s.stopReason.get() ?: INTERRUPTED
        s.live?.stop()
        s.callback?.let { context.getSystemService(AudioManager::class.java)?.unregisterAudioRecordingCallback(it) }
        try {
            s.record.stop()
        } catch (_: IllegalStateException) {
        }
        s.record.release()
        val duration = s.writer.durationSec
        s.writer.close()
        logGaps(s, duration, reason)
        if (s.writer.dataBytes == 0L) {
            s.part.delete()
            return RecordingResult(null, 0.0, reason)
        }
        val done = s.target.keep(s.part)
        return RecordingResult(done.absolutePath, duration, reason)
    }

    // Сколько звука не попало в файл: время записи минус записанное. Если поток
    // записи не успевал читать микрофон или микрофон отваливался, это видно здесь. Только
    // числа: звук и текст в лог не пишутся.
    private fun logGaps(s: Session, writtenSec: Double, reason: String) {
        val now = SystemClock.elapsedRealtime()
        val elapsedSec = (now - s.startedAt) / 1000.0
        val lostSec = elapsedSec - writtenSec
        Log.i(
            TAG,
            String.format(
                Locale.ROOT,
                "запись %s: прошло %.1f с, записано %.1f с, микрофон открыт заново %d раз, не записано %.1f с",
                reason,
                elapsedSec,
                writtenSec,
                s.gate.reopens(),
                lostSec,
            ),
        )
    }

    @SuppressLint("MissingPermission")
    private fun openMicrophone(rates: IntArray): Pair<AudioRecord, Int>? {
        for (rate in rates) {
            val min = AudioRecord.getMinBufferSize(rate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
            if (min <= 0) continue
            for (size in MicBuffer.sizes(rate, min)) {
                val record = try {
                    AudioRecord(
                        MediaRecorder.AudioSource.VOICE_RECOGNITION,
                        rate,
                        AudioFormat.CHANNEL_IN_MONO,
                        AudioFormat.ENCODING_PCM_16BIT,
                        size,
                    )
                } catch (e: Exception) {
                    continue
                }
                if (record.state == AudioRecord.STATE_INITIALIZED) return record to rate
                record.release()
            }
        }
        return null
    }

    // Звонок или другое приложение заглушают наш микрофон: Android даёт тишину вместо
    // ошибки. Такую запись останавливаем и сохраняем то, что успели записать (MicGate).
    // Запись могла начаться уже во время звонка, поэтому текущее состояние берётся сразу,
    // не дожидаясь перемены: такая запись сразу останавливается.
    private fun watchSilencing(s: Session): AudioManager.AudioRecordingCallback? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return null
        val manager = context.getSystemService(AudioManager::class.java) ?: return null
        val callback = object : AudioManager.AudioRecordingCallback() {
            override fun onRecordingConfigChanged(configs: List<AudioRecordingConfiguration>) = silencing(s, configs)
        }
        manager.registerAudioRecordingCallback(callback, Handler(Looper.getMainLooper()))
        silencing(s, manager.activeRecordingConfigurations)
        return callback
    }

    @TargetApi(Build.VERSION_CODES.Q)
    private fun silencing(s: Session, configs: List<AudioRecordingConfiguration>) {
        val ours = configs.firstOrNull { it.clientAudioSessionId == s.record.audioSessionId } ?: return
        s.gate.silenced(ours.isClientSilenced)
    }

    companion object {
        private const val TAG = "NixyonRecorder"
        private const val INTERRUPTED = "interrupted"
        private val RATES = intArrayOf(16_000, 48_000, 44_100)
        private const val ROOM_CHECK_SECONDS = 10
        // Отвалившийся микрофон: 10 с попыток открыть заново, раз в полсекунды.
        private const val REOPEN_MS = 10_000L
        private const val REOPEN_RETRY_MS = 500L
    }
}
