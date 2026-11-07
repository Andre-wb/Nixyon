package dev.demo.nixyon.speech

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.media.session.MediaController
import android.media.session.MediaSession
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicBoolean

// Foreground service на время проигрывания расшифровки: с ним Android не усыпляет приложение,
// когда его свернули или погасили экран. Уведомление показывает название и кнопки «назад
// на 10 с», «играть/пауза», «вперёд на 10 с». Они отдаются в MediaSession, то есть идут тем же
// путём, что команды с экрана блокировки и гарнитуры.
class PlaybackService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val shown = current
        intent?.action?.let { action -> if (shown != null) sendControl(shown, action) }
        try {
            ensureChannel(this)
            // Остановку начали раньше, чем сервис запустился: он всё равно сначала встаёт в
            // foreground, потом сразу уходит, иначе Android сочтёт это ошибкой.
            val notification = build(this, shown ?: Shown(null, "", false))
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
            } else {
                startForeground(NOTIFICATION_ID, notification)
            }
            active.set(true)
            if (shown == null) stopSelf()
        } catch (e: Exception) {
            Log.e(TAG, "foreground service проигрывания не запустился", e)
            active.set(false)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        active.set(false)
        super.onDestroy()
    }

    private fun sendControl(shown: Shown, action: String) {
        val token = shown.token ?: return
        val controls = MediaController(this, token).transportControls
        when (action) {
            ACTION_TOGGLE -> if (shown.playing) controls.pause() else controls.play()
            ACTION_REWIND -> controls.rewind()
            ACTION_FORWARD -> controls.fastForward()
        }
    }

    // Что показывает уведомление: сеанс для кнопок, название и играет ли сейчас.
    private class Shown(val token: MediaSession.Token?, val title: String, val playing: Boolean)

    companion object {
        private const val TAG = "NixyonPlayback"
        private const val CHANNEL_ID = "nixyon-playback"
        private const val NOTIFICATION_ID = NotificationIds.PLAYBACK
        private const val ACTION_TOGGLE = "dev.demo.nixyon.speech.PLAYBACK_TOGGLE"
        private const val ACTION_REWIND = "dev.demo.nixyon.speech.PLAYBACK_REWIND"
        private const val ACTION_FORWARD = "dev.demo.nixyon.speech.PLAYBACK_FORWARD"

        // Сервис запущен и держит уведомление в foreground.
        val active = AtomicBoolean(false)
        @Volatile private var current: Shown? = null

        // Показывает уведомление и держит сервис. Сервис запускается, когда звук пошёл
        // (PlaybackNotice), следующие вызовы только обновляют уведомление.
        fun show(context: Context, token: MediaSession.Token, title: String, playing: Boolean) {
            val shown = Shown(token, title, playing)
            current = shown
            try {
                when (PlaybackNotice.choose(active.get(), playing)) {
                    PlaybackNotice.UPDATE -> {
                        ensureChannel(context)
                        context.getSystemService(NotificationManager::class.java)
                            ?.notify(NOTIFICATION_ID, build(context, shown))
                    }
                    PlaybackNotice.START ->
                        ContextCompat.startForegroundService(context, Intent(context, PlaybackService::class.java))
                    PlaybackNotice.NONE -> Unit
                }
            } catch (e: Exception) {
                Log.w(TAG, "уведомление проигрывания не показалось", e)
            }
        }

        fun stop(context: Context) {
            current = null
            active.set(false)
            context.stopService(Intent(context, PlaybackService::class.java))
        }

        private fun ensureChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            // Тихий канал: переключение кнопок не должно звучать.
            val channel = NotificationChannel(
                CHANNEL_ID,
                context.getString(R.string.playback_channel),
                NotificationManager.IMPORTANCE_LOW,
            )
            context.getSystemService(NotificationManager::class.java)?.createNotificationChannel(channel)
        }

        private fun build(context: Context, shown: Shown): Notification {
            val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                Notification.Builder(context, CHANNEL_ID)
            } else {
                @Suppress("DEPRECATION")
                Notification.Builder(context)
            }
            val style = Notification.MediaStyle().setShowActionsInCompactView(0, 1, 2)
            shown.token?.let { style.setMediaSession(it) }
            val toggle = if (shown.playing) {
                action(context, android.R.drawable.ic_media_pause, R.string.playback_pause, ACTION_TOGGLE)
            } else {
                action(context, android.R.drawable.ic_media_play, R.string.playback_play, ACTION_TOGGLE)
            }
            return builder
                .setSmallIcon(R.drawable.nixyon_recording)
                .setContentTitle(shown.title)
                .setContentIntent(openApp(context))
                .setOngoing(true)
                .setVisibility(Notification.VISIBILITY_PUBLIC)
                .setShowWhen(false)
                .setStyle(style)
                .addAction(action(context, android.R.drawable.ic_media_rew, R.string.playback_back, ACTION_REWIND))
                .addAction(toggle)
                .addAction(action(context, android.R.drawable.ic_media_ff, R.string.playback_forward, ACTION_FORWARD))
                .build()
        }

        private fun action(context: Context, icon: Int, label: Int, name: String): Notification.Action {
            val intent = Intent(context, PlaybackService::class.java).setAction(name)
            val pending = PendingIntent.getService(
                context,
                name.hashCode(),
                intent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            return Notification.Action.Builder(
                Icon.createWithResource(context, icon),
                context.getString(label),
                pending,
            ).build()
        }

        // Нажатие на уведомление открывает приложение там, где оно было.
        private fun openApp(context: Context): PendingIntent? {
            val launch = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return null
            launch.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            return PendingIntent.getActivity(
                context,
                0,
                launch,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        }
    }
}
