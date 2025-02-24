// ExtractStep.tsx (modified from AudioExtractor.tsx)
import { useEffect, useState } from "react"
import {
  Alert,
  Button,
  CircularProgress,
  Container,
  Typography,
} from "@mui/material"
import { useFfmpegContext } from "../services/FfmpegService/FfmpegContext"
import FileInput from "./FileInput"

const ExtractStep = ({ onExtractionComplete }) => {
  const [videoFile, setVideoFile] = useState<File | null>(null)
  const { isLoaded, extractAudioFromVideo, error } = useFfmpegContext()

  const handleExtract = async () => {
    if (!videoFile) return
    try {
      await extractAudioFromVideo(videoFile)
      onExtractionComplete() // Trigger step transition
    } catch (err) {
      console.error("Extraction failed:", err)
    }
  }

  return (
    <Container maxWidth="sm" sx={{ py: 4 }}>
      {error && <Alert severity="error">{error}</Alert>}
      <Typography variant="h4" sx={{ mb: 2 }}>
        Extract Audio from Video
      </Typography>
      <FileInput onFileChange={setVideoFile} />
      <Button
        onClick={handleExtract}
        variant="contained"
        disabled={!isLoaded || !videoFile}
        sx={{ mt: 2 }}
      >
        {!isLoaded ? <CircularProgress size={24} /> : "Extract Audio"}
      </Button>
    </Container>
  )
}

export default ExtractStep
