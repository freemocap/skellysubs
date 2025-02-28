import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  selectIsTranscribeReady,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import { Box, Button, Typography } from "@mui/material"
import { transcribeAudioThunk } from "../../store/thunks"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"
import type React from "react"

const TranscribeButton: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranscribeReady)
  const processingContext = useAppSelector(selectProcessingContext)

  const handleTranscribeClick = () => {
    console.log("Transcribe button clicked")
    dispatch(transcribeAudioThunk()) // No argument needed
  }

  const handleDownloadClick = () => {
    const json = JSON.stringify(processingContext.transcription, null, 2)
    const blob = new Blob([json], { type: "application/json" })
    const url = URL.createObjectURL(blob)

    const a = document.createElement("a")
    a.href = url
    a.download = "transcription.json"
    a.click()

    // Clean up the URL object
    URL.revokeObjectURL(url)
  }

  return (
    <Box
      sx={{
        m: 3,
        p: 3,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        borderStyle: "solid",
        borderColor: "#aa00aa",
        borderWidth: "1px",
        borderRadius: 2,
      }}
    >
      <Button
        variant="contained"
        color="secondary"
        sx={{ m: 2 }}
        onClick={handleTranscribeClick}
        disabled={!isReady}
      >
        Transcribe Audio
      </Button>

      {processingContext.transcription && (
        <>
          <Typography>{processingContext.transcription.text}</Typography>
          <Button
            variant="contained"
            onClick={handleDownloadClick}
            sx={{
              backgroundColor: extendedPaperbaseTheme.palette.primary.light,
              borderColor: "#222222",
              borderStyle: "solid",
              borderWidth: "1px",
            }}
          >
            Download transcription results (openai verbose json format)
          </Button>
        </>
      )}
    </Box>
  )
}
export default TranscribeButton
