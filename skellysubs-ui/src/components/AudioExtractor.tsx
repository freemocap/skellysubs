import { Button, Container } from "@mui/material"
import { ffmpegService } from "../services/FfmpegService"
import FileInput from "./FileInput"
import { useEffect, useState } from "react"

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

  const handleExtractAudio = async () => {
    if (!videoFile) {
      alert("Please select an MP4 file first")
      return
    }
    await ffmpegService.extractAudioFromVideo(videoFile)
  }

  return (
    <Container maxWidth="sm" className="my-8 rounded-lg bg-gray-50 p-6">
      <FileInput onFileChange={setVideoFile} />
      <Button
        onClick={handleExtractAudio}
        variant="contained"
        color="primary"
        disabled={!videoFile}
        style={{ marginTop: "16px" }}
      >
        Extract Audio
      </Button>
    </Container>
  )
}

export default AudioExtractor
