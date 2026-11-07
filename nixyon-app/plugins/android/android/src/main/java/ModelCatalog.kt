package dev.demo.nixyon.speech

import android.content.res.AssetManager

// Каталог в APK: какие файлы есть на зеркале, их размеры и SHA-256. Его собирает
// scripts/build-model-catalog.py. По нему проверяется всё скачанное, поэтому он живёт в
// подписанном APK, а не приходит с зеркала.
class ModelCatalog(val speech: ModelManifest, val llm: LlmManifest) {

    fun files(pack: ModelPack): List<CatalogFile> =
        if (pack == ModelPack.LLM) {
            listOf(CatalogFile(ModelTarget.LLM, null, llm.model))
        } else {
            speech.files
                .filter { (role, _) -> ModelPack.of(role) == pack }
                .map { (role, file) -> CatalogFile(ModelTarget.SPEECH, role, file) }
        }

    fun size(pack: ModelPack): Long = files(pack).sumOf { it.file.size }

    // Необязательной модели в каталоге может не быть, тогда её нет и на экране.
    val packs: List<ModelPack>
        get() = ModelPack.entries.filter { files(it).isNotEmpty() }

    companion object {
        private const val ASSETS_DIR = "catalog"
        private val SHA256 = Regex("^[0-9a-f]{64}$")

        fun read(assets: AssetManager): ModelCatalog {
            val speech = assets.readTextOrNull("$ASSETS_DIR/${ModelTarget.SPEECH.manifestName}")
            val llm = assets.readTextOrNull("$ASSETS_DIR/${ModelTarget.LLM.manifestName}")
            if (speech == null || llm == null) {
                throw ModelsException(ModelsException.BAD_MANIFEST, "в APK нет каталога моделей")
            }
            return parse(speech, llm)
        }

        // Сумма идёт в адрес на зеркале, поэтому принимаются только 64 шестнадцатеричных знака.
        fun parse(speechText: String, llmText: String): ModelCatalog {
            val speech = ModelManifest.parse(speechText)
            val llm = try {
                LlmManifest.parse(llmText)
            } catch (e: LlmException) {
                throw ModelsException(ModelsException.BAD_MANIFEST, "каталог языковой модели не читается: ${e.message}", e)
            }
            val files = speech.files.values + llm.model
            if (files.any { !SHA256.matches(it.sha256) || it.size <= 0 }) {
                throw ModelsException(ModelsException.BAD_MANIFEST, "в каталоге моделей неверная сумма или размер")
            }
            return ModelCatalog(speech, llm)
        }
    }
}
