package dev.demo.nixyon.speech

import java.io.File

// Поиск перекрытий по модели сегментации: куда записать итог, порог вероятности перекрытия
// в кадре и наименьшая длина перекрытия в секундах.
class OverlapRequest(val out: File, val threshold: Float, val minSec: Double) {
    companion object {
        // Без файла поиск не нужен. Порог вне (0, 1) или нечисловая длина значат ошибку в
        // запросе от Rust-части.
        fun of(path: String?, threshold: Float, minSec: Double): OverlapRequest? {
            if (path == null) return null
            if (!threshold.isFinite() || threshold <= 0f || threshold >= 1f || !minSec.isFinite() || minSec < 0.0) {
                throw AnalysisException(AnalysisException.ENGINE_FAILED, "перекрытия: порог $threshold, длина $minSec с")
            }
            return OverlapRequest(File(path), threshold, minSec)
        }
    }
}
