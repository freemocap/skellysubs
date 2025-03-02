import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  selectIsTranscribeReady,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import { Box, Button, CircularProgress, Typography } from "@mui/material"
import { transcribeAudioThunk } from "../../store/thunks"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"
import type React from "react"

const TranscribeAudioPanel: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranscribeReady)
  const processingContext = useAppSelector(selectProcessingContext)
  const transcriptionStatus = useAppSelector(
    state => state.processing.stages.transcription.status,
  )

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
    a.download = `${processingContext.originalFile?.name}_transcription.json`
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
      <Typography
        variant="body1"
        color={extendedPaperbaseTheme.palette.text.disabled}
      >
        {!processingContext.mp3Audio &&
          " No audio file available! upload a video or audio file first. "}
      </Typography>
      <Button
        variant="contained"
        color="secondary"
        sx={{ m: 2, position: "relative" }}
        onClick={handleTranscribeClick}
        disabled={!isReady || transcriptionStatus === "processing"}
      >
        {transcriptionStatus === "processing"
          ? "Processing..."
          : "Transcribe Audio"}
        {transcriptionStatus === "processing" && (
          <CircularProgress
            size={24}
            sx={{
              position: "absolute",
              top: "50%",
              left: "50%",
              marginTop: "-12px",
              marginLeft: "-12px",
            }}
          />
        )}
      </Button>
      {processingContext.transcription && (
        <>
          <Typography>{processingContext.transcription.text}</Typography>

          <Button
            variant="contained"
            onClick={handleDownloadClick}
            sx={{
              m: 3,
              p: 4,
              backgroundColor: extendedPaperbaseTheme.palette.primary.light,
              borderColor: "#222222",
              borderStyle: "solid",
              borderWidth: "1px",
            }}
          >
            Download transcription results
            <br />
            (openai verbose json format)
          </Button>
        </>
      )}
    </Box>
  )
}
export default TranscribeAudioPanel
