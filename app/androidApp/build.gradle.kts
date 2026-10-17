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
        // The id the Capacitor app had: an update installs over it and keeps its tasks and its connection (the engine's folder
        // and the Keystore vault are the same).
        applicationId = "me.utsob.meghnaad"
        minSdk = 24
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersionName
        // The engine is Rust (crates/android), built for these by scripts/build-android.mjs.
        ndk { abiFilters += (System.getenv("ANDROID_ABIS") ?: "arm64-v8a,x86_64").split(",") }
    }
    // A release is signed when the keystore is given (the Release workflow does): updating an installed app needs the same key
    // every time. Without it the release build is unsigned and cannot be installed.
    val keystore = System.getenv("ANDROID_KEYSTORE_FILE")
    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }
    buildTypes {
        release {
            if (keystore != null) signingConfig = signingConfigs.getByName("release")
            // Unused code and resources are cut (R8). What is looked up by name is kept in proguard-rules.pro.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android.txt"), "proguard-rules.pro")
        }
    }
    // Libraries are packed compressed, not stored whole for mapping: the download is smaller (a phone unpacks them on install).
    packaging { jniLibs { useLegacyPackaging = true } }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
    testOptions { unitTests { isIncludeAndroidResources = true } }
}

kotlin { jvmToolchain(21) }

dependencies {
    implementation(project(":shared"))
    implementation(compose.foundation)
    implementation(libs.activity.compose)
    // The background sync, for the widgets and the reminders.
    implementation(libs.work.runtime)

    testImplementation("junit:junit:4.13.2")
    // Android's own org.json is only a stub on the JVM that runs the unit tests.
    testImplementation("org.json:json:20240303")
    // Real Android graphics on the JVM, so the widgets' pictures can be drawn (and looked at) in a test.
    testImplementation("org.robolectric:robolectric:4.17")
    testImplementation("androidx.test:core:1.6.1")
}
