package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.FakeSource
import dev.demo.nixyon.speech.ModelDownloadFixtures.catalog
import dev.demo.nixyon.speech.ModelDownloadFixtures.contents
import dev.demo.nixyon.speech.ModelDownloadFixtures.sha
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

class ModelInstallerTest {
    @get:Rule
    val dir = TemporaryFolder()

    private val catalog = catalog()
    private val folders by lazy { ModelFolders(File(dir.root, "files")) }
    private val journal by lazy { DownloadJournal(File(dir.root, "journal.json")) }

    private fun installer(source: RangeSource, free: Long, connections: Int) =
        ModelInstaller(catalog, folders, ParallelFetcher(source, connections, retryDelayMs = { 0 }, pause = {}), journal, { free })

    private fun install(
        source: RangeSource,
        packs: Set<ModelPack>,
        cancelled: AtomicBoolean = AtomicBoolean(false),
        free: Long = Long.MAX_VALUE,
        connections: Int = 4,
    ) = runBlocking { installer(source, free, connections).install(packs, false, { true }, cancelled) {} }

    private fun rejected(block: () -> Unit): String =
        try {
            block()
            fail("загрузка прошла")
            ""
        } catch (e: ModelsException) {
            e.code
        }

    @Test
    fun aFinishedDownloadLeavesFilesUnderTheirNamesAndDescriptionsTheLocatorsRead() {
        val source = FakeSource()
        install(source, setOf(ModelPack.LLM))

        val speech = ModelManifest.parse(folders.manifest(ModelTarget.SPEECH).readText())
        assertEquals(ModelRole.entries.filter { !it.optional }.toSet(), speech.files.keys)
        for ((role, file) in speech.files) {
            assertEquals(contents.getValue(role.key).toList(), folders.file(ModelTarget.SPEECH, file).readBytes().toList())
            assertFalse(folders.part(ModelTarget.SPEECH, file).exists())
        }
        val llm = LlmManifest.parse(folders.manifest(ModelTarget.LLM).readText())
        assertEquals(sha(contents.getValue("llm")), llm.model.sha256)
        assertTrue(folders.file(ModelTarget.LLM, llm.model).isFile)
        assertNull(journal.read())
    }

    @Test
    fun aFileWhoseSumDiffersFromTheCatalogLeavesNoDescriptionAndIsFetchedAgainOnRetry() {
        val vad = catalog.speech.file(ModelRole.VAD)
        val bad = FakeSource(bodies = mapOf(vad.sha256 to ByteArray(vad.size.toInt())))
        assertEquals(ModelsException.DOWNLOAD_CORRUPT, rejected { install(bad, emptySet()) })
        assertFalse(folders.manifest(ModelTarget.SPEECH).exists())
        assertFalse(folders.file(ModelTarget.SPEECH, vad).exists())
        assertEquals(emptySet<Int>(), journal.read()!!.chunks[folders.partSubPath(ModelTarget.SPEECH, vad)])

        val good = FakeSource()
        install(good, emptySet())
        assertEquals(listOf("${vad.sha256}:0"), good.reads.toList())
        assertTrue(folders.manifest(ModelTarget.SPEECH).isFile)
    }

    @Test
    fun aDownloadInterruptedBeforeARestartFetchesOnlyTheMissingFiles() {
        // Одно соединение: файлы идут по порядку каталога, VAD и кодер успевают скачаться
        // до файла, которого нет на зеркале.
        val decoder = catalog.speech.file(ModelRole.ASR_DECODER)
        val missing = FakeSource(missing = setOf(decoder.sha256))
        assertEquals(ModelsException.DOWNLOAD_FAILED, rejected { install(missing, emptySet(), connections = 1) })
        assertFalse(folders.manifest(ModelTarget.SPEECH).exists())

        val good = FakeSource()
        install(good, emptySet(), connections = 1)
        val again = good.reads.map { it.substringBefore(':') }.toSet()
        assertFalse(catalog.speech.file(ModelRole.VAD).sha256 in again)
        assertFalse(catalog.speech.file(ModelRole.ASR_ENCODER).sha256 in again)
        assertTrue(decoder.sha256 in again)
        assertTrue(folders.manifest(ModelTarget.SPEECH).isFile)
    }

    @Test
    fun cancellingRemovesTheTemporaryFilesAndTheJournal() {
        val cancelled = AtomicBoolean(true)
        assertEquals(ModelsException.CANCELLED, rejected { install(FakeSource(), setOf(ModelPack.LLM), cancelled) })
        assertNull(journal.read())
        assertFalse(folders.manifest(ModelTarget.SPEECH).exists())
        val leftovers = folders.base.walk().filter { it.isFile }.toList()
        assertEquals(emptyList<File>(), leftovers)
    }

    @Test
    fun withoutRoomForTheModelsNothingIsFetched() {
        val source = FakeSource()
        assertEquals(ModelsException.STORAGE_LOW, rejected { install(source, setOf(ModelPack.LLM), free = 1024) })
        assertTrue(source.reads.isEmpty())
    }
}
