package dev.demo.nixyon.speech

import android.app.ActivityManager
import android.content.Context

// Память устройства по оценке системы: сколько ещё можно занять и считает ли система,
// что память уже на исходе.
class MemorySnapshot(val availableBytes: Long, val lowMemory: Boolean)

object DeviceMemory {
    fun read(context: Context): MemorySnapshot {
        val manager = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val info = ActivityManager.MemoryInfo()
        manager.getMemoryInfo(info)
        return MemorySnapshot(info.availMem, info.lowMemory)
    }

    // Память перед загрузкой большой модели. Если её не хватает (enough), free отпускает
    // что может (модели речи) и память смотрится заново. Второе значение: отпущено ли
    // что-то. Хватает ли в итоге, решает вызывающий по тому же enough.
    fun check(
        read: () -> MemorySnapshot,
        enough: (MemorySnapshot) -> Boolean,
        free: () -> Boolean,
    ): Pair<MemorySnapshot, Boolean> {
        val first = read()
        if (enough(first) || !free()) return first to false
        return read() to true
    }
}
