plugins {
    alias(libs.plugins.androidApplication)
    alias(libs.plugins.kotlinAndroid)
    alias(libs.plugins.composeMultiplatform)
    alias(libs.plugins.composeCompiler)
}

// One version for everything: the root package.json's (scripts/version.mjs), unless the release build passes its own.
val rootVersion = Regex("\"version\"\\s*:\\s*\"(\\d+)\\.(\\d+)\\.(\\d+)([^\"]*)\"")
    .find(rootDir.resolve("../package.json").readText())!!.groupValues
val appVersionName = System.getenv("ANDROID_VERSION_NAME") ?: "${rootVersion[1]}.${rootVersion[2]}.${rootVersion[3]}${rootVersion[4]}"
val appVersionCode = System.getenv("ANDROID_VERSION_CODE")?.toInt()
    ?: (rootVersion[1].toInt() * 1_000_000 + rootVersion[2].toInt() * 1_000 + rootVersion[3].toInt())

android {
    namespace = "me.utsob.meghnaad.app"
    compileSdk = 36
    defaultConfig {
        // Its own id until it replaces the Capacitor app, so both can be installed side by side.
        applicationId = "me.utsob.meghnaad.next"
        minSdk = 24
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersionName
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
    sourceSets["main"].jniLibs.srcDir(rootDir.resolve("../android/app/src/main/jniLibs"))
}

kotlin { jvmToolchain(21) }

dependencies {
    implementation(project(":shared"))
    implementation(compose.foundation)
    implementation(libs.activity.compose)
}
