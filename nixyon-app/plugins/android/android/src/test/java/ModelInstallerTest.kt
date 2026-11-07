package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.FakeQueue
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

    private fun installer(queue: DownloadQueue, free: Long = Long.MAX_VALUE) =
        ModelInstaller(catalog, folders, queue, journal, { free }, pollMs = 1)

    private fun install(queue: DownloadQueue, packs: Set<ModelPack>, cancelled: AtomicBoolean = AtomicBoolean(false)) =
        runBlocking { installer(queue).install(packs, false, "Nixyon", cancelled) {} }

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
        val queue = FakeQueue(folders.base)
        install(queue, setOf(ModelPack.LLM))

        val speech = ModelManifest.parse(folders.manifest(ModelTarget.SPEECH).readText())
        assertEquals(ModelRole.entries.filter { !it.optional }.toSet(), speech.files.keys)
        for ((role, file) in speech.files) {
            assertEquals(contents.getValue(role.key).toList(), folders.file(ModelTarget.SPEECH, file).readBytes().toList())
            assertFalse(folders.part(ModelTarget.SPEECH, file).exists())
        }
        val llm = LlmManifest.parse(folders.manifest(ModelTarget.LLM).readText())
        assertEquals(sha(contents.getValue("llm")), llm.model.sha256)
        assertTrue(queue.requests.all { it.url.startsWith(ModelInstaller.MIRROR + "/") })
        assertNull(journal.read())
    }

    @Test
    fun aFileWhoseSumDiffersFromTheCatalogLeavesNoDescription() {
        val vad = catalog.speech.file(ModelRole.VAD)
        val queue = FakeQueue(folders.base, bodies = mapOf(vad.sha256 to ByteArray(vad.size.toInt())))
        assertEquals(ModelsException.DOWNLOAD_CORRUPT, rejected { install(queue, emptySet()) })
        assertFalse(folders.manifest(ModelTarget.SPEECH).exists())
        assertFalse(folders.file(ModelTarget.SPEECH, vad).exists())
    }

    @Test
    fun aFailedFileStopsUnfinishedDownloadsAndKeepsFinishedOnesForTheRetry() {
        val vad = catalog.speech.file(ModelRole.VAD)
        val llm = catalog.llm.model
        val failing = FakeQueue(folders.base, fail = setOf(vad.sha256), running = setOf(llm.sha256))
        assertEquals(ModelsException.DOWNLOAD_FAILED, rejected { install(failing, setOf(ModelPack.LLM)) })
        val kept = journal.read()!!.ids
        assertFalse(kept.containsKey(folders.partSubPath(ModelTarget.SPEECH, vad)))
        assertFalse(kept.containsKey(folders.partSubPath(ModelTarget.LLM, llm)))
        assertEquals(catalog.files(ModelPack.SPEECH).size - 1, kept.size)
        assertTrue(failing.statuses.values.none { it.state != DownloadStatus.State.DONE })
        assertFalse(folders.manifest(ModelTarget.SPEECH).exists())
    }

    @Test
    fun aDownloadStartedBeforeARestartIsPickedUpWithoutQueueingItAgain() {
        val queue = FakeQueue(folders.base)
        val vad = catalog.speech.file(ModelRole.VAD)
        val subPath = folders.partSubPath(ModelTarget.SPEECH, vad)
        val id = queue.enqueue(DownloadRequest("${ModelInstaller.MIRROR}/${vad.sha256}", subPath, "Nixyon", false))
        journal.write(DownloadJournal.Entry(setOf(ModelPack.SPEECH), false, mapOf(subPath to id)))

        install(queue, emptySet())
        assertEquals(1, queue.requests.count { it.subPath == subPath })
        assertTrue(folders.manifest(ModelTarget.SPEECH).isFile)
    }

    @Test
    fun cancellingRemovesTheDownloadsAndTheJournal() {
        val queue = FakeQueue(folders.base, fail = emptySet())
        val cancelled = AtomicBoolean(true)
        assertEquals(ModelsException.CANCELLED, rejected { install(queue, setOf(ModelPack.LLM), cancelled) })
        assertNull(journal.read())
        assertTrue(queue.statuses.isEmpty())
        assertFalse(folders.manifest(ModelTarget.SPEECH).exists())
    }

    @Test
    fun withoutRoomForTheModelsNothingIsQueued() {
        val queue = FakeQueue(folders.base)
        val code = rejected { runBlocking { installer(queue, free = 1024).install(setOf(ModelPack.LLM), false, "Nixyon", AtomicBoolean(false)) {} } }
        assertEquals(ModelsException.STORAGE_LOW, code)
        assertTrue(queue.requests.isEmpty())
    }
}
