package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Test

class ChunkTest {
    @Test
    fun chunksCoverTheFileWithoutGapsAndTheLastOneIsShorter() {
        val size = Chunk.SIZE * 2 + 5
        val chunks = Chunk.of(3, size)
        assertEquals(3, chunks.size)
        assertEquals(listOf(0L, Chunk.SIZE, Chunk.SIZE * 2), chunks.map { it.start })
        assertEquals(size - 1, chunks.last().end)
        assertEquals(5L, chunks.last().size)
        assertEquals(size, chunks.sumOf { it.size })
        assertEquals(setOf(3), chunks.map { it.file }.toSet())
    }

    @Test
    fun aSmallFileIsOneChunk() {
        assertEquals(1, Chunk.count(1))
        assertEquals(1, Chunk.count(Chunk.SIZE))
        assertEquals(2, Chunk.count(Chunk.SIZE + 1))
    }
}
