//https://blog.kumard3.com/blog/extracting-audio-from-video-browser-react-ffmpeg-wasm
import type React from "react"
import { useEffect, useState } from "react"
import { ffmpegService } from "../services/FfmpegService"

const AudioExtractor: React.FC = () => {
  const [videoFile, setVideoFile] = useState<File | null>(null)

  useEffect(() => {
    const loadFfmpeg = async () => {
      try {
        await ffmpegService.loadFfmpeg()
      } catch (error) {
        console.error("Error loading FFmpeg:", error)
      }
    }
    loadFfmpeg()
  }, [])

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    if (event.target.files && event.target.files[0]) {
      const file = event.target.files[0]
      if (file.type === "video/mp4") {
        setVideoFile(file)
      } else {
        alert("Please select an MP4 file.")
        event.target.value = ""
      }
    }
  }

  const handleExtractAudio = async () => {
    if (!videoFile) {
      alert("Please select an MP4 file first")
      return
    }
    await ffmpegService.extractAudioFromVideo(videoFile)
  }

  return (
    <div className="my-8 rounded-lg border bg-gray-50 p-6">
      {
        <>
          <input
            type="file"
            accept="video/mp4"
            onChange={handleFileChange}
            className="mb-4"
          />
          <br />
          <button
            onClick={handleExtractAudio}
            disabled={!videoFile}
            className="rounded bg-green-500 px-4 py-2 font-bold text-white hover:bg-green-700 disabled:opacity-50"
          >
            Extract Audio
          </button>
        </>
      }
    </div>
  )
}

export default AudioExtractor
