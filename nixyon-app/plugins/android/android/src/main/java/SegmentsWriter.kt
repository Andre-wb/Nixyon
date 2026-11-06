package dev.demo.nixyon.speech

import org.json.JSONArray
import org.json.JSONObject
import java.io.Closeable
import java.io.File

// Результаты разбора пишутся в файл массивом JSON по мере готовности (JsonArrayWriter).
class SegmentsWriter(file: File) : Closeable {
    private val out = JsonArrayWriter(file)
    val count: Int
        get() = out.count

    fun write(
        start: Double,
        end: Double,
        text: String,
        tokens: Array<String>,
        tokenTimes: FloatArray,
        embedding: FloatArray,
        windows: List<WindowEmbedding>,
    ) {
        val item = JSONObject()
        item.put("start", start)
        item.put("end", end)
        item.put("text", text)
        item.put("tokens", JSONArray(tokens.toList()))
        item.put("tokenTimes", tokenTimes.toJsonArray())
        item.put("embedding", embedding.toJsonArray())
        val windowItems = JSONArray()
        for (w in windows) {
            val window = JSONObject()
            window.put("start", w.start)
            window.put("end", w.end)
            window.put("embedding", w.embedding.toJsonArray())
            windowItems.put(window)
        }
        item.put("windows", windowItems)
        out.add(item)
    }

    fun finish() = out.finish()

    override fun close() = out.close()
}
