package dev.demo.nixyon.speech

import android.content.Context
import java.io.File

// Пути загрузки внутри getExternalFilesDir(null). Файл качается под именем с .part и
// получает своё имя только после проверки суммы: недокачанный файл никогда не выглядит
// готовым.
class ModelFolders(val base: File) {
    fun dir(target: ModelTarget): File = File(base, target.dirName)

    fun manifest(target: ModelTarget): File = File(dir(target), target.manifestName)

    fun file(target: ModelTarget, file: ModelFile): File = File(dir(target), file.path)

    fun part(target: ModelTarget, file: ModelFile): File = File(dir(target), file.path + PART)

    // Путь для DownloadManager относительно getExternalFilesDir(null).
    fun partSubPath(target: ModelTarget, file: ModelFile): String = "${target.dirName}/${file.path}$PART"

    companion object {
        private const val PART = ".part"

        fun of(context: Context): ModelFolders =
            context.getExternalFilesDir(null)?.let { ModelFolders(it) }
                ?: throw ModelsException(ModelsException.DOWNLOAD_FAILED, "нет папки приложения для моделей")
    }
}
