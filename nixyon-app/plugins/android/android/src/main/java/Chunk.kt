package dev.demo.nixyon.speech

// Кусок файла номер index: байты от start до end включительно. file: номер файла в загрузке.
class Chunk(val file: Int, val index: Int, val start: Long, val end: Long) {
    val size: Long
        get() = end - start + 1

    companion object {
        // 8 МБ: языковая модель делится на сотни кусков, и 52 соединения заняты до конца;
        // журнал при этом обновляется не чаще раза в несколько мегабайт.
        const val SIZE = 8L * 1024 * 1024

        fun count(size: Long): Int = ((size + SIZE - 1) / SIZE).toInt()

        fun of(file: Int, size: Long): List<Chunk> =
            (0 until count(size)).map { i ->
                val start = i * SIZE
                Chunk(file, i, start, minOf(size, start + SIZE) - 1)
            }
    }
}
