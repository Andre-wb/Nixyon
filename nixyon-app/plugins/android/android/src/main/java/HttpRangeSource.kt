package dev.demo.nixyon.speech

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

// Кусок файла по HTTPS запросом Range. Ответ 206 принимается, только если Content-Range
// совпадает с запрошенным; 200 только когда просили файл целиком.
class HttpRangeSource : RangeSource {
    override fun read(url: String, start: Long, end: Long, total: Long, sink: (ByteArray, Int) -> Unit) {
        if (!url.startsWith("https://")) throw RangeException(RangeException.Kind.WRONG, "адрес зеркала не https")
        val connection = try {
            URL(url).openConnection() as HttpURLConnection
        } catch (e: IOException) {
            throw RangeException(RangeException.Kind.TRANSIENT, "соединение не открылось: ${e.message}", e)
        }
        try {
            connection.connectTimeout = CONNECT_TIMEOUT_MS
            connection.readTimeout = READ_TIMEOUT_MS
            connection.useCaches = false
            connection.setRequestProperty("Range", "bytes=$start-$end")
            val code = try {
                connection.responseCode
            } catch (e: IOException) {
                throw RangeException(RangeException.Kind.TRANSIENT, "нет ответа: ${e.message}", e)
            }
            checkAnswer(connection, code, start, end, total)
            val expected = end - start + 1
            var got = 0L
            try {
                connection.inputStream.use { input ->
                    val buffer = ByteArray(BUFFER_SIZE)
                    while (got < expected) {
                        val read = input.read(buffer)
                        if (read < 0) break
                        if (got + read > expected) throw RangeException(RangeException.Kind.WRONG, "кусок длиннее запрошенного")
                        sink(buffer, read)
                        got += read
                    }
                }
            } catch (e: IOException) {
                throw RangeException(RangeException.Kind.TRANSIENT, "обрыв на $got из $expected байт: ${e.message}", e)
            }
            if (got != expected) throw RangeException(RangeException.Kind.TRANSIENT, "кусок оборвался на $got из $expected байт")
        } finally {
            connection.disconnect()
        }
    }

    private fun checkAnswer(connection: HttpURLConnection, code: Int, start: Long, end: Long, total: Long) {
        when {
            code == HttpURLConnection.HTTP_PARTIAL -> {
                val range = connection.getHeaderField("Content-Range")
                if (range != "bytes $start-$end/$total") {
                    throw RangeException(RangeException.Kind.WRONG, "Content-Range $range, просили $start-$end/$total")
                }
            }
            code == HttpURLConnection.HTTP_OK && start == 0L && end == total - 1 -> {
                if (connection.contentLengthLong != total) {
                    throw RangeException(RangeException.Kind.WRONG, "длина ${connection.contentLengthLong}, ожидалась $total")
                }
            }
            code == HTTP_TOO_MANY -> throw RangeException(RangeException.Kind.BUSY, "сервер занят")
            code >= 500 -> throw RangeException(RangeException.Kind.TRANSIENT, "ошибка сервера $code")
            code == HttpURLConnection.HTTP_OK -> throw RangeException(RangeException.Kind.WRONG, "сервер не понял Range")
            else -> throw RangeException(RangeException.Kind.MISSING, "ответ $code")
        }
    }

    companion object {
        private const val CONNECT_TIMEOUT_MS = 15_000
        private const val READ_TIMEOUT_MS = 30_000
        private const val BUFFER_SIZE = 64 * 1024
        private const val HTTP_TOO_MANY = 429
    }
}
