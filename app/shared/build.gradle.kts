import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlinMultiplatform)
    alias(libs.plugins.androidLibrary)
    alias(libs.plugins.composeMultiplatform)
    alias(libs.plugins.composeCompiler)
    alias(libs.plugins.kotlinSerialization)
}

// Where `cargo build --profile android -p tc-android` puts the engine for this machine: the unit tests load it from there.
val engineDir = rootDir.resolve("../target/android")

// Android only for now; the screens, state and engine API are in `commonMain` so that another target can share them later.
kotlin {
    androidTarget { compilerOptions { jvmTarget.set(JvmTarget.JVM_21) } }

    sourceSets {
        commonMain.dependencies {
            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(compose.ui)
            implementation(libs.coroutines.core)
            implementation(libs.serialization.json)
        }
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}

android {
    namespace = "me.utsob.meghnaad.shared"
    compileSdk = 36
    defaultConfig { minSdk = 24 }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
    testOptions {
        unitTests.all {
            it.systemProperty("java.library.path", engineDir.absolutePath)
            it.systemProperty("tc.engine.dir", engineDir.absolutePath)
        }
    }
}
