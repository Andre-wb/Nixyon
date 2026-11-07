package dev.demo.nixyon.speech

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicBoolean

// Foreground service на время записи: с ним Android не глушит микрофон, когда приложение
// свёрнуто или экран выключен. Сама запись идёт в AudioRecorder, сервис только держит
// уведомление «Идёт запись» с таймером. Тексты уведомления приходят из интерфейса.
class RecordingService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val title = intent?.getStringExtra(EXTRA_TITLE).orEmpty()
        val channel = intent?.getStringExtra(EXTRA_CHANNEL).orEmpty()
        try {
            createChannel(channel)
            val notification = NotificationCompat.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.nixyon_recording)
                .setContentTitle(title)
                .setUsesChronometer(true)
                .setWhen(System.currentTimeMillis())
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
                .setContentIntent(openApp())
                .build()
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
            } else {
                startForeground(NOTIFICATION_ID, notification)
            }
            active.set(true)
            // Запись могла остановиться раньше, чем сервис успел запуститься: тогда он
            // останавливается сразу после startForeground.
            if (gate.enteredForeground()) {
                active.set(false)
                stopSelf()
            }
        } catch (e: Exception) {
            // Без сервиса запись остановится при сворачивании, как раньше.
            Log.e(TAG, "foreground service записи не запустился", e)
            active.set(false)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        active.set(false)
        gate.destroyed()
        super.onDestroy()
    }

    private fun createChannel(name: String) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        // Тихий канал: уведомление о записи не должно звучать во время разговора.
        val channel = NotificationChannel(CHANNEL_ID, name, NotificationManager.IMPORTANCE_LOW)
        getSystemService(NotificationManager::class.java)?.createNotificationChannel(channel)
    }

    // Нажатие на уведомление открывает приложение там, где оно было.
    private fun openApp(): PendingIntent? {
        val launch = packageManager.getLaunchIntentForPackage(packageName) ?: return null
        launch.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
        return PendingIntent.getActivity(this, 0, launch, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    companion object {
        private const val TAG = "NixyonSpeech"
        private const val CHANNEL_ID = "nixyon-recording"
        private const val NOTIFICATION_ID = 1
        private const val EXTRA_TITLE = "title"
        private const val EXTRA_CHANNEL = "channel"

        // Сервис запущен и держит запись в foreground.
        val active = AtomicBoolean(false)
        private val gate = ForegroundStopGate()

        // Запуск при начале записи, пока приложение на экране: из фона Android 14+ не даёт
        // запустить foreground service для микрофона. false: запустить не удалось.
        fun start(context: Context, title: String, channel: String): Boolean {
            val intent = Intent(context, RecordingService::class.java)
                .putExtra(EXTRA_TITLE, title)
                .putExtra(EXTRA_CHANNEL, channel)
            gate.starting()
            return try {
                ContextCompat.startForegroundService(context, intent)
                true
            } catch (e: Exception) {
                Log.e(TAG, "foreground service записи не запускается", e)
                false
            }
        }

        fun stop(context: Context) {
            if (gate.requestStop()) context.stopService(Intent(context, RecordingService::class.java))
            active.set(false)
        }
    }
}
