package dev.demo.nixyon.speech

import android.media.MediaDataSource
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import javax.crypto.SecretKey

// Зашифрованная запись для MediaPlayer: читает с любого места, расшифровывая один кусок.
// Куски лежат на постоянных местах, поэтому нужный находится сразу. Открытый текст на
// диск не пишется. MediaPlayer зовёт readAt из своего потока.
class SealedMediaSource(file: File, private val key: SecretKey) : MediaDataSource() {
    private val input = RandomAccessFile(file, "r")
    private val header = ByteArray(SealedFormat.HEADER)
    private val chunks: Long
    private val lastFrame: Int
    private val plainSize: Long
    private var cachedIndex = -1L
    private var cached = ByteArray(0)

    init {
        try {
            input.readFully(header)
            if (!SealedFormat.checkHeader(header)) throw IOException("не тот формат файла")
            val body = input.length() - SealedFormat.HEADER
            if (body < SealedFormat.NONCE + SealedFormat.TAG) throw IOException("файл обрезан")
            chunks = (body + SealedFormat.FRAME - 1) / SealedFormat.FRAME
            lastFrame = (body - (chunks - 1) * SealedFormat.FRAME).toInt()
            if (lastFrame < SealedFormat.NONCE + SealedFormat.TAG) throw IOException("файл обрезан")
            plainSize = (chunks - 1) * SealedFormat.CHUNK + lastFrame - SealedFormat.NONCE - SealedFormat.TAG
        } catch (e: Throwable) {
            input.close()
            throw e
        }
    }

    override fun getSize(): Long = plainSize

    @Synchronized
    override fun readAt(position: Long, buffer: ByteArray, offset: Int, size: Int): Int {
        if (position >= plainSize) return -1
        if (size <= 0) return 0
        val i = position / SealedFormat.CHUNK
        val plain = chunk(i)
        val from = (position - i * SealedFormat.CHUNK).toInt()
        val n = minOf(size, plain.size - from)
        System.arraycopy(plain, from, buffer, offset, n)
        return n
    }

    private fun chunk(i: Long): ByteArray {
        if (i == cachedIndex) return cached
        val last = i == chunks - 1
        val frame = ByteArray(if (last) lastFrame else SealedFormat.FRAME)
        input.seek(SealedFormat.HEADER + i * SealedFormat.FRAME)
        input.readFully(frame)
        cached = try {
            SealedFormat.open(key, frame, SealedFormat.aad(header, i, last))
        } catch (e: Exception) {
            throw IOException("кусок записи не расшифровывается", e)
        }
        cachedIndex = i
        return cached
    }

    @Synchronized
    override fun close() {
        input.close()
    }
}
