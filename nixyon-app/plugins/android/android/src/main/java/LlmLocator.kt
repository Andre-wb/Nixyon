package dev.demo.nixyon.speech

import android.content.Context
import android.os.ParcelFileDescriptor
import java.io.Closeable
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException

// Открытая модель: дескриптор файла и смещение, с которого в нём начинается GGUF.
class LocatedLlm(val size: Long, val offset: Long, private val descriptor: ParcelFileDescriptor) : Closeable {
    val fd: Int get() = descriptor.fd

    override fun close() = descriptor.close()
}

// Где лежит языковая модель. Если в папке приложения есть llm/llm.json (кладёт
// push-llm.sh), модель берётся оттуда и проверяется по размеру и SHA-256; иначе из APK
// (демо-сборка), где она лежит несжатой и читается прямо из файла APK, без копии.
class LlmLocator(private val context: Context) {

    fun isAvailable(): Boolean = folderManifest()?.isFile == true || readApkManifest() != null

    fun locate(): LocatedLlm {
        val folderManifest = folderManifest()
        if (folderManifest != null && folderManifest.isFile) return fromFolder(folderManifest)
        val text = readApkManifest() ?: throw LlmException(LlmException.NOT_FOUND, "нет llm.json")
        return fromApk(LlmManifest.parse(text))
    }

    private fun fromFolder(manifestFile: File): LocatedLlm {
        val dir = manifestFile.parentFile ?: throw LlmException(LlmException.NOT_FOUND, "нет папки llm")
        val manifest = LlmManifest.parse(manifestFile.readText())
        try {
            ModelVerifier(File(context.noBackupFilesDir, VERIFIED_MARKER))
                .verify(dir, manifest.version, listOf("llm" to manifest.model))
        } catch (e: ModelsException) {
            throw LlmException(LlmException.BROKEN, e.message ?: e.code, e)
        }
        val file = File(dir, manifest.model.path)
        val descriptor = try {
            ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
        } catch (e: FileNotFoundException) {
            throw LlmException(LlmException.NOT_FOUND, "файл модели не открылся", e)
        }
        return LocatedLlm(manifest.model.size, 0L, descriptor)
    }

    // APK подписан, поэтому SHA-256 модели при каждом запуске не считается, как и у моделей
    // распознавания в APK. openFd работает только с несжатым файлом.
    private fun fromApk(manifest: LlmManifest): LocatedLlm {
        val asset = try {
            context.assets.openFd("$FOLDER/${manifest.model.path}")
        } catch (e: IOException) {
            throw LlmException(LlmException.BROKEN, "модель в APK не открылась: ${e.message}", e)
        }
        if (asset.length != manifest.model.size) {
            asset.close()
            throw LlmException(LlmException.BROKEN, "размер модели в APK ${asset.length}, ожидался ${manifest.model.size}")
        }
        return LocatedLlm(asset.length, asset.startOffset, asset.parcelFileDescriptor)
    }

    private fun folderManifest(): File? =
        context.getExternalFilesDir(null)?.let { File(File(it, FOLDER), MANIFEST) }

    private fun readApkManifest(): String? = context.assets.readTextOrNull("$FOLDER/$MANIFEST")

    companion object {
        private const val FOLDER = "llm"
        private const val MANIFEST = "llm.json"
        private const val VERIFIED_MARKER = "llm-verified.txt"
    }
}
