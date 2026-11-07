package dev.demo.nixyon.speech

// Что скачать и какие описания набора записать в конце. Уже установленные модели остаются
// в наборе, обязательный набор речи входит всегда. Файл не качается заново, если он уже
// установлен с той же суммой и лежит нужного размера.
class DownloadPlan(
    val items: List<CatalogFile>,
    // Роли речевого набора после загрузки; null: описание речи не меняется.
    val speechFiles: Map<ModelRole, ModelFile>?,
    // Языковая модель после загрузки; null: её описание не меняется.
    val llm: LlmManifest?,
) {
    val totalBytes: Long
        get() = items.sumOf { it.file.size }

    companion object {
        fun build(
            catalog: ModelCatalog,
            selected: Set<ModelPack>,
            installed: InstalledModels,
            sizeOnDisk: (ModelTarget, ModelFile) -> Long?,
        ): DownloadPlan {
            val packs = selected + installed.packs + ModelPack.SPEECH
            val items = mutableListOf<CatalogFile>()

            val speechFiles = catalog.speech.files.filterKeys { ModelPack.of(it) in packs }
            for ((role, file) in speechFiles) {
                val have = installed.speech?.files?.get(role)
                if (!sameFile(have, file) || sizeOnDisk(ModelTarget.SPEECH, file) != file.size) {
                    items += CatalogFile(ModelTarget.SPEECH, role, file)
                }
            }
            val old = installed.speech
            val speechChanged = items.isNotEmpty() || old == null ||
                old.version != catalog.speech.version || old.files.keys != speechFiles.keys

            var llm: LlmManifest? = null
            if (ModelPack.LLM in packs) {
                val file = catalog.llm.model
                val fresh = !sameFile(installed.llm?.model, file) ||
                    sizeOnDisk(ModelTarget.LLM, file) != file.size
                if (fresh) items += CatalogFile(ModelTarget.LLM, null, file)
                if (fresh || installed.llm?.version != catalog.llm.version) llm = catalog.llm
            }
            return DownloadPlan(items, if (speechChanged) speechFiles else null, llm)
        }

        private fun sameFile(have: ModelFile?, want: ModelFile): Boolean =
            have != null && have.path == want.path && have.sha256 == want.sha256 && have.size == want.size
    }
}
