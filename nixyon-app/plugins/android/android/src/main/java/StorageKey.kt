package dev.demo.nixyon.speech

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Log
import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

// Ключ данных для шифрования файлов. Сам ключ хранится обёрнутым ключом Android
// Keystore, который из Keystore не извлекается; обёртка лежит в noBackupFilesDir и в
// резервную копию не попадает. Если ключ Keystore пропал (восстановление на другом
// телефоне, сброс хранилища ключей), создаётся новый ключ данных: файлы под старым
// прочитать уже нельзя, ядро их удалит. Временный сбой Keystore ключ не заменяет.
class StorageKey(private val context: Context) {
    private var cached: SecretKey? = null

    @Synchronized
    fun get(): SecretKey {
        cached?.let { return it }
        val key = load() ?: create()
        cached = key
        return key
    }

    private fun file() = File(context.noBackupFilesDir, FILE)

    private fun load(): SecretKey? {
        val f = file()
        if (!f.exists()) return null
        val wrapped = f.readBytes()
        val wrapping = existingWrappingKey()
        if (wrapping == null || wrapped.size != IV + KEY_BYTES + TAG) {
            Log.w(TAG_LOG, "ключ данных не разворачивается: нет ключа Keystore или файл испорчен")
            return null
        }
        return try {
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.DECRYPT_MODE, wrapping, GCMParameterSpec(TAG * 8, wrapped, 0, IV))
            SecretKeySpec(cipher.doFinal(wrapped, IV, wrapped.size - IV), "AES")
        } catch (e: AEADBadTagException) {
            Log.w(TAG_LOG, "ключ данных не разворачивается этим ключом Keystore")
            null
        }
    }

    private fun create(): SecretKey {
        val raw = ByteArray(KEY_BYTES).also { SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, newWrappingKey())
        val wrapped = cipher.iv + cipher.doFinal(raw)
        val part = File(context.noBackupFilesDir, "$FILE.part")
        part.outputStream().use { out ->
            out.write(wrapped)
            out.fd.sync()
        }
        if (!part.renameTo(file())) throw IllegalStateException("ключ данных не сохранён")
        Log.i(TAG_LOG, "создан новый ключ данных")
        return SecretKeySpec(raw, "AES")
    }

    private fun keyStore() = KeyStore.getInstance(KEYSTORE).apply { load(null) }

    private fun existingWrappingKey(): SecretKey? = keyStore().getKey(ALIAS, null) as? SecretKey

    // Новый ключ Keystore заменяет прежний: ключ данных под прежним всё равно потерян.
    private fun newWrappingKey(): SecretKey {
        keyStore().let { if (it.containsAlias(ALIAS)) it.deleteEntry(ALIAS) }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    companion object {
        private const val TAG_LOG = "NixyonStorage"
        private const val KEYSTORE = "AndroidKeyStore"
        private const val ALIAS = "nixyon-storage"
        private const val FILE = "storage-key.bin"
        private const val TRANSFORM = "AES/GCM/NoPadding"
        private const val IV = 12
        private const val TAG = 16
        const val KEY_BYTES = 32
    }
}
