package dev.demo.nixyon.speech

import java.io.Closeable
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.crypto.SecretKey

// WAV PCM 16 бит любой частоты и числа каналов, на диске зашифрованный (SealedOutput).
// Заголовок с частотой и числом каналов пишется сразу, а размеры в нём ставятся при
// закрытии: до него длина записи неизвестна. Если приложение упало и файл не закрыт,
// размеры ставит SealedRecovery по тому, что успело лечь на диск.
class WavPcm16Writer(
    file: File,
    key: SecretKey,
    private val sampleRate: Int,
    private val channels: Int,
) : Closeable {
    private val out = SealedOutput(file, key)
    var dataBytes = 0L
        private set

    init {
        out.write(header(sampleRate, channels, 0), HEADER_BYTES)
    }

    fun write(bytes: ByteArray, length: Int) {
        out.write(bytes, length)
        dataBytes += length
    }

    val durationSec: Double
        get() = dataBytes.toDouble() / (2L * channels * sampleRate)

    override fun close() {
        out.close(header(sampleRate, channels, dataBytes))
    }

    companion object {
        const val HEADER_BYTES = 44
        private const val UINT32_MAX = 0xFFFF_FFFFL

        fun header(sampleRate: Int, channels: Int, dataBytes: Long): ByteArray {
            val h = ByteBuffer.allocate(HEADER_BYTES).order(ByteOrder.LITTLE_ENDIAN)
            val byteRate = sampleRate * channels * 2
            h.put("RIFF".toByteArray(Charsets.US_ASCII))
            h.putInt((36 + dataBytes).coerceAtMost(UINT32_MAX).toInt())
            h.put("WAVE".toByteArray(Charsets.US_ASCII))
            h.put("fmt ".toByteArray(Charsets.US_ASCII))
            h.putInt(16)
            h.putShort(1)
            h.putShort(channels.toShort())
            h.putInt(sampleRate)
            h.putInt(byteRate)
            h.putShort((channels * 2).toShort())
            h.putShort(16)
            h.put("data".toByteArray(Charsets.US_ASCII))
            h.putInt(dataBytes.coerceAtMost(UINT32_MAX - 36).toInt())
            return h.array()
        }
    }
}
