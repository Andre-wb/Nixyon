package dev.demo.nixyon.speech

import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File
import java.nio.file.Files

// Файлы, которые читает Rust: массив кусков разбора и массив отрезков нейросегментации.
class ResultFilesTest {
    private val dir = Files.createTempDirectory("nixyon-json").toFile()

    @Test
    fun segmentsFileIsOneJsonArrayWithAllFields() {
        val file = File(dir, "segments.json")
        val count = SegmentsWriter(file).use { w ->
            w.write(0.5, 1.5, "привет", arrayOf("при", "вет"), floatArrayOf(0.1f, 0.4f), floatArrayOf(0.25f, -1f), emptyList())
            w.write(2.0, 3.0, "", emptyArray(), FloatArray(0), FloatArray(0), listOf(WindowEmbedding(2.0, 2.5, floatArrayOf(0.5f))))
            w.finish()
            w.count
        }
        assertEquals(2, count)
        val items = JSONArray(file.readText())
        assertEquals(2, items.length())
        val first = items.getJSONObject(0)
        assertEquals(0.5, first.getDouble("start"), 0.0)
        assertEquals(1.5, first.getDouble("end"), 0.0)
        assertEquals("привет", first.getString("text"))
        assertEquals("вет", first.getJSONArray("tokens").getString(1))
        assertEquals(0.4f.toDouble(), first.getJSONArray("tokenTimes").getDouble(1), 0.0)
        assertEquals(-1.0, first.getJSONArray("embedding").getDouble(1), 0.0)
        assertEquals(0, first.getJSONArray("windows").length())
        val window = items.getJSONObject(1).getJSONArray("windows").getJSONObject(0)
        assertEquals(2.5, window.getDouble("end"), 0.0)
        assertEquals(0.5, window.getJSONArray("embedding").getDouble(0), 0.0)
    }

    @Test
    fun spansFileIsAnArrayEvenWhenEmpty() {
        val file = File(dir, "spans.json")
        SpansWriter.write(file, emptyList())
        assertEquals("[]", file.readText())
        SpansWriter.write(file, listOf(VoiceSpan(1.0, 2.0, floatArrayOf(0.5f)), VoiceSpan(3.0, 4.0, floatArrayOf())))
        val items = JSONArray(file.readText())
        assertEquals(2, items.length())
        assertEquals(3.0, items.getJSONObject(1).getDouble("start"), 0.0)
        assertEquals(0.5, items.getJSONObject(0).getJSONArray("embedding").getDouble(0), 0.0)
    }
}
