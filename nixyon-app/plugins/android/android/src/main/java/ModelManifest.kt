package dev.demo.nixyon.speech

import org.json.JSONException
import org.json.JSONObject

class ModelFile(val path: String, val size: Long, val sha256: String) {
    companion object {
        // Имя файла из манифеста: только внутри папки моделей, без каталогов и «.» в начале.
        fun isSafePath(path: String): Boolean =
            path.isNotEmpty() && !path.contains('/') && !path.contains('\\') && !path.startsWith(".")
    }
}

class ModelManifest(val version: String, val files: Map<ModelRole, ModelFile>) {
    fun file(role: ModelRole): ModelFile = files.getValue(role)

    val hasLiveModel: Boolean
        get() = liveLanguages.isNotEmpty()

    // Языки, для которых в наборе есть живая модель.
    val liveLanguages: Set<SpeechLanguage>
        get() = SpeechLanguage.entries.filter { language -> ModelRole.live(language).all { it in files } }.toSet()

    val hasDenoiser: Boolean
        get() = ModelRole.DENOISER in files
    val hasAgeGender: Boolean
        get() = ModelRole.AGE_GENDER in files
    val hasEmotion: Boolean
        get() = ModelRole.EMOTION in files

    companion object {
        fun parse(text: String): ModelManifest {
            try {
                val root = JSONObject(text)
                val version = root.getString("version")
                val filesJson = root.getJSONObject("files")
                val present = ModelRole.entries.filter { role ->
                    if (!filesJson.has(role.key) && !role.optional) {
                        throw ModelsException(ModelsException.BAD_MANIFEST, "нет роли ${role.key}")
                    }
                    filesJson.has(role.key)
                }
                for (language in SpeechLanguage.entries) {
                    val roles = ModelRole.live(language)
                    val live = roles.count { it in present }
                    if (live != 0 && live != roles.size) {
                        throw ModelsException(ModelsException.BAD_MANIFEST, "у живой модели ${language.code} есть не все файлы")
                    }
                }
                val files = present.associateWith { role ->
                    val entry = filesJson.getJSONObject(role.key)
                    val path = entry.getString("path")
                    if (!ModelFile.isSafePath(path)) {
                        throw ModelsException(ModelsException.BAD_MANIFEST, "недопустимый путь у роли ${role.key}")
                    }
                    ModelFile(path, entry.getLong("size"), entry.getString("sha256").lowercase())
                }
                return ModelManifest(version, files)
            } catch (e: JSONException) {
                throw ModelsException(ModelsException.BAD_MANIFEST, "models.json не читается: ${e.message}", e)
            }
        }
    }
}
