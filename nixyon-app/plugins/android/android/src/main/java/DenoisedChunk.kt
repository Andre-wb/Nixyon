package dev.demo.nixyon.speech

// Ответ шумоподавителя на кусок звука. На куске короче своего окна GTCRN ничего не отдаёт,
// и частота у пустого ответа 0: такой ответ пропускается, а сверяется частота только у
// звука, который пришёл.
object DenoisedChunk {
    // true: звук есть, его слушает VAD; false: пустой ответ.
    fun usable(size: Int, rate: Int): Boolean {
        if (size == 0) return false
        if (rate != SpeechModels.SAMPLE_RATE) {
            throw AnalysisException(AnalysisException.ENGINE_FAILED, "шумоподавитель отдал $rate Гц вместо ${SpeechModels.SAMPLE_RATE}")
        }
        return true
    }
}
