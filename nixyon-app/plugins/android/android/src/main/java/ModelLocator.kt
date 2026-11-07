package dev.demo.nixyon.speech

import android.content.Context
import java.io.File

class LocatedModels(val source: ModelSource, val manifest: ModelManifest)

// Если в папке приложения есть models.json, модели берутся оттуда и должны
// пройти проверку: тихо переключаться на APK нельзя, иначе после замены моделей
// незаметно загрузится старый набор. Без models.json в папке берутся модели из APK.
// Модели, выключенные в настройках, из набора убираются уже после проверки файлов.
class ModelLocator(private val context: Context) {

    fun locate(): LocatedModels {
        val dir = folderDir()
        val folderManifest = dir?.let { File(it, MANIFEST) }
        if (dir != null && folderManifest != null && folderManifest.isFile) {
            val manifest = ModelManifest.parse(folderManifest.readText())
            ModelVerifier(File(context.noBackupFilesDir, VERIFIED_MARKER)).verify(dir, manifest)
            return LocatedModels(ModelSource.Folder(dir), DisabledModels.of(context).apply(manifest))
        }
        val apkManifest = readApkManifest()
            ?: throw ModelsException(
                ModelsException.NOT_FOUND,
                "нет ${dir?.absolutePath ?: "папки моделей"}/$MANIFEST и нет моделей в APK",
            )
        return LocatedModels(ModelSource.Apk(context.assets), DisabledModels.of(context).apply(ModelManifest.parse(apkManifest)))
    }

    private fun folderDir(): File? = context.getExternalFilesDir(null)?.let { File(it, FOLDER) }

    private fun readApkManifest(): String? = context.assets.readTextOrNull("${ModelSource.Apk.ASSETS_DIR}/$MANIFEST")

    companion object {
        private const val FOLDER = "models"
        private const val MANIFEST = "models.json"
        private const val VERIFIED_MARKER = "models-verified.txt"
    }
}
