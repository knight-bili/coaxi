package app.codeg.android.feature.sessiondetail

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.Base64
import app.codeg.android.core.model.ImageData
import app.codeg.android.core.model.PromptInputBlock
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.util.UUID
import kotlin.math.max

/**
 * A prepared image attachment ready to send: base64 payload + mime type. Mirrors
 * iOS `Attachment` — the bytes are already downscaled/encoded by [AttachmentPrep].
 */
data class Attachment(
    val id: String,
    val base64: String,
    val mimeType: String,
) {
    val promptBlock: PromptInputBlock get() = PromptInputBlock.Image(data = base64, mimeType = mimeType)
    val imageData: ImageData get() = ImageData(data = base64, mimeType = mimeType)
}

/**
 * Downscales + encodes a picked image to keep prompts under Axum's JSON body
 * limit. Port of iOS `AttachmentPrep`: longest side ≤ 1568 px, JPEG quality 0.75,
 * ≤ 4 MB per image. Originals already within limits keep their bytes + sniffed mime.
 */
object AttachmentPrep {
    const val MAX_DIMENSION = 1568
    private const val JPEG_QUALITY = 75
    const val MAX_BYTES = 4_000_000
    const val MAX_TOTAL_BYTES = 1_300_000
    const val MAX_COUNT = 10

    suspend fun fromUri(uri: Uri, resolver: ContentResolver): Attachment? = withContext(Dispatchers.Default) {
        val bytes = runCatching { resolver.openInputStream(uri)?.use { it.readBytes() } }.getOrNull() ?: return@withContext null
        if (bytes.isEmpty()) return@withContext null
        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size) ?: return@withContext null
        val longest = max(bitmap.width, bitmap.height)
        val needsReencode = bytes.size > MAX_BYTES || longest > MAX_DIMENSION
        val (outBytes, mime) = if (needsReencode) {
            val scale = if (longest > MAX_DIMENSION) MAX_DIMENSION.toFloat() / longest else 1f
            val w = (bitmap.width * scale).toInt().coerceAtLeast(1)
            val h = (bitmap.height * scale).toInt().coerceAtLeast(1)
            val scaled = if (scale < 1f) Bitmap.createScaledBitmap(bitmap, w, h, true) else bitmap
            val baos = ByteArrayOutputStream()
            scaled.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, baos)
            baos.toByteArray() to "image/jpeg"
        } else {
            bytes to sniffMime(bytes)
        }
        if (outBytes.isEmpty() || outBytes.size > MAX_BYTES) return@withContext null
        Attachment(
            id = UUID.randomUUID().toString(),
            base64 = Base64.encodeToString(outBytes, Base64.NO_WRAP),
            mimeType = mime,
        )
    }

    /** Sniff a mime type from magic bytes; default to PNG. */
    private fun sniffMime(b: ByteArray): String = when {
        b.size >= 3 && b[0] == 0xFF.toByte() && b[1] == 0xD8.toByte() && b[2] == 0xFF.toByte() -> "image/jpeg"
        b.size >= 4 && b[0] == 0x89.toByte() && b[1] == 0x50.toByte() -> "image/png"
        b.size >= 4 && b[0] == 0x47.toByte() && b[1] == 0x49.toByte() -> "image/gif"
        b.size >= 12 && b[0] == 0x52.toByte() && b[8] == 0x57.toByte() && b[9] == 0x45.toByte() -> "image/webp"
        else -> "image/png"
    }
}
