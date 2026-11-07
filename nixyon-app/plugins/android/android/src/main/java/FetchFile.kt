package dev.demo.nixyon.speech

import java.io.File

// Файл загрузки: адрес, временный файл, полный размер и номера уже скачанных кусков.
class FetchFile(val url: String, val part: File, val size: Long, val done: Set<Int>)
