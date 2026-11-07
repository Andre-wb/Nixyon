package dev.demo.nixyon.speech

import android.content.Context
import java.io.File
import java.io.IOException
import java.util.zip.ZipException
import java.util.zip.ZipFile

// Видеокарта (Vulkan) для языковой модели. По умолчанию выключена: быстрее ли она
// процессора на телефоне, ещё не мерилось, а сбой драйвера видеокарты закрывает приложение,
// и перехватить его нельзя. Включается в настройках и только в APK с libggml-vulkan.so
// (свойство Gradle nixyonLlmVulkan). Видеокарта телефона берёт память у той же, что у
// процессора, а на неё уходят все веса, поэтому она берётся только при быстрой загрузке
// (LlmLoad.FAST). Нет видеокарты или модель на ней не загрузилась: модель на процессоре,
// как без настройки.
class LlmGpu(private val marker: File, built: () -> Boolean) {
    // APK не меняется, пока приложение запущено.
    val isBuilt: Boolean by lazy(built)

    // Включена ли в настройках: есть файл-признак.
    val enabled: Boolean
        get() = marker.isFile

    // На чём считала модель при последней загрузке: «CPU» или «Vulkan: …»; null: после
    // запуска приложения модель ещё не грузилась.
    @Volatile
    var lastDevice: String? = null

    fun setEnabled(on: Boolean) {
        if (on) {
            AtomicFile.write(marker, "on")
        } else if (marker.exists() && !marker.delete()) {
            throw IOException("${marker.name} не удалился")
        }
    }

    fun choose(load: LlmLoad): GpuChoice = choose(enabled, isBuilt, load)

    companion object {
        const val MARKER = "llm-gpu"
        const val LIBRARY = "libggml-vulkan.so"
        const val NOT_BUILT_CODE = "llm_gpu_not_built"

        fun of(context: Context): LlmGpu {
            val info = context.applicationInfo
            val apks = listOfNotNull(info.sourceDir) + info.splitSourceDirs.orEmpty()
            return LlmGpu(File(context.noBackupFilesDir, MARKER)) { hasLibrary(info.nativeLibraryDir?.let(::File), apks.map(::File)) }
        }

        fun choose(enabled: Boolean, built: Boolean, load: LlmLoad): GpuChoice = when {
            !enabled -> GpuChoice.OFF
            !built -> GpuChoice.NOT_BUILT
            load != LlmLoad.FAST -> GpuChoice.LOW_MEMORY
            else -> GpuChoice.USE
        }

        // Библиотеки лежат несжатыми внутри APK (их тогда нет в nativeLibraryDir) или
        // распакованы в nativeLibraryDir.
        fun hasLibrary(nativeLibraryDir: File?, apks: List<File>): Boolean {
            if (nativeLibraryDir != null && File(nativeLibraryDir, LIBRARY).isFile) return true
            return apks.any { apk -> apkHasLibrary(apk) }
        }

        private fun apkHasLibrary(apk: File): Boolean = try {
            ZipFile(apk).use { zip -> zip.entries().asSequence().any { it.name.startsWith("lib/") && it.name.endsWith("/$LIBRARY") } }
        } catch (_: ZipException) {
            false
        } catch (_: IOException) {
            false
        }
    }
}

// Брать ли видеокарту на эту загрузку и почему нет (для лога).
enum class GpuChoice(val wanted: Boolean, val reason: String) {
    USE(true, "видеокарта включена"),
    OFF(false, "видеокарта выключена в настройках"),
    NOT_BUILT(false, "APK собран без видеокарты"),
    LOW_MEMORY(false, "на видеокарту не хватает памяти, модель на процессоре из файла"),
}
