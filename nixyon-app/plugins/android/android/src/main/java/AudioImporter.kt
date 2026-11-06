package dev.demo.nixyon.speech

import android.content.Context
import android.media.AudioFormat
import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.net.Uri
import android.provider.OpenableColumns
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicBoolean

class ImportResult(val path: String, val durationSec: Double, val name: String)

// Импорт звука из файла, выбранного пользователем: Android раскодирует его сам
// (m4a/AAC, mp3, wav, ogg/opus, flac и что ещё умеет устройство) в PCM 16 бит с
// исходной частотой и каналами, а в 16 кГц моно его переводит ядро.
class AudioImporter(
    private val context: Context,
    private val recordings: RecordingsDir,
    private val keys: StorageKey,
) {
    fun import(
        uri: Uri,
        cancelled: AtomicBoolean,
        progress: (doneSec: Double, totalSec: Double) -> Unit,
    ): ImportResult {
        if (!recordings.hasRoom()) throw MediaException(MediaException.STORAGE_LOW, "мало места для импорта")
        val name = displayName(uri)
        val extractor = MediaExtractor()
        var codec: MediaCodec? = null
        val part = recordings.newPart("imp")
        var writer: WavPcm16Writer? = null
        // Частоту и каналы WAV берёт из формата, который отдаёт декодер.
        fun newWriter(f: MediaFormat) =
            WavPcm16Writer(part, keys.get(), f.getInteger(MediaFormat.KEY_SAMPLE_RATE), f.getInteger(MediaFormat.KEY_CHANNEL_COUNT))
        try {
            try {
                extractor.setDataSource(context, uri, null)
            } catch (e: Exception) {
                throw MediaException(MediaException.UNSUPPORTED_AUDIO, "файл не открывается как звук: ${e.message}", e)
            }
            val track = (0 until extractor.trackCount).firstOrNull {
                extractor.getTrackFormat(it).getString(MediaFormat.KEY_MIME)?.startsWith("audio/") == true
            } ?: throw MediaException(MediaException.NO_AUDIO_TRACK, "в файле нет звуковой дорожки")
            extractor.selectTrack(track)
            val format = extractor.getTrackFormat(track)
            val mime = format.getString(MediaFormat.KEY_MIME)!!
            val totalSec = if (format.containsKey(MediaFormat.KEY_DURATION)) format.getLong(MediaFormat.KEY_DURATION) / 1e6 else 0.0
            if (totalSec > RecordingsDir.MAX_SECONDS) throw MediaException(MediaException.TOO_LONG, "файл длиннее 3 часов")
            codec = try {
                MediaCodec.createDecoderByType(mime).also { it.configure(format, null, null, 0) }
            } catch (e: Exception) {
                throw MediaException(MediaException.UNSUPPORTED_AUDIO, "нет декодера для $mime: ${e.message}", e)
            }
            codec.start()

            val info = MediaCodec.BufferInfo()
            var inputDone = false
            var pcmFloat = false
            var lastReport = 0L
            var chunk = ByteArray(0)
            while (true) {
                if (cancelled.get()) throw MediaException(MediaException.CANCELLED, "импорт отменён")
                if (!inputDone) {
                    val index = codec.dequeueInputBuffer(TIMEOUT_US)
                    if (index >= 0) {
                        val buffer = codec.getInputBuffer(index)!!
                        val size = extractor.readSampleData(buffer, 0)
                        if (size < 0) {
                            codec.queueInputBuffer(index, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                            inputDone = true
                        } else {
                            codec.queueInputBuffer(index, 0, size, extractor.sampleTime, 0)
                            extractor.advance()
                        }
                    }
                }
                val out = codec.dequeueOutputBuffer(info, TIMEOUT_US)
                when {
                    out == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                        val f = codec.outputFormat
                        pcmFloat = f.containsKey(MediaFormat.KEY_PCM_ENCODING) &&
                            f.getInteger(MediaFormat.KEY_PCM_ENCODING) == AudioFormat.ENCODING_PCM_FLOAT
                        if (writer != null) throw MediaException(MediaException.UNSUPPORTED_AUDIO, "формат звука меняется посреди файла")
                        writer = newWriter(f)
                    }
                    out >= 0 -> {
                        val buffer = codec.getOutputBuffer(out)!!
                        if (info.size > 0) {
                            val w = writer ?: newWriter(codec.outputFormat).also { writer = it }
                            buffer.position(info.offset)
                            buffer.limit(info.offset + info.size)
                            if (pcmFloat) {
                                val floats = buffer.order(ByteOrder.nativeOrder()).asFloatBuffer()
                                if (chunk.size < floats.remaining() * 2) chunk = ByteArray(floats.remaining() * 2)
                                var i = 0
                                while (floats.hasRemaining()) {
                                    Pcm16.put(chunk, i, Pcm16.fromFloat(floats.get()))
                                    i += 2
                                }
                                w.write(chunk, i)
                            } else {
                                if (chunk.size < info.size) chunk = ByteArray(info.size)
                                buffer.get(chunk, 0, info.size)
                                w.write(chunk, info.size)
                            }
                            if (w.durationSec > RecordingsDir.MAX_SECONDS) throw MediaException(MediaException.TOO_LONG, "файл длиннее 3 часов")
                            val now = System.currentTimeMillis()
                            if (now - lastReport >= REPORT_EVERY_MS) {
                                if (!recordings.hasRoom()) throw MediaException(MediaException.STORAGE_LOW, "кончается место")
                                progress(info.presentationTimeUs / 1e6, totalSec)
                                lastReport = now
                            }
                        }
                        codec.releaseOutputBuffer(out, false)
                        if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) break
                    }
                }
            }
            val w = writer ?: throw MediaException(MediaException.UNSUPPORTED_AUDIO, "декодер не дал звука")
            w.close()
            writer = null
            if (w.dataBytes == 0L) throw MediaException(MediaException.UNSUPPORTED_AUDIO, "в файле нет звука")
            progress(w.durationSec, w.durationSec)
            val done = recordings.keep(part)
            return ImportResult(done.absolutePath, w.durationSec, name)
        } catch (e: MediaException) {
            throw e
        } catch (e: Exception) {
            throw MediaException(MediaException.IMPORT_FAILED, "импорт не удался: ${e.message}", e)
        } finally {
            try {
                writer?.close()
            } catch (_: Exception) {
            }
            try {
                codec?.stop()
            } catch (_: Exception) {
            }
            codec?.release()
            extractor.release()
            if (part.exists()) part.delete()
        }
    }

    // Имя файла для расшифровки: только последнее имя, без пути.
    private fun displayName(uri: Uri): String {
        val fromProvider = try {
            context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
                if (it.moveToFirst()) it.getString(0) else null
            }
        } catch (_: Exception) {
            null
        }
        return (fromProvider ?: uri.lastPathSegment ?: "").substringAfterLast('/').take(200)
    }

    companion object {
        private const val TIMEOUT_US = 10_000L
        private const val REPORT_EVERY_MS = 250L
    }
}
