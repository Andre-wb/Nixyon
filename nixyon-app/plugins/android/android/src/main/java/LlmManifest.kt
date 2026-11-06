package dev.demo.nixyon.speech

import org.json.JSONException
import org.json.JSONObject

// llm.json в папке llm: версия и файл модели GGUF с размером и SHA-256.
class LlmManifest(val version: String, val model: ModelFile) {
    companion object {
        fun parse(text: String): LlmManifest {
            try {
                val root = JSONObject(text)
                val entry = root.getJSONObject("model")
                val path = entry.getString("path")
                if (!ModelFile.isSafePath(path)) {
                    throw LlmException(LlmException.BROKEN, "недопустимый путь модели")
                }
                return LlmManifest(
                    root.getString("version"),
                    ModelFile(path, entry.getLong("size"), entry.getString("sha256").lowercase()),
                )
            } catch (e: JSONException) {
                throw LlmException(LlmException.BROKEN, "llm.json не читается: ${e.message}", e)
            }
        }
    }
}
