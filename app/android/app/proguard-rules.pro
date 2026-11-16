# ProGuard Rules for Universal AI Assistant (Offline-First Flutter + Rust C-ABI)

# Keep Flutter engine and embedding classes
-keep class io.flutter.** { *; }
-dontwarn io.flutter.**
-keep class io.flutter.plugin.** { *; }
-keep class io.flutter.util.** { *; }
-keep class io.flutter.view.** { *; }

# Keep application package, foreground service, and accessibility bridge
-keep class com.aissistant.aissistant.** { *; }
-dontwarn com.aissistant.aissistant.**

# Keep all native C/C++ / Rust JNI method entrypoints
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep annotations, signatures, and reflection attributes
-keepattributes *Annotation*,Signature,InnerClasses,EnclosingMethod,SourceFile,LineNumberTable

# Keep @Keep annotated classes and members
-keep @androidx.annotation.Keep class * { *; }
-keepclassmembers class * {
    @androidx.annotation.Keep *;
}

# Keep Kotlin metadata and suppress standard third-party warnings
-dontwarn kotlin.**
-dontwarn kotlinx.**
