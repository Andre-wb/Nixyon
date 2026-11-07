package dev.demo.nixyon.speech

import java.io.File
import java.security.MessageDigest

// Каталог и подделка очереди загрузок для тестов загрузки моделей.
object ModelDownloadFixtures {
    fun sha(data: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(data).joinToString("") { "%02x".format(it) }

    // Содержимое файла каждой роли: короткие разные байты.
    val contents: Map<String, ByteArray> =
        (ModelRole.entries.map { it.key } + "llm").associateWith { "data of $it".toByteArray() }

    fun catalog(roles: List<ModelRole> = ModelRole.entries, version: String = "set-1"): ModelCatalog {
        val files = roles.joinToString(",") { role ->
            val data = contents.getValue(role.key)
            """"${role.key}":{"path":"${role.key}.onnx","size":${data.size},"sha256":"${sha(data)}"}"""
        }
        val llm = contents.getValue("llm")
        return ModelCatalog.parse(
            """{"version":"$version","files":{$files}}""",
            """{"version":"llm-1","model":{"path":"model.gguf","size":${llm.size},"sha256":"${sha(llm)}"}}""",
        )
    }

    // По сумме из адреса находит содержимое и сразу «скачивает» файл. bodies переопределяет
    // содержимое по сумме; fail: суммы, загрузка которых падает.
    class FakeQueue(
        private val base: File,
        private val bodies: Map<String, ByteArray> = emptyMap(),
        private val fail: Set<String> = emptySet(),
        // Суммы, загрузка которых идёт и не кончается.
        private val running: Set<String> = emptySet(),
    ) : DownloadQueue {
        val requests = mutableListOf<DownloadRequest>()
        val removed = mutableListOf<Long>()
        val statuses = HashMap<Long, DownloadStatus>()
        private val paths = HashMap<Long, String>()
        private var next = 1L

        override fun enqueue(request: DownloadRequest): Long {
            requests += request
            val id = next++
            paths[id] = request.subPath
            val sum = request.url.substringAfterLast('/')
            val data = bodies[sum] ?: contents.values.firstOrNull { sha(it) == sum }
            if (sum in running) {
                statuses[id] = DownloadStatus(DownloadStatus.State.RUNNING, 0, -1)
            } else if (data == null || sum in fail) {
                statuses[id] = DownloadStatus(DownloadStatus.State.FAILED, 0, -1)
            } else {
                val file = File(base, request.subPath)
                file.parentFile?.mkdirs()
                file.writeBytes(data)
                statuses[id] = DownloadStatus(DownloadStatus.State.DONE, data.size.toLong(), data.size.toLong())
            }
            return id
        }

        override fun query(ids: Collection<Long>): Map<Long, DownloadStatus> =
            ids.mapNotNull { id -> statuses[id]?.let { id to it } }.toMap()

        // Как DownloadManager: удаляет и файл загрузки, если он ещё на месте.
        override fun remove(ids: Collection<Long>) {
            removed += ids
            for (id in ids) {
                statuses.remove(id)
                paths.remove(id)?.let { File(base, it).delete() }
            }
        }
    }
}
