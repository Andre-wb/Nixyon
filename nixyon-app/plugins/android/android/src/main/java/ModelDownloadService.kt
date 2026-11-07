package dev.demo.nixyon.speech

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

// Foreground service на время загрузки моделей: с ним Android не останавливает процесс,
// когда приложение свёрнуто. Саму загрузку ведёт ModelInstaller, сервис держит уведомление
// с ходом загрузки. Если процесс всё же убит, следующий запуск докачает недостающие куски.
class ModelDownloadService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val title = intent?.getStringExtra(EXTRA_TITLE).orEmpty()
        try {
            createChannel(title)
            val notification = notification(this, title, 0, 0)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
            } else {
                startForeground(NOTIFICATION_ID, notification)
            }
            // Загрузка могла кончиться раньше, чем сервис успел запуститься: тогда он
            // останавливается сразу после startForeground.
            if (gate.enteredForeground()) stopSelf()
        } catch (e: Exception) {
            // Без сервиса загрузка идёт, пока приложение на экране, и докачивается потом.
            Log.e(TAG, "foreground service загрузки моделей не запустился", e)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    // Android 15 даёт службам dataSync 6 часов в сутки; по истечении служба должна
    // остановиться за несколько секунд. Загрузка продолжится при следующем запуске.
    override fun onDestroy() {
        gate.destroyed()
        super.onDestroy()
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        Log.w(TAG, "время службы загрузки моделей вышло")
        stopSelf()
    }

    private fun createChannel(name: String) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(CHANNEL_ID, name, NotificationManager.IMPORTANCE_LOW)
        getSystemService(NotificationManager::class.java)?.createNotificationChannel(channel)
    }

    companion object {
        private const val TAG = "NixyonModels"
        private const val CHANNEL_ID = "nixyon-models"
        private const val NOTIFICATION_ID = 2
        private const val EXTRA_TITLE = "title"
        // Уведомление обновляется не чаще раза в секунду: чаще система его отбрасывает.
        private const val UPDATE_MS = 1_000L

        @Volatile
        private var title = ""
        @Volatile
        private var lastUpdate = 0L
        private val gate = ForegroundStopGate()

        fun start(context: Context, title: String): Boolean {
            this.title = title
            gate.starting()
            val intent = Intent(context, ModelDownloadService::class.java).putExtra(EXTRA_TITLE, title)
            return try {
                ContextCompat.startForegroundService(context, intent)
                true
            } catch (e: Exception) {
                Log.e(TAG, "foreground service загрузки моделей не запускается", e)
                false
            }
        }

        fun update(context: Context, done: Long, total: Long) {
            val now = SystemClock.elapsedRealtime()
            if (now - lastUpdate < UPDATE_MS) return
            lastUpdate = now
            try {
                context.getSystemService(NotificationManager::class.java)
                    ?.notify(NOTIFICATION_ID, notification(context, title, done, total))
            } catch (e: SecurityException) {
                Log.w(TAG, "уведомление о загрузке не обновилось: ${e.message}")
            }
        }

        fun stop(context: Context) {
            if (gate.requestStop()) context.stopService(Intent(context, ModelDownloadService::class.java))
        }

        // Ход в процентах: целые числа, потому что байт больше, чем помещается в Int.
        private fun notification(context: Context, title: String, done: Long, total: Long): Notification {
            val percent = if (total > 0) (done * 100 / total).toInt().coerceIn(0, 100) else 0
            return NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(android.R.drawable.stat_sys_download)
                .setContentTitle(title)
                .setProgress(100, percent, total <= 0)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
                .setContentIntent(openApp(context))
                .build()
        }

        // Нажатие на уведомление открывает приложение там, где оно было.
        private fun openApp(context: Context): PendingIntent? {
            val launch = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return null
            launch.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            return PendingIntent.getActivity(context, 0, launch, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }
    }
}
