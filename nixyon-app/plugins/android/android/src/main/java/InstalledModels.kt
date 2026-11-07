package dev.demo.nixyon.speech

import android.util.Log

// Что уже лежит в папке приложения, по описаниям набора: загрузка пишет их последними.
// Нечитаемое описание считается отсутствующим, тогда набор можно скачать заново.
class InstalledModels(val speech: ModelManifest?, val llm: LlmManifest?) {

    val packs: Set<ModelPack>
        get() = buildSet {
            speech?.let { manifest ->
                add(ModelPack.SPEECH)
                manifest.files.keys.forEach { add(ModelPack.of(it)) }
            }
            if (llm != null) add(ModelPack.LLM)
        }

    // Пакеты, у которых все файлы из описания лежат на диске нужного размера. Недокачанный
    // или испорченный набор не считается скачанным: интерфейс предложит скачать его снова.
    fun ready(folders: ModelFolders): Set<ModelPack> = buildSet {
        val speechFiles = speech?.files.orEmpty()
        for (pack in packs - ModelPack.LLM) {
            val files = speechFiles.filterKeys { ModelPack.of(it) == pack }.values
            if (files.all { onDisk(folders, ModelTarget.SPEECH, it) }) add(pack)
        }
        if (ModelPack.SPEECH in packs && ModelPack.SPEECH !in this) removeAll(packs)
        llm?.let { if (onDisk(folders, ModelTarget.LLM, it.model)) add(ModelPack.LLM) }
    }

    private fun onDisk(folders: ModelFolders, target: ModelTarget, file: ModelFile): Boolean {
        val f = folders.file(target, file)
        return f.isFile && f.length() == file.size
    }

    companion object {
        private const val TAG = "NixyonModels"

        fun read(folders: ModelFolders): InstalledModels =
            InstalledModels(
                readOrNull(folders, ModelTarget.SPEECH) { ModelManifest.parse(it) },
                readOrNull(folders, ModelTarget.LLM) { LlmManifest.parse(it) },
            )

        private fun <T> readOrNull(folders: ModelFolders, target: ModelTarget, parse: (String) -> T): T? {
            val file = folders.manifest(target)
            if (!file.isFile) return null
            return try {
                parse(file.readText())
            } catch (e: Exception) {
                Log.w(TAG, "описание ${target.manifestName} не читается: ${e.message}")
                null
            }
        }
    }
}
