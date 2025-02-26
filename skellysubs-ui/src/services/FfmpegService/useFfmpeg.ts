import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"
import { AppDispatch } from "../../store/appStateStore"

type ProgressCallback = (progress: number) => void

class FfmpegWrapper  {
  public isLoaded: boolean
  private ffmpeg: FFmpeg
  private dispatch?: AppDispatch

  constructor() {
    this.ffmpeg = new FFmpeg()
    this.isLoaded = false
  }

  async loadFfmpeg(dispatch?: AppDispatch): Promise<void> {
    if (this.isLoaded) return
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
      onProgress?: ProgressCallback
  ): Promise<Blob|null> {
    if (!this.isLoaded) throw new Error('FFmpeg not initialized')
    const inputPath = 'input.' + (file.type.includes('audio') ? 'mp3' : 'mp4')
    const outputPath = 'output.mp3'
    let blob: Blob | null = null
    try {

      // Cleanup previous runs
      await this.cleanFiles([inputPath, outputPath])

      // Write input file
      await this.ffmpeg.writeFile(inputPath, await fetchFile(file))

      // Calculate target bitrate
      const duration = await this.getMediaDuration(file)
      let targetBitrate = this.calculateBitrate(duration, maxSizeMB)

      // Configure progress handler
      this.ffmpeg.on('progress', ({ progress }) => {
        onProgress?.(Math.round(progress * 100))
      })

      let audioBlobReady: boolean = false
      while (audioBlobReady) {
        // Execute conversion with normalization
        await this.ffmpeg.exec([
          '-i', inputPath,
          '-vn',                          // Disable video
          '-ac', '2',                     // Stereo audio
          '-ar', '44100',                 // Sample rate
          '-b:a', `${targetBitrate}k`,    // Calculated bitrate
          '-af', 'loudnorm=I=-16:TP=-1',  // Audio normalization
          '-y',                           // Overwrite files
          outputPath
        ])

        // Read and validate output
        const outputData = await this.ffmpeg.readFile(outputPath)
        blob = new Blob([outputData], {type: 'audio/mpeg'})

        if (blob.size > maxSizeMB * 1024 * 1024) {
          const logMsg = `FFmpeg: Initial audio file too large: ${blob.size / 1024 *1024} MB - decreasing bitrate and trying again`
          console.log(logMsg)
          this.dispatch?.({
            type: "logs/addLog",
            payload: {
              message: logMsg,
              severity: "info",
            },
          })
          targetBitrate *=.9
          audioBlobReady = true
        } else {
          const logMsg = `FFmpeg: Initial audio file ready to process! - ${blob.size/1024*1024} MB`
          console.log(logMsg)
          this.dispatch?.({
            type: "logs/addLog",
            payload: {
              message: logMsg,
              severity: "info",
            },
          })
        }
      }

      return blob
    } catch (error) {
      this.logError('Conversion failed', error)
      throw error
    } finally {
      await this.cleanFiles([inputPath, outputPath])
    }
  }

  private async getMediaDuration(file: File): Promise<number> {
    return new Promise((resolve) => {
      const url = URL.createObjectURL(file)
      const media = new Audio(url)
      media.onloadedmetadata = () => {
        URL.revokeObjectURL(url)
        resolve(media.duration)
      }
    })
  }

  private calculateBitrate(duration: number, maxMB: number): number {
    const targetBits = maxMB * 8 * 1024 * 1024 // Convert MB to bits
    return Math.floor(targetBits / duration / 1000) // kbps
  }

  private async cleanFiles(files: string[]): Promise<void> {
    await Promise.all(files.map(file =>
        this.ffmpeg.deleteFile(file).catch(() => undefined)
    ))
  }

  private logError(message: string, error: unknown): void {
    const errorMessage = error instanceof Error ? error.message : 'Unknown error'
    this.dispatch?.({
      type: 'logs/addLog',
      payload: { message: `${message}: ${errorMessage}`, severity: 'error' }
    })
  }
}

export const ffmpegService = new FfmpegWrapper ()