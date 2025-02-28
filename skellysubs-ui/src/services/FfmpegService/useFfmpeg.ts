import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"
import type { AppDispatch } from "../../store/appStateStore"

class FfmpegWrapper {
  public isLoaded: boolean
  private ffmpeg: FFmpeg
  private dispatch?: AppDispatch

  constructor() {
    this.ffmpeg = new FFmpeg()
    this.isLoaded = false
  }

  async loadFfmpeg(dispatch?: AppDispatch): Promise<void> {
    if (this.isLoaded) return
    console.log("Loading FFmpeg...")
    this.dispatch = dispatch
    try {
      const ffmpegFilesBaseFolder = `${window.location.origin}/ffmpeg.wasm@0.12.9`
      const ffmpegCorePath = `${ffmpegFilesBaseFolder}/ffmpeg-core.js`
      const ffmpegWasmPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.wasm`
      const ffmpegWorkerPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.worker.js`

      this.ffmpeg.on("log", ({ message }) => {
        console.log(`ffmpeg: ${message}`)
        this.dispatch?.({
          type: "logs/addLog",
          payload: {
            message: `FFmpeg: ${message}`,
            severity: "info",
          },
        })
      })
      // // Configure progress handler
      // this.ffmpeg.on('progress', ({ progress }) => {
      //     console.log(Math.round(progress * 100))
      // })
      await this.ffmpeg.load({
        coreURL: await toBlobURL(ffmpegCorePath, "text/javascript"),
        wasmURL: await toBlobURL(ffmpegWasmPath, "application/wasm"),
        workerURL: await toBlobURL(ffmpegWorkerPath, "text/javascript"),
      })
    } catch (error) {
      console.error(error)
      this.dispatch?.({
        type: "logs/addLog",
        payload: {
          message: `FFmpeg: ${error}`,
          severity: "info",
        },
      })
    }
    this.isLoaded = true
  }

  async convertToMP3(
    file: File,
    maxSizeMB: number = 25,
  ): Promise<{ audioBlob: Blob | null; bitrate: number; duration: number }> {
    if (!this.isLoaded) throw new Error("FFmpeg not initialized")
    const duration = await this.getMediaDuration(file)
    const originalBitrate = file.size / duration
    let audioBlob: Blob | null = null
    let bitrate: number

    console.log(
      `Converting '${file.name} (${(file.size / 1024) * 1024} MB, duration: ${duration} seconds, bitrate: ${originalBitrate} kbps)' to MP3...`,
    )
    const inputPath = "input." + (file.type.includes("audio") ? "mp3" : "mp4")
    const outputPath = "output.mp3"
    try {
      // Cleanup previous runs
      await this.cleanFiles([inputPath, outputPath])

      // Write input file
      await this.ffmpeg.writeFile(inputPath, await fetchFile(file))

      // Calculate target bitrate
      if (file.size >= maxSizeMB * 1024 * 1024) {
        console.log(
          `FFmpeg: File larger than 25MB transcription endpoint limit... Calculating target bitrate...`,
        )
        bitrate = this.calculateTargetBitrate(duration, maxSizeMB)
        console.log(`FFmpeg: Target bitrate: ${bitrate} kbps`)
      } else {
        console.log(
          `FFmpeg: File smaller than 25MB transcription endpoint limit... Using original bitrate...`,
        )
        bitrate = Math.floor(originalBitrate / 1000)
      }

      // Execute conversion with normalization
      await this.ffmpeg.exec([
        "-i",
        inputPath,
        "-vn", // Disable video
        "-ac",
        "2", // Stereo audio
        "-ar",
        "44100", // Sample rate
        "-b:a",
        `${bitrate}k`, // Calculated bitrate
        "-af",
        "loudnorm=I=-16:TP=-1", // Audio normalization
        "-y", // Overwrite files
        outputPath,
      ])

      // Read and validate output
      const outputData = await this.ffmpeg.readFile(outputPath)
      audioBlob = new Blob([outputData], { type: "audio/mpeg" })

      if (audioBlob.size > maxSizeMB * 1024 * 1024) {
        new Error(
          `FFmpeg: Output file size ${(audioBlob.size / 1024) * 1024} MB exceeds limit of ${maxSizeMB} MB - check bitrate calculation bc its supposed to stop this error`,
        )
      }
      const logMsg = `FFmpeg: Initial audio file ready to process! - ${(audioBlob.size / 1024) * 1024} MB`
      console.log(logMsg)
      this.dispatch?.({
        type: "logs/addLog",
        payload: {
          message: logMsg,
          severity: "info",
        },
      })

      return { audioBlob, bitrate, duration }
    } catch (error) {
      this.logError("Conversion failed", error)
      throw error
    } finally {
      await this.cleanFiles([inputPath, outputPath])
    }
  }

  private async getMediaDuration(file: File): Promise<number> {
    return new Promise(resolve => {
      const url = URL.createObjectURL(file)
      const media = new Audio(url)
      media.onloadedmetadata = () => {
        URL.revokeObjectURL(url)
        resolve(media.duration)
      }
    })
  }

  private calculateTargetBitrate(duration: number, maxMB: number): number {
    const targetBits = maxMB * 8 * 1024 * 1024 // Convert MB to bits
    return Math.floor(targetBits / duration / 1000) // kbps
  }

  private async cleanFiles(files: string[]): Promise<void> {
    await Promise.all(
      files.map(file => this.ffmpeg.deleteFile(file).catch(() => undefined)),
    )
  }

  private logError(message: string, error: unknown): void {
    const errorMessage =
      error instanceof Error ? error.message : "Unknown error"
    this.dispatch?.({
      type: "logs/addLog",
      payload: { message: `${message}: ${errorMessage}`, severity: "error" },
    })
  }

  public async getAvFileDetails(
    file: File,
  ): Promise<{ bitrate: number; duration: number }> {
    if (!this.isLoaded) throw new Error("FFmpeg not initialized")
    const duration = await this.getMediaDuration(file)
    const bitrate = file.size / duration
    return { bitrate, duration }
  }
}

export const ffmpegService = new FfmpegWrapper()
