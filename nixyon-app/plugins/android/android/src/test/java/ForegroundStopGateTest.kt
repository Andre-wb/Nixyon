package dev.demo.nixyon.speech

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ForegroundStopGateTest {
    @Test
    fun aStopBeforeStartForegroundIsDeferredAndTheServiceStopsItselfAfterwards() {
        val gate = ForegroundStopGate()
        gate.starting()
        assertFalse("stopService до startForeground роняет приложение", gate.requestStop())
        assertTrue(gate.enteredForeground())
    }

    @Test
    fun aStopAfterStartForegroundStopsTheServiceAtOnce() {
        val gate = ForegroundStopGate()
        gate.starting()
        assertFalse(gate.enteredForeground())
        assertTrue(gate.requestStop())
    }

    @Test
    fun aNewStartForgetsTheStopOfThePreviousRun() {
        val gate = ForegroundStopGate()
        gate.starting()
        gate.requestStop()
        gate.destroyed()
        gate.starting()
        assertFalse(gate.enteredForeground())
    }

    @Test
    fun afterTheServiceIsGoneAStopDoesNotCallStopService() {
        val gate = ForegroundStopGate()
        gate.starting()
        gate.enteredForeground()
        gate.destroyed()
        assertFalse(gate.requestStop())
    }
}
