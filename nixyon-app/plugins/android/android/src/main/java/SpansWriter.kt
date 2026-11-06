package dev.demo.nixyon.speech

import org.json.JSONObject
import java.io.File

// Отрезки нейросегментации пишутся отдельным файлом массивом JSON, как куски речи
// в SegmentsWriter: формат файла кусков от этого не меняется.
object SpansWriter {
    fun write(file: File, spans: List<VoiceSpan>) {
        JsonArrayWriter(file).use { out ->
            for (span in spans) {
                val item = JSONObject()
                item.put("start", span.start)
                item.put("end", span.end)
                item.put("embedding", span.embedding.toJsonArray())
                out.add(item)
            }
            out.finish()
        }
    }
}
