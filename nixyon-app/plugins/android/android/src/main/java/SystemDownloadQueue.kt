package dev.demo.nixyon.speech

import android.app.DownloadManager
import android.content.Context
import android.net.Uri
import android.util.Log

// DownloadManager качает в отдельном системном процессе: загрузка идёт, когда приложение
// свёрнуто, и сама докачивает после обрыва связи. Ход загрузки виден в уведомлении.
class SystemDownloadQueue(private val context: Context) : DownloadQueue {
    private val manager: DownloadManager = context.getSystemService(DownloadManager::class.java)
        ?: throw ModelsException(ModelsException.DOWNLOAD_FAILED, "нет DownloadManager")

    override fun enqueue(request: DownloadRequest): Long {
        val r = DownloadManager.Request(Uri.parse(request.url))
            .setDestinationInExternalFilesDir(context, null, request.subPath)
            .setTitle(request.title)
            .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE)
            .setAllowedOverMetered(request.allowMetered)
        return manager.enqueue(r)
    }

    override fun query(ids: Collection<Long>): Map<Long, DownloadStatus> {
        if (ids.isEmpty()) return emptyMap()
        val out = HashMap<Long, DownloadStatus>()
        val query = DownloadManager.Query().setFilterById(*ids.toLongArray())
        manager.query(query)?.use { cursor ->
            val idColumn = cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_ID)
            val statusColumn = cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS)
            val reasonColumn = cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_REASON)
            val doneColumn = cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_BYTES_DOWNLOADED_SO_FAR)
            val totalColumn = cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_TOTAL_SIZE_BYTES)
            while (cursor.moveToNext()) {
                val id = cursor.getLong(idColumn)
                val reason = cursor.getInt(reasonColumn)
                val state = when (cursor.getInt(statusColumn)) {
                    DownloadManager.STATUS_SUCCESSFUL -> DownloadStatus.State.DONE
                    DownloadManager.STATUS_FAILED -> {
                        Log.w(TAG, "загрузка $id не удалась, причина $reason")
                        DownloadStatus.State.FAILED
                    }
                    DownloadManager.STATUS_PAUSED ->
                        if (reason == DownloadManager.PAUSED_WAITING_FOR_NETWORK || reason == DownloadManager.PAUSED_QUEUED_FOR_WIFI) {
                            DownloadStatus.State.WAITING
                        } else {
                            DownloadStatus.State.RUNNING
                        }
                    else -> DownloadStatus.State.RUNNING
                }
                out[id] = DownloadStatus(state, cursor.getLong(doneColumn), cursor.getLong(totalColumn))
            }
        }
        return out
    }

    override fun remove(ids: Collection<Long>) {
        if (ids.isNotEmpty()) manager.remove(*ids.toLongArray())
    }

    companion object {
        private const val TAG = "NixyonModels"
    }
}
