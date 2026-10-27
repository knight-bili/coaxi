package app.coaxi.android.core.network

import java.io.InputStream
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

private const val GENERATED_BYTE = 0x5a

class CoaxiUploadSourceTest {
    @Test
    fun emptyInputReturnsEmptyBytes() {
        val input = GeneratedInputStream(0)

        assertArrayEquals(byteArrayOf(), readBoundedUploadBytes(input))
        assertEquals(0L, input.bytesRead)
    }

    @Test
    fun shortReadsAreCollectedCompletely() {
        val input = GeneratedInputStream(totalBytes = 19, maxChunk = 3)

        assertArrayEquals(ByteArray(19) { GENERATED_BYTE.toByte() }, readBoundedUploadBytes(input))
        assertEquals(19L, input.bytesRead)
    }

    @Test
    fun exactlyTwentyMiBIsAccepted() {
        val input = GeneratedInputStream(COAXI_UPLOAD_MAX_BYTES.toLong())

        val bytes = readBoundedUploadBytes(input)

        assertEquals(COAXI_UPLOAD_MAX_BYTES, bytes.size)
        assertEquals(GENERATED_BYTE.toByte(), bytes.first())
        assertEquals(GENERATED_BYTE.toByte(), bytes.last())
        assertEquals(COAXI_UPLOAD_MAX_BYTES.toLong(), input.bytesRead)
    }

    @Test
    fun inputOverTwentyMiBIsRejected() {
        val input = GeneratedInputStream(COAXI_UPLOAD_MAX_BYTES.toLong() + 8)

        assertThrows(IllegalArgumentException::class.java) {
            readBoundedUploadBytes(input)
        }
        assertEquals(COAXI_UPLOAD_MAX_BYTES.toLong() + 1, input.bytesRead)
    }

    @Test
    fun growingInputIsStoppedAfterOneBytePastLimit() {
        val input = GeneratedInputStream(Long.MAX_VALUE, maxChunk = 32 * 1024)

        assertThrows(IllegalArgumentException::class.java) {
            readBoundedUploadBytes(input)
        }
        assertEquals(COAXI_UPLOAD_MAX_BYTES.toLong() + 1, input.bytesRead)
    }

    private class GeneratedInputStream(
        private val totalBytes: Long,
        private val maxChunk: Int = 64 * 1024
    ) : InputStream() {
        var bytesRead = 0L
            private set

        override fun read(): Int {
            if (bytesRead >= totalBytes) return -1
            bytesRead++
            return GENERATED_BYTE
        }

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
            if (length == 0) return 0
            if (bytesRead >= totalBytes) return -1
            val remaining = (totalBytes - bytesRead).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
            val count = minOf(length, maxChunk, remaining)
            java.util.Arrays.fill(buffer, offset, offset + count, GENERATED_BYTE.toByte())
            bytesRead += count
            return count
        }
    }
}
