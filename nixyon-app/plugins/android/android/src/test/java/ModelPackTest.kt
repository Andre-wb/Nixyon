package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ModelPackTest {
    @Test
    fun requiredRolesBelongToTheSpeechSetAndOptionalOnesToTheirOwnPacks() {
        for (role in ModelRole.entries) {
            if (role.optional) assertNotEquals(role.key, ModelPack.SPEECH, ModelPack.of(role)) else assertEquals(role.key, ModelPack.SPEECH, ModelPack.of(role))
        }
        assertEquals(ModelPack.LIVE, ModelPack.of(ModelRole.LIVE_JOINER))
        assertEquals(ModelPack.DENOISER, ModelPack.of(ModelRole.DENOISER))
        assertEquals(ModelPack.AGE, ModelPack.of(ModelRole.AGE_GENDER))
        assertEquals(ModelPack.EMOTION, ModelPack.of(ModelRole.EMOTION))
    }

    @Test
    fun packsAreFoundByTheirInterfaceKeys() {
        assertEquals(ModelPack.LLM, ModelPack.fromKey("llm"))
        assertEquals(ModelPack.EMOTION, ModelPack.fromKey("emotion"))
        assertNull(ModelPack.fromKey("tagging"))
    }
}
