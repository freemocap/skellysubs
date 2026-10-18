package com.freemocap.skellyspeak

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.content.pm.LabeledIntent
import android.os.Build
import android.os.Process
import android.webkit.WebView
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import androidx.activity.result.ActivityResult
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import org.json.JSONArray
import org.json.JSONObject

@InvokeArg
class DiagnosticShareArgs {
    lateinit var root: String
    lateinit var nativeVersion: String
}

@TauriPlugin
class DiagnosticSharePlugin(private val activity: Activity) : Plugin(activity) {
    private val busy = AtomicBoolean(false)
    private val privacy by lazy { DiagnosticPrivacy(activity) }

    private var pendingArchive: File? = null

    @Command
    fun save(invoke: Invoke) = prepare(invoke, true)

    @Command
    fun share(invoke: Invoke) = prepare(invoke, false)

    private fun prepare(invoke: Invoke, save: Boolean) {
        if (!busy.compareAndSet(false, true)) {
            invoke.reject("Log sharing is already in progress.", "busy")
            return
        }
        // Compression and filesystem reads must never block recording or the UI thread.
        Thread {
            var archive: File? = null
            var stage = "locate_logs"
            try {
                val args = invoke.parseArgs(DiagnosticShareArgs::class.java)
                val root = File(args.root).canonicalFile
                require(root == File(activity.applicationInfo.dataDir, "logs").canonicalFile)
                val logs = DiagnosticArchive.files(root)
                stage = "create_archive"
                val directory = File(activity.cacheDir, "diagnostic-shares")
                check(directory.isDirectory || directory.mkdirs())
                // Unique files keep a later share from changing an earlier recipient's attachment.
                archive = File.createTempFile("skellyspeak-logs-", ".zip", directory)
                stage = "collect_metadata"
                val graphics = graphicsSnapshot()
                val manifest = JSONObject()
                    .put("formatVersion", 1)
                    .put("createdAtMs", System.currentTimeMillis())
                    .put("appVersion", activity.packageManager.getPackageInfo(activity.packageName, 0).versionName)
                    .put("nativeVersion", args.nativeVersion)
                    .put("androidVersion", Build.VERSION.RELEASE)
                    .put("deviceModel", Build.MODEL)
                    .put("webViewVersion", if (Build.VERSION.SDK_INT >= 26) WebView.getCurrentWebViewPackage()?.versionName else JSONObject.NULL)
                    .put("rendering", "software_activity")
                    .put("logFileCount", logs.size)
                    .put("content", "Structured app diagnostics and redacted execution history; no conversation database, audio, credentials or raw system log.")
                    .put("snapshot", "All retained diagnostic runs. Active files may end with an incomplete final record.")
                    .put("graphics", graphics)
                stage = "write_archive"
                ZipOutputStream(archive.outputStream().buffered()).use { zip ->
                    zip.putNextEntry(ZipEntry("manifest.json"))
                    zip.write(manifest.toString(2).toByteArray(Charsets.UTF_8))
                    zip.closeEntry()
                    DiagnosticArchive.copyLogs(root, logs, zip)
                }
                val ready = archive
                activity.runOnUiThread {
                    try {
                        if (save) {
                            pendingArchive = ready
                            val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                                addCategory(Intent.CATEGORY_OPENABLE)
                                type = "application/zip"
                                putExtra(Intent.EXTRA_TITLE, ready.name)
                            }
                            startActivityForResult(invoke, intent, "saveResult")
                        } else {
                            val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", ready)
                            val intent = Intent(Intent.ACTION_SEND).apply {
                                type = "application/zip"
                                putExtra(Intent.EXTRA_STREAM, uri)
                                clipData = ClipData.newRawUri("SkellySpeak logs", uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }
                            val saveIntent = Intent(activity, DiagnosticSaveActivity::class.java).apply {
                                data = uri
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }
                            val chooser = Intent.createChooser(intent, "Share SkellySpeak logs").apply {
                                putExtra(Intent.EXTRA_INITIAL_INTENTS, arrayOf(
                                    LabeledIntent(saveIntent, activity.packageName, "Save logs", android.R.drawable.ic_menu_save)
                                ))
                            }
                            activity.startActivity(chooser)
                            busy.set(false)
                            invoke.resolve()
                        }
                    } catch (error: Exception) {
                        if (save) { pendingArchive = null; ready.delete() }
                        busy.set(false)
                        reject(invoke, "open_export_destination", error)
                    }
                }
            } catch (error: Exception) {
                archive?.delete()
                busy.set(false)
                reject(invoke, stage, error)
            }
        }.start()
    }

    @ActivityCallback
    fun saveResult(invoke: Invoke, result: ActivityResult) {
        val archive = pendingArchive
        pendingArchive = null
        if (result.resultCode != Activity.RESULT_OK) {
            archive?.delete()
            busy.set(false)
            invoke.resolve() // Cancellation is not a saved file.
            return
        }
        Thread {
            try {
                checkNotNull(archive) { "Prepared archive unavailable" }
                val uri = checkNotNull(result.data?.data) { "Document destination unavailable" }
                val output = checkNotNull(activity.contentResolver.openOutputStream(uri, "wt")) { "Document output unavailable" }
                output.use { destination -> archive.inputStream().use { source -> source.copyTo(destination) } }
                invoke.resolveObject(archive.name)
            } catch (error: Exception) {
                reject(invoke, "save_document", error)
            } finally {
                archive?.delete()
                busy.set(false)
            }
        }.start()
    }

    /** Redact sensitive spans while retaining explanations, types, stages and OS causes. */
    private fun failureDetails(error: Exception): String {
        val causes = generateSequence(error as Throwable?) { it.cause }.take(8).map { cause ->
            val errno = (cause as? android.system.ErrnoException)?.errno
            "${cause.javaClass.simpleName}${if (errno != null) " errno=$errno" else ""}: ${privacy.scrub(cause.message ?: "No exception message")}"
        }.toList()
        return causes.joinToString(" caused by ")
    }
    private fun reject(invoke: Invoke, stage: String, error: Exception) {
        invoke.reject("Diagnostic export failed at $stage: ${failureDetails(error)}", stage)
    }

    private fun graphicsSnapshot(): JSONObject {
        val result = JSONObject().put("scope", "Recent app-UID logcat buffer; fixed graphics codes only")
            .put("rawContentOmitted", true).put("maxInputLines", 4000)
        val executor = Executors.newSingleThreadExecutor()
        var process: java.lang.Process? = null
        try {
            process = ProcessBuilder("/system/bin/logcat", "-d", "-v", "threadtime", "--uid=${Process.myUid()}", "-t", "4000")
                .redirectErrorStream(true).start()
            val running = process
            val read = executor.submit<JSONArray> {
                val records = JSONArray()
                running.inputStream.bufferedReader().useLines { lines ->
                    lines.forEach { line ->
                        val code = DiagnosticArchive.graphicsCode(line)
                        val header = Regex("^(\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}\\.\\d{3})\\s+(\\d+)\\s+(\\d+)\\s+[VDIWEF]\\s+(chromium|RustStdoutStderr):").find(line)
                        if (code != null && header != null) {
                            records.put(JSONObject().put("time", header.groupValues[1])
                                .put("pid", header.groupValues[2].toLong()).put("code", code))
                        }
                    }
                }
                records
            }
            result.put("records", read.get(5, TimeUnit.SECONDS))
            // Process.waitFor(timeout, unit) requires API 26; the app supports API 24.
            val deadline = android.os.SystemClock.elapsedRealtime() + 1000
            var exitCode: Int? = null
            while (exitCode == null && android.os.SystemClock.elapsedRealtime() < deadline) {
                try { exitCode = running.exitValue() }
                catch (_: IllegalThreadStateException) { Thread.sleep(10) }
            }
            check(exitCode == 0)
            result.put("status", "captured")
        } catch (error: Exception) {
            // App logs remain shareable; the manifest explicitly marks this additional source unavailable.
            result.put("status", "unavailable").put("reason", failureDetails(error))
        } finally {
            process?.destroy()
            executor.shutdownNow()
        }
        return result
    }
}
