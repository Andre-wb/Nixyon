package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Test

class KeyOpeningTest {
    private val sealed = KeyState(
        canAuthenticate = true,
        bioFile = true,
        bioFileValid = true,
        bioKey = true,
        bioKeyInvalidated = false,
        legacyFile = false,
    )

    @Test
    fun intactSealedKeyAsksForTheFinger() {
        assertEquals(KeyOpening.BIOMETRIC, KeyOpenings.choose(sealed))
    }

    @Test
    fun keyWipedBySystemIsReportedLostInsteadOfReplaced() {
        assertEquals(KeyOpening.LOST, KeyOpenings.choose(sealed.copy(bioKeyInvalidated = true)))
        assertEquals(KeyOpening.LOST, KeyOpenings.choose(sealed.copy(bioKey = false)))
        assertEquals(KeyOpening.LOST, KeyOpenings.choose(sealed.copy(bioFileValid = false)))
    }

    @Test
    fun interruptedMigrationIsFinishedFromTheLegacyKey() {
        assertEquals(KeyOpening.MIGRATE, KeyOpenings.choose(sealed.copy(bioKey = false, legacyFile = true)))
    }

    @Test
    fun withoutFingerprintSealedKeyIsUnavailableAndPlainKeyOpens() {
        assertEquals(KeyOpening.UNAVAILABLE, KeyOpenings.choose(sealed.copy(canAuthenticate = false)))
        assertEquals(
            KeyOpening.PLAIN,
            KeyOpenings.choose(sealed.copy(canAuthenticate = false, bioFile = false, legacyFile = true)),
        )
    }

    @Test
    fun firstLaunchCreatesAndOldInstallMigrates() {
        val fresh = sealed.copy(bioFile = false, bioFileValid = false, bioKey = false)
        assertEquals(KeyOpening.CREATE, KeyOpenings.choose(fresh))
        assertEquals(KeyOpening.MIGRATE, KeyOpenings.choose(fresh.copy(legacyFile = true)))
    }
}
