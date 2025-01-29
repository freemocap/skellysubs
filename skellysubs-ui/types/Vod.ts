export class Vod {
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

