import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"
import type { AppDispatch } from "../../store/AppStateStore"
import { useLogger } from "../loggerService/loggerContext"
import { logger } from "../../utils/logger"

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
    logger("Loading FFmpeg...", "info")

    this.dispatch = dispatch
    try {
      const ffmpegFilesBaseFolder = `${window.location.origin}/ffmpeg.wasm@0.12.9`
      const ffmpegCorePath = `${ffmpegFilesBaseFolder}/ffmpeg-core.js`
      const ffmpegWasmPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.wasm`
      const ffmpegWorkerPath = `${ffmpegFilesBaseFolder}/ffmpeg-core.worker.js`

      this.ffmpeg.on("log", ({ message }) => {
        logger(`ffmpeg: ${message}`, "info")
      })
      // Configure progress handler
      // this.ffmpeg.on('progress', ({ progress }) => {
      //     logger(Math.round(progress * 100))
      // })
      await this.ffmpeg.load({
        coreURL: await toBlobURL(ffmpegCorePath, "text/javascript"),
        wasmURL: await toBlobURL(ffmpegWasmPath, "application/wasm"),
        workerURL: await toBlobURL(ffmpegWorkerPath, "text/javascript"),
      })
    } catch (error) {
      logger(`FFmpeg: ${error}`, "error")
    }
    this.isLoaded = true
  }

  async convertToMP3(
    file: File,
    maxSizeMB: number = 25,
  ): Promise<{
    audioBlob: Blob | null
    audioFileName: string
    audioFileType: string
    bitrate: number
    duration: number
  }> {
    if (!this.isLoaded) throw new Error("FFmpeg not initialized")
    const duration = await this.getMediaDuration(file)
    let audioBlob: Blob | null = null
    let bitrate: number
    const audioFileType: string = "audio/mpeg"

    logger(
      `Converting '${file.name} (${(file.size / (1024 * 1024)).toFixed(2)} MB, duration: ${duration} seconds)' to MP3...`,
      "info",
    )
    const inputExtension = file.name.split(".").pop() || "tmp"
    const inputPath = "input." + file.type
    const outputPath = "output.mp3"
    const audioFileName = `${file.name.replace(/\.[^/.]+$/, "")}.audio.mp3`
    try {
      // Cleanup previous runs
      await this.cleanFiles([inputPath, outputPath])

      // Write input file
      await this.ffmpeg.writeFile(inputPath, await fetchFile(file))
      // Calculate target bitrate based on desired output size and duration
      bitrate = this.calculateTargetBitrate(duration, maxSizeMB)
      logger(`FFmpeg: Target bitrate: ${bitrate} kbps`)

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
      ]);

      // Read and validate output
      const outputData = await this.ffmpeg.readFile(outputPath)
      audioBlob = new Blob([outputData], { type: audioFileType })

      if (audioBlob.size > maxSizeMB * 1024 * 1024) {
        new Error(
          `FFmpeg: Output file size ${(audioBlob.size / 1024) * 1024} MB exceeds limit of ${maxSizeMB} MB - check bitrate calculation bc its supposed to stop this error`,
        )
      }
      logger(`FFmpeg: Initial audio file ready to process! - ${(audioBlob.size / 1024) * 1024} MB`, 'info')

      return {
        audioBlob,
        audioFileName,
        audioFileType,
        bitrate,
        duration,
      }
    } catch (error) {
      this.logError("Conversion failed", error)
      throw error
    } finally {
      await this.cleanFiles([inputPath, outputPath])
    }
  }

  private async getMediaDuration(file: File): Promise<number> {
    const inputExtension = file.name.split(".").pop() || "tmp";
    const inputPath = `input.${inputExtension}`;
    try {
      await this.ffmpeg.writeFile(inputPath, await fetchFile(file));
      let duration = 0;
      // Capture logs to find duration
      const logPromise = new Promise<void>((resolve) => {
        const logHandler = ({ message }: { message: string }) => {
          const durationMatch = message.match(/Duration:\s(\d{2}):(\d{2}):(\d{2})\.\d+/);
          if (durationMatch) {
            const hours = parseInt(durationMatch[1], 10);
            const minutes = parseInt(durationMatch[2], 10);
            const seconds = parseInt(durationMatch[3], 10);
            duration = hours * 3600 + minutes * 60 + seconds;
            resolve();
          }
        };
        this.ffmpeg.on("log", logHandler);
      });

      await Promise.race([
        logPromise,
        this.ffmpeg.exec(["-i", inputPath]),
      ]);

      if (duration === 0) throw new Error("Duration not found in logs");
      return duration;
    } catch (error) {
      this.logError("Failed to get duration via FFmpeg, using fallback", error);
      // Fallback to Audio element
      return new Promise((resolve) => {
        const url = URL.createObjectURL(file);
        const media = new Audio(url);
        media.onloadedmetadata = () => {
          URL.revokeObjectURL(url);
          resolve(media.duration);
        };
        media.onerror = () => resolve(0);
      });
    } finally {
      await this.cleanFiles([inputPath]);
    }
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
