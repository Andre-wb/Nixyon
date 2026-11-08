package dev.demo.nixyon.speech

import android.util.Log
import com.k2fsa.sherpa.onnx.Vad
import java.io.File
import java.util.Locale

// Где в записи речь: куски VAD (начало, конец) в сэмплах и длина записи. Звук подаётся по
// 0,1 с, как в движке на ноутбуке.
//
// Если в наборе есть шумоподавитель (ModelRole.DENOISER), рядом слушает второй VAD, по звуку
// после него. На замере ровный шум (вентилятор, кондиционер, даже на 40 дБ тише голоса) и
// стук посуды держали VAD открытым через паузы: куски по 10-20 с склеивали речь нескольких
// людей, а после шумоподавления шли по фразам. Но без шума куски после шумоподавления
// хуже: на тестовых встречах в кусках с двумя людьми 30 % речи вместо 15 %. Поэтому куски
// берутся от VAD по исходному звуку, и только длинный кусок (от LONG_PIECE_SECONDS)
// заменяется кусками второго VAD внутри него (splitLong). Без шума VAD режет по фразам
// (на тестовых встречах куски не длиннее 7 с) и куски остаются как без шумоподавителя.
//
// Распознавание и модель голоса берут звук из исходной записи по этим границам: на замере
// распознавание звука после шумоподавления ошибалось при шуме на 10 и 5 дБ тише голоса вдвое
// чаще, а модель голоса по нему различала людей хуже. Поток шумоподавления отдаёт звук без
// сдвига, только с задержкой в кадр, поэтому границы совпадают с исходной записью; хвост
// приходит с flush.
//
// Шумоподавитель (GTCRN) входит в основной набор моделей. На ноутбуке тот же разрез
// (nixyon-sherpa, vad_denoiser.rs и long_pieces.rs) включается той же ролью denoiser.
// Замер на ноутбуке (eval, без шумоподавителя через --no-vad-denoise, B6, порог 0,7,
// число говорящих неизвестно) на 14 записях: ru2, ru4, ru6, ru4_fast, ru4_overlap и ru2, ru4, ru6 с шумом
// DEMAND (офис, столовая, ресторан) при 20, 10 и 5 дБ. Без шумоподавителя и с ним при
// пороге 15 с:
// - в целом лучше: число говорящих верно на 8 и 10 записях из 14, DER по словам 32,1 и
//   23,8 %, путаница 10,9 и 5,0 %, WER 27,7 и 25,6 %;
// - на шуме 20 дБ сильно лучше: DER по словам ru2 19,5 и 0,9 %, ru4 41,8 и 10,1 %, ru6
//   80,8 и 33,2 % (найдено 1 и 5 говорящих из 6); на 10 дБ ru4 21,1 и 16,8 %, ru6 44,8
//   и 38,4 %;
// - хуже только ru6-snr5: 64,5 и 72,4 %, путаница 8,8 и 18,2 %;
// - длинные куски бывают и без шума: DER по границам ходов на чистой ru2 39,7 и 26,3 %,
//   ru6 22,1 и 11,8 %; DER по словам на чистых записях тот же, WER ru2 2,6 и 0,0 %, ru4
//   5,9 и 6,4 %.
// Порог 10 с портил ru4_overlap (голос почти весь поверх чужого, DER по словам 20,3 и
// 44,8 %, говорящих 3 из 4) и ru4-snr5 (28,5 и 56,8 %); порог 20 с терял ru6-snr20 (80,8 %,
// 1 говорящий из 6). Скорость: RTF без шумоподавителя 0,335, с ним при пороге 10 с 0,379;
// при 15 с не мерилась из-за нагрузки ноутбука.
// Прежние схемы с GTCRN были хуже без оговорок: поиск речи целиком по очищенной копии
// (замер на 12 записях) дал число говорящих верно на 5 записях из 12 против 7,
// DER 48,5 % против 45,6 %; GTCRN перед всем конвейером на чистых ru2, ru4, ru6 дал WER
// 9,0 % против 6,6 %, DER 14,1 % против 4,2 %.
class SpeechFinder(private val models: SpeechModels) {
    // checkCancelled зовётся перед каждым куском звука и может бросить исключение.
    fun run(wav: File, checkCancelled: () -> Unit): Pair<List<Pair<Long, Long>>, Long> {
        val plain = mutableListOf<Pair<Long, Long>>()
        val clean = mutableListOf<Pair<Long, Long>>()
        WavPcm16Reader(wav).use { reader ->
            val assets = models.located.source.assetManager
            val vad = Vad(assets, models.vadConfig)
            var denoiseNanos = 0L
            var denoised = false
            try {
                val denoiser = models.newDenoiser()
                val cleanVad = try {
                    denoiser?.let { Vad(assets, models.vadConfig) }
                } catch (e: Throwable) {
                    denoiser?.release()
                    throw e
                }
                try {
                    while (true) {
                        checkCancelled()
                        val chunk = reader.next(CHUNK) ?: break
                        vad.acceptWaveform(chunk)
                        drain(vad, plain)
                        if (denoiser != null && cleanVad != null) {
                            val started = System.nanoTime()
                            val out = denoiser.run(chunk, SpeechModels.SAMPLE_RATE)
                            denoiseNanos += System.nanoTime() - started
                            hear(cleanVad, out.samples, out.sampleRate)
                            drain(cleanVad, clean)
                        }
                    }
                    vad.flush()
                    drain(vad, plain)
                    if (denoiser != null && cleanVad != null) {
                        val tail = denoiser.flush()
                        hear(cleanVad, tail.samples, tail.sampleRate)
                        cleanVad.flush()
                        drain(cleanVad, clean)
                        denoised = true
                    }
                } finally {
                    cleanVad?.release()
                    denoiser?.release()
                }
            } finally {
                vad.release()
            }
            if (!denoised) return plain to reader.totalSamples
            val long = plain.count { (a, b) -> b - a >= LONG_PIECE_SAMPLES }
            val audioSec = reader.totalSamples.toDouble() / SpeechModels.SAMPLE_RATE
            Log.i(
                TAG,
                String.format(
                    Locale.ROOT,
                    "шумоподавление для VAD: %.1f с на %.1f с звука, длинных кусков %d из %d",
                    denoiseNanos / 1e9, audioSec, long, plain.size,
                ),
            )
            return splitLong(plain, clean, LONG_PIECE_SAMPLES) to reader.totalSamples
        }
    }

    private fun hear(vad: Vad, samples: FloatArray, rate: Int) {
        if (DenoisedChunk.usable(samples.size, rate)) vad.acceptWaveform(samples)
    }

    private fun drain(vad: Vad, bounds: MutableList<Pair<Long, Long>>) {
        while (!vad.empty()) {
            val segment = vad.front()
            vad.pop()
            val start = segment.start.coerceAtLeast(0L)
            bounds.add(start to start + segment.samples.size)
        }
    }

    companion object {
        private const val TAG = "NixyonTiming"
        private const val CHUNK = 1_600

        // На тестовых встречах в шуме доля речи в кусках с двумя людьми: 93-98 % без
        // шумоподавления, 4-18 %, если резать куски от 19,5 с, и 2-10 % от 10 с (при гуле
        // голосов 21 % вместо 34 %); без шума куски не длиннее 7 с и остаются как есть.
        // На замере eval (ниже в описании класса) от 10 с портились запись с наложением
        // голосов и шум 5 дБ, от 20 с терялся выигрыш на шуме 20 дБ; взято 15 с, как
        // LONG_PIECE_SAMPLES в nixyon-sherpa.
        private const val LONG_PIECE_SECONDS = 15
        private const val LONG_PIECE_SAMPLES = LONG_PIECE_SECONDS.toLong() * SpeechModels.SAMPLE_RATE

        // Куски plain, а вместо куска от long сэмплов и длиннее части кусков clean внутри
        // него. Обе последовательности по возрастанию и без перекрытий, итог тоже. Если
        // внутри длинного куска clean речи не нашёл, от него ничего не остаётся: там шум.
        fun splitLong(plain: List<Pair<Long, Long>>, clean: List<Pair<Long, Long>>, long: Long): List<Pair<Long, Long>> {
            val out = ArrayList<Pair<Long, Long>>(plain.size)
            var first = 0
            for ((a, b) in plain) {
                if (b - a < long) {
                    out.add(a to b)
                    continue
                }
                while (first < clean.size && clean[first].second <= a) first++
                var i = first
                while (i < clean.size && clean[i].first < b) {
                    val from = maxOf(a, clean[i].first)
                    val to = minOf(b, clean[i].second)
                    if (to > from) out.add(from to to)
                    i++
                }
            }
            return out
        }

        // Речь из записи по границам, подряд. Границы по возрастанию; перекрытия берутся
        // один раз, что за концом записи, пропускается.
        fun cut(reader: WavPcm16Reader, bounds: List<Pair<Long, Long>>): FloatArray {
            val total = bounds.sumOf { (a, b) -> (b - a).coerceAtLeast(0L) }
            val speech = FloatArray(total.coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
            var at = 0
            var position = 0L
            for ((a, b) in bounds) {
                val from = maxOf(a, position)
                if (b <= from) continue
                position += reader.skipSamples(from - position)
                if (position < from) break
                val piece = reader.readSamples(b - from)
                piece.copyInto(speech, at)
                at += piece.size
                position += piece.size
            }
            return if (at == speech.size) speech else speech.copyOf(at)
        }
    }
}
