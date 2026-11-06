package dev.demo.nixyon.speech

import java.io.File

// Записи образцов голоса: отдельно от записей встреч, чтобы образец не стирал запись
// последней расшифровки. Образец живёт до «Сохранить голос» или отказа от него; хранится
// только последний, остатки ядро удаляет при запуске.
class SamplesDir(filesDir: File) : RecordingTarget(File(filesDir, "samples"))
