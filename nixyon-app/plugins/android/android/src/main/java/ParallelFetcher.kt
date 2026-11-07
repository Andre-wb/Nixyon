package dev.demo.nixyon.speech

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.launch
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.channels.FileChannel
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

// Качает файлы кусками в несколько соединений сразу: каждый кусок своим запросом Range,
// байты пишутся в свой участок временного файла. Упавший кусок качается заново с паузой;
// если повторы кончились или файла нет на зеркале, загрузка останавливается целиком.
class ParallelFetcher(
    private val source: RangeSource,
    private val connections: Int,
    private val retryDelayMs: (Int) -> Long = ::backoff,
    private val pause: suspend (Long) -> Unit = { delay(it) },
) {

    // canUseNetwork: false, пока сеть платная, а пользователь её не разрешил; тогда куски
    // ждут. onChunk зовётся после каждого скачанного куска, progress примерно 4 раза в
    // секунду: байт скачано всего и ждёт ли загрузка сети.
    suspend fun fetch(
        files: List<FetchFile>,
        cancelled: AtomicBoolean,
        canUseNetwork: () -> Boolean,
        onChunk: (file: Int, index: Int) -> Unit,
        progress: (done: Long, waiting: Boolean) -> Unit,
    ) {
        val queue = ConcurrentLinkedQueue<Chunk>()
        val bytes = AtomicLong(0)
        files.forEachIndexed { i, f ->
            for (chunk in Chunk.of(i, f.size)) {
                if (chunk.index in f.done) bytes.addAndGet(chunk.size) else queue.add(chunk)
            }
        }
        if (queue.isEmpty()) return
        val waiting = AtomicInteger(0)
        // RandomAccessFile, а не FileChannel.open: тот появился только в API 26.
        val opened = mutableListOf<RandomAccessFile>()
        try {
            for (f in files) {
                f.part.parentFile?.mkdirs()
                val raf = RandomAccessFile(f.part, "rw")
                opened += raf
                if (raf.length() != f.size) raf.setLength(f.size)
            }
            val channels = opened.map { it.channel }
            coroutineScope {
                val ticker = launch {
                    while (isActive) {
                        progress(bytes.get(), waiting.get() > 0)
                        delay(PROGRESS_MS)
                    }
                }
                try {
                    (1..minOf(connections, queue.size)).map {
                        launch(Dispatchers.IO) {
                            while (true) {
                                val chunk = queue.poll() ?: break
                                fetchChunk(files[chunk.file], channels[chunk.file], chunk, bytes, waiting, cancelled, canUseNetwork)
                                onChunk(chunk.file, chunk.index)
                            }
                        }
                    }.joinAll()
                } finally {
                    ticker.cancel()
                }
            }
            progress(bytes.get(), false)
        } finally {
            opened.forEach { it.close() }
        }
    }

    private suspend fun fetchChunk(
        file: FetchFile,
        channel: FileChannel,
        chunk: Chunk,
        bytes: AtomicLong,
        waiting: AtomicInteger,
        cancelled: AtomicBoolean,
        canUseNetwork: () -> Boolean,
    ) {
        var attempt = 0
        while (true) {
            checkCancelled(cancelled)
            if (!canUseNetwork()) {
                waitFor(waiting, NETWORK_POLL_MS)
                continue
            }
            var position = chunk.start
            try {
                source.read(file.url, chunk.start, chunk.end, file.size) { buffer, count ->
                    checkCancelled(cancelled)
                    val data = ByteBuffer.wrap(buffer, 0, count)
                    while (data.hasRemaining()) position += channel.write(data, position)
                    bytes.addAndGet(count.toLong())
                }
                return
            } catch (e: RangeException) {
                bytes.addAndGet(chunk.start - position)
                when (e.kind) {
                    RangeException.Kind.MISSING ->
                        throw ModelsException(ModelsException.DOWNLOAD_FAILED, "файла нет на зеркале: ${e.message}", e)
                    RangeException.Kind.WRONG ->
                        throw ModelsException(ModelsException.DOWNLOAD_CORRUPT, "зеркало отдало не тот кусок: ${e.message}", e)
                    RangeException.Kind.TRANSIENT, RangeException.Kind.BUSY -> {
                        attempt += 1
                        if (attempt > MAX_ATTEMPTS) {
                            throw ModelsException(ModelsException.DOWNLOAD_FAILED, "кусок не скачался за $MAX_ATTEMPTS попыток: ${e.message}", e)
                        }
                        waitFor(waiting, retryDelayMs(attempt))
                    }
                }
            }
        }
    }

    private suspend fun waitFor(waiting: AtomicInteger, ms: Long) {
        waiting.incrementAndGet()
        try {
            pause(ms)
        } finally {
            waiting.decrementAndGet()
        }
    }

    private fun checkCancelled(cancelled: AtomicBoolean) {
        if (cancelled.get()) throw ModelsException(ModelsException.CANCELLED, "загрузка моделей отменена")
    }

    companion object {
        // Повторы одного куска: с паузами до 30 с это около 3 минут без связи.
        const val MAX_ATTEMPTS = 10
        private const val PROGRESS_MS = 250L
        private const val NETWORK_POLL_MS = 2_000L

        fun backoff(attempt: Int): Long = minOf(30_000L, 1_000L shl minOf(attempt - 1, 5))
    }
}
