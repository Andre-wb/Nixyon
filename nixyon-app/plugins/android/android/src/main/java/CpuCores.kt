package dev.demo.nixyon.speech

import android.util.Log
import java.io.File

// Ядра процессора и число потоков для моделей.
//
// У телефонов ядра разные: быстрые и экономичные (Cortex-A55, A510, A520 слабее быстрых в
// 3-4 раза). Прежнее правило «не больше 4 потоков», как в примере llama.cpp для Android,
// оставляло без дела часть быстрых ядер на телефонах, где их 5-8. Силу ядра даёт
// cpu_capacity (0-1024): по ней раскладывает задачи сам планировщик Android. Быстрое ядро:
// не слабее половины самого сильного. Потоков столько, сколько быстрых ядер, но не меньше
// прежних min(ядра, 4) и не больше 8. Где быстрых ядер 4 и меньше или cpu_capacity не
// прочитать, всё как раньше.
//
// Наборы инструкций (dotprod, i8mm, SVE, fp16) llama.cpp и onnxruntime выбирают сами при
// запуске; здесь они только пишутся в лог вместе с числом потоков, чтобы было видно, на чём
// считает телефон.
object CpuCores {
    const val BASE_THREADS = 4
    const val MAX_THREADS = 8
    private const val TAG = "NixyonSpeech"
    private val SHOWN_FEATURES = listOf("asimddp", "i8mm", "sve", "sve2", "sme", "bf16", "asimdhp")

    // Потоки для разбора записи и чтения текста языковой моделью.
    val computeThreads: Int by lazy {
        val cpus = Runtime.getRuntime().availableProcessors()
        val capacities = capacities(File("/sys/devices/system/cpu"), cpus)
        val fast = fastCount(capacities)
        val threads = threads(cpus, fast)
        Log.i(
            TAG,
            "процессор: ядер $cpus, быстрых ${fast ?: "?"} (cpu_capacity ${capacities ?: "нет"}), " +
                "инструкции ${features(readText(File("/proc/cpuinfo")))}, потоков $threads",
        )
        threads
    }

    // Потоки для выдачи ответа языковой моделью: она упирается в скорость памяти, а не ядер,
    // и лишние потоки только ждут друг друга, поэтому как раньше.
    val generateThreads: Int
        get() = Runtime.getRuntime().availableProcessors().coerceIn(1, BASE_THREADS)

    fun threads(cpus: Int, fast: Int?): Int {
        val base = cpus.coerceIn(1, BASE_THREADS)
        return maxOf(base, minOf(fast ?: 0, cpus, MAX_THREADS))
    }

    // null: силы ядер неизвестны.
    fun fastCount(capacities: List<Int>?): Int? {
        if (capacities == null || capacities.any { it <= 0 }) return null
        val top = capacities.maxOrNull() ?: return null
        return capacities.count { 2 * it >= top }
    }

    // cpu_capacity каждого ядра по порядку; null, если хоть одно не прочитать.
    fun capacities(root: File, cpus: Int): List<Int>? {
        val out = ArrayList<Int>(cpus)
        for (i in 0 until cpus) {
            out.add(readText(File(root, "cpu$i/cpu_capacity"))?.trim()?.toIntOrNull() ?: return null)
        }
        return out
    }

    // Нужные наборы инструкций из строки Features в /proc/cpuinfo, в порядке SHOWN_FEATURES.
    fun features(cpuinfo: String?): String {
        val line = cpuinfo?.lineSequence()?.firstOrNull { it.trimStart().startsWith("Features") } ?: return "?"
        val present = line.substringAfter(':').trim().split(Regex("\\s+")).toSet()
        val shown = SHOWN_FEATURES.filter { it in present }
        return if (shown.isEmpty()) "базовые" else shown.joinToString(" ")
    }

    private fun readText(file: File): String? = try {
        file.readText()
    } catch (e: Exception) {
        null
    }
}
