//https://blog.kumard3.com/blog/extracting-audio-from-video-browser-react-ffmpeg-wasm
import type React from "react"
import { useRef, useState } from "react"
import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"

const AudioExtractor: React.FC = () => {
  const [loaded, setLoaded] = useState(false)
  const [videoFile, setVideoFile] = useState<File | null>(null)
  const [message, setMessage] = useState("")
  const ffmpegRef = useRef(new FFmpeg())

  const load = async () => {
    // load ffmpeg-core.js, ffmpeg-core.wasm, and ffmpeg-core.worker.js from the src/public folder
    const ffmpegFilesBaseFolder = `${window.location.origin}/ffmpeg.wasm@0.12.9`
    const ffmpegCorePath = `${ffmpegFilesBaseFolder}/ffmpeg-core.js`
    const ffmpegWasmPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.wasm`
    const ffmpegWorkerPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.worker.js`
    console.log(`ffmpegCorePath: ${ffmpegCorePath}`)
    console.log(`ffmpegWasmPath: ${ffmpegWasmPath}`)
    console.log(`ffmpegWorkerPath: ${ffmpegWorkerPath}`)

    const ffmpeg = ffmpegRef.current
    ffmpeg.on("log", ({ message }) => {
      setMessage(message)
    })

    try {
      await ffmpeg.load({
        coreURL: await toBlobURL(ffmpegCorePath, "text/javascript"),
        wasmURL: await toBlobURL(ffmpegWasmPath, "application/wasm"),
        workerURL: await toBlobURL(ffmpegWorkerPath, "text/javascript"),
      })
      setLoaded(true)
      console.log("FFmpeg loaded")
    } catch (error) {
      console.error(error)
      setMessage("Failed to load FFmpeg")
    }
  }

  const extractAudio = async () => {
    if (!videoFile) {
      alert("Please select an MP4 file first")
      return
    }
    const inputPath = "input.mp4"
    const outputPath = "output.mp3"
    const downloadPath = "extracted_audio.mp3"
    const ffmpeg = ffmpegRef.current

    try {
      console.log("=== STARTING PROCESS ===")

      // 1. Clean previous files
      console.log("Cleaning previous files...")
      await ffmpeg
        .deleteFile(inputPath)
        .catch(e => console.log("Clean input error:", e.message))
      await ffmpeg
        .deleteFile(outputPath)
        .catch(e => console.log("Clean output error:", e.message))

      // 2. Write input file
      console.log("Writing input file...")
      const videoData = await fetchFile(videoFile)
      console.log("Video data size:", videoData.byteLength, "bytes")
      await ffmpeg.writeFile(inputPath, videoData)
      console.log("Input file written successfully")

      // 3. Verify input exists
      const inputExists = await ffmpeg
        .listDir("/")
        .then(files => files.some(f => f.name === inputPath))
      console.log("Input file exists in FS:", inputExists)

      // 4. Execute FFmpeg command
      console.log("Executing FFmpeg command...")
      await ffmpeg.exec([
        "-i",
        inputPath,
        "-vn",
        "-acodec",
        "libmp3lame",
        "-q:a",
        "2",
        outputPath,
      ])
      console.log("FFmpeg command completed")

      // 5. List all files in FS
      console.log("Listing all files in FS:")
      const files = await ffmpeg.listDir("/")
      console.log(
        "FS Contents:",
        files.map(f => `${f.name} (${f.isDir ? "dir" : "file"})`),
      )

      // 6. Verify output exists
      const outputExists = files.some(f => f.name === outputPath)
      console.log("Output file exists:", outputExists)

      if (!outputExists) {
        throw new Error("Output file not found after conversion")
      }

      // 7. Read output file
      console.log("Reading output file...")
      const data = await ffmpeg.readFile(outputPath)
      console.log("Output file:", data)
      const audioBlob = new Blob([data], { type: "audio/mp3" })
      const audioUrl = URL.createObjectURL(audioBlob)
      const link = document.createElement("a")
      link.href = audioUrl
      link.download = downloadPath
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)
      // Cleanup files
      ffmpeg.deleteFile(inputPath).catch(() => {})
      ffmpeg.deleteFile(outputPath).catch(() => {})
    } catch (error) {
      console.error("Processing error:", error)
      setMessage(
        `Error: ${error instanceof Error ? error.message : "Conversion failed"}`,
      )
    }
  }

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    if (event.target.files && event.target.files[0]) {
      const file = event.target.files[0]
      if (file.type === "video/mp4") {
        setVideoFile(file)
      } else {
        alert("Please select an MP4 file.")
        event.target.value = ""
      }
    }
  }

  return (
    <div className="my-8 rounded-lg border bg-gray-50 p-6">
      <h2 className="mb-4 text-2xl font-semibold">Audio Extractor</h2>
      {!loaded ? (
        <button
          onClick={load}
          className="rounded bg-blue-500 px-4 py-2 font-bold text-white hover:bg-blue-700"
        >
          Load FFmpeg
        </button>
      ) : (
        <>
          <input
            type="file"
            accept="video/mp4"
            onChange={handleFileChange}
            className="mb-4"
          />
          <br />
          <button
            onClick={extractAudio}
            disabled={!videoFile}
            className="rounded bg-green-500 px-4 py-2 font-bold text-white hover:bg-green-700 disabled:opacity-50"
          >
            Extract Audio
          </button>
          <p className="mt-4 text-sm text-gray-600">{message}</p>
        </>
      )}
    </div>
  )
}

export default AudioExtractor
