package dev.demo.nixyon.speech

import android.content.Context
import android.net.ConnectivityManager

object NetworkState {
    // Платная сеть (мобильная или точка доступа): перед большой загрузкой спрашиваем.
    // Без сведений о сети считаем её платной.
    fun isMetered(context: Context): Boolean =
        context.getSystemService(ConnectivityManager::class.java)?.isActiveNetworkMetered ?: true
}
