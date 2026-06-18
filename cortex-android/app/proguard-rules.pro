# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.

# Keep JNI methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep Room entities
-keep class nz.co.icb.cortex.android.db.** { *; }

# Keep Gson serialization
-keepattributes Signature
-keepattributes *Annotation*
-dontwarn sun.misc.**
-keep class com.google.gson.** { *; }
-keep class * implements com.google.gson.TypeAdapterFactory
-keep class * implements com.google.gson.JsonSerializer
-keep class * implements com.google.gson.JsonDeserializer

# WebSocket
-keep class org.java_websocket.** { *; }
