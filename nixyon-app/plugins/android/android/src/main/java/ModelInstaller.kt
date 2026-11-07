package dev.demo.nixyon.speech

import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

// Загрузка выбранных моделей с зеркала. Файлы качаются кусками в несколько соединений под
// временными именами, после загрузки сверяются по размеру и SHA-256 из каталога и только
// затем получают свои имена. Описания набора пишутся последними: оборванная загрузка
// никогда не выглядит готовым набором.
class ModelInstaller(
    private val catalog: ModelCatalog,
    private val folders: ModelFolders,
    private val fetcher: ParallelFetcher,
    private val journal: DownloadJournal,
    private val freeBytes: () -> Long,
) {

    // canUseNetwork: можно ли сейчас качать; false, пока сеть платная и не разрешена.
    suspend fun install(
        selected: Set<ModelPack>,
        allowMetered: Boolean,
        canUseNetwork: () -> Boolean,
        cancelled: AtomicBoolean,
        progress: (DownloadProgress) -> Unit,
    ) {
        val plan = DownloadPlan.build(catalog, selected, InstalledModels.read(folders)) { target, file ->
            folders.file(target, file).takeIf { it.isFile }?.length()
        }
        val done = resume(plan)
        val lock = Any()
        val save = { synchronized(lock) { journal.write(DownloadJournal.Entry(selected, allowMetered, done)) } }
        save()
        try {
            val files = plan.items.map { item ->
                val subPath = folders.partSubPath(item.target, item.file)
                FetchFile(urlOf(item.file), folders.part(item.target, item.file), item.file.size, done.getValue(subPath).toSet())
            }
            val total = plan.totalBytes
            fetcher.fetch(
                files,
                cancelled,
                canUseNetwork,
                onChunk = { file, index ->
                    val subPath = folders.partSubPath(plan.items[file].target, plan.items[file].file)
                    synchronized(lock) { done.getValue(subPath).add(index) }
                    save()
                },
                progress = { bytes, waiting ->
                    val phase = if (waiting) DownloadProgress.WAITING else DownloadProgress.DOWNLOADING
                    progress(DownloadProgress(phase, bytes, total))
                },
            )
            progress(DownloadProgress(DownloadProgress.CHECKING, total, total))
            check(plan, done, save)
            finish(plan)
        } catch (e: ModelsException) {
            if (e.code == ModelsException.CANCELLED) cancelAll(plan)
            throw e
        }
        journal.clear()
    }

    // Скачанные куски из журнала засчитываются, только если временный файл на месте и
    // нужной длины. Временные файлы, которых в новом плане нет, удаляются. Затем
    // проверяется место под то, что осталось скачать.
    private fun resume(plan: DownloadPlan): MutableMap<String, MutableSet<Int>> {
        val previous = journal.read()
        val done = LinkedHashMap<String, MutableSet<Int>>()
        var needed = 0L
        for (item in plan.items) {
            val subPath = folders.partSubPath(item.target, item.file)
            val part = folders.part(item.target, item.file)
            val count = Chunk.count(item.file.size)
            val kept = previous?.chunks?.get(subPath)
                ?.filter { it in 0 until count }
                ?.takeIf { part.isFile && part.length() == item.file.size }
                .orEmpty()
                .toMutableSet()
            if (kept.isEmpty()) part.delete()
            done[subPath] = kept
            needed += Chunk.of(0, item.file.size).filter { it.index !in kept }.sumOf { it.size }
        }
        // Журнал пишет только этот класс, но путь всё равно проверяется: удалять можно
        // только временные файлы внутри папки моделей.
        previous?.chunks?.keys
            ?.filter { it !in done && it.endsWith(".part") && ".." !in it }
            ?.forEach { File(folders.base, it).delete() }
        if (freeBytes() < needed + RESERVE_BYTES) {
            throw ModelsException(ModelsException.STORAGE_LOW, "для моделей нужно $needed байт и запас")
        }
        return done
    }

    // Файл с неверной суммой удаляется вместе с его кусками в журнале: повтор скачает его
    // заново, остальные файлы останутся.
    private fun check(plan: DownloadPlan, done: MutableMap<String, MutableSet<Int>>, save: () -> Unit) {
        for (item in plan.items) {
            val part = folders.part(item.target, item.file)
            val good = part.isFile && part.length() == item.file.size && Sha256.hex(part) == item.file.sha256
            if (!good) {
                part.delete()
                done[folders.partSubPath(item.target, item.file)]?.clear()
                save()
                throw ModelsException(ModelsException.DOWNLOAD_CORRUPT, "SHA-256 ${item.file.path} не совпала с каталогом")
            }
        }
    }

    // Старое описание удаляется до переименования: пока файлы меняются, набор не выглядит
    // готовым, а новое описание появляется, когда все файлы уже на месте.
    private fun finish(plan: DownloadPlan) {
        plan.speechFiles?.let { files ->
            folders.manifest(ModelTarget.SPEECH).delete()
            moveInPlace(plan, ModelTarget.SPEECH)
            AtomicFile.write(folders.manifest(ModelTarget.SPEECH), ManifestJson.speech(catalog.speech.version, files))
        }
        plan.llm?.let { llm ->
            folders.manifest(ModelTarget.LLM).delete()
            moveInPlace(plan, ModelTarget.LLM)
            AtomicFile.write(folders.manifest(ModelTarget.LLM), ManifestJson.llm(llm))
        }
    }

    private fun moveInPlace(plan: DownloadPlan, target: ModelTarget) {
        for (item in plan.items.filter { it.target == target }) {
            val part = folders.part(target, item.file)
            val file = folders.file(target, item.file)
            file.delete()
            if (!part.renameTo(file)) {
                throw ModelsException(ModelsException.DOWNLOAD_FAILED, "${item.file.path} не переименовался")
            }
        }
    }

    private fun cancelAll(plan: DownloadPlan) {
        for (item in plan.items) folders.part(item.target, item.file).delete()
        journal.clear()
    }

    private fun urlOf(file: ModelFile): String = "$MIRROR/${file.sha256}"

    companion object {
        // Зеркало моделей на сервере Nixyon Hub, тот же адрес, что MIRROR_DEFAULT в
        // scripts/model-mirror.sh. Файл лежит под своей SHA-256.
        const val MIRROR = "https://nixyon.onyxar.org/models"
        // Столько же соединений хаб разрешает одному адресу.
        const val CONNECTIONS = 52
        // Запас сверх размера моделей, как у записи: телефон без места работает плохо.
        private const val RESERVE_BYTES = 200L * 1024 * 1024

        fun freeBytesOf(dir: File): () -> Long = { android.os.StatFs(dir.absolutePath).availableBytes }
    }
}
