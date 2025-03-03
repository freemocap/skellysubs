import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  selectIsTranslateReady,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import { Box, Button, CircularProgress, Typography } from "@mui/material"
import { translateTextThunk } from "../../store/thunks"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"
import React, {useState} from "react"
import { logger } from "../../utils/logger"
import {ProcessingButton, ProcessingPanelLayout} from "./ProcessingPanelLayout";
import {TranslationControls} from "./TranslationControls";

const TranslateTranscriptPanel: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranslateReady)
  const processingContext = useAppSelector(selectProcessingContext)
  const translationStatus = useAppSelector(
    state => state.processing.stages.translation.status,
  )
    const [targetLanguages, setTargetLanguages] = useState("")
    const [romanize, setRomanize] = useState(false)
    const [romanizationMethod, setRomanizationMethod] = useState("");

    const handleTranslateClick = () => {
        logger("Translate button clicked");
        dispatch(translateTextThunk({
            targetLanguages: targetLanguages.split(","),
            romanize,
            romanizationMethod
        }));
    };

  const handleDownloadClick = () => {
    const json = JSON.stringify(processingContext.translation, null, 2)
    const blob = new Blob([json], { type: "application/json" })
    const url = URL.createObjectURL(blob)

    const a = document.createElement("a")
    a.href = url
    a.download = `${processingContext.originalFile?.name}_translation.json`
    a.click()

    // Clean up the URL object
    URL.revokeObjectURL(url)
  }

  return (
    <ProcessingPanelLayout borderColor="#00aa3c"
      sx={{
        m: 3,
        p: 3,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        borderStyle: "solid",
        borderWidth: "1px",
        borderRadius: 2,
      }}
    >
      <Typography
        variant="body1"
        color={extendedPaperbaseTheme.palette.text.disabled}
      >
        {!processingContext.transcription &&
          " No transcript available, transcribe audio first. "}
      </Typography>
        <TranslationControls
            targetLanguages={targetLanguages}
            setTargetLanguages={setTargetLanguages}
            romanize={romanize}
            setRomanize={setRomanize}
            romanizationMethod={romanizationMethod}
            setRomanizationMethod={setRomanizationMethod}
        />
        <ProcessingButton
            status={translationStatus}
            isReady={isReady}
            label="Translate Text"
            onClick={handleTranslateClick}
        />
      {processingContext.translation && (
        <>
          <Typography sx={{ mb: 2, textAlign: "center" }}>
            Original text: <br />
            {processingContext.translation.original_text}
          </Typography>
          <Typography variant="h6" sx={{ mb: 1 }}>
            Translations:
          </Typography>
          {Object.entries(
            processingContext.translation.translations.translations,
          ).map(([key, translation], i) => (
            <Box
              key={i}
              sx={{
                mb: 2,
                p: 2,
                border: "1px solid #ddd",
                borderRadius: 1,
                width: "100%",
                textAlign: "left",
              }}
            >
              <Typography variant="subtitle1" sx={{ fontWeight: "bold" }}>
                {translation.translated_language_name}:
              </Typography>
              <Typography variant="body1">
                {translation.translated_text}
              </Typography>
              {translation.romanized_text && (
                <Typography variant="body2" sx={{ fontStyle: "italic" }}>
                  Romanized: {translation.romanized_text}
                </Typography>
              )}
            </Box>
          ))}
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
            Download translation results
          </Button>
        </>
      )}
    </ProcessingPanelLayout>
  )
}
export default TranslateTranscriptPanel
