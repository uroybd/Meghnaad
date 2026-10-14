import org.jetbrains.compose.reload.gradle.ComposeHotRun

// A window for looking at the screens live: `./gradlew :desktopApp:hotRun --auto` (it reloads what is saved). A preview,
// not an app: it keeps its tasks in build/preview-data, saves no keys, and is not shipped.
plugins {
    alias(libs.plugins.kotlinJvm)
    alias(libs.plugins.composeMultiplatform)
    alias(libs.plugins.composeCompiler)
    alias(libs.plugins.composeHotReload)
}

// Where `cargo build --profile android -p tc-android` puts the engine for this machine.
val engineDir = rootDir.resolve("../target/android")

kotlin { jvmToolchain(21) }

dependencies {
    implementation(project(":shared"))
    implementation(compose.desktop.currentOs)
    implementation(libs.coroutines.swing)
}

compose.desktop {
    application {
        mainClass = "me.utsob.meghnaad.app.MainKt"
        jvmArgs += "-Djava.library.path=${engineDir.absolutePath}"
        jvmArgs += "-Dmeghnaad.data=${layout.buildDirectory.dir("preview-data").get().asFile.absolutePath}"
    }
}

tasks.withType<ComposeHotRun>().configureEach {
    mainClass.set("me.utsob.meghnaad.app.MainKt")
    jvmArgs("-Djava.library.path=${engineDir.absolutePath}")
    jvmArgs("-Dmeghnaad.data=${layout.buildDirectory.dir("preview-data").get().asFile.absolutePath}")
}
