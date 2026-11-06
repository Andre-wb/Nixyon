package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class ModelManifestTest {
    private val required = ModelRole.entries.filter { !it.optional }

    private fun manifest(roles: List<ModelRole>, path: (ModelRole) -> String = { "${it.key}.onnx" }): String {
        val files = roles.joinToString(",") { """"${it.key}":{"path":"${path(it)}","size":1,"sha256":"AB"}""" }
        return """{"version":"v1","files":{$files}}"""
    }

    private fun rejected(text: String): String =
        try {
            ModelManifest.parse(text)
            fail("манифест принят")
            ""
        } catch (e: ModelsException) {
            e.code
        }

    @Test
    fun aFullSetWithoutOptionalRolesIsAccepted() {
        val parsed = ModelManifest.parse(manifest(required))
        assertEquals("v1", parsed.version)
        assertEquals("ab", parsed.file(ModelRole.VAD).sha256)
        assertFalse(parsed.hasLiveModel)
        assertFalse(parsed.hasDenoiser)
        assertFalse(parsed.hasAgeGender)
        assertTrue(ModelManifest.parse(manifest(required + ModelRole.AGE_GENDER)).hasAgeGender)
        assertTrue(ModelManifest.parse(manifest(required + ModelRole.DENOISER)).hasDenoiser)
        assertTrue(ModelManifest.parse(manifest(required + ModelRole.LIVE)).hasLiveModel)
    }

    @Test
    fun missingRoleOrHalfALiveModelIsRejected() {
        assertEquals(ModelsException.BAD_MANIFEST, rejected(manifest(required - ModelRole.VAD)))
        assertEquals(ModelsException.BAD_MANIFEST, rejected(manifest(required + ModelRole.LIVE_ENCODER)))
    }

    @Test
    fun pathsOutsideTheModelsFolderAreRejected() {
        for (bad in listOf("", "../vad.onnx", "sub/vad.onnx", "sub\\\\vad.onnx", ".vad")) {
            assertEquals(bad, ModelsException.BAD_MANIFEST, rejected(manifest(required) { if (it == ModelRole.VAD) bad else "${it.key}.onnx" }))
        }
        assertTrue(ModelFile.isSafePath("silero_vad.v5.onnx"))
    }
}
