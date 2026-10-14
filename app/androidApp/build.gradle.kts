plugins {
    alias(libs.plugins.androidApplication)
    alias(libs.plugins.kotlinAndroid)
    alias(libs.plugins.composeMultiplatform)
    alias(libs.plugins.composeCompiler)
}

android {
    namespace = "me.utsob.meghnaad.app"
    compileSdk = 36
    defaultConfig {
        // Its own id until it replaces the Capacitor app, so both can be installed side by side.
        applicationId = "me.utsob.meghnaad.next"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.1"
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
