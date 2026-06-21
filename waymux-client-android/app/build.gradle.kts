plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "app.appthere.waymux"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.appthere.waymux"
        minSdk = 29
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    // The Rust .so files are produced into this directory by `cargoNdkBuild`.
    sourceSets["main"].jniLibs.srcDir(layout.projectDirectory.dir("src/main/jniLibs"))
}

// --- Rust native library build via cargo-ndk -------------------------------
// Prerequisites (see README.md):
//   rustup target add aarch64-linux-android x86_64-linux-android
//   cargo install cargo-ndk
//   Android NDK installed and ANDROID_NDK_HOME set.
val rustAbis = listOf("arm64-v8a", "x86_64")

// The Cargo workspace root is the parent of this Android project directory.
val workspaceRoot = rootProject.projectDir.parentFile

val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuild") {
    group = "rust"
    description = "Builds libwaymux_client.so for Android via cargo-ndk."
    workingDir = workspaceRoot
    val outDir = layout.projectDirectory.dir("src/main/jniLibs").asFile.absolutePath
    val command = mutableListOf("cargo", "ndk")
    rustAbis.forEach { command += listOf("-t", it) }
    command += listOf("-o", outDir, "build", "--release", "-p", "waymux-client")
    commandLine = command
}

tasks.named("preBuild") { dependsOn(cargoNdkBuild) }

dependencies {
    // Intentionally no AndroidX/Material dependencies: the app uses a platform
    // Activity + fullscreen theme and delegates all logic to the Rust library.
}
