package app.codeg.android.feature.sessiondetail.rendering

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.ImageData

/** Decodes a base64 [ImageData] and renders it (max height 320dp), with an optional caption. */
@Composable
fun InlineImage(image: ImageData, caption: String?, modifier: Modifier = Modifier) {
    val colors = CodegTheme.colors
    val bitmap = remember(image.data) {
        runCatching {
            val bytes = Base64.decode(image.data, Base64.DEFAULT)
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
        }.getOrNull()
    }
    Column(modifier.fillMaxWidth()) {
        if (bitmap != null) {
            Image(
                bitmap = bitmap,
                contentDescription = caption,
                contentScale = ContentScale.Fit,
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 320.dp)
                    .clip(RoundedCornerShape(10.dp)),
            )
        } else {
            Text(
                "[image]",
                fontSize = 12.sp,
                color = colors.textTertiary,
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .background(colors.codeSurface)
                    .padding(horizontal = 10.dp, vertical = 8.dp),
            )
        }
        if (!caption.isNullOrBlank()) {
            Text(
                caption,
                fontSize = 11.sp,
                fontStyle = FontStyle.Italic,
                color = colors.textTertiary,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
}
