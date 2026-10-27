# Scaffold milestone: R8 is disabled (isMinifyEnabled = false), so these rules
# are not yet exercised. Captured here so they are ready when shrinking is turned
# on for release builds.

# kotlinx.serialization — keep @Serializable metadata and generated serializers.
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class **$$serializer { *; }
-keep,includedescriptorclasses class app.coaxi.android.**$$serializer { *; }
-keepclassmembers class app.coaxi.android.** {
    *** Companion;
}
-keepclasseswithmembers class app.coaxi.android.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# Ktor / OkHttp
-dontwarn org.slf4j.**
-dontwarn io.ktor.**
-keep class io.ktor.** { *; }
