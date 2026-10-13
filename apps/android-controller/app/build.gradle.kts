import java.io.File
import java.nio.file.Files
import java.security.MessageDigest
import java.util.Properties
import groovy.json.JsonOutput
import org.gradle.api.artifacts.component.ModuleComponentIdentifier
import org.gradle.api.artifacts.result.ResolvedArtifactResult
import org.gradle.maven.MavenModule
import org.gradle.maven.MavenPomArtifact

plugins {
    id("com.android.application")
}

android {
    namespace = "dev.webp2p.controllerprobe"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.webp2p.controllerprobe"
        minSdk = 26
        targetSdk = 35
        versionCode = providers.environmentVariable("THEPEER_ANDROID_VERSION_CODE").map { it.toInt() }.orElse(6).get()
        versionName = providers.environmentVariable("THEPEER_ANDROID_VERSION_NAME").orElse("0.6-direct-relay-preview").get()
    }

    // Native forwarding only: no bundled diagnostic site or browser-controller/WASM assets.
    sourceSets.getByName("main").assets.setSrcDirs(emptyList<String>())
    sourceSets.getByName("main").jniLibs.srcDir(rootProject.file("generated/jniLibs"))

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // Embedded preview + local decoder only. No Play Services or remote model download.
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
    implementation("com.google.zxing:core:3.5.3")
}

// Native transport is required by Connect. Reject missing, stale or partial staging.
val verifyNativeProbe by tasks.registering {
    doLast {
        val jniRoot = rootProject.file("generated/jniLibs")
        val receipt = rootProject.file("generated/native.properties")
        check(jniRoot.isDirectory && receipt.isFile) { "Native transport required; run native-build.sh" }
        val artifactName = "arm64-v8a/libcontroller_saorsa_probe.so"
        val actual = jniRoot.walkTopDown().filter { it.isFile }
            .map { it.relativeTo(jniRoot).invariantSeparatorsPath }.toSet()
        check(actual == setOf(artifactName)) { "Partial native staging; run native-build.sh" }
        val properties = Properties().apply { receipt.inputStream().use { load(it) } }
        val sources = setOf("../../Cargo.toml", "../../Cargo.lock",
            "../../crates/tunnel-core/Cargo.toml", "../../crates/tunnel-android/Cargo.toml",
            "../../crates/cli/Cargo.toml", "native-build.sh", "native-stage.mjs",
            "app/src/main/java/dev/webp2p/controllerprobe/NativeSaorsaProbe.java",
            "app/src/main/java/dev/webp2p/controllerprobe/ViewerLaunch.java",
            "app/src/main/java/dev/webp2p/controllerprobe/ViewerScan.java",
            "app/src/main/java/dev/webp2p/controllerprobe/MainActivity.java",
            "app/src/main/java/dev/webp2p/controllerprobe/ConnectionState.java",
            "app/src/main/java/dev/webp2p/controllerprobe/ViewerQrScanner.java",
            "app/src/main/java/dev/webp2p/controllerprobe/RelayQrDecoder.java",
            "app/src/main/java/dev/webp2p/controllerprobe/EmbeddedQrDecoderFactory.java",
            "app/src/main/java/dev/webp2p/controllerprobe/ViewerService.java",
            "app/src/main/java/dev/webp2p/controllerprobe/ConnectionHandoff.java",
            "../../patches/saorsa-transport/prepare-native.sh",
            "../../patches/saorsa-transport/0001-stream-forwarding-policy.patch",
            "../../patches/saorsa-transport/0002-bounded-stream-socket.patch",
            "../../patches/saorsa-transport/0003-client-relay-response-bound.patch",
            "../../patches/saorsa-transport/0004-cancellable-peer-attempt.patch",
            "../../patches/saorsa-transport/0005-peer-handshake-hook.patch") +
            listOf("../../crates/tunnel-core/src", "../../crates/tunnel-android/src").flatMap { directory ->
                rootProject.file(directory).walkTopDown().filter { it.isFile && it.extension == "rs" }
                    .map { it.relativeTo(rootProject.projectDir).invariantSeparatorsPath }.toList()
            }.toSet()
        check(properties.getProperty("transport") == "saorsa-routing-v2") { "Routing native build required; run native-build.sh" }
        check(properties.stringPropertyNames() == setOf("artifact", "transport") + sources.map { "source.$it" }.toSet()) {
            "Native source list changed; run native-build.sh"
        }
        fun verify(file: File, key: String) {
            check(file.isFile && !Files.isSymbolicLink(file.toPath())) { "Invalid native input: $key" }
            val hash = MessageDigest.getInstance("SHA-256").digest(file.readBytes())
                .joinToString("") { "%02x".format(it) }
            check(hash == properties.getProperty(key)) { "Stale native probe: $key; run native-build.sh" }
        }
        verify(jniRoot.resolve(artifactName), "artifact")
        sources.forEach { verify(rootProject.file(it), "source.$it") }
    }
}
tasks.named("preBuild") { dependsOn(verifyNativeProbe) }

// Resolved release dependencies only; copied to an ignored build directory for
// notice collection, never packaged wholesale into a release asset.
tasks.register("collectReleaseDependencies") {
    doLast {
        val configuration = configurations.getByName("releaseRuntimeClasspath")
        val output = layout.buildDirectory.dir("release-dependencies").get().asFile
        output.deleteRecursively()
        val artifacts = output.resolve("artifacts").apply { mkdirs() }
        val poms = output.resolve("poms").apply { mkdirs() }
        fun safe(value: String) = value.replace(Regex("[^A-Za-z0-9_.-]"), "_")
        val report = configuration.resolvedConfiguration.resolvedArtifacts.map { artifact ->
            val id = artifact.moduleVersion.id
            val name = safe("${id.group}-${id.name}-${id.version}-${artifact.file.name}")
            artifact.file.copyTo(artifacts.resolve(name), overwrite = true)
            mapOf("group" to id.group, "name" to id.name, "version" to id.version, "artifact" to name)
        }
        output.resolve("DEPENDENCIES.json").writeText(JsonOutput.prettyPrint(JsonOutput.toJson(report)))
        val components = configuration.incoming.resolutionResult.allComponents
            .map { it.id }.filterIsInstance<ModuleComponentIdentifier>()
        val result = dependencies.createArtifactResolutionQuery().forComponents(components)
            .withArtifacts(MavenModule::class.java, MavenPomArtifact::class.java).execute()
        check(result.components.none { it is org.gradle.api.artifacts.result.UnresolvedComponentResult }) {
            "Unresolved release dependency metadata"
        }
        result.resolvedComponents.forEach { component ->
            val metadata = component.getArtifacts(MavenPomArtifact::class.java)
            check(metadata.isNotEmpty()) { "Missing dependency POM: ${component.id}" }
            metadata.forEach { artifact ->
                check(artifact is ResolvedArtifactResult) { "Unresolved dependency POM: ${component.id}" }
                artifact.file.copyTo(poms.resolve(safe(component.id.displayName) + ".pom"), overwrite = true)
            }
        }
    }
}
