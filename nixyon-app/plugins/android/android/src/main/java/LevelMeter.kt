package dev.demo.nixyon.speech

import kotlin.math.log10
import kotlin.math.sqrt

// Уровень громкости для индикатора записи: RMS по окну в дБ полной шкалы.
class LevelMeter(private val windowSamples: Int, private val onLevel: (Float) -> Unit) {
    private var sum = 0.0
    private var count = 0

    fun add(samples: ShortArray, length: Int) {
        for (i in 0 until length) {
            val x = samples[i] / 32768.0
            sum += x * x
            count += 1
            if (count == windowSamples) {
                onLevel(dbfs(sqrt(sum / count)))
                sum = 0.0
                count = 0
            }
        }
    }

    companion object {
        const val FLOOR_DBFS = -90f

        fun dbfs(rms: Double): Float =
            if (rms <= 0.0) FLOOR_DBFS else (20 * log10(rms)).toFloat().coerceAtLeast(FLOOR_DBFS)
    }
}
