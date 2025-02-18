import ffmpeg from "ffmpeg.js/ffmpeg-mp4"

class AudioVideoFileHandler {
  constructor() {
    console.log("Creating AudioVideoFileHandler")
  }

  async extractAudio(videoFile) {
    console.log("Extracting audio from video file")

    // Ensure the file type is supported
    if (
      !videoFile.type.startsWith("video") &&
      !videoFile.type.startsWith("audio")
    ) {
      throw new Error(`Unsupported file type: ${videoFile.type}`)
    }

    const reader = new FileReader()
    reader.readAsArrayBuffer(videoFile)

    return new Promise((resolve, reject) => {
      reader.onload = async () => {
        try {
          const result = ffmpeg({
            MEMFS: [{ name: "input.mp4", data: new Uint8Array(reader.result) }],
            arguments: [
              "-i",
              "input.mp4",
              "-vn",
              "-acodec",
              "pcm_s16le",
              "-ar",
              "44100",
              "-ac",
              "2",
              "output.wav",
            ],
            stdin: () => {},
          })

          const audioFile = result.MEMFS[0]

          const audioBlob = new Blob([audioFile.data], { type: "audio/wav" })
          resolve(audioBlob)
        } catch (error) {
          console.error("An error occurred while extracting the audio:", error)
          reject(error)
        }
      }

      reader.onerror = () => {
        reject(new Error("Failed to read the file as array buffer"))
      }
    })
  }
}
