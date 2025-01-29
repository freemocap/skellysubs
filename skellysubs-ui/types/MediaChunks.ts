export class VideoChunk {
  id: string
  timestamp: number
  videoBlob: Blob

  constructor(videoBlob: Blob) {
    this.id = Math.random().toString(36).substr(2, 9)
    this.timestamp = Date.now()
    this.videoBlob = videoBlob
  }

  getSize(): number {
    return this.videoBlob.size
  }
}

export class AudioChunk {
  id: string
  timestamp: number
  audioBlob: Blob

  constructor(audioBlob: Blob) {
    this.id = Math.random().toString(36).substr(2, 9)
    this.timestamp = Date.now()
    this.audioBlob = audioBlob
  }

  getSize(): number {
    return this.audioBlob.size
  }
}

