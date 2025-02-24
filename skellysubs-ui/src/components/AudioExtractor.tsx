import { Button, CircularProgress, Container, Alert } from "@mui/material"
import FileInput from "./FileInput"
import { useEffect, useState } from "react"
import { useFfmpegContext } from "../services/FfmpegService/FfmpegContext"

const AudioExtractor: React.FC = () => {
  const [videoFile, setVideoFile] = useState<File | null>(null)
  const { isLoaded, extractAudioFromVideo, error } = useFfmpegContext()

  const handleExtractAudio = async () => {
    if (!videoFile) return
    try {
      await extractAudioFromVideo(videoFile)
    } catch (err) {
      console.error("Extraction failed:", err)
    }
  }

  return (
    <Container maxWidth="sm" className="my-8 rounded-lg bg-gray-50 p-6">
      {error && (
        <Alert severity="error" className="mb-4">
          {error}
        </Alert>
      )}

      <FileInput onFileChange={setVideoFile} />

      <Button
        onClick={handleExtractAudio}
        variant="contained"
        color="secondary"
        disabled={!isLoaded || !videoFile}
        style={{ marginTop: "16px", backgroundColor: "#d75056" }}
      >
        {!isLoaded ? (
          <CircularProgress size={24} color="inherit" />
        ) : (
          "Extract Audio"
        )}
      </Button>
    </Container>
  )
}

export default AudioExtractor
