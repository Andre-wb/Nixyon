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

    private val plain = sealed.copy(bioFile = false, bioFileValid = false, bioKey = false, legacyFile = true)

    @Test
    fun intactSealedKeyAsksForTheFinger() {
        assertEquals(KeyOpening.BIOMETRIC, KeyOpenings.choose(sealed))
        assertEquals(KeyOpening.BIOMETRIC, KeyOpenings.choose(sealed.copy(legacyFile = true)))
    }

    @Test
    fun keyWipedBySystemIsReportedLostInsteadOfReplaced() {
        assertEquals(KeyOpening.LOST, KeyOpenings.choose(sealed.copy(bioKeyInvalidated = true)))
        assertEquals(KeyOpening.LOST, KeyOpenings.choose(sealed.copy(bioKey = false)))
        assertEquals(KeyOpening.LOST, KeyOpenings.choose(sealed.copy(bioFileValid = false)))
    }

    @Test
    fun brokenSealedWrapperIsResealedFromTheLegacyKey() {
        assertEquals(KeyOpening.MIGRATE, KeyOpenings.choose(sealed.copy(bioKey = false, legacyFile = true)))
    }

    @Test
    fun withoutFingerprintSealedKeyIsUnavailable() {
        assertEquals(KeyOpening.UNAVAILABLE, KeyOpenings.choose(sealed.copy(canAuthenticate = false)))
    }

    @Test
    fun firstLaunchOpensWithoutFingerprintEvenWhenPhoneHasOne() {
        val fresh = plain.copy(legacyFile = false)
        assertEquals(KeyOpening.PLAIN, KeyOpenings.choose(fresh))
        assertEquals(KeyOpening.PLAIN, KeyOpenings.choose(fresh.copy(canAuthenticate = false)))
    }

    @Test
    fun plainKeyIsNotMovedUnderFingerprintByItself() {
        assertEquals(KeyOpening.PLAIN, KeyOpenings.choose(plain))
        assertEquals(KeyOpening.PLAIN, KeyOpenings.choose(plain.copy(canAuthenticate = false)))
    }
}
