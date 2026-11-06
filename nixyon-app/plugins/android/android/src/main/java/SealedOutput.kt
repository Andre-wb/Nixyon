package dev.demo.nixyon.speech

import java.io.Closeable
import java.io.File
import java.io.RandomAccessFile
import javax.crypto.SecretKey

// Потоковое шифрование в файл формата SealedFormat. Начало файла можно поправить при
// закрытии (`close(head)`): так WAV получает длины в заголовке. Каждый полный кусок, и
// первый тоже, сразу уходит в файл: если приложение упадёт посреди записи, на диске
// останутся все полные куски, кроме последнего неполного, и SealedRecovery соберёт из них
// читаемый файл. Первый кусок держится ещё и в памяти: при закрытии он шифруется заново,
// уже с поправкой. nonce у каждого шифрования свой, случайный, поэтому повторное
// шифрование куска nonce не повторяет. Без close(head) файл не читается, пока его не
// восстановят.
// fsync только при закрытии: после падения приложения записанное лежит в кэше ядра и
// никуда не пропадает, а fsync в потоке записи может затянуться и оборвать звук.
class SealedOutput(file: File, private val key: SecretKey) : Closeable {
    private val out = RandomAccessFile(file, "rw")
    private val header = SealedFormat.header()
    private val buf = ByteArray(SealedFormat.CHUNK)
    private var filled = 0
    private var index = 0L
    private var first: ByteArray? = null
    private var closed = false

    init {
        out.setLength(0)
        out.write(header)
    }

    fun write(bytes: ByteArray, length: Int) {
        var from = 0
        while (from < length) {
            if (filled == buf.size) emitFull()
            val take = minOf(length - from, buf.size - filled)
            System.arraycopy(bytes, from, buf, filled, take)
            filled += take
            from += take
        }
    }

    // Полный кусок уходит, только когда пришли следующие байты: иначе неизвестно,
    // последний ли он.
    private fun emitFull() {
        if (index == 0L) first = buf.copyOf()
        writeFrame(index, buf, filled, last = false)
        index += 1
        filled = 0
    }

    fun close(head: ByteArray) {
        if (closed) return
        closed = true
        try {
            val start = first
            if (start == null) {
                require(head.size <= filled) { "поправка длиннее записанного" }
                System.arraycopy(head, 0, buf, 0, head.size)
                writeFrame(0, buf, filled, last = true)
            } else {
                System.arraycopy(head, 0, start, 0, head.size)
                writeFrame(0, start, start.size, last = false)
                writeFrame(index, buf, filled, last = true)
            }
            out.fd.sync()
        } finally {
            out.close()
        }
    }

    // Без поправки: просто закрыть. Недописанный файл не читается, пока его не восстановят.
    override fun close() {
        if (closed) return
        closed = true
        out.close()
    }

    private fun writeFrame(i: Long, plain: ByteArray, length: Int, last: Boolean) {
        val frame = SealedFormat.seal(key, plain, length, SealedFormat.aad(header, i, last))
        out.seek(SealedFormat.HEADER + i * SealedFormat.FRAME)
        out.write(frame)
    }
}
