import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  selectIsTranscribeReady,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import { Box, Button, CircularProgress, Typography } from "@mui/material"
import { transcribeAudioThunk } from "../../store/thunks"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"
import React, { useState } from "react"
import { logger } from "../../utils/logger"
import {ProcessingButton, ProcessingPanelLayout} from "./ProcessingPanelLayout"
import { TranscriptionControls } from "./TranscriptionControls"

const TranscribeAudioPanel: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranscribeReady)
  const processingContext = useAppSelector(selectProcessingContext)
  const transcriptionStatus = useAppSelector(
    state => state.processing.stages.transcription.status,
  )
    const [language, setLanguage] = useState("auto-detect");
    const [prompt, setPrompt] = useState("None");

    const handleTranscribeClick = () => {
        logger(`Transcribe button clicked with parameters -  language: ${language}, prompt: ${prompt}`)
        dispatch(transcribeAudioThunk({
            language: language === "auto-detect" ? "" : language,
            prompt: prompt === "None" ? "" : prompt
        }));
    };

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
    <ProcessingPanelLayout
      borderColor="#ff0"
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
        <TranscriptionControls
            language={language}
            setLanguage={setLanguage}
            prompt={prompt}
            setPrompt={setPrompt}
        />
        <ProcessingButton
            status={transcriptionStatus}
            isReady={isReady}
            label="Transcribe Audio"
            onClick={handleTranscribeClick}
        />
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
    </ProcessingPanelLayout>
  )
}
export default TranscribeAudioPanel
