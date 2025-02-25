import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"

class UseFfmpeg {
  private ffmpeg: FFmpeg
  isLoaded: boolean

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
      console.log(`ffmpeg: ${message}`)
    })

    await this.ffmpeg.load({
      coreURL: await toBlobURL(ffmpegCorePath, "text/javascript"),
      wasmURL: await toBlobURL(ffmpegWasmPath, "application/wasm"),
      workerURL: await toBlobURL(ffmpegWorkerPath, "text/javascript"),
    })

    this.isLoaded = true
  }

  async extractAudioFromVideo(videoFile: File): Promise<Blob> {
    if (!this.isLoaded) {
      throw new Error("FFmpeg is not loaded yet.")
    }

    const inputPath = "input.mp4"
    const outputPath = "output.mp3"

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

      // Cleanup
      await this.ffmpeg.deleteFile(inputPath).catch(() => {})
      await this.ffmpeg.deleteFile(outputPath).catch(() => {})

      return audioBlob
    } catch (error) {
      console.error("Extraction failed:", error)
      throw error
    }
  }
}

export const ffmpegService = new UseFfmpeg()
