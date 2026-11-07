package dev.demo.nixyon.speech

import org.json.JSONArray
import org.json.JSONException
import java.io.File

// Необязательные модели, которые пользователь выключил в настройках. Файлы остаются на
// месте (в полной сборке они внутри APK), выключенную модель просто не грузят. Набор речи
// выключить нельзя.
class DisabledModels(private val file: File) {

    fun read(): Set<ModelPack> {
        if (!file.isFile) return emptySet()
        return try {
            val list = JSONArray(file.readText())
            (0 until list.length()).mapNotNull { ModelPack.fromKey(list.getString(it)) }.filter { !it.required }.toSet()
        } catch (_: JSONException) {
            emptySet()
        }
    }

    fun write(packs: Set<ModelPack>) {
        AtomicFile.write(file, JSONArray(packs.filter { !it.required }.map { it.key }).toString())
    }

    // Набор речи без ролей выключенных моделей.
    fun apply(manifest: ModelManifest, disabled: Set<ModelPack> = read()): ModelManifest =
        if (disabled.isEmpty()) manifest else ModelManifest(manifest.version, manifest.files.filterKeys { ModelPack.of(it) !in disabled })

    companion object {
        const val FILE = "disabled-models.json"

        fun of(context: android.content.Context) = DisabledModels(File(context.noBackupFilesDir, FILE))
    }
}
