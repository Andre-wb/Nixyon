package dev.demo.nixyon.speech

import java.util.concurrent.atomic.AtomicBoolean

// Загруженные модели речи, которые можно отпустить, когда другой модели (языковой или
// audEERING) не хватает памяти. Живой текст записи держит их (hold) до конца своего потока:
// пока кто-то держит, unload ничего не отпускает, иначе распознавание пошло бы по
// освобождённой памяти. Разбор записи их не держит: движок не зовёт unload, пока идёт
// разбор (SpeechEngine.exclusive).
class ModelSlot<T : Any>(private val release: (T) -> Unit) {
    private val lock = Any()
    private var value: T? = null
    private var holders = 0

    val current: T?
        get() = synchronized(lock) { value }

    fun put(loaded: T) {
        synchronized(lock) { value = loaded }
    }

    // Модели, пока не вызван done; null, если они не загружены.
    fun hold(): Hold<T>? {
        synchronized(lock) {
            val loaded = value ?: return null
            holders++
            return Hold(loaded) { synchronized(lock) { holders-- } }
        }
    }

    // Отпускает модели, если они загружены и их никто не держит. true, если отпустил.
    fun unload(): Boolean {
        val loaded = synchronized(lock) {
            if (holders > 0) return false
            value.also { value = null }
        } ?: return false
        release(loaded)
        return true
    }

    // Повторный done ничего не делает.
    class Hold<T>(val models: T, private val onDone: () -> Unit) {
        private val finished = AtomicBoolean(false)

        fun done() {
            if (finished.compareAndSet(false, true)) onDone()
        }
    }
}
