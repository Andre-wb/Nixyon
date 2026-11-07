package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.catalog
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Test

class ModelCatalogTest {
    @Test
    fun filesAreGroupedByPackAndSizesAddUp() {
        val catalog = catalog()
        assertEquals(ModelPack.entries, catalog.packs)
        assertEquals(4, catalog.files(ModelPack.LIVE).size)
        assertEquals(listOf(ModelTarget.LLM), catalog.files(ModelPack.LLM).map { it.target })
        val speech = catalog.files(ModelPack.SPEECH)
        assertEquals(ModelRole.entries.filter { !it.optional }.toSet(), speech.map { it.role }.toSet())
        assertEquals(speech.sumOf { it.file.size }, catalog.size(ModelPack.SPEECH))
    }

    @Test
    fun anOptionalModelMissingFromTheCatalogIsNotOffered() {
        val catalog = catalog(ModelRole.entries.filter { it != ModelRole.AGE_GENDER })
        assertEquals(listOf(ModelPack.SPEECH, ModelPack.LIVE, ModelPack.DENOISER, ModelPack.EMOTION, ModelPack.LLM), catalog.packs)
    }

    @Test
    fun aSumThatIsNotSixtyFourHexDigitsIsRejectedBecauseItGoesIntoTheMirrorAddress() {
        val roles = ModelRole.entries.filter { !it.optional }
        val files = roles.joinToString(",") { """"${it.key}":{"path":"${it.key}.onnx","size":1,"sha256":"../x"}""" }
        try {
            ModelCatalog.parse(
                """{"version":"v","files":{$files}}""",
                """{"version":"l","model":{"path":"m.gguf","size":1,"sha256":"${"a".repeat(64)}"}}""",
            )
            fail("каталог принят")
        } catch (e: ModelsException) {
            assertEquals(ModelsException.BAD_MANIFEST, e.code)
        }
    }
}
