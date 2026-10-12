# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# --- Meghnaad ---
# The Rust engine's functions are found by name from the library.
-keep class me.utsob.meghnaad.NativeEngine { *; }
-keepclasseswithmembernames class * { native <methods>; }
# Capacitor finds the plugin and its methods by their annotations.
-keep @com.getcapacitor.annotation.CapacitorPlugin public class * { @com.getcapacitor.annotation.PluginMethod public <methods>; }
# The widgets and screens are named in the manifest; their layouts find views by id.
-keep class me.utsob.meghnaad.** extends android.app.Activity
-keep class me.utsob.meghnaad.** extends android.appwidget.AppWidgetProvider
-keep class me.utsob.meghnaad.** extends android.widget.RemoteViewsService
-keep class me.utsob.meghnaad.** extends androidx.work.ListenableWorker
