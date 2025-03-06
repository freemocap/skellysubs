import { useAppDispatch, useAppSelector } from "../../../store/hooks"
import {
  selectIsTranscribeReady,
  selectProcessingContext,
} from "../../../store/slices/processing-status/processingStatusSlice"
import { Button, IconButton, Typography } from "@mui/material"

import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme"
import type React from "react"
import { useState } from "react"
import { logger } from "../../../utils/logger"
import {
  ProcessingButton,
  ProcessingPanelLayout,
} from "../ProcessingPanelLayout"
import { transcriptionThunk } from "../../../store/thunks/transcriptionThunk"
import SettingsIcon from "@mui/icons-material/Settings"
import { TranscriptionControls } from "./TranscriptionControls"

const TranscriptionPanel: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranscribeReady)
  const processingContext = useAppSelector(selectProcessingContext)
  const transcriptionStatus = useAppSelector(
    state => state.processing.stages.transcription.status,
  )
  const [language, setLanguage] = useState("auto-detect")
  const [prompt, setPrompt] = useState("None")
  const [showControls, setShowControls] = useState(false)

  const handleTranscribeClick = () => {
    logger(
      `Transcribe button clicked with parameters -  language: ${language}, prompt: ${prompt}`,
    )
    dispatch(
      transcriptionThunk({
        language: language === "auto-detect" ? "" : language,
        prompt: prompt === "None" ? "" : prompt,
      }),
    )
  }

  const handleDownloadJSONClick = () => {
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
  const handleDownloadSRTClick = () => {
    if (!processingContext.transcription?.srt_subtitles_string) return
    logger(`[TranscriptionPanel] Downloading SRT subtitles...`)
    const srtBlob = new Blob(
      [processingContext.transcription.srt_subtitles_string],
      {
        type: "text/srt",
      },
    )
    const url = URL.createObjectURL(srtBlob)

    const a = document.createElement("a")
    a.href = url
    a.download = `${processingContext.originalFile?.name}_transcription.subtitles.srt`
    a.click()

    // Clean up the URL object
    URL.revokeObjectURL(url)
    logger("done downloading srt")
  }
  return (
    <ProcessingPanelLayout
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
        sx={{ mb: 2 }}
      >
        {!processingContext.mp3Audio
          ? "No audio file available! Please upload a video or audio file first."
          : !processingContext.transcription
            ? "Audio file available, ready to transcribe!"
            : null}
      </Typography>

      <IconButton onClick={() => setShowControls(!showControls)}>
        <SettingsIcon />
      </IconButton>
      {showControls && (
        <TranscriptionControls
          language={language}
          setLanguage={setLanguage}
          prompt={prompt}
          setPrompt={setPrompt}
        />
      )}

      <ProcessingButton
        status={transcriptionStatus}
        isReady={isReady}
        label="Transcribe Audio"
        onClick={handleTranscribeClick}
      />

      {processingContext.transcription && (
        <>
          <Typography>
            {processingContext.transcription.transcript?.text}
          </Typography>

          <Button
            variant="contained"
            onClick={handleDownloadJSONClick}
            sx={{
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

          <Button
            variant="contained"
            onClick={handleDownloadSRTClick}
            sx={{
              m: 3,
              backgroundColor: extendedPaperbaseTheme.palette.secondary.light,
              borderColor: "#222222",
              borderStyle: "solid",
              borderWidth: "1px",
            }}
          >
            Download subtitles (.srt format)
          </Button>
        </>
      )}
    </ProcessingPanelLayout>
  )
}
export default TranscriptionPanel
