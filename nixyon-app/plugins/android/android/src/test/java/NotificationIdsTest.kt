package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Test

class NotificationIdsTest {
    @Test
    fun everyForegroundServiceHasItsOwnNotificationNumber() {
        assertEquals(NotificationIds.all.size, NotificationIds.all.toSet().size)
    }
}
