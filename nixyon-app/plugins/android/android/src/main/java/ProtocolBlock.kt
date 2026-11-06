package dev.demo.nixyon.speech

import org.json.JSONArray
import org.json.JSONObject

// Блок протокола встречи в том виде, в каком его сериализует `nixyon-render`:
// поле `kind` выбирает вид блока. Содержимое уже посчитано в Rust, здесь только вёрстка.
sealed class ProtocolBlock {
    data class Title(val text: String) : ProtocolBlock()
    data class Fact(val label: String, val value: String) : ProtocolBlock()
    data class Table(val header: List<String>, val rows: List<List<String>>) : ProtocolBlock()
    data class Turn(val speaker: String, val time: String, val text: String) : ProtocolBlock()

    companion object {
        fun parseAll(json: String): List<ProtocolBlock> {
            val array = JSONArray(json)
            return (0 until array.length()).map { parse(array.getJSONObject(it)) }
        }

        private fun parse(o: JSONObject): ProtocolBlock = when (val kind = o.getString("kind")) {
            "title" -> Title(o.getString("text"))
            "fact" -> Fact(o.getString("label"), o.getString("value"))
            "table" -> Table(strings(o.getJSONArray("header")), rows(o.getJSONArray("rows")))
            "turn" -> Turn(o.getString("speaker"), o.getString("time"), o.getString("text"))
            else -> throw IllegalArgumentException("неизвестный блок протокола: $kind")
        }

        private fun strings(a: JSONArray): List<String> = (0 until a.length()).map { a.getString(it) }

        private fun rows(a: JSONArray): List<List<String>> = (0 until a.length()).map { strings(a.getJSONArray(it)) }
    }
}
