package dev.demo.nixyon.speech

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaPlayer
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log

// Событие плеера для интерфейса: вид и позиция в секундах.
class PlayerEvent(val type: String, val sec: Double)

// Проигрывание исходной записи расшифровки. Все вызовы из главного потока: там же
// MediaPlayer шлёт свои колбэки и идёт таймер позиции.
class AudioPlayer(context: Context, private val keys: StorageKey) {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val handler = Handler(Looper.getMainLooper())
    private var player: MediaPlayer? = null
    private var onEvent: ((PlayerEvent) -> Unit)? = null
    private var focusRequest: AudioFocusRequest? = null

    private val attributes = AudioAttributes.Builder()
        .setUsage(AudioAttributes.USAGE_MEDIA)
        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
        .build()

    // Звонок или другое приложение со звуком забирает фокус: проигрывание встаёт на паузу.
    private val focusListener = AudioManager.OnAudioFocusChangeListener { change ->
        if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) pause()
    }

    private val tick = object : Runnable {
        override fun run() {
            val p = player ?: return
            if (!p.isPlaying) return
            send("position", p)
            handler.postDelayed(this, TICK_MS)
        }
    }

    val isOpen: Boolean
        get() = player != null

    // Открывает файл и возвращает длительность в секундах. Прежний файл закрывается.
    fun open(path: String, onEvent: (PlayerEvent) -> Unit): Double {
        close()
        val p = MediaPlayer()
        try {
            p.setAudioAttributes(attributes)
            val file = java.io.File(path)
            // Записи прежних версий могли остаться открытыми: их ядро шифрует при запуске,
            // но открытый файл всё равно играется как раньше.
            if (SealedFormat.isSealed(file)) p.setDataSource(SealedMediaSource(file, keys.get())) else p.setDataSource(path)
            p.prepare()
        } catch (e: Throwable) {
            p.release()
            throw MediaException(MediaException.PLAYBACK_FAILED, "запись не открылась", e)
        }
        p.setOnCompletionListener {
            handler.removeCallbacks(tick)
            abandonFocus()
            this.onEvent?.invoke(PlayerEvent("ended", it.duration / 1000.0))
        }
        p.setOnErrorListener { _, what, extra ->
            Log.e(TAG, "ошибка проигрывания: $what, $extra")
            this.onEvent?.invoke(PlayerEvent("error", 0.0))
            close()
            true
        }
        player = p
        this.onEvent = onEvent
        return p.duration / 1000.0
    }

    // Играет с позиции `fromSec` или с текущей, если она не задана.
    fun play(fromSec: Double?) {
        val p = player ?: throw MediaException(MediaException.NOT_PLAYING, "запись не открыта")
        if (!requestFocus()) throw MediaException(MediaException.PLAYBACK_FAILED, "звук занят другим приложением")
        if (fromSec != null) {
            val ms = (fromSec.coerceIn(0.0, p.duration / 1000.0) * 1000).toLong()
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                p.seekTo(ms, MediaPlayer.SEEK_CLOSEST)
            } else {
                p.seekTo(ms.toInt())
            }
        }
        p.start()
        send("playing", p)
        handler.removeCallbacks(tick)
        handler.postDelayed(tick, TICK_MS)
    }

    fun pause() {
        val p = player ?: return
        if (p.isPlaying) p.pause()
        handler.removeCallbacks(tick)
        abandonFocus()
        send("paused", p)
    }

    fun close() {
        handler.removeCallbacks(tick)
        abandonFocus()
        player?.release()
        player = null
        onEvent = null
    }

    private fun send(type: String, p: MediaPlayer) {
        onEvent?.invoke(PlayerEvent(type, p.currentPosition / 1000.0))
    }

    private fun requestFocus(): Boolean {
        val manager = audio ?: return true
        val granted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(attributes)
                .setOnAudioFocusChangeListener(focusListener, handler)
                .build()
            focusRequest = request
            manager.requestAudioFocus(request)
        } else {
            @Suppress("DEPRECATION")
            manager.requestAudioFocus(focusListener, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN)
        }
        return granted == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
    }

    private fun abandonFocus() {
        val manager = audio ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            focusRequest?.let { manager.abandonAudioFocusRequest(it) }
            focusRequest = null
        } else {
            @Suppress("DEPRECATION")
            manager.abandonAudioFocus(focusListener)
        }
    }

    companion object {
        private const val TAG = "NixyonPlayer"
        // Позиция 10 раз в секунду: подсветка слова успевает за речью.
        private const val TICK_MS = 100L
    }
}
