plugins { id("com.android.application") }

android {
    namespace = "com.reframe.optchat.gpui.android"
    compileSdk = 36
    buildToolsVersion = "36.0.0"
    ndkVersion = "29.0.14206865"
    defaultConfig {
        applicationId = "com.reframe.optchat.gpui.android"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-dev"
        testInstrumentationRunner = "com.reframe.optchat.gpui.android.OptChatDeviceChecks"
        ndk { abiFilters += "arm64-v8a" }
    }
    buildTypes { debug { isDebuggable = true; isJniDebuggable = true } }
    sourceSets["main"].jniLibs.srcDir("../../../../build/android/app/jniLibs")
    packaging { jniLibs { useLegacyPackaging = false } }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    lint { abortOnError = true }
}
