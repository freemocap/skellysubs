package com.freemocap.skellyspeak

import java.io.File
import java.nio.file.Files
import java.util.zip.ZipFile
import java.util.zip.ZipOutputStream
import org.junit.Assert.*
import org.junit.Test

class DiagnosticArchiveTest {
    @Test fun archiveIncludesPreviousRunsAndExcludesContent() {
        val root = Files.createTempDirectory("diagnostics-test").toFile()
        try {
            for (name in listOf("native-0c33be7e-3663-4e75-b4a7-079d6a186064", "native-d6d8e4d5-f4a5-4fab-befc-688d2b5bf763")) {
                val run = File(root, name).apply { mkdir() }
                File(run, "diagnostics.jsonl").writeText("{\"code\":\"microphone_failed\"}\n")
                File(run, "native.jsonl").writeText("{\"requestId\":\"safe-request-id\"}\n")
                File(run, "native-2.manifest.json").writeText("{}")
                File(run, "credentials.json").writeText("SECRET")
                File(run, "recording.wav").writeText("AUDIO")
            }
            File(root, "execution-diagnostics.jsonl").writeText("{\"request_id\":\"req-speech-failed\"}\n")
            File(root, "skellyspeak.sqlite3").writeText("PRIVATE CONVERSATION")
            val files = DiagnosticArchive.files(root)
            assertEquals(7, files.size)
            val out = File(root, "bundle.zip")
            ZipOutputStream(out.outputStream()).use { DiagnosticArchive.copyLogs(root, files, it) }
            ZipFile(out).use { zip ->
                assertEquals(7, zip.size())
                val text = zip.entries().asSequence().joinToString { zip.getInputStream(it).reader().readText() }
                assertTrue(text.contains("safe-request-id"))
                assertTrue(text.contains("req-speech-failed"))
                assertNotNull(zip.getEntry("logs/execution-diagnostics.jsonl"))
                assertFalse(text.contains("SECRET"))
                assertFalse(text.contains("PRIVATE CONVERSATION"))
                assertFalse(text.contains("AUDIO"))
            }
        } finally { root.deleteRecursively() }
    }

    @Test fun refusesLinkedLogFiles() {
        val root = Files.createTempDirectory("diagnostics-test").toFile()
        try {
            File(root, "secret").writeText("PRIVATE")
            Files.createSymbolicLink(File(root, "native.jsonl").toPath(), File(root, "secret").toPath())
            assertThrows(IllegalArgumentException::class.java) { DiagnosticArchive.files(root) }
        } finally { root.deleteRecursively() }
    }

    @Test fun graphicsFilteringDropsArbitraryContent() {
        assertNull(DiagnosticArchive.graphicsCode("Authorization: Bearer SECRET"))
        assertNull(DiagnosticArchive.graphicsCode("Transcript: PRIVATE"))
        assertEquals("vulkan_device_lost", DiagnosticArchive.graphicsCode("Failed vulkan call. Error: -4, PRIVATE"))
        assertEquals("skia_context_lost", DiagnosticArchive.graphicsCode("SharedContextState context lost via Skia."))
    }
}
