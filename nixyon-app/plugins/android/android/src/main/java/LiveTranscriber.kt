package dev.demo.nixyon.speech

import android.util.Log
import com.k2fsa.sherpa.onnx.OnlineRecognizer
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

// Живой текст во время записи. Куски звука из потока записи кладутся в очередь, а
// распознаются на своём потоке: распознавание не должно задерживать чтение микрофона.
// Наружу уходят последние законченные фразы и текущая фраза, только когда они изменились.
// Текст в лог не пишется: это содержание разговора.
// onDone зовётся, когда поток живого текста закончился и распознаватель больше не трогает:
// с этого момента модели речи можно отпускать (SpeechEngine.startLive).
class LiveTranscriber(
    private val recognizer: OnlineRecognizer,
    private val rate: Int,
    private val onDone: () -> Unit,
    private val onLines: (List<String>) -> Unit,
) {
    private val queue = LinkedBlockingQueue<FloatArray>(MAX_QUEUED)
    private val thread = Thread(::loop, "nixyon-live")
    @Volatile
    private var stopped = false
    private var dropped = false

    init {
        thread.start()
    }

    // Вызывается из потока записи. Если распознавание не успевает и очередь полна, кусок
    // пропускается: запись важнее живого текста.
    fun offer(samples: ShortArray, length: Int) {
        if (stopped) return
        val chunk = FloatArray(length) { Pcm16.toFloat(samples[it]) }
        if (!queue.offer(chunk) && !dropped) {
            dropped = true
            Log.w(TAG, "живой текст не успевает, куски звука пропускаются")
        }
    }

    // Останавливает распознавание и ждёт конца потока; что не успело распознаться, отбрасывается.
    fun stop() {
        stopped = true
        thread.join(STOP_WAIT_MS)
    }

    private fun loop() {
        val stream = recognizer.createStream("")
        val phrases = ArrayDeque<String>()
        var shown = emptyList<String>()
        try {
            while (!stopped) {
                val chunk = queue.poll(POLL_MS, TimeUnit.MILLISECONDS) ?: continue
                stream.acceptWaveform(chunk, rate)
                while (recognizer.isReady(stream)) recognizer.decode(stream)
                var current = recognizer.getResult(stream).text.trim()
                if (recognizer.isEndpoint(stream)) {
                    if (current.isNotEmpty()) {
                        phrases.addLast(current)
                        if (phrases.size > KEPT_PHRASES) phrases.removeFirst()
                    }
                    recognizer.reset(stream)
                    current = ""
                }
                val lines = if (current.isEmpty()) phrases.toList() else phrases.toList() + current
                if (lines != shown) {
                    shown = lines
                    onLines(lines)
                }
            }
        } catch (e: Throwable) {
            Log.e(TAG, "живой текст остановлен ошибкой распознавания", e)
        } finally {
            stream.release()
            onDone()
        }
    }

    companion object {
        private const val TAG = "NixyonLive"
        // Законченных фраз на экране: вместе с текущей не больше четырёх строк.
        private const val KEPT_PHRASES = 3
        // Куски по 0,1 с: в очереди не больше 10 с звука.
        private const val MAX_QUEUED = 100
        private const val POLL_MS = 100L
        private const val STOP_WAIT_MS = 2_000L
    }
}
