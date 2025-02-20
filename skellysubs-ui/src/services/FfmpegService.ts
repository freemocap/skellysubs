import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"

class FfmpegService {
  private ffmpeg: FFmpeg
  private isLoaded: boolean

  constructor() {
    this.ffmpeg = new FFmpeg()
    this.isLoaded = false
  }

  async loadFfmpeg(): Promise<void> {
    if (this.isLoaded) return

    const ffmpegFilesBaseFolder = `${window.location.origin}/ffmpeg.wasm@0.12.9`
    const ffmpegCorePath = `${ffmpegFilesBaseFolder}/ffmpeg-core.js`
    const ffmpegWasmPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.wasm`
    const ffmpegWorkerPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.worker.js`

    this.ffmpeg.on("log", ({ message }) => {
      console.log(`${message}`)
    })

    await this.ffmpeg.load({
      coreURL: await toBlobURL(ffmpegCorePath, "text/javascript"),
      wasmURL: await toBlobURL(ffmpegWasmPath, "application/wasm"),
      workerURL: await toBlobURL(ffmpegWorkerPath, "text/javascript"),
    })

    this.isLoaded = true
  }

  async extractAudioFromVideo(videoFile: File): Promise<void> {
    if (!this.isLoaded) {
      throw new Error("FFmpeg is not loaded yet.")
    }

    const inputPath = "input.mp4"
    const outputPath = "output.mp3"
    const downloadPath = "extracted_audio.mp3"

    try {
      await this.ffmpeg.deleteFile(inputPath).catch(() => {})
      await this.ffmpeg.deleteFile(outputPath).catch(() => {})

      const videoData = await fetchFile(videoFile)
      await this.ffmpeg.writeFile(inputPath, videoData)

      await this.ffmpeg.exec([
        "-i",
        inputPath,
        "-vn",
        "-acodec",
        "libmp3lame",
        "-q:a",
        "2",
        outputPath,
      ])

      const data = await this.ffmpeg.readFile(outputPath)
      const audioBlob = new Blob([data], { type: "audio/mp3" })
      const audioUrl = URL.createObjectURL(audioBlob)
      const link = document.createElement("a")
      link.href = audioUrl
      link.download = downloadPath
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)

      this.ffmpeg.deleteFile(inputPath).catch(() => {})
      this.ffmpeg.deleteFile(outputPath).catch(() => {})
    } catch (error) {
      console.log(
        `Error: ${error instanceof Error ? error.message : "Conversion failed"}`,
      )
    }
  }
}

export const ffmpegService = new FfmpegService()
