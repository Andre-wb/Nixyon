package dev.demo.nixyon.speech

import app.tauri.annotation.InvokeArg
import app.tauri.plugin.Channel

// Аргументы команд SpeechPlugin: Tauri разбирает в них JSON из Rust по именам полей.

@InvokeArg
class AnalyzeArgs {
    lateinit var wavPath: String
    lateinit var outPath: String
    // Окна для эмбеддингов голоса в секундах; 0 и 0: окна не нужны.
    var windowSec: Double = 0.0
    var stepSec: Double = 0.0
    // Нейросегментация: файл для отрезков и длина блока в секундах; 0: не нужна.
    var spansPath: String? = null
    var neuralBlockSec: Double = 0.0
    // Горячие слова через «/»; пусто: без них.
    var hotwords: String = ""
    var onProgress: Channel? = null
}

@InvokeArg
class SampleArgs {
    lateinit var wavPath: String
    var minSpeechSec: Double = 0.0
}

@InvokeArg
class AgePieceArgs {
    var startSec: Double = 0.0
    var endSec: Double = 0.0
}

@InvokeArg
class AgeSpeakerArgs {
    var id: Int = 0
    var pieces: Array<AgePieceArgs> = arrayOf()
}

@InvokeArg
class SpeakerAgesArgs {
    lateinit var wavPath: String
    var speakers: Array<AgeSpeakerArgs> = arrayOf()
}

@InvokeArg
class SystemBarsArgs {
    var dark: Boolean = false
}

@InvokeArg
class RecordArgs {
    var onEvent: Channel? = null
    // Тексты уведомления о записи на языке интерфейса.
    var notificationTitle: String = ""
    var notificationChannel: String = ""
    // Образец голоса: пишется отдельно от записей встреч.
    var sample: Boolean = false
}

@InvokeArg
class FileArgs {
    lateinit var path: String
    lateinit var name: String
    lateinit var mime: String
}

@InvokeArg
class PdfArgs {
    lateinit var path: String
    lateinit var blocks: String
}

@InvokeArg
class ImportArgs {
    var onProgress: Channel? = null
}

@InvokeArg
class PlayerOpenArgs {
    lateinit var path: String
    var onEvent: Channel? = null
}

@InvokeArg
class SummaryArgs {
    lateinit var system: String
    lateinit var user: String
    var maxTokens: Int = 0
    var onProgress: Channel? = null
}

// Участник и реплика последней расшифровки для подсказки имён. name: имя, уже данное
// пользователем; null, если участник подписан «Спикер N».
@InvokeArg
class NamingSpeakerArgs {
    var id: Int = 0
    var name: String? = null
}

@InvokeArg
class NamingTurnArgs {
    var id: Int = 0
    var speaker: Int = 0
    var text: String = ""
}

@InvokeArg
class SpeakerNamesArgs {
    var speakers: Array<NamingSpeakerArgs> = arrayOf()
    var turns: Array<NamingTurnArgs> = arrayOf()
    var onProgress: Channel? = null
}

@InvokeArg
class PlayArgs {
    // Позиция в секундах; null: с текущего места.
    var fromSec: Double? = null
}

@InvokeArg
class DownloadModelsArgs {
    // Ключи ModelPack; обязательный набор речи входит всегда.
    var packs: Array<String> = arrayOf()
    // Разрешена ли платная сеть (мобильная, точка доступа).
    var allowMetered: Boolean = false
    // Заголовок уведомления о загрузке на языке интерфейса.
    var notificationTitle: String = ""
    var onProgress: Channel? = null
}
