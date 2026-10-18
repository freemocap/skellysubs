package com.freemocap.skellyspeak

import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/** Only files authored by the structured diagnostic sink may leave app storage. */
internal object DiagnosticArchive {
    private val runName = Regex("native-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
    private val manifestName = Regex("native-[0-9]+\\.manifest\\.json")
    fun isLog(name: String) = name == "diagnostics.jsonl" || name == "native.jsonl" || name == "execution-diagnostics.jsonl" || manifestName.matches(name)

    fun files(root: File): List<File> {
        require(root.isDirectory) { "Log directory unavailable" }
        val entries = root.listFiles() ?: error("Cannot list logs")
        val files = entries.sortedBy { it.name }.flatMap { entry ->
            require(entry.canonicalFile == File(root.canonicalFile, entry.name)) { "Linked log entry refused" }
            when {
                entry.isFile && isLog(entry.name) -> listOf(entry)
                entry.isDirectory && runName.matches(entry.name) -> {
                    (entry.listFiles() ?: error("Cannot list log run")).sortedBy { it.name }.filter { isLog(it.name) }.onEach {
                        require(it.isFile && it.canonicalFile == File(entry.canonicalFile, it.name)) { "Invalid log file" }
                    }
                }
                else -> emptyList()
            }
        }
        require(files.isNotEmpty()) { "No diagnostic logs available" }
        return files
    }

    fun copyLogs(root: File, files: List<File>, zip: ZipOutputStream) {
        files.forEach { file ->
            zip.putNextEntry(ZipEntry("logs/" + file.relativeTo(root).invariantSeparatorsPath))
            // Snapshot each file's size: ongoing recording must not grow an export forever.
            val length = file.length()
            file.inputStream().use { input ->
                var remaining = length
                val buffer = ByteArray(32 * 1024)
                while (remaining > 0) {
                    val read = input.read(buffer, 0, minOf(buffer.size.toLong(), remaining).toInt())
                    check(read > 0) { "Log changed while sharing" }
                    zip.write(buffer, 0, read)
                    remaining -= read
                }
            }
            zip.closeEntry()
        }
    }

    /** Never retain raw logcat text; recognize fixed graphics failures only. */
    fun graphicsCode(line: String): String? = when {
        "Failed vulkan call. Error: -4," in line -> "vulkan_device_lost"
        "SharedContextState context lost via Skia." in line -> "skia_context_lost"
        "SharedImageStub: context already lost" in line -> "shared_image_context_lost"
        "SharedImageManager::ProduceSkia: Trying to Produce a Skia representation from a non-existent mailbox." in line -> "missing_image_mailbox"
        "Unable to initialize SkSurface" in line -> "surface_initialization_failed"
        else -> null
    }
}
