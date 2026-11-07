package dev.demo.nixyon.speech

import java.io.File
import java.io.IOException

// Запись текстового файла через временный и переименование: читатель видит либо старый
// файл целиком, либо новый.
object AtomicFile {
    fun write(file: File, text: String) {
        file.parentFile?.mkdirs()
        val tmp = File(file.parentFile, ".${file.name}.tmp")
        tmp.writeText(text)
        if (!tmp.renameTo(file)) {
            tmp.delete()
            throw IOException("${file.name} не записался")
        }
    }
}
