package dev.demo.nixyon.speech

import android.os.SystemClock
import android.util.Log

// Время фазы обработки и доля в нём модели голоса. В лог идут только длительности и число
// вызовов: по ним видно, сколько времени уходит на эмбеддинги, без звука и текста.
class PhaseTiming(private val voice: VoiceModel) {
    private val startedMs = SystemClock.elapsedRealtime()
    private val calls = voice.calls
    private val nanos = voice.nanos

    fun log(phase: String, recordingSec: Double) {
        val phaseMs = SystemClock.elapsedRealtime() - startedMs
        val voiceMs = (voice.nanos - nanos) / 1_000_000
        Log.i(TAG, "замер $phase: запись ${"%.1f".format(recordingSec)} с, фаза $phaseMs мс, модель голоса ${voice.calls - calls} вызовов, $voiceMs мс")
    }

    companion object {
        private const val TAG = "NixyonTiming"
    }
}
