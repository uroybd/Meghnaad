# The Rust engine's functions are found by name from the library.
-keep class me.utsob.meghnaad.NativeEngine { *; }
-keepclasseswithmembernames class * { native <methods>; }
# The widgets and screens are named in the manifest (which keeps them); their layouts find views by id.
