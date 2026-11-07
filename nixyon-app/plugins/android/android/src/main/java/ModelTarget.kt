package dev.demo.nixyon.speech

// Папка внутри getExternalFilesDir(null), куда ложатся файлы, и имя описания набора в ней.
// Те же имена ищут ModelLocator и LlmLocator.
enum class ModelTarget(val dirName: String, val manifestName: String) {
    SPEECH("models", "models.json"),
    LLM("llm", "llm.json"),
}
