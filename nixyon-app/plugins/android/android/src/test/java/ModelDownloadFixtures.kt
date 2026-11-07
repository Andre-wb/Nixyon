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

    // Зеркало в памяти: по сумме из адреса отдаёт кусок файла. bodies переопределяет
    // содержимое по сумме; missing: суммы, которых на зеркале нет; flaky: столько раз каждый
    // кусок сначала обрывается.
    class FakeSource(
        private val bodies: Map<String, ByteArray> = emptyMap(),
        private val missing: Set<String> = emptySet(),
        private val flaky: Int = 0,
    ) : RangeSource {
        val reads = java.util.concurrent.ConcurrentLinkedQueue<String>()
        private val failures = java.util.concurrent.ConcurrentHashMap<String, java.util.concurrent.atomic.AtomicInteger>()

        override fun read(url: String, start: Long, end: Long, total: Long, sink: (ByteArray, Int) -> Unit) {
            val sum = url.substringAfterLast('/')
            reads += "$sum:$start"
            if (sum in missing) throw RangeException(RangeException.Kind.MISSING, "404")
            val data = bodies[sum] ?: contents.values.firstOrNull { sha(it) == sum }
                ?: throw RangeException(RangeException.Kind.MISSING, "404")
            val count = failures.getOrPut("$sum:$start") { java.util.concurrent.atomic.AtomicInteger() }
            if (count.getAndIncrement() < flaky) {
                sink(data.copyOfRange(start.toInt(), start.toInt() + 1), 1)
                throw RangeException(RangeException.Kind.TRANSIENT, "обрыв")
            }
            val piece = data.copyOfRange(start.toInt(), end.toInt() + 1)
            sink(piece, piece.size)
        }
    }
}
