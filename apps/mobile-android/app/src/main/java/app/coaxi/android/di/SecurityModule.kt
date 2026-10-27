package app.coaxi.android.di

import app.coaxi.android.core.security.KeystoreSecretStore
import app.coaxi.android.core.security.SecretStore
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class SecurityModule {

    @Binds
    @Singleton
    abstract fun bindSecretStore(impl: KeystoreSecretStore): SecretStore
}
