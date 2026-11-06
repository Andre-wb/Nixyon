package dev.demo.nixyon.speech

import java.io.File
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.crypto.AEADBadTagException
import javax.crypto.SecretKey

// Восстановление WAV от WavPcm16Writer, если приложение упало, не закрыв его. На диске тогда
// заголовок файла и полные куски подряд с начала (SealedOutput пишет их сразу), а хвост
// мог оборваться посреди куска. Последним не помечен ни один кусок, а в заголовке WAV
// нулевые размеры. Восстановление оставляет куски, которые расшифровываются подряд с
// начала, последний из них шифрует заново как последний, ставит в WAV размеры по
// оставшемуся звуку и обрезает хвост. Пропадает только неполный последний кусок: до 2 с
// звука при 16 кГц моно.
// Если упасть посреди восстановления, следующий запуск его доделает: последний кусок
// пишется раньше заголовка WAV, а уже помеченный последним кусок с неверными размерами в
// заголовке тоже исправляется.
object SealedRecovery {
    // Длительность восстановленного звука в секундах. null: восстанавливать нечего (не тот
    // формат, чужой ключ, ни одного целого куска, не WAV или в нём нет звука); файл тогда
    // не меняется.
    fun recoverWav(file: File, key: SecretKey): Double? {
        RandomAccessFile(file, "rw").use { io ->
            if (io.length() < SealedFormat.HEADER) return null
            val header = ByteArray(SealedFormat.HEADER)
            io.readFully(header)
            if (!SealedFormat.checkHeader(header)) return null
            val scan = scan(io, header, key) ?: return null
            val (rate, channels) = wavFormat(scan.first) ?: return null

            var lastSize = scan.last.size
            var plainSize = (scan.count - 1) * SealedFormat.CHUNK + lastSize
            if (!scan.finished) {
                // Сэмпл, оборванный посреди каналов, отбрасывается.
                val extra = ((plainSize - WavPcm16Writer.HEADER_BYTES) % (2L * channels)).toInt()
                lastSize -= extra
                plainSize -= extra
            }
            val dataBytes = plainSize - WavPcm16Writer.HEADER_BYTES
            if (dataBytes <= 0) return null

            val head = WavPcm16Writer.header(rate, channels, dataBytes)
            val sizesRight = scan.first.copyOf(head.size).contentEquals(head)
            System.arraycopy(head, 0, scan.first, 0, head.size)
            val lastIndex = scan.count - 1
            if (lastIndex == 0L) {
                // Один кусок: он же первый, scan.last тот же массив, что scan.first.
                if (!scan.finished || !sizesRight) writeFrame(io, key, header, 0, scan.first, lastSize, last = true)
            } else {
                if (!scan.finished) writeFrame(io, key, header, lastIndex, scan.last, lastSize, last = true)
                if (!sizesRight) writeFrame(io, key, header, 0, scan.first, scan.first.size, last = false)
            }
            io.setLength(SealedFormat.HEADER + lastIndex * SealedFormat.FRAME + SealedFormat.NONCE + lastSize + SealedFormat.TAG)
            io.fd.sync()
            return dataBytes.toDouble() / (2L * channels * rate)
        }
    }

    // count кусков подряд с начала расшифровываются; finished: последний из них уже помечен
    // последним, файл был дописан. first и last: открытый текст первого и последнего.
    private class Scan(val count: Long, val finished: Boolean, val first: ByteArray, val last: ByteArray)

    private fun scan(io: RandomAccessFile, header: ByteArray, key: SecretKey): Scan? {
        val body = io.length() - SealedFormat.HEADER
        val frame = ByteArray(SealedFormat.FRAME)
        var count = 0L
        var first: ByteArray? = null
        var last: ByteArray? = null
        var finished = false
        while (!finished) {
            val left = body - count * SealedFormat.FRAME
            if (left < SealedFormat.NONCE + SealedFormat.TAG) break
            val size = minOf(left, SealedFormat.FRAME.toLong()).toInt()
            io.seek(SealedFormat.HEADER + count * SealedFormat.FRAME)
            io.readFully(frame, 0, size)
            val bytes = if (size == frame.size) frame else frame.copyOf(size)
            // Непоследний кусок всегда полный; последним может оказаться и полный.
            val plain = (if (size == frame.size) open(key, bytes, header, count, last = false) else null)
                ?: open(key, bytes, header, count, last = true)?.also { finished = true }
                ?: break
            if (count == 0L) first = plain
            last = plain
            count += 1
        }
        if (first == null || last == null) return null
        return Scan(count, finished, first, last)
    }

    private fun open(key: SecretKey, frame: ByteArray, header: ByteArray, index: Long, last: Boolean): ByteArray? =
        try {
            SealedFormat.open(key, frame, SealedFormat.aad(header, index, last))
        } catch (_: AEADBadTagException) {
            null
        }

    private fun writeFrame(io: RandomAccessFile, key: SecretKey, header: ByteArray, index: Long, plain: ByteArray, length: Int, last: Boolean) {
        io.seek(SealedFormat.HEADER + index * SealedFormat.FRAME)
        io.write(SealedFormat.seal(key, plain, length, SealedFormat.aad(header, index, last)))
    }

    // Частота и число каналов, если начало куска совпадает с заголовком WavPcm16Writer во
    // всём, кроме размеров; иначе null.
    private fun wavFormat(plain: ByteArray): Pair<Int, Int>? {
        if (plain.size < WavPcm16Writer.HEADER_BYTES) return null
        val b = ByteBuffer.wrap(plain).order(ByteOrder.LITTLE_ENDIAN)
        val channels = b.getShort(22).toInt() and 0xffff
        val rate = b.getInt(24)
        if (channels == 0 || rate <= 0) return null
        val expected = WavPcm16Writer.header(rate, channels, 0)
        for (i in expected.indices) {
            if (i in 4..7 || i in 40..43) continue
            if (plain[i] != expected[i]) return null
        }
        return rate to channels
    }
}
