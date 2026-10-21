package app.codeg.android.core.network

/** Maps a thrown error (typically an [ApiError]) to a short user-facing message. */
fun Throwable.displayMessage(): String = when (this) {
    is ApiError.Unauthorized -> "Invalid or missing token"
    is ApiError.TurnInProgress -> "A turn is already in progress"
    is ApiError.Transport -> detail
    is ApiError.Server -> serverMessage
    is ApiError.Decoding -> "Unexpected response from the server"
    else -> message ?: "Something went wrong"
}
