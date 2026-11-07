package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.FakeSource
import dev.demo.nixyon.speech.ModelDownloadFixtures.sha
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.random.Random

class ParallelFetcherTest {
    @get:Rule
    val dir = TemporaryFolder()

    // Три куска и ещё немного: так проверяется и неполный последний кусок.
    private val big = Random(7).nextBytes((Chunk.SIZE * 3 + 1234).toInt())
    private val url = "${ModelInstaller.MIRROR}/${sha(big)}"

    private fun fetcher(source: RangeSource) = ParallelFetcher(source, connections = 8, retryDelayMs = { 0 }, pause = {})

    private fun fetch(source: RangeSource, file: FetchFile, cancelled: AtomicBoolean = AtomicBoolean(false), chunks: MutableList<Int> = mutableListOf()) =
        runBlocking {
            fetcher(source).fetch(listOf(file), cancelled, { true }, { _, i -> synchronized(chunks) { chunks += i } }, { _, _ -> })
        }

    private fun code(block: () -> Unit): String =
        try {
            block()
            fail("загрузка прошла")
            ""
        } catch (e: ModelsException) {
            e.code
        }

    @Test
    fun chunksFetchedInParallelAddUpToTheWholeFile() {
        val part = File(dir.root, "big.part")
        val chunks = mutableListOf<Int>()
        fetch(FakeSource(bodies = mapOf(sha(big) to big)), FetchFile(url, part, big.size.toLong(), emptySet()), chunks = chunks)
        assertArrayEquals(big, part.readBytes())
        assertEquals(listOf(0, 1, 2, 3), chunks.sorted())
    }

    @Test
    fun chunksAlreadyDoneAreNotFetchedAgain() {
        val part = File(dir.root, "big.part")
        part.writeBytes(big)
        val source = FakeSource(bodies = mapOf(sha(big) to big))
        fetch(source, FetchFile(url, part, big.size.toLong(), setOf(0, 2)))
        assertEquals(setOf("${sha(big)}:${Chunk.SIZE}", "${sha(big)}:${Chunk.SIZE * 3}"), source.reads.toSet())
        assertArrayEquals(big, part.readBytes())
    }

    @Test
    fun aBrokenConnectionIsRetriedAndTheFileStillComesOutWhole() {
        val part = File(dir.root, "big.part")
        fetch(FakeSource(bodies = mapOf(sha(big) to big), flaky = 2), FetchFile(url, part, big.size.toLong(), emptySet()))
        assertArrayEquals(big, part.readBytes())
    }

    @Test
    fun aConnectionThatKeepsBreakingStopsTheDownload() {
        val part = File(dir.root, "big.part")
        val source = FakeSource(bodies = mapOf(sha(big) to big), flaky = ParallelFetcher.MAX_ATTEMPTS + 1)
        assertEquals(ModelsException.DOWNLOAD_FAILED, code { fetch(source, FetchFile(url, part, big.size.toLong(), emptySet())) })
    }

    @Test
    fun aFileMissingFromTheMirrorStopsTheDownloadAtOnce() {
        val part = File(dir.root, "big.part")
        val source = FakeSource(missing = setOf(sha(big)))
        assertEquals(ModelsException.DOWNLOAD_FAILED, code { fetch(source, FetchFile(url, part, big.size.toLong(), emptySet())) })
        assertTrue(source.reads.size <= 4)
    }

    @Test
    fun cancellingStopsTheDownload() {
        val part = File(dir.root, "big.part")
        val source = FakeSource(bodies = mapOf(sha(big) to big))
        assertEquals(ModelsException.CANCELLED, code { fetch(source, FetchFile(url, part, big.size.toLong(), emptySet()), AtomicBoolean(true)) })
    }

    @Test
    fun withoutAllowedNetworkChunksWaitInsteadOfFailing() {
        val part = File(dir.root, "big.part")
        var checks = 0
        runBlocking {
            fetcher(FakeSource(bodies = mapOf(sha(big) to big))).fetch(
                listOf(FetchFile(url, part, big.size.toLong(), emptySet())),
                AtomicBoolean(false),
                { synchronized(this@ParallelFetcherTest) { checks += 1; checks > 6 } },
                { _, _ -> },
                { _, _ -> },
            )
        }
        assertArrayEquals(big, part.readBytes())
        assertTrue(checks > 6)
    }
}
