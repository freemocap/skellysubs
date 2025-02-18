class AudioVideoFileHandler {
  constructor() {
    console.log("Creating AudioVideoFileHandler")
  }

  async extractAudio(videoFile: File): Promise<Blob> {
    console.log("Extracting audio from video file")

    if (!videoFile.type.startsWith("video")) {
      throw new Error(`Unsupported file type: ${videoFile.type}`)
    }

    const audioContext = new AudioContext()
    const videoElement = document.createElement("video")
    videoElement.src = URL.createObjectURL(videoFile)

    return new Promise((resolve, reject) => {
      videoElement.onloadedmetadata = async () => {
        try {
          const audioBuffer = await audioContext.decodeAudioData(
            await this.readBlobAsArrayBuffer(videoFile),
          )
          console.log(`Successfully extracted audioBuffer`)
          const wavBlob: Blob = this.bufferToWavBlob(audioBuffer)
          console.log(`Successfully converted audiobuffer to WAV blob`)
          resolve(wavBlob)
        } catch (error) {
          console.log(`An error occurred while extracting the audio: ${error}`)
          reject(error)
        }
      }
    })
  }

  // Convert AudioBuffer to a WAV Blob
  private bufferToWavBlob(audioBuffer: AudioBuffer): Blob {
    const channels = audioBuffer.numberOfChannels
    const sampleRate = audioBuffer.sampleRate
    const length = audioBuffer.length * channels * 2 + 44 // 44 bytes for WAV header
    const wavBuffer = new ArrayBuffer(length)
    const view = new DataView(wavBuffer)

    this.writeWavHeader(view, audioBuffer.length, channels, sampleRate)

    let offset = 44
    for (let channel = 0; channel < channels; channel++) {
      const channelData = audioBuffer.getChannelData(channel)
      for (let i = 0; i < channelData.length; i++, offset += 2) {
        const s = Math.max(-1, Math.min(1, channelData[i]))
        view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7fff, true)
      }
    }

    return new Blob([wavBuffer], { type: "audio/wav" })
  }

  // Write WAV header to DataView
  private writeWavHeader(
    view: DataView,
    samples: number,
    channels: number,
    sampleRate: number,
  ): void {
    const blockAlign = channels * 2
    const byteRate = sampleRate * blockAlign

    view.setUint32(0, 0x46464952, false) // "RIFF"
    view.setUint32(4, 36 + samples * blockAlign, true)
    view.setUint32(8, 0x45564157, false) // "WAVE"
    view.setUint32(12, 0x20746d66, false) // "fmt "
    view.setUint32(16, 16, true) // PCM header length
    view.setUint16(20, 1, true) // Format: PCM
    view.setUint16(22, channels, true)
    view.setUint32(24, sampleRate, true)
    view.setUint32(28, byteRate, true)
    view.setUint16(32, blockAlign, true)
    view.setUint16(34, 16, true) // Bits per sample
    view.setUint32(36, 0x61746164, false) // "data"
    view.setUint32(40, samples * blockAlign, true)
  }

  // Helper function to read a Blob as ArrayBuffer
  async readBlobAsArrayBuffer(blob: Blob): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(reader.result as ArrayBuffer)
      reader.onerror = reject
      reader.readAsArrayBuffer(blob)
    })
  }
}

export default AudioVideoFileHandler

// Example usage:
// const processVideoFile = async () => {
//   const handler = new AudioVideoFileHandler();
//   const videoFile = ...; // Obtain the video file from an input element or other source
//
//   try {
//     const audioBlob = await handler.extractAudio(videoFile);
//     console.log('Extracted audio as WAV Blob:', audioBlob);
//   } catch (error) {
//     console.error('Error processing audio:', error);
//   }
// };
