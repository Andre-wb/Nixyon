package dev.demo.nixyon.speech

import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Log
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import java.util.concurrent.atomic.AtomicBoolean
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

// Ошибка для интерфейса: приложите палец (locked) или на телефоне нет отпечатка.
class StorageLockedException(val code: String) : Exception(code)

// Ключ данных для шифрования файлов. Сам ключ хранится обёрнутым ключом Android Keystore,
// который открывается только после настоящей проверки отпечатка (BiometricPrompt с
// CryptoObject). Развёрнутый ключ остаётся в памяти процесса: запись в фоне и чтение
// файлов после разблокировки отпечатка не просят его снова. Обёртка лежит в
// noBackupFilesDir и в резервную копию не попадает.
// Если ключ Keystore пропал (восстановление на другом телефоне, сброс ключей), создаётся
// новый ключ данных: файлы под старым прочитать уже нельзя, ядро их удалит. Временный
// сбой Keystore ключ не заменяет. Прежняя обёртка без отпечатка переносится на новую
// при первой разблокировке: для этого тоже нужен палец. Если на телефоне нет отпечатка,
// блокировки нет совсем: ключ открывается сразу.
class StorageKey(private val context: Context) {
    @Volatile
    private var cached: SecretKey? = null
    private val prompting = AtomicBoolean(false)

    // Ключ после разблокировки. До неё запись и чтение файлов невозможны.
    fun get(): SecretKey = cached ?: throw StorageLockedException(LOCKED)

    // Ответ приходит в главном потоке. Пока ключ уже открыт, палец не спрашивается.
    fun unlock(activity: FragmentActivity, onDone: (Result<SecretKey>) -> Unit) {
        cached?.let {
            onDone(Result.success(it))
            return
        }
        if (!prompting.compareAndSet(false, true)) {
            onDone(Result.failure(StorageLockedException(LOCKED)))
            return
        }
        val finish = { result: Result<SecretKey> ->
            result.onSuccess { cached = it }
            prompting.set(false)
            onDone(result)
        }
        if (BiometricManager.from(context).canAuthenticate(STRENGTH) != BiometricManager.BIOMETRIC_SUCCESS) {
            // Отпечатка нет: блокировки нет, ключ открывается сразу. Файл, запертый отпечатком,
            // без отпечатка не открыть: нужно добавить отпечаток в настройках телефона.
            if (read(BIO_FILE) != null) finish(Result.failure(StorageLockedException(UNAVAILABLE)))
            else finish(runCatching { openPlain() })
            return
        }
        try {
            open(activity, finish)
        } catch (e: Throwable) {
            Log.e(TAG_LOG, "ключ данных не открылся", e)
            finish(Result.failure(e))
        }
    }

    private fun open(activity: FragmentActivity, finish: (Result<SecretKey>) -> Unit) {
        val wrapping = keyStore().getKey(BIO_ALIAS, null) as? SecretKey
        val wrapped = read(BIO_FILE)
        if (wrapping != null && wrapped != null && wrapped.size == IV + KEY_BYTES + TAG) {
            val cipher = Cipher.getInstance(TRANSFORM)
            try {
                cipher.init(Cipher.DECRYPT_MODE, wrapping, GCMParameterSpec(TAG * 8, wrapped, 0, IV))
            } catch (e: KeyPermanentlyInvalidatedException) {
                Log.w(TAG_LOG, "ключ Keystore сброшен системой")
                create(activity, null, finish)
                return
            }
            authenticate(activity, cipher, finish) { authed ->
                SecretKeySpec(authed.doFinal(wrapped, IV, wrapped.size - IV), "AES")
            }
            return
        }
        create(activity, legacyKey(), finish)
    }

    // Новый ключ данных: либо перенесённый из прежней обёртки, либо случайный.
    private fun create(activity: FragmentActivity, legacy: ByteArray?, finish: (Result<SecretKey>) -> Unit) {
        val raw = legacy ?: ByteArray(KEY_BYTES).also { SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, newBiometricKey())
        authenticate(activity, cipher, finish) { authed ->
            write(BIO_FILE, authed.iv + authed.doFinal(raw))
            forgetLegacy()
            if (legacy == null) Log.i(TAG_LOG, "создан новый ключ данных")
            SecretKeySpec(raw, "AES")
        }
    }

    // Прежняя обёртка без отпечатка: открывается без запроса и переносится на новую.
    private fun legacyKey(): ByteArray? {
        val wrapped = read(LEGACY_FILE) ?: return null
        val wrapping = keyStore().getKey(LEGACY_ALIAS, null) as? SecretKey
        if (wrapping == null || wrapped.size != IV + KEY_BYTES + TAG) {
            Log.w(TAG_LOG, "прежний ключ данных не разворачивается: нет ключа Keystore или файл испорчен")
            return null
        }
        return try {
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.DECRYPT_MODE, wrapping, GCMParameterSpec(TAG * 8, wrapped, 0, IV))
            cipher.doFinal(wrapped, IV, wrapped.size - IV)
        } catch (e: AEADBadTagException) {
            Log.w(TAG_LOG, "прежний ключ данных не разворачивается этим ключом Keystore")
            null
        }
    }

    private fun forgetLegacy() {
        File(context.noBackupFilesDir, LEGACY_FILE).delete()
        keyStore().let { if (it.containsAlias(LEGACY_ALIAS)) it.deleteEntry(LEGACY_ALIAS) }
    }

    // Телефон без отпечатка: ключ обёрнут обычным ключом Keystore, без блокировки. Если
    // отпечаток появится позже, при следующем запуске данные перейдут на защищённую обёртку.
    private fun openPlain(): SecretKey {
        legacyKey()?.let { return SecretKeySpec(it, "AES") }
        val raw = ByteArray(KEY_BYTES).also { SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, newWrappingKey(LEGACY_ALIAS, biometric = false))
        write(LEGACY_FILE, cipher.iv + cipher.doFinal(raw))
        Log.i(TAG_LOG, "создан новый ключ данных")
        return SecretKeySpec(raw, "AES")
    }

    // Отпечаток нужен на каждое использование ключа. Смена отпечатков ключ не сбрасывает,
    // иначе после добавления нового пальца все файлы стали бы недоступны.
    private fun newBiometricKey(): SecretKey = newWrappingKey(BIO_ALIAS, biometric = true)

    private fun newWrappingKey(alias: String, biometric: Boolean): SecretKey {
        keyStore().let { if (it.containsAlias(alias)) it.deleteEntry(alias) }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        val spec = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
        if (biometric) {
            spec.setUserAuthenticationRequired(true)
                .setInvalidatedByBiometricEnrollment(false)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                spec.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
            }
        }
        generator.init(spec.build())
        return generator.generateKey()
    }

    // Шифр уходит в BiometricPrompt: только им можно будет работать после проверки пальца.
    private fun authenticate(
        activity: FragmentActivity,
        cipher: Cipher,
        finish: (Result<SecretKey>) -> Unit,
        use: (Cipher) -> SecretKey,
    ) {
        val callback = object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                val authed = result.cryptoObject?.cipher
                finish(
                    if (authed == null) Result.failure(StorageLockedException(LOCKED))
                    else runCatching { use(authed) },
                )
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                Log.i(TAG_LOG, "разблокировка отпечатком не прошла: код $errorCode")
                finish(Result.failure(StorageLockedException(LOCKED)))
            }
        }
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(activity.getString(R.string.storage_unlock_title))
            .setNegativeButtonText(activity.getString(R.string.storage_unlock_cancel))
            .setAllowedAuthenticators(STRENGTH)
            .build()
        BiometricPrompt(activity, ContextCompat.getMainExecutor(activity), callback)
            .authenticate(info, BiometricPrompt.CryptoObject(cipher))
    }

    private fun read(name: String): ByteArray? =
        File(context.noBackupFilesDir, name).takeIf { it.exists() }?.readBytes()

    private fun write(name: String, bytes: ByteArray) {
        val part = File(context.noBackupFilesDir, "$name.part")
        part.outputStream().use { out ->
            out.write(bytes)
            out.fd.sync()
        }
        if (!part.renameTo(File(context.noBackupFilesDir, name))) throw IllegalStateException("ключ данных не сохранён")
    }

    private fun keyStore() = KeyStore.getInstance(KEYSTORE).apply { load(null) }

    companion object {
        private const val TAG_LOG = "NixyonStorage"
        private const val KEYSTORE = "AndroidKeyStore"
        private const val LEGACY_ALIAS = "nixyon-storage"
        private const val BIO_ALIAS = "nixyon-storage-bio"
        private const val LEGACY_FILE = "storage-key.bin"
        private const val BIO_FILE = "storage-key-bio.bin"
        private const val TRANSFORM = "AES/GCM/NoPadding"
        private const val IV = 12
        private const val TAG = 16
        const val KEY_BYTES = 32
        const val LOCKED = "locked"
        const val UNAVAILABLE = "biometric_unavailable"
        private const val STRENGTH = BiometricManager.Authenticators.BIOMETRIC_STRONG
    }
}
