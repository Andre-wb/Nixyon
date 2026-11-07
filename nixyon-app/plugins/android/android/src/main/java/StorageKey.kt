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

// Ошибка для интерфейса: приложите палец (locked), на телефоне нет отпечатка у запертого
// ключа (biometric_unavailable) или при включении входа (biometric_missing), ключ стёрт
// системой (storage_key_lost).
class StorageLockedException(val code: String) : Exception(code)

// Ключ данных для шифрования файлов. Сам ключ хранится обёрнутым ключом Android Keystore.
// По умолчанию обёртка обычная и открывается без запроса. Если в настройках включён вход
// по отпечатку, обёртка запирается ключом Keystore, который открывается только после
// настоящей проверки отпечатка (BiometricPrompt с CryptoObject). Включено ли, видно по
// тому, какая обёртка лежит на телефоне: отдельного флага нет, и рассинхрона с ним тоже.
// Развёрнутый ключ остаётся в памяти процесса: запись в фоне и чтение файлов после
// разблокировки не просят палец снова. Обёртки лежат в noBackupFilesDir и в резервную
// копию не попадают.
// Ключ под отпечатком система стирает, если на телефоне снять блокировку экрана. Тогда
// новый ключ молча не создаётся: приложение сообщает, что старые записи не прочитать, и
// начинает заново только по `reset`, после подтверждения пользователя. Временный сбой
// Keystore ключ не заменяет.
class StorageKey(private val context: Context) {
    @Volatile
    private var cached: SecretKey? = null
    private val prompting = AtomicBoolean(false)

    // Последнее открытие нашло стёртый ключ: только тогда разрешён `reset`.
    @Volatile
    private var lost = false

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
        try {
            open(activity, finish)
        } catch (e: Throwable) {
            Log.e(TAG_LOG, "ключ данных не открылся", e)
            finish(Result.failure(e))
        }
    }

    // Стирает ключ, который система уже сделала бесполезным, чтобы начать заново. Работает,
    // только если последнее открытие нашло ключ стёртым: иначе это удаление живых данных.
    fun reset() {
        if (!lost) throw StorageLockedException(LOCKED)
        forgetSealed()
        lost = false
        Log.w(TAG_LOG, "стёртый ключ данных удалён по просьбе пользователя")
    }

    // Включён ли вход по отпечатку и есть ли отпечаток на телефоне.
    fun biometricStatus(): BiometricStatus = BiometricStatus(
        enabled = File(context.noBackupFilesDir, BIO_FILE).exists(),
        available = canAuthenticate(),
    )

    // Включает или выключает вход по отпечатку, когда ключ уже открыт. Палец нужен в обе
    // стороны: без него ключ не запереть, а выключение без пальца снимало бы защиту с
    // разблокированного телефона в чужих руках. Ответ приходит в главном потоке.
    fun setBiometric(activity: FragmentActivity, enabled: Boolean, onDone: (Result<Unit>) -> Unit) {
        if (cached == null) {
            onDone(Result.failure(StorageLockedException(LOCKED)))
            return
        }
        if (biometricStatus().enabled == enabled) {
            onDone(Result.success(Unit))
            return
        }
        if (!canAuthenticate()) {
            onDone(Result.failure(StorageLockedException(if (enabled) MISSING else UNAVAILABLE)))
            return
        }
        if (!prompting.compareAndSet(false, true)) {
            onDone(Result.failure(StorageLockedException(LOCKED)))
            return
        }
        val finish = { result: Result<Unit> ->
            prompting.set(false)
            onDone(result)
        }
        try {
            if (enabled) {
                seal(activity, cached!!.encoded, finish) { Log.i(TAG_LOG, "вход по отпечатку включён") }
            } else {
                unseal(activity, finish)
            }
        } catch (e: Throwable) {
            Log.e(TAG_LOG, "вход по отпечатку не переключён", e)
            finish(Result.failure(e))
        }
    }

    private fun open(activity: FragmentActivity, finish: (Result<SecretKey>) -> Unit) {
        val wrapped = read(BIO_FILE)
        val wrapping = keyStore().getKey(BIO_ALIAS, null) as? SecretKey
        val cipher = Cipher.getInstance(TRANSFORM)
        val valid = wrapped != null && wrapped.size == IV + KEY_BYTES + TAG
        var invalidated = false
        if (valid && wrapping != null) {
            try {
                cipher.init(Cipher.DECRYPT_MODE, wrapping, GCMParameterSpec(TAG * 8, wrapped, 0, IV))
            } catch (e: KeyPermanentlyInvalidatedException) {
                Log.w(TAG_LOG, "ключ Keystore стёрт системой")
                invalidated = true
            }
        }
        val state = KeyState(
            canAuthenticate = canAuthenticate(),
            bioFile = wrapped != null,
            bioFileValid = valid,
            bioKey = wrapping != null,
            bioKeyInvalidated = invalidated,
            legacyFile = File(context.noBackupFilesDir, LEGACY_FILE).exists(),
        )
        lost = false
        when (KeyOpenings.choose(state)) {
            KeyOpening.PLAIN -> finish(runCatching { openPlain() })
            KeyOpening.UNAVAILABLE -> finish(Result.failure(StorageLockedException(UNAVAILABLE)))
            KeyOpening.LOST -> {
                Log.w(TAG_LOG, "ключ данных под отпечатком потерян")
                lost = true
                finish(Result.failure(StorageLockedException(KEY_LOST)))
            }
            KeyOpening.BIOMETRIC -> authenticate(activity, cipher, finish) { authed ->
                val key = SecretKeySpec(authed.doFinal(wrapped!!, IV, wrapped.size - IV), "AES")
                // Обычная обёртка могла остаться после оборванного переключения. Ключ заперт
                // и цел, поэтому незапертая копия только ослабляет защиту.
                forgetLegacy()
                key
            }
            KeyOpening.MIGRATE -> {
                // Обычная обёртка не разворачивается: при запертом ключе это потеря, о
                // которой надо сказать.
                val legacy = legacyKey()
                if (legacy == null) {
                    lost = true
                    finish(Result.failure(StorageLockedException(KEY_LOST)))
                } else {
                    // Перезапирание заменяет ключ Keystore под отпечатком ещё до пальца. Если
                    // палец отменят, старая обёртка с новым ключом выглядела бы целой, и каждый
                    // запуск падал бы на расшифровке. Пустая обёртка заранее оставляет вход
                    // включённым и при следующем запуске снова ведёт сюда.
                    write(BIO_FILE, ByteArray(0))
                    seal(activity, legacy, finish) { SecretKeySpec(legacy, "AES") }
                }
            }
        }
    }

    // Запирает ключ данных отпечатком. Запертая обёртка пишется целиком до удаления обычной:
    // при падении посередине ключ остаётся хотя бы в одной из них.
    private fun <T> seal(activity: FragmentActivity, raw: ByteArray, finish: (Result<T>) -> Unit, done: () -> T) {
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, newBiometricKey())
        authenticate(activity, cipher, finish) { authed ->
            write(BIO_FILE, authed.iv + authed.doFinal(raw))
            forgetLegacy()
            done()
        }
    }

    // Снимает отпечаток. Палец открывает запертую обёртку, так проверяется, что выключает
    // владелец. Затем ключ пишется в обычную обёртку и только после этого удаляется
    // запертая: при падении посередине останутся обе, и при запуске спросят палец.
    private fun unseal(activity: FragmentActivity, finish: (Result<Unit>) -> Unit) {
        val wrapped = read(BIO_FILE)
        val wrapping = keyStore().getKey(BIO_ALIAS, null) as? SecretKey
        if (wrapped == null || wrapping == null || wrapped.size != IV + KEY_BYTES + TAG) {
            throw IllegalStateException("запертая обёртка ключа не читается")
        }
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.DECRYPT_MODE, wrapping, GCMParameterSpec(TAG * 8, wrapped, 0, IV))
        authenticate(activity, cipher, finish) { authed ->
            val raw = authed.doFinal(wrapped, IV, wrapped.size - IV)
            try {
                writePlain(raw)
            } finally {
                raw.fill(0)
            }
            forgetSealed()
            Log.i(TAG_LOG, "вход по отпечатку выключен")
        }
    }

    private fun forgetSealed() {
        File(context.noBackupFilesDir, BIO_FILE).delete()
        keyStore().let { if (it.containsAlias(BIO_ALIAS)) it.deleteEntry(BIO_ALIAS) }
    }

    // Обычная обёртка: открывается без запроса.
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

    // Вход по отпечатку выключен: ключ обёрнут обычным ключом Keystore, без блокировки.
    // Под отпечаток он переходит только по выбору в настройках.
    private fun openPlain(): SecretKey {
        legacyKey()?.let { return SecretKeySpec(it, "AES") }
        val raw = ByteArray(KEY_BYTES).also { SecureRandom().nextBytes(it) }
        writePlain(raw)
        Log.i(TAG_LOG, "создан новый ключ данных")
        return SecretKeySpec(raw, "AES")
    }

    private fun writePlain(raw: ByteArray) {
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, newWrappingKey(LEGACY_ALIAS, biometric = false))
        write(LEGACY_FILE, cipher.iv + cipher.doFinal(raw))
    }

    private fun canAuthenticate(): Boolean =
        BiometricManager.from(context).canAuthenticate(STRENGTH) == BiometricManager.BIOMETRIC_SUCCESS

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
    private fun <T> authenticate(
        activity: FragmentActivity,
        cipher: Cipher,
        finish: (Result<T>) -> Unit,
        use: (Cipher) -> T,
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
        const val MISSING = "biometric_missing"
        const val KEY_LOST = "storage_key_lost"
        private const val STRENGTH = BiometricManager.Authenticators.BIOMETRIC_STRONG
    }
}
