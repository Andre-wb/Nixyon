package dev.demo.nixyon.speech

import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.io.File

// Начатая загрузка моделей: что выбрано и номера загрузок DownloadManager по пути файла.
// Если процесс убит посреди загрузки, следующий запуск подхватывает те же загрузки.
class DownloadJournal(private val file: File) {

    class Entry(val packs: Set<ModelPack>, val allowMetered: Boolean, val ids: Map<String, Long>)

    // Нечитаемый журнал считается пустым: загрузка тогда начнётся заново.
    fun read(): Entry? {
        if (!file.isFile) return null
        return try {
            val root = JSONObject(file.readText())
            val packsJson = root.getJSONArray("packs")
            val packs = (0 until packsJson.length()).mapNotNull { ModelPack.fromKey(packsJson.getString(it)) }.toSet()
            val idsJson = root.getJSONObject("ids")
            val ids = idsJson.keys().asSequence().associateWith { idsJson.getLong(it) }
            Entry(packs, root.getBoolean("allowMetered"), ids)
        } catch (_: JSONException) {
            null
        }
    }

    fun write(entry: Entry) {
        val ids = JSONObject()
        for ((path, id) in entry.ids) ids.put(path, id)
        val root = JSONObject()
            .put("packs", JSONArray(entry.packs.map { it.key }))
            .put("allowMetered", entry.allowMetered)
            .put("ids", ids)
        AtomicFile.write(file, root.toString())
    }

    fun clear() {
        file.delete()
    }
}
