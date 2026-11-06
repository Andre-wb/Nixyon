package dev.demo.nixyon.speech

import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedWriter
import java.io.Closeable
import java.io.File

// Массив JSON в файл по одному элементу: для часа записи это сотни кусков с эмбеддингами,
// собирать их строкой в памяти и гнать через мост Tauri незачем. finish закрывает массив;
// без него файл не дописан.
class JsonArrayWriter(file: File) : Closeable {
    private val out: BufferedWriter = file.bufferedWriter()
    var count = 0
        private set

    init {
        out.write("[")
    }

    fun add(item: JSONObject) {
        if (count > 0) out.write(",")
        out.write(item.toString())
        count += 1
    }

    fun finish() {
        out.write("]")
        out.flush()
    }

    override fun close() = out.close()
}

// Эмбеддинг или времена токенов числами JSON.
fun FloatArray.toJsonArray(): JSONArray = JSONArray(map { it.toDouble() })
