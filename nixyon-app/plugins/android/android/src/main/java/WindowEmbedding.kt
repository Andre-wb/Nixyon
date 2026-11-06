package dev.demo.nixyon.speech

// Эмбеддинг голоса на окне внутри куска речи; время в секундах от начала записи.
class WindowEmbedding(val start: Double, val end: Double, val embedding: FloatArray)
