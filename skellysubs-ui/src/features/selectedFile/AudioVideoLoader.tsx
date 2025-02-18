import { useRef, useState } from "react"
import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"

function AudioVideoLoader() {
  const ffmpeg = useRef(new FFmpeg())
  const [loaded, setLoaded] = useState(false)
  const [processing, setProcessing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Initialize FFmpeg with jsDelivr URLs
  const loadFFmpeg = async () => {
    try {
      if (loaded) return

      console.time("FFmpeg load time")
      await ffmpeg.current.load({
        coreURL: await toBlobURL(
          `https://cdn.jsdelivr.net/npm/@ffmpeg/core@0.12.6/dist/ffmpeg-core.js`,
          "text/javascript",
        ),
        wasmURL: await toBlobURL(
          `https://cdn.jsdelivr.net/npm/@ffmpeg/core@0.12.6/dist/ffmpeg-core.wasm`,
          "application/wasm",
        ),
        workerURL: await toBlobURL(
          `https://cdn.jsdelivr.net/npm/@ffmpeg/core@0.12.6/dist/ffmpeg-core.worker.js`,
          "text/javascript",
        ),
      })
      setLoaded(true)
      console.timeEnd("FFmpeg load time")
      console.log("FFmpeg initialized!")
    } catch (err) {
      console.error("FFmpeg initialization failed:", err)
      setError(
        "Failed to initialize audio processor. Please refresh and try again.",
      )
    }
  }

  const handleFileProcessing = async (file: File) => {
    if (!file) return
    setError(null)

    try {
      setProcessing(true)
      console.log("Processing file:", {
        name: file.name,
        type: file.type,
        sizeMB: (file.size / 1024 / 1024).toFixed(2),
      })

      if (!loaded) await loadFFmpeg()

      const isVideo = file.type.startsWith("video/")
      const inputExt = isVideo ? "mp4" : file.name.split(".").pop() || "webm"
      const outputExt = isVideo ? "mp3" : "ogg"

      const inputFile = `input.${inputExt}`
      const outputFile = `output.${outputExt}`

      // Write input file
      await ffmpeg.current.writeFile(inputFile, await fetchFile(file))
      console.log(`Input written: ${inputFile}`)

      // Build FFmpeg command
      const command = [
        "-i",
        inputFile,
        ...(isVideo
          ? ["-vn", "-acodec", "libmp3lame", "-q:a", "2"]
          : ["-c:a", "copy"]),
        outputFile,
      ]

      console.log("Executing:", command.join(" "))
      await ffmpeg.current.exec(command)

      // Read output
      const outputData = await ffmpeg.current.readFile(outputFile)
      console.log(
        "Output size:",
        `${(outputData.length / 1024 / 1024).toFixed(2)}MB`,
      )

      return outputData
    } catch (err) {
      console.error("Processing failed:", err)
      setError(
        `Processing failed: ${err instanceof Error ? err.message : "Unknown error"}`,
      )
      throw err
    } finally {
      setProcessing(false)
    }
  }

  const handleFileInput = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return

    try {
      const output = await handleFileProcessing(file)
      console.log("Processed output:", output)
      // Reset input to allow re-selection
      e.target.value = ""
    } catch {
      // Error already handled
    }
  }

  return (
    <div className="audio-video-loader">
      {!loaded && (
        <button
          onClick={loadFFmpeg}
          disabled={processing}
          className="init-button"
        >
          {processing ? "Initializing..." : "Initialize Audio Processor"}
        </button>
      )}

      <input
        type="file"
        accept="video/*,audio/*"
        onChange={handleFileInput}
        disabled={!loaded || processing}
        className="file-input"
      />

      {processing && (
        <div className="processing-status">
          ⏳ Processing... (Check console for details)
        </div>
      )}

      {error && <div className="error-message">❌ {error}</div>}
    </div>
  )
}

export default AudioVideoLoader
