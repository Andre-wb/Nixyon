package dev.demo.nixyon.speech

import java.io.BufferedInputStream
import java.io.Closeable
import java.io.EOFException
import java.io.File
import java.io.FileInputStream
import java.io.InputStream

// Потоковое чтение WAV, который готовит ядро: PCM 16 бит, моно, 16 кГц. Файл читается
// кусками, а не целиком: час записи во float занял бы около 230 МБ памяти.
class WavPcm16Reader(file: File) : Closeable {
    private val input: InputStream = BufferedInputStream(FileInputStream(file), 1 shl 16)
    val totalSamples: Long
    private var left: Long

    init {
        try {
            require(readTag() == "RIFF") { "нет заголовка RIFF" }
            readU32()
            require(readTag() == "WAVE") { "это не WAVE" }
            var format = false
            var dataBytes = -1L
            while (dataBytes < 0) {
                val tag = readTag()
                val size = readU32()
                when (tag) {
                    "fmt " -> {
                        val audioFormat = readU16()
                        val channels = readU16()
                        val rate = readU32()
                        skip(6)
                        val bits = readU16()
                        skip(size - 16)
                        require(audioFormat == 1 && channels == 1 && rate == SpeechModels.SAMPLE_RATE.toLong() && bits == 16) {
                            "ожидался PCM 16 бит моно 16 кГц, а не формат $audioFormat, $channels кан., $rate Гц, $bits бит"
                        }
                        format = true
                    }
                    "data" -> dataBytes = size
                    else -> skip(size + (size and 1))
                }
            }
            require(format) { "нет описания формата перед данными" }
            totalSamples = dataBytes / 2
            left = totalSamples
        } catch (e: IllegalArgumentException) {
            input.close()
            throw AnalysisException(AnalysisException.BAD_AUDIO, "WAV не подходит: ${e.message}", e)
        } catch (e: EOFException) {
            input.close()
            throw AnalysisException(AnalysisException.BAD_AUDIO, "WAV обрывается в заголовке", e)
        }
    }

    // Следующий кусок до maxSamples сэмплов в диапазоне -1..1 или null в конце данных.
    fun next(maxSamples: Int): FloatArray? {
        if (left <= 0) return null
        val count = minOf(maxSamples.toLong(), left).toInt()
        val bytes = ByteArray(count * 2)
        var read = 0
        while (read < bytes.size) {
            val n = input.read(bytes, read, bytes.size - read)
            if (n < 0) break
            read += n
        }
        val samples = read / 2
        if (samples == 0) {
            left = 0
            return null
        }
        left -= samples
        return FloatArray(samples) { i -> Pcm16.toFloat(Pcm16.get(bytes, 2 * i)) }
    }

    // Пропускает до count сэмплов; ответ: сколько пропущено, в конце данных меньше.
    fun skipSamples(count: Long): Long {
        var skipped = 0L
        while (skipped < count) {
            val chunk = next(minOf(READ_CHUNK.toLong(), count - skipped).toInt()) ?: break
            skipped += chunk.size
        }
        return skipped
    }

    // До count сэмплов одним массивом; в конце данных он короче.
    fun readSamples(count: Long): FloatArray {
        val out = FloatArray(count.coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
        var filled = 0
        while (filled < out.size) {
            val chunk = next(minOf(READ_CHUNK, out.size - filled)) ?: break
            chunk.copyInto(out, filled)
            filled += chunk.size
        }
        return if (filled == out.size) out else out.copyOf(filled)
    }

    override fun close() = input.close()

    private fun readTag(): String = String(readBytes(4), Charsets.US_ASCII)

    private fun readU16(): Int {
        val b = readBytes(2)
        return (b[0].toInt() and 0xff) or ((b[1].toInt() and 0xff) shl 8)
    }

    private fun readU32(): Long {
        val b = readBytes(4)
        return (0 until 4).fold(0L) { acc, i -> acc or ((b[i].toLong() and 0xff) shl (8 * i)) }
    }

    private fun readBytes(n: Int): ByteArray {
        val out = ByteArray(n)
        var read = 0
        while (read < n) {
            val r = input.read(out, read, n - read)
            if (r < 0) throw EOFException()
            read += r
        }
        return out
    }

    private fun skip(n: Long) {
        var rest = n
        while (rest > 0) {
            val skipped = input.skip(rest)
            if (skipped <= 0) {
                if (input.read() < 0) throw EOFException()
                rest -= 1
            } else {
                rest -= skipped
            }
        }
    }

    private companion object {
        // Чтение и пропуск по 0,1 с: буфер не растёт с длиной куска.
        const val READ_CHUNK = 1_600
    }
}
