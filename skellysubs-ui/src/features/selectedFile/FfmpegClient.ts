import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile, toBlobURL } from "@ffmpeg/util"

class FfmpegClient {
  private ffmpeg: FFmpeg
  private loaded: boolean = false
  private baseURL: string

  constructor(
    baseURL: string = "https://unpkg.com/@ffmpeg/core@0.12.6/dist/esm",
  ) {
    console.log("Creating FfmpegClient")
    this.ffmpeg = new FFmpeg()
    this.baseURL = baseURL
  }

  async load() {
    if (!this.loaded) {
      console.log("Loading FFmpeg.wasm...")
      this.ffmpeg.on("log", ({ message }) => {
        console.log(message)
      })
      await this.ffmpeg.load({
        coreURL: await toBlobURL(
          `${this.baseURL}/ffmpeg-core.js`,
          "text/javascript",
        ),
        wasmURL: await toBlobURL(
          `${this.baseURL}/ffmpeg-core.wasm`,
          "application/wasm",
        ),
      })
      this.loaded = true
      console.log("FFmpeg.wasm loaded!")
    }
  }

  async transcode(inputURL: string, outputFileName: string): Promise<Blob> {
    if (!this.loaded) {
      throw new Error("FFmpeg is not loaded")
    }
    await this.ffmpeg.writeFile("input.webm", await fetchFile(inputURL))
    await this.ffmpeg.exec(["-i", "input.webm", outputFileName])
    const data = await this.ffmpeg.readFile(outputFileName)
    return new Blob([data?.buffer], { type: "video/mp4" })
  }

  async extractAudio(
    inputURL: string,
    outputFileName: string = "output.mp3",
  ): Promise<Blob> {
    console.log("Extracting audio from", inputURL)
    if (!this.loaded) {
      throw new Error("FFmpeg is not loaded")
    }
    await this.ffmpeg.writeFile("input.webm", await fetchFile(inputURL))
    await this.ffmpeg.exec([
      "-i",
      "input.webm",
      "-q:a",
      "0",
      "-map",
      "a",
      outputFileName,
    ])
    const data = await this.ffmpeg.readFile(outputFileName)
    console.log(`Audio extracted to ${outputFileName}`)
    return new Blob([data?.buffer], { type: "audio/mp3" })
  }

  async splitAudio(
    inputURL: string,
    segmentTime: number,
    overlap: number = 0,
    outputPattern: string = "output_%d.mp3",
  ): Promise<Blob[]> {
    if (!this.loaded) {
      throw new Error("FFmpeg is not loaded")
    }
    await this.ffmpeg.writeFile("input.mp3", await fetchFile(inputURL))
    const ffmpegArgs = [
      "-i",
      "input.mp3",
      "-f",
      "segment",
      "-segment_time",
      segmentTime.toString(),
      "-segment_wrap",
      "100",
      "-segment_overlap",
      overlap.toString(),
      "-c",
      "copy",
      outputPattern,
    ]
    await this.ffmpeg.exec(ffmpegArgs)

    const segments = []
    for (let i = 0; i < 100; i++) {
      // Attempt to read up to 100 segments
      try {
        const data = await this.ffmpeg.readFile(
          outputPattern.replace("%d", i.toString()),
        )
        segments.push(new Blob([data.buffer], { type: "audio/mp3" }))
      } catch (error) {
        break // Stop when there are no more segments
      }
    }
    return segments
  }

  async splitVideo(
    inputURL: string,
    segmentTime: number,
    outputPattern: string,
  ): Promise<Blob[]> {
    if (!this.loaded) {
      throw new Error("FFmpeg is not loaded")
    }
    await this.ffmpeg.writeFile("input.webm", await fetchFile(inputURL))
    await this.ffmpeg.exec([
      "-i",
      "input.webm",
      "-f",
      "segment",
      "-segment_time",
      segmentTime.toString(),
      "-g",
      "9",
      "-sc_threshold",
      "0",
      "-force_key_frames",
      "expr:gte(t,n_forced*9)",
      "-reset_timestamps",
      "1",
      "-map",
      "0",
      outputPattern,
    ])
    const segments = []
    for (let i = 0; i < 3; i++) {
      // Adjust this loop based on expected segments
      const data = await this.ffmpeg.readFile(
        outputPattern.replace("%d", i.toString()),
      )
      segments.push(new Blob([data.buffer], { type: "video/mp4" }))
    }
    return segments
  }

  // Add more methods as needed for other operations like text overlay, interlacing videos, etc.
}

export default FfmpegClient
//
// Example usage:
// ```typescript
// import FfmpegClient from './ffmpegClient';
//
// const splitAudioFile = async () => {
//   const ffmpegClient = new FfmpegClient();
//   await ffmpegClient.load();
//
//   try {
//     const audioSegments = await ffmpegClient.splitAudio('https://example.com/input.mp3', 10, 2, 'output_%d.mp3');
//     console.log('Audio split into segments');
//     // Do something with audioSegments, like playing them or saving them
//   } catch (error) {
//     console.error('Error splitting audio:', error);
//   }
// };
//
// splitAudioFile();
// ```
