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

// Android is the app. The desktop target is only a preview for looking at the screens live with hot reload (`desktopApp`):
// it is not shipped, and keeps no secrets. The screens, state and engine API are in `commonMain`; what both read from the
// engine's library is in `jvmCommonMain`.
kotlin {
    androidTarget { compilerOptions { jvmTarget.set(JvmTarget.JVM_21) } }
    jvm("desktop")

    applyDefaultHierarchyTemplate {
        common { group("jvmCommon") { withAndroidTarget(); withJvm() } }
    }

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
        val desktopMain by getting
        desktopMain.dependencies { implementation(libs.coroutines.swing) }
    }

    tasks.named<Test>("desktopTest") {
        systemProperty("java.library.path", engineDir.absolutePath)
        systemProperty("tc.engine.dir", engineDir.absolutePath)
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
