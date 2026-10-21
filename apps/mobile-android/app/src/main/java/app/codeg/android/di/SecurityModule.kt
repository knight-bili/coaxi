package app.codeg.android.di

import app.codeg.android.core.security.KeystoreSecretStore
import app.codeg.android.core.security.SecretStore
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
