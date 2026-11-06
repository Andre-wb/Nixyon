package dev.demo.nixyon.speech

import android.content.res.AssetManager
import java.io.File
import java.io.FileNotFoundException

// Откуда грузятся модели. sherpa-onnx умеет оба способа: путь к файлу
// (assetManager = null) или путь внутри assets APK.
sealed class ModelSource {
    abstract val label: String
    abstract val assetManager: AssetManager?
    abstract fun pathOf(file: ModelFile): String

    class Folder(val dir: File) : ModelSource() {
        override val label = "folder"
        override val assetManager: AssetManager? = null
        override fun pathOf(file: ModelFile): String = File(dir, file.path).absolutePath
    }

    class Apk(private val assets: AssetManager) : ModelSource() {
        override val label = "apk"
        override val assetManager: AssetManager = assets
        override fun pathOf(file: ModelFile): String = "$ASSETS_DIR/${file.path}"

        companion object {
            const val ASSETS_DIR = "models"
        }
    }
}

// Текстовый файл из assets APK или null, если его там нет.
fun AssetManager.readTextOrNull(path: String): String? =
    try {
        open(path).bufferedReader().use { it.readText() }
    } catch (_: FileNotFoundException) {
        null
    }
