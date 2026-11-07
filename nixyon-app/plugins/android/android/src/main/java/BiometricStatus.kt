package dev.demo.nixyon.speech

// Вход по отпечатку: включён ли он и есть ли на телефоне отпечаток, чтобы его включить.
data class BiometricStatus(val enabled: Boolean, val available: Boolean)
