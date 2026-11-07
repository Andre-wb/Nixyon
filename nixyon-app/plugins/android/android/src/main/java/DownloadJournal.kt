package dev.demo.nixyon.speech

import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.io.File

// Начатая загрузка моделей: что выбрано, разрешена ли платная сеть и какие куски каждого
// временного файла уже скачаны. Если процесс убит посреди загрузки, следующий запуск
// докачивает только недостающие куски.
class DownloadJournal(private val file: File) {

    class Entry(val packs: Set<ModelPack>, val allowMetered: Boolean, val chunks: Map<String, Set<Int>>)

    // Нечитаемый журнал считается пустым: загрузка тогда начнётся заново.
    fun read(): Entry? {
        if (!file.isFile) return null
        return try {
            val root = JSONObject(file.readText())
            val packsJson = root.getJSONArray("packs")
            val packs = (0 until packsJson.length()).mapNotNull { ModelPack.fromKey(packsJson.getString(it)) }.toSet()
            val chunksJson = root.getJSONObject("chunks")
            val chunks = chunksJson.keys().asSequence().associateWith { path ->
                val list = chunksJson.getJSONArray(path)
                (0 until list.length()).map { list.getInt(it) }.toSet()
            }
            Entry(packs, root.getBoolean("allowMetered"), chunks)
        } catch (_: JSONException) {
            null
        }
    }

    fun write(entry: Entry) {
        val chunks = JSONObject()
        for ((path, done) in entry.chunks) chunks.put(path, JSONArray(done.sorted()))
        val root = JSONObject()
            .put("packs", JSONArray(entry.packs.map { it.key }))
            .put("allowMetered", entry.allowMetered)
            .put("chunks", chunks)
        AtomicFile.write(file, root.toString())
    }

    fun clear() {
        file.delete()
    }
}
