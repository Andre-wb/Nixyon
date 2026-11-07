package dev.demo.nixyon.speech

import android.content.Context
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Handler
import android.os.Looper

// Сеанс проигрывания для системы: экран блокировки, шторка и гарнитура управляют записью
// через MediaSession. Команды приходят на главном потоке и отдаются в Controls, то есть в
// AudioPlayer, тем же путём, что и кнопки в приложении.
class PlaybackSession(context: Context, private val controls: Controls) {
    interface Controls {
        fun play()
        fun pause()
        fun skip(deltaSec: Double)
        fun seek(sec: Double)
    }

    private val session = MediaSession(context, TAG).apply {
        setCallback(
            object : MediaSession.Callback() {
                override fun onPlay() = controls.play()

                override fun onPause() = controls.pause()

                override fun onStop() = controls.pause()

                override fun onRewind() = controls.skip(-SKIP_SEC)

                override fun onFastForward() = controls.skip(SKIP_SEC)

                override fun onSeekTo(pos: Long) = controls.seek(pos / 1000.0)
            },
            Handler(Looper.getMainLooper()),
        )
        isActive = true
    }

    val token: MediaSession.Token
        get() = session.sessionToken

    fun setTitle(title: String, durationSec: Double) {
        session.setMetadata(
            MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, title)
                .putLong(MediaMetadata.METADATA_KEY_DURATION, (durationSec * 1000).toLong())
                .build(),
        )
    }

    fun update(playing: Boolean, sec: Double, speed: Float) {
        val state = if (playing) PlaybackState.STATE_PLAYING else PlaybackState.STATE_PAUSED
        val actions = PlaybackState.ACTION_PLAY or
            PlaybackState.ACTION_PAUSE or
            PlaybackState.ACTION_PLAY_PAUSE or
            PlaybackState.ACTION_STOP or
            PlaybackState.ACTION_REWIND or
            PlaybackState.ACTION_FAST_FORWARD or
            PlaybackState.ACTION_SEEK_TO
        session.setPlaybackState(
            PlaybackState.Builder()
                .setActions(actions)
                .setState(state, (sec * 1000).toLong(), if (playing) speed else 0f)
                .build(),
        )
    }

    fun release() {
        session.isActive = false
        session.release()
    }

    private companion object {
        const val TAG = "NixyonPlayback"
        const val SKIP_SEC = 10.0
    }
}
