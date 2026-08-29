/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * Parts of this file are derived from SDL 2's Android project template, which
 * has a different license. Please see vendor/SDL/LICENSE.txt for details.
 */
import org.gradle.nativeplatform.platform.internal.DefaultNativePlatform

plugins {
    id("com.android.application") version("8.10.1")
    id("com.github.willir.rust.cargo-ndk-android") version("0.3.4")
    id("org.jetbrains.kotlin.android") version("2.0.21")
}

fun runTapHLEVersionTool(wantBranding: Boolean): String {
    val output = providers.exec {
        commandLine("cargo", "run", "--package", "tapHLE_version")
        if (wantBranding) {
            args("--", "--branding")
        }
    }.standardOutput.asText.get().trim()

    return output
}

fun getTapHLEBranding(): String {
    return runTapHLEVersionTool(/* wantBranding: */ true)
}

fun getTapHLEVersionName(): String {
    return runTapHLEVersionTool(/* wantBranding: */ false)
}

fun join(prefix: String, separator: String, branding: String): String {
    return if (branding.isEmpty()) prefix else prefix + separator + branding
}

android {
    ndkVersion = "25.2.9519653"
    compileSdk = 31
    buildFeatures {
        buildConfig = true
    }
    defaultConfig {
        val branding = getTapHLEBranding()
        applicationId = "org.taphle.android"
        if (!branding.isEmpty()) {
            applicationIdSuffix = branding.lowercase()
        }
        resValue("string", "app_name", join("tapHLE", " ", branding))
        buildConfigField("String", "APP_NAME", "\"${join("tapHLE", " ", branding)}\"")
        manifestPlaceholders["icon"] = join("@drawable/icon", "_", branding.lowercase())
        buildConfigField("int", "APP_ICON", join("R.drawable.icon", "_", branding.lowercase()))
        versionName = join(getTapHLEVersionName(), " ", branding)

        minSdk = 21 // first version with AArch64
        targetSdk = 31
        externalNativeBuild {
            ndkBuild {
                arguments("APP_PLATFORM=android-21")
                // abiFilters 'armeabi-v7a', 'arm64-v8a', 'x86', 'x86_64'
                // Dynarmic supports both 64-bit Android ABIs. Keeping x86_64
                // in development products lets the same APK run on the
                // project's Cuttlefish validation device.
                // Make sure this matches the cargoNdk targets below.
                abiFilters("arm64-v8a", "x86_64")
            }
        }
    }
    // The target JVM version must be the same for Java and Kotlin.
    compileOptions {
        sourceCompatibility(JavaVersion.VERSION_11)
        targetCompatibility(JavaVersion.VERSION_11)
    }
    kotlinOptions {
        jvmTarget = "11"
    }
    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("debug")
            isMinifyEnabled = false
            isDebuggable = true // allow use of ADB to manage files, etc
        }
        debug {
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("**/*.so")
            }
            isDebuggable = true
            isJniDebuggable = true
        }
    }

    applicationVariants.all {
        val variantName = name.replaceFirstChar { char ->
            if (char.isLowerCase()) char.titlecase() else char.toString()
        }
        tasks.named("merge${variantName}Assets").configure {
            dependsOn("externalNativeBuild${variantName}")
        }
    }

    sourceSets {
        getByName("main") {
            java.srcDir("${rootDir.parentFile.parentFile}/vendor/SDL/android-project/app/src/main/java")
        }
    }

    if (!project.hasProperty("EXCLUDE_NATIVE_LIBS")) {
        sourceSets {
            getByName("main") {
                jniLibs.srcDir("${projectDir}/jniLibs")
            }
        }
        externalNativeBuild {
            ndkBuild {
                path("jni/Android.mk")
            }
        }
    }

    lint {
        abortOnError = false
    }
    namespace = "org.taphle.android"
}

cargoNdk {
    // Make sure this matches the android abiFilters above.
    targets = arrayListOf("arm64", "x86_64")
    module = "../.."
    librariesNames = arrayListOf("libtapHLE_gui.so", "libSDL2.so", "libc++_shared.so")
    extraCargoEnv = mapOf(
        "ANDROID_NDK" to android.ndkDirectory.toString(),
        "ANDROID_NDK_HOME" to android.ndkDirectory.toString(),
    )

    if (DefaultNativePlatform.host().operatingSystem.isWindows) {
        val binPath =
            android.ndkDirectory.toPath().resolve("toolchains/llvm/prebuilt/windows-x86_64/bin")
        val clangPath = binPath.resolve("clang.exe")
        val clangXXPath = binPath.resolve("clang++.exe")

        if (!clangPath.toFile().exists()) {
            throw GradleException("NDK clang compiler not found at expected location: $clangPath")
        }
        if (!clangXXPath.toFile().exists()) {
            throw GradleException("NDK clang++ compiler not found at expected location: $clangXXPath")
        }

        extraCargoEnv.putAll(
            mapOf(
                "CC" to clangPath.toString(),
                "CXX" to clangXXPath.toString(),
                // The default generator on Windows (Visual Studio) does not respect
                // the CC and CXX environment variables. Using Ninja ensures that
                // the specified compilers are used
                "CMAKE_GENERATOR" to "Ninja",
            )
        )
    }
    // The default feature, "static", makes us use static linking for SDL2 and OpenAL Soft.
    // For Android, we need dynamic linking for SDL2, but static linking for OpenAL Soft.
    extraCargoBuildArguments = arrayListOf(
        "--package",
        "tapHLE_gui",
        "--lib",
        "--no-default-features",
        "--features",
        "android"
    )
}

dependencies {
    implementation(fileTree("libs") {
        include("*.jar")
    })
}
