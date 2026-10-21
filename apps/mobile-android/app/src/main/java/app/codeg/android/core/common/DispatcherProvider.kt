package app.codeg.android.core.common

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers

/**
 * Injectable dispatchers so repositories/clients can be unit-tested with a test
 * dispatcher instead of the hard-coded [Dispatchers]. Mirrors the structured
 * concurrency boundaries the iOS app gets for free with Swift actors.
 */
interface DispatcherProvider {
    val io: CoroutineDispatcher
    val default: CoroutineDispatcher
    val main: CoroutineDispatcher
}

object DefaultDispatcherProvider : DispatcherProvider {
    override val io: CoroutineDispatcher get() = Dispatchers.IO
    override val default: CoroutineDispatcher get() = Dispatchers.Default
    override val main: CoroutineDispatcher get() = Dispatchers.Main
}
