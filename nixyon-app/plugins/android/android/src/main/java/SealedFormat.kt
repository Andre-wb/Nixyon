package dev.demo.nixyon.speech

import java.io.File
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.crypto.Cipher
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

// Формат зашифрованного файла, общий с ядром (vault/format.rs). Заголовок: сигнатура,
// размер куска открытого текста u32 LE, четыре нуля, случайный id файла. Дальше куски:
// nonce, шифртекст, тег. Все куски, кроме последнего, полные. Связанные данные куска:
// заголовок, номер куска u64 LE и признак последнего.
object SealedFormat {
    val MAGIC = "NXSEAL01".toByteArray(Charsets.US_ASCII)
    const val CHUNK = 65_536
    const val HEADER = 32
    const val NONCE = 12
    const val TAG = 16
    const val FRAME = NONCE + CHUNK + TAG
    private const val FILE_ID = 16
    private const val TAG_BITS = TAG * 8

    fun header(): ByteArray {
        val id = ByteArray(FILE_ID).also { Random.secure.nextBytes(it) }
        return ByteBuffer.allocate(HEADER).order(ByteOrder.LITTLE_ENDIAN)
            .put(MAGIC).putInt(CHUNK).putInt(0).put(id).array()
    }

    fun checkHeader(h: ByteArray): Boolean {
        if (h.size != HEADER) return false
        val b = ByteBuffer.wrap(h).order(ByteOrder.LITTLE_ENDIAN)
        val magic = ByteArray(MAGIC.size).also { b.get(it) }
        return magic.contentEquals(MAGIC) && b.int == CHUNK && b.int == 0
    }

    fun aad(header: ByteArray, index: Long, last: Boolean): ByteArray =
        ByteBuffer.allocate(HEADER + 9).order(ByteOrder.LITTLE_ENDIAN)
            .put(header).putLong(index).put(if (last) 1 else 0).array()

    // nonce, шифртекст и тег одним куском; nonce свой у каждого куска.
    fun seal(key: SecretKey, plain: ByteArray, length: Int, aad: ByteArray): ByteArray {
        val nonce = ByteArray(NONCE).also { Random.secure.nextBytes(it) }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key, GCMParameterSpec(TAG_BITS, nonce))
        cipher.updateAAD(aad)
        return nonce + cipher.doFinal(plain, 0, length)
    }

    // Чужой ключ, подмена или повреждение: AEADBadTagException.
    fun open(key: SecretKey, frame: ByteArray, aad: ByteArray): ByteArray {
        require(frame.size >= NONCE + TAG) { "кусок короче nonce и тега" }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, frame, 0, NONCE))
        cipher.updateAAD(aad)
        return cipher.doFinal(frame, NONCE, frame.size - NONCE)
    }

    fun isSealed(file: File): Boolean {
        if (file.length() < MAGIC.size) return false
        val start = ByteArray(MAGIC.size)
        RandomAccessFile(file, "r").use { it.readFully(start) }
        return start.contentEquals(MAGIC)
    }

    private object Random {
        val secure = java.security.SecureRandom()
    }
}
