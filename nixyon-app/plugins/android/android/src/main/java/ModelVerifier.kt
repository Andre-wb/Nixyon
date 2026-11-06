package dev.demo.nixyon.speech

import java.io.File

// Размеры сверяются при каждом запуске: это дёшево и ловит недокачанный файл.
// SHA-256 всего набора (сотни мегабайт) считается, только когда набор изменился:
// результат запоминается ключом из версии, размеров и времени изменения файлов.
class ModelVerifier(private val markerFile: File) {

    fun verify(dir: File, manifest: ModelManifest) {
        verify(dir, manifest.version, manifest.files.map { (role, file) -> role.key to file })
    }

    // files: имя роли (для текста ошибки) и файл.
    fun verify(dir: File, version: String, files: List<Pair<String, ModelFile>>) {
        verifySizes(dir, files)
        val key = stateKey(dir, version, files.map { it.second })
        if (readMarker() == key) return
        verifyHashes(dir, files.map { it.second })
        markerFile.parentFile?.mkdirs()
        markerFile.writeText(key)
    }

    private fun verifySizes(dir: File, files: List<Pair<String, ModelFile>>) {
        for ((role, file) in files) {
            val onDisk = File(dir, file.path)
            if (!onDisk.isFile) {
                throw ModelsException(ModelsException.INCOMPLETE, "нет файла ${file.path} ($role)")
            }
            if (onDisk.length() != file.size) {
                throw ModelsException(
                    ModelsException.INCOMPLETE,
                    "размер ${file.path}: ${onDisk.length()}, ожидался ${file.size}",
                )
            }
        }
    }

    private fun verifyHashes(dir: File, files: List<ModelFile>) {
        for (file in files) {
            val actual = Sha256.hex(File(dir, file.path))
            if (actual != file.sha256) {
                throw ModelsException(ModelsException.CORRUPT, "SHA-256 ${file.path} не совпала")
            }
        }
    }

    private fun stateKey(dir: File, version: String, files: List<ModelFile>): String =
        buildString {
            append(version)
            for (file in files.sortedBy { it.path }) {
                val onDisk = File(dir, file.path)
                append('|').append(file.path).append(':').append(file.sha256)
                append(':').append(onDisk.length()).append(':').append(onDisk.lastModified())
            }
        }

    private fun readMarker(): String? = if (markerFile.isFile) markerFile.readText() else null
}
