package dev.demo.nixyon.speech

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import javax.crypto.SecretKey
import javax.crypto.spec.SecretKeySpec
import kotlin.random.Random

class SealedRecoveryTest {
    private val dir = Files.createTempDirectory("nixyon-recovery").toFile()
    private val key: SecretKey = SecretKeySpec(ByteArray(32) { it.toByte() }, "AES")
    private val chunk = SealedFormat.CHUNK
    private val header = SealedFormat.HEADER
    private val frame = SealedFormat.FRAME

    @Test
    fun aClosedRecordingReadsBackWithItsSizesInTheWavHeader() {
        for (size in listOf(2, chunk - 44, chunk - 42, 3 * chunk + 122)) {
            val pcm = pcm(size)
            val file = File(dir, "closed-$size.wav")
            WavPcm16Writer(file, key, 16_000, 1).use { it.write(pcm, pcm.size) }
            assertWav(readSealed(file), 16_000, 1, pcm)
        }
    }

    @Test
    fun theFirstChunkIsOnDiskWhileTheRecordingGoesOn() {
        val file = File(dir, "open.wav.part")
        val pcm = pcm(chunk)
        val writer = WavPcm16Writer(file, key, 48_000, 1)
        writer.write(pcm, pcm.size)
        val bytes = file.readBytes()
        val plain = SealedFormat.open(
            key,
            bytes.copyOfRange(header, header + frame),
            SealedFormat.aad(bytes.copyOf(header), 0, last = false),
        )
        val wav = ByteBuffer.wrap(plain).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(1, wav.getShort(22).toInt())
        assertEquals(48_000, wav.getInt(24))
        writer.close()
    }

    @Test
    fun aRecordingCutAnywhereKeepsEveryWholeChunkBeforeTheCut() {
        val pcm = pcm(5 * chunk + 1_000)
        val crashed = crashed(pcm, 16_000, 1)
        // Полных кусков на диске: последний неполный остался в памяти.
        val written = (crashed.size - header) / frame
        assertEquals(5, written)
        val random = Random(7)
        val cuts = (0..crashed.size step frame / 4).toList() +
            (0..written).flatMap { i -> listOf(header + i * frame - 1, header + i * frame, header + i * frame + 1) } +
            List(40) { random.nextInt(crashed.size + 1) }
        for (cut in cuts.filter { it in 0..crashed.size }.distinct()) {
            for (tail in listOf("cut", "zeros")) {
                val file = File(dir, "rec-$cut-$tail.wav.part")
                val body = crashed.copyOf(cut)
                // После сбоя питания хвост файла может оказаться нулями, а не обрезанным.
                val onDisk = if (tail == "cut") body else body + ByteArray(crashed.size - cut)
                file.writeBytes(onDisk)
                val kept = intactChunks(onDisk, crashed, written)
                val duration = SealedRecovery.recoverWav(file, key)
                if (kept == 0) {
                    assertNull("$cut $tail", duration)
                    continue
                }
                val dataBytes = kept * chunk - 44
                assertEquals("$cut $tail", dataBytes / 2.0 / 16_000, duration!!, 1e-9)
                assertWav(readSealed(file), 16_000, 1, pcm.copyOf(dataBytes))
            }
        }
    }

    @Test
    fun zeroesThatRestoreTheCutTagKeepThatChunk() {
        // Запись, у первого куска которой последний байт метки нулевой: обрыв перед ним и
        // нули вместо хвоста восстанавливают кусок полностью.
        val (pcm, crashed) = (1..2_000).asSequence()
            .map { seed -> Random(seed).nextBytes(2 * chunk + 1_000) }
            .map { pcm -> pcm to crashed(pcm, 16_000, 1) }
            .first { (_, bytes) -> bytes[header + frame - 1] == 0.toByte() }
        val cut = header + frame - 1
        val file = File(dir, "rec-zero-tag.wav.part")
        file.writeBytes(crashed.copyOf(cut) + ByteArray(crashed.size - cut))
        val dataBytes = chunk - 44
        assertEquals(dataBytes / 2.0 / 16_000, SealedRecovery.recoverWav(file, key)!!, 1e-9)
        assertWav(readSealed(file), 16_000, 1, pcm.copyOf(dataBytes))
    }

    @Test
    fun aRecordingStoppedNormallyButNotRenamedStaysAsItIs() {
        val pcm = pcm(2 * chunk + 500)
        val file = File(dir, "rec-closed.wav.part")
        WavPcm16Writer(file, key, 16_000, 1).use { it.write(pcm, pcm.size) }
        val before = file.readBytes()
        assertEquals(pcm.size / 2.0 / 16_000, SealedRecovery.recoverWav(file, key)!!, 1e-9)
        assertArrayEquals(before, file.readBytes())
    }

    @Test
    fun aCrashWhileClosingLosesOnlyTheLastChunk() {
        val pcm = pcm(3 * chunk + 20_000)
        val file = File(dir, "rec-closing.wav.part")
        WavPcm16Writer(file, key, 16_000, 1).use { it.write(pcm, pcm.size) }
        // Первый кусок уже с размерами всей записи, последний оборван.
        file.writeBytes(file.readBytes().let { it.copyOf(it.size - 100) })
        val duration = SealedRecovery.recoverWav(file, key)
        val dataBytes = 3 * chunk - 44
        assertEquals(dataBytes / 2.0 / 16_000, duration!!, 1e-9)
        assertWav(readSealed(file), 16_000, 1, pcm.copyOf(dataBytes))
    }

    @Test
    fun recoveryCutShortBeforeTheWavHeaderIsFinishedOnTheNextLaunch() {
        val pcm = pcm(4 * chunk)
        val crashed = crashed(pcm, 16_000, 1)
        val file = File(dir, "rec-twice.wav.part")
        file.writeBytes(crashed)
        val duration = SealedRecovery.recoverWav(file, key)
        // Как будто упали после последнего куска, но до заголовка WAV: первый кусок прежний.
        val half = file.readBytes()
        crashed.copyInto(half, header, header, header + frame)
        file.writeBytes(half)
        assertEquals(duration!!, SealedRecovery.recoverWav(file, key)!!, 1e-9)
        assertWav(readSealed(file), 16_000, 1, pcm.copyOf(4 * chunk - 44))
    }

    @Test
    fun recoveringARecoveredFileChangesNothing() {
        val file = File(dir, "rec-again.wav.part")
        file.writeBytes(crashed(pcm(2 * chunk + 10), 16_000, 1))
        val first = SealedRecovery.recoverWav(file, key)
        val after = file.readBytes()
        assertEquals(first!!, SealedRecovery.recoverWav(file, key)!!, 1e-9)
        assertArrayEquals(after, file.readBytes())
    }

    @Test
    fun aRecordingShorterThanOneChunkHasNothingToRecover() {
        val file = File(dir, "rec-short.wav.part")
        val crashed = crashed(pcm(chunk - 44), 16_000, 1)
        assertEquals(header, crashed.size)
        file.writeBytes(crashed)
        assertNull(SealedRecovery.recoverWav(file, key))
        assertArrayEquals(crashed, file.readBytes())
    }

    @Test
    fun filesUnderAnotherKeyOrNotSealedAreLeftAlone() {
        val crashed = crashed(pcm(3 * chunk), 16_000, 1)
        val foreign = File(dir, "rec-foreign.wav.part").also { it.writeBytes(crashed) }
        assertNull(SealedRecovery.recoverWav(foreign, SecretKeySpec(ByteArray(32) { 9 }, "AES")))
        assertArrayEquals(crashed, foreign.readBytes())
        val plain = WavPcm16Writer.header(16_000, 1, 0) + pcm(3 * chunk)
        val open = File(dir, "rec-plain.wav.part").also { it.writeBytes(plain) }
        assertNull(SealedRecovery.recoverWav(open, key))
        assertArrayEquals(plain, open.readBytes())
        assertNull(SealedRecovery.recoverWav(File(dir, "rec-empty.wav.part").also { it.writeBytes(ByteArray(0)) }, key))
    }

    @Test
    fun aSampleCutBetweenChannelsIsDropped() {
        val pcm = pcm(3 * chunk)
        val file = File(dir, "rec-3ch.wav.part")
        file.writeBytes(crashed(pcm, 16_000, 3))
        val duration = SealedRecovery.recoverWav(file, key)
        val dataBytes = (3 * chunk - 44) / 6 * 6
        assertEquals(dataBytes / 6.0 / 16_000, duration!!, 1e-9)
        assertWav(readSealed(file), 16_000, 3, pcm.copyOf(dataBytes))
    }

    @Test
    fun aCrashedMeetingRecordingIsRecoveredNextToThePreviousOne() {
        val recordings = RecordingsDir(dir)
        recordings.dir.mkdirs()
        val previous = File(recordings.dir, "rec-1.wav").also { it.writeText("previous") }
        val pcm = pcm(2 * chunk + 300)
        File(recordings.dir, "rec-20.wav.part").writeBytes(crashed(pcm, 16_000, 1))
        File(recordings.dir, "rec-3.wav.part").writeBytes(crashed(pcm, 48_000, 1))
        val import = File(recordings.dir, "imp-4.wav.part").also { it.writeBytes(crashed(pcm, 44_100, 2)) }
        val broken = File(recordings.dir, "rec-5.wav.part").also { it.writeBytes(ByteArray(100)) }

        val found = recordings.recoverParts(key)

        assertEquals(
            listOf(File(recordings.dir, "rec-3.wav").absolutePath, File(recordings.dir, "rec-20.wav").absolutePath),
            found.map { it.path },
        )
        assertTrue(found.all { it.reason == RecordingsDir.CRASHED })
        assertEquals((2 * chunk - 44) / 2.0 / 16_000, found.last().durationSec, 1e-9)
        assertWav(readSealed(File(found.last().path!!)), 16_000, 1, pcm.copyOf(2 * chunk - 44))
        assertTrue(previous.exists())
        assertTrue(import.exists())
        assertTrue(broken.exists())
        assertFalse(File(recordings.dir, "rec-20.wav.part").exists())
    }

    // Файл, каким его оставило бы падение приложения посреди записи: всё, что успело уйти
    // в файл, без закрытия.
    private fun crashed(pcm: ByteArray, rate: Int, channels: Int): ByteArray {
        val file = File(dir, "crash-${System.nanoTime()}.wav.part")
        val writer = WavPcm16Writer(file, key, rate, channels)
        var at = 0
        while (at < pcm.size) {
            val n = minOf(3_200, pcm.size - at)
            writer.write(pcm.copyOfRange(at, at + n), n)
            at += n
        }
        val snapshot = file.readBytes()
        writer.close()
        file.delete()
        return snapshot
    }

    // Читает файл так же строго, как ядро (vault/sealed_reader.rs) и SealedMediaSource:
    // последний кусок тот, за которым конец файла.
    private fun readSealed(file: File): ByteArray {
        val bytes = file.readBytes()
        val head = bytes.copyOf(header)
        assertTrue(SealedFormat.checkHeader(head))
        val out = ByteArrayOutputStream()
        var at = header
        var index = 0L
        while (at < bytes.size) {
            val end = minOf(at + frame, bytes.size)
            out.write(SealedFormat.open(key, bytes.copyOfRange(at, end), SealedFormat.aad(head, index, end == bytes.size)))
            at = end
            index += 1
        }
        assertTrue(index > 0)
        return out.toByteArray()
    }

    private fun assertWav(plain: ByteArray, rate: Int, channels: Int, data: ByteArray) {
        assertArrayEquals(WavPcm16Writer.header(rate, channels, data.size.toLong()), plain.copyOf(44))
        assertArrayEquals(data, plain.copyOfRange(44, plain.size))
    }

    private fun pcm(size: Int) = Random(size).nextBytes(size)

    // Сколько кусков с начала лежит на диске целиком: заголовок и все байты куска те же,
    // что записал WavPcm16Writer. Нули вместо оборванного хвоста иногда совпадают с ним:
    // байт метки GCM бывает нулём, тогда кусок цел и расшифровывается.
    private fun intactChunks(onDisk: ByteArray, written: ByteArray, chunks: Int): Int {
        fun same(from: Int, to: Int) = to <= onDisk.size && (from until to).all { onDisk[it] == written[it] }
        if (!same(0, header)) return 0
        return (0 until chunks).takeWhile { same(header + it * frame, header + (it + 1) * frame) }.count()
    }
}
