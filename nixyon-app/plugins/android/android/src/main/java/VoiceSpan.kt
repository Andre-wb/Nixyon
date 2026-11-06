package dev.demo.nixyon.speech

// Отрезок голоса одного говорящего от нейросегментации, обрезанный по куску VAD,
// с эмбеддингом голоса; время в секундах от начала записи.
class VoiceSpan(val start: Double, val end: Double, val embedding: FloatArray)
