import { Button, CircularProgress, Container, Alert } from "@mui/material"
import FileInput from "./FileInput"
import { useEffect, useState } from "react"
import { useFfmpegContext } from "../services/FfmpegService/FfmpegContext"
import { setSelectedFile } from "../store/slices/appState"

const ProcessingButtons: React.FC = () => {
  return (
    <Container maxWidth="sm" className="my-8 rounded-lg bg-gray-50 p-6">
      <FileInput onFileChange={setSelectedFile} />

      <Button
        onClick={setSelectedFile}
        variant="contained"
        color="secondary"
        disabled={!isLoaded || !videoFile}
        style={{ marginTop: "16px", backgroundColor: "#d75056" }}
      >
        {!isLoaded ? (
          <CircularProgress size={24} color="inherit" />
        ) : (
          "Transcribe Audio"
        )}
      </Button>
    </Container>
  )
}

export default ProcessingButtons
