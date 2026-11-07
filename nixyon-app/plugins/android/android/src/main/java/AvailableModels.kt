package dev.demo.nixyon.speech

// Какие модели есть на телефоне, так же, как их выбирают ModelLocator и LlmLocator: если в
// папке приложения есть описание набора, берётся папка, и её файлы должны быть на месте;
// иначе берутся модели из APK.
object AvailableModels {
    fun packs(local: InstalledModels, localReady: Set<ModelPack>, bundled: InstalledModels): Set<ModelPack> {
        val speech = if (local.speech != null) localReady - ModelPack.LLM else bundled.packs - ModelPack.LLM
        val llm = if (local.llm != null) localReady.filter { it == ModelPack.LLM } else bundled.packs.filter { it == ModelPack.LLM }
        return speech + llm
    }
}
