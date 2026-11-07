package dev.demo.nixyon.speech

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaPlayer
import android.media.PlaybackParams
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log

// Событие плеера для интерфейса: вид и позиция в секундах.
class PlayerEvent(val type: String, val sec: Double)

// Отрезок записи в секундах, который играется; всё между отрезками пропускается.
class PlaybackSpan(val start: Double, val end: Double)

// Проигрывание исходной записи расшифровки. Все вызовы из главного потока: там же
// MediaPlayer шлёт свои колбэки, идёт таймер позиции и приходят команды из MediaSession.
// Проигрывание ведёт сеанс (экран блокировки, гарнитура) и foreground service с
// уведомлением, поэтому свёрнутое приложение его не останавливает.
class AudioPlayer(private val context: Context, private val keys: StorageKey) : PlaybackSession.Controls {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val handler = Handler(Looper.getMainLooper())
    private var player: MediaPlayer? = null
    private var onEvent: ((PlayerEvent) -> Unit)? = null
    private var focusRequest: AudioFocusRequest? = null
    private var session: PlaybackSession? = null
    private var title = ""
    // Скорость и отрезки: интерфейс задаёт их при открытии и при каждом изменении. null: вся запись.
    private var speed = 1f
    private var ranges: List<PlaybackSpan>? = null

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
            if (!inRanges(p)) {
                finish(p, "ended")
                return
            }
            send("position", positionSec(p))
            handler.postDelayed(this, TICK_MS)
        }
    }

    val isOpen: Boolean
        get() = player != null

    // Открывает файл и возвращает длительность в секундах. Прежний файл закрывается.
    // title: название для экрана блокировки и уведомления.
    fun open(path: String, title: String, onEvent: (PlayerEvent) -> Unit): Double {
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
            publish(false)
            send("ended", it.duration / 1000.0)
        }
        p.setOnErrorListener { _, what, extra ->
            Log.e(TAG, "ошибка проигрывания: $what, $extra")
            this.onEvent?.invoke(PlayerEvent("error", 0.0))
            close()
            true
        }
        player = p
        this.onEvent = onEvent
        this.title = title
        val seconds = p.duration / 1000.0
        session = PlaybackSession(context, this).also { it.setTitle(title, seconds) }
        session?.update(false, 0.0, speed)
        return seconds
    }

    // Играет с позиции `fromSec` или с текущей, если она не задана. Если позиция вне отрезков,
    // играется от начала следующего; если отрезки кончились, с начала первого.
    fun play(fromSec: Double?) {
        val p = player ?: throw MediaException(MediaException.NOT_PLAYING, "запись не открыта")
        if (!requestFocus()) throw MediaException(MediaException.PLAYBACK_FAILED, "звук занят другим приложением")
        if (fromSec != null) seekTo(p, fromSec)
        if (!inRanges(p)) {
            val first = ranges?.firstOrNull() ?: return finish(p, "ended")
            seekTo(p, first.start)
        }
        p.start()
        applySpeed(p)
        publish(true)
        send("playing", positionSec(p))
        handler.removeCallbacks(tick)
        handler.postDelayed(tick, TICK_MS)
    }

    override fun play() {
        try {
            play(null)
        } catch (e: MediaException) {
            Log.w(TAG, "управление с экрана не сработало: ${e.code}")
        }
    }

    override fun pause() {
        val p = player ?: return
        if (p.isPlaying) p.pause()
        handler.removeCallbacks(tick)
        abandonFocus()
        publish(false)
        send("paused", positionSec(p))
    }

    // Перемотка от текущего места; при отрицательном значении назад.
    override fun skip(deltaSec: Double) {
        val p = player ?: return
        seek(positionSec(p) + deltaSec)
    }

    override fun seek(sec: Double) {
        val p = player ?: return
        val target = seekTo(p, sec)
        if (!p.isPlaying) {
            send("paused", target)
            return
        }
        if (!inRanges(p)) {
            finish(p, "ended")
            return
        }
        send("position", target)
    }

    // Скорость 0,75–2: тон голоса не меняется.
    fun setSpeed(value: Double) {
        speed = value.toFloat()
        player?.let { applySpeed(it) }
    }

    // null: играется вся запись. Иначе только эти отрезки подряд.
    fun setRanges(list: List<PlaybackSpan>?) {
        ranges = list?.sortedBy { it.start }
    }

    fun close() {
        handler.removeCallbacks(tick)
        abandonFocus()
        player?.release()
        player = null
        onEvent = null
        ranges = null
        session?.release()
        session = null
        PlaybackService.stop(context)
    }

    // Проигрывание кончилось или отрезки сыграны: пауза, фокус отдаётся, интерфейс узнаёт.
    private fun finish(p: MediaPlayer, type: String) {
        if (p.isPlaying) p.pause()
        handler.removeCallbacks(tick)
        abandonFocus()
        publish(false)
        send(type, positionSec(p))
    }

    // Если позиция вне отрезков, перематывает к началу следующего. false: отрезков не осталось.
    private fun inRanges(p: MediaPlayer): Boolean {
        val pos = positionSec(p)
        val target = PlaybackSpans.target(ranges, pos) ?: return false
        if (target > pos) seekTo(p, target)
        return true
    }

    // Перематывает и возвращает позицию, до которой перемотали (в пределах записи).
    private fun seekTo(p: MediaPlayer, sec: Double): Double {
        val target = sec.coerceIn(0.0, p.duration / 1000.0)
        val ms = (target * 1000).toLong()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            p.seekTo(ms, MediaPlayer.SEEK_CLOSEST)
        } else {
            p.seekTo(ms.toInt())
        }
        return target
    }

    // PlaybackParams держит тон голоса на месте; смена скорости не должна запускать звук.
    private fun applySpeed(p: MediaPlayer) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return
        val playing = p.isPlaying
        try {
            p.playbackParams = PlaybackParams().setSpeed(speed).setPitch(1f)
        } catch (e: IllegalStateException) {
            Log.w(TAG, "скорость не применилась", e)
            return
        }
        if (!playing && p.isPlaying) p.pause()
    }

    private fun positionSec(p: MediaPlayer): Double = p.currentPosition / 1000.0

    private fun send(type: String, sec: Double) {
        val playing = type == "playing" || type == "position"
        session?.update(playing, sec, speed)
        onEvent?.invoke(PlayerEvent(type, sec))
    }

    // Уведомление с кнопками: пока проигрывание идёт или на паузе, сервис его держит.
    private fun publish(playing: Boolean) {
        val s = session ?: return
        PlaybackService.show(context, s.token, title, playing)
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
