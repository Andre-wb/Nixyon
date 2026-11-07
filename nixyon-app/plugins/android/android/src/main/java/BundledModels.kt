package dev.demo.nixyon.speech

import android.content.res.AssetManager
import android.util.Log

// Модели, вшитые в полную сборку APK: описания models/models.json и llm/llm.json в assets,
// по тем же путям их ищут ModelLocator и LlmLocator. В лёгкой сборке их нет.
object BundledModels {
    private const val TAG = "NixyonModels"

    fun read(assets: AssetManager): InstalledModels =
        InstalledModels(
            parsed(assets, "${ModelTarget.SPEECH.dirName}/${ModelTarget.SPEECH.manifestName}") { ModelManifest.parse(it) },
            parsed(assets, "${ModelTarget.LLM.dirName}/${ModelTarget.LLM.manifestName}") { LlmManifest.parse(it) },
        )

    private fun <T> parsed(assets: AssetManager, path: String, parse: (String) -> T): T? {
        val text = assets.readTextOrNull(path) ?: return null
        return try {
            parse(text)
        } catch (e: Exception) {
            Log.w(TAG, "описание $path в APK не читается: ${e.message}")
            null
        }
    }
}
