package dev.demo.nixyon.speech

import kotlinx.coroutines.delay
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

// Загрузка выбранных моделей с зеркала. Файлы качаются под временными именами, после
// загрузки сверяются по размеру и SHA-256 из каталога и только затем получают свои имена.
// Описания набора пишутся последними: оборванная загрузка никогда не выглядит готовым
// набором.
class ModelInstaller(
    private val catalog: ModelCatalog,
    private val folders: ModelFolders,
    private val queue: DownloadQueue,
    private val journal: DownloadJournal,
    private val freeBytes: () -> Long,
    private val pollMs: Long = POLL_MS,
) {

    suspend fun install(
        selected: Set<ModelPack>,
        allowMetered: Boolean,
        title: String,
        cancelled: AtomicBoolean,
        progress: (DownloadProgress) -> Unit,
    ) {
        val plan = DownloadPlan.build(catalog, selected, InstalledModels.read(folders)) { target, file ->
            folders.file(target, file).takeIf { it.isFile }?.length()
        }
        val ids = start(plan, selected, allowMetered, title)
        val save = { journal.write(DownloadJournal.Entry(selected, allowMetered, ids)) }
        try {
            wait(plan, ids, cancelled, progress, save)
            progress(DownloadProgress(DownloadProgress.CHECKING, plan.totalBytes, plan.totalBytes))
            check(plan, ids, save)
            finish(plan)
        } catch (e: ModelsException) {
            if (e.code == ModelsException.CANCELLED) cancelAll(plan, ids)
            throw e
        }
        queue.remove(ids.values)
        journal.clear()
    }

    // Ставит в очередь недостающие файлы. Загрузка из журнала продолжается, если она жива
    // и разрешение на платную сеть то же; иначе файл качается заново.
    private fun start(plan: DownloadPlan, selected: Set<ModelPack>, allowMetered: Boolean, title: String): MutableMap<String, Long> {
        val previous = journal.read()
        val alive = previous?.let { queue.query(it.ids.values) }.orEmpty()
        val ids = LinkedHashMap<String, Long>()
        var needed = 0L
        for (item in plan.items) {
            val subPath = folders.partSubPath(item.target, item.file)
            val oldId = previous?.ids?.get(subPath)
            val status = oldId?.let { alive[it] }
            val sameNetwork = previous?.allowMetered == allowMetered
            if (oldId != null && status != null && reusable(status, sameNetwork)) {
                ids[subPath] = oldId
                needed += item.file.size - status.downloaded.coerceIn(0, item.file.size)
            } else {
                if (oldId != null) queue.remove(listOf(oldId))
                ids[subPath] = -1
                needed += item.file.size
            }
        }
        previous?.ids?.filterKeys { it !in ids }?.values?.let { queue.remove(it) }
        if (freeBytes() < needed + RESERVE_BYTES) {
            throw ModelsException(ModelsException.STORAGE_LOW, "для моделей нужно $needed байт и запас")
        }
        for (item in plan.items) {
            val subPath = folders.partSubPath(item.target, item.file)
            if (ids[subPath] != -1L) continue
            val part = folders.part(item.target, item.file)
            part.parentFile?.mkdirs()
            part.delete()
            ids[subPath] = queue.enqueue(DownloadRequest(urlOf(item.file), subPath, title, allowMetered))
            journal.write(DownloadJournal.Entry(selected, allowMetered, ids.filterValues { it != -1L }))
        }
        journal.write(DownloadJournal.Entry(selected, allowMetered, ids))
        return ids
    }

    private fun reusable(status: DownloadStatus, sameNetwork: Boolean): Boolean =
        status.state == DownloadStatus.State.DONE ||
            (status.state != DownloadStatus.State.FAILED && sameNetwork)

    private suspend fun wait(
        plan: DownloadPlan,
        ids: MutableMap<String, Long>,
        cancelled: AtomicBoolean,
        progress: (DownloadProgress) -> Unit,
        save: () -> Unit,
    ) {
        while (true) {
            if (cancelled.get()) throw ModelsException(ModelsException.CANCELLED, "загрузка моделей отменена")
            val statuses = queue.query(ids.values)
            var done = 0L
            var waiting = false
            var finished = true
            for (item in plan.items) {
                val subPath = folders.partSubPath(item.target, item.file)
                val id = ids.getValue(subPath)
                val status = statuses[id]
                if (status == null || status.state == DownloadStatus.State.FAILED) {
                    stopUnfinished(ids, statuses, save)
                    throw ModelsException(ModelsException.DOWNLOAD_FAILED, "не скачался ${item.file.path}")
                }
                // Сервер обещает или уже отдал больше, чем в каталоге: файл не тот, качать дальше незачем.
                if ((status.total > 0 && status.total != item.file.size) || status.downloaded > item.file.size) {
                    stopUnfinished(ids, statuses, save)
                    throw ModelsException(ModelsException.DOWNLOAD_CORRUPT, "размер ${item.file.path} не совпал с каталогом")
                }
                done += status.downloaded.coerceAtLeast(0)
                if (status.state == DownloadStatus.State.WAITING) waiting = true
                if (status.state != DownloadStatus.State.DONE) finished = false
            }
            if (finished) return
            val phase = if (waiting) DownloadProgress.WAITING else DownloadProgress.DOWNLOADING
            progress(DownloadProgress(phase, done, plan.totalBytes))
            delay(pollMs)
        }
    }

    private fun check(plan: DownloadPlan, ids: MutableMap<String, Long>, save: () -> Unit) {
        for (item in plan.items) {
            val part = folders.part(item.target, item.file)
            val good = part.isFile && part.length() == item.file.size && Sha256.hex(part) == item.file.sha256
            if (!good) {
                drop(ids, folders.partSubPath(item.target, item.file), save)
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

    // DownloadManager сам повторяет загрузку при обрывах, FAILED у него окончательный.
    // Тогда недокачанное останавливается, чтобы не тратить трафик на загрузку, которая уже
    // не удалась; скачанные целиком файлы остаются в журнале, повтор их не качает.
    private fun stopUnfinished(ids: MutableMap<String, Long>, statuses: Map<Long, DownloadStatus>, save: () -> Unit) {
        val unfinished = ids.filterValues { statuses[it]?.state != DownloadStatus.State.DONE }
        unfinished.keys.forEach { ids.remove(it) }
        queue.remove(unfinished.values)
        save()
    }

    // Файл с неверной суммой уходит из очереди и журнала; остальные остаются для повтора.
    private fun drop(ids: MutableMap<String, Long>, subPath: String, save: () -> Unit) {
        ids.remove(subPath)?.let { queue.remove(listOf(it)) }
        save()
    }

    private fun cancelAll(plan: DownloadPlan, ids: Map<String, Long>) {
        queue.remove(ids.values)
        for (item in plan.items) folders.part(item.target, item.file).delete()
        journal.clear()
    }

    private fun urlOf(file: ModelFile): String = "$MIRROR/${file.sha256}"

    companion object {
        // Зеркало моделей на сервере Nixyon Hub, тот же адрес, что MIRROR_DEFAULT в
        // scripts/model-mirror.sh. Файл лежит под своей SHA-256.
        const val MIRROR = "https://nixyon.onyxar.org/models"
        private const val POLL_MS = 500L
        // Запас сверх размера моделей, как у записи: телефон без места работает плохо.
        private const val RESERVE_BYTES = 200L * 1024 * 1024

        fun freeBytesOf(dir: File): () -> Long = { android.os.StatFs(dir.absolutePath).availableBytes }
    }
}
