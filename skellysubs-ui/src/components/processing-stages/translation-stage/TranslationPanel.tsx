// src/components/processing-panel/translation/TranslationPanel.tsx

import { Box, Button, Drawer, IconButton, Typography } from "@mui/material"

import {
  ProcessingButton,
  ProcessingPanelLayout,
} from "../ProcessingPanelLayout"
import { useAppDispatch, useAppSelector } from "../../../store/hooks"
import {
  selectIsTranslateReady,
  selectProcessingContext,
} from "../../../store/slices/processing-status/processingStatusSlice"
import { useEffect, useState } from "react"
import type { LanguageConfig } from "../../../schemas/languageConfigSchemas"
import { logger } from "../../../utils/logger"
import { translationTextThunk } from "../../../store/thunks/translationTextThunk"
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme"
import SettingsIcon from "@mui/icons-material/Settings"
import { TranslationControls } from "./TranslationControls"
import Chip from "@mui/material/Chip"
import {
  fetchLanguageConfigs,
  selectLanguageOptions,
  selectSelectedTargetLanguages,
  toggleLanguage,
} from "../../../store/slices/processing-configs/translationConfigSlice"

const TranslationPanel: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranslateReady)
  const processingContext = useAppSelector(selectProcessingContext)
  const translationStatus = useAppSelector(
    state => state.processing.stages.translation.status,
  )
  const [showControls, setShowControls] = useState(false)
  const languageOptions = useAppSelector(selectLanguageOptions)
  const selectedTargetLanguages = useAppSelector(selectSelectedTargetLanguages)

  useEffect(() => {
    dispatch(fetchLanguageConfigs())
  }, [dispatch])

  const handleTranslateClick = () => {
    if (!processingContext.transcription) return

    const targetLanguagesConfig = selectedTargetLanguages.reduce(
      (acc, code) => {
        const language = languageOptions[code] // Direct access by code
        if (language) {
          acc[language.language_code] = language
        }
        return acc
      },
      {} as Record<string, LanguageConfig>,
    )
    const translateThunkArgs = {
      text: processingContext.transcription.transcript.text,
      targetLanguages: targetLanguagesConfig,
      originalLanguage: processingContext.transcription.transcript.language,
    }
    logger(
      `Translate button clicked with parameters -  targetLanguages: ${JSON.stringify(translateThunkArgs, null, 2)}`,
    )
    dispatch(translationTextThunk(translateThunkArgs))
  }
  const handleDownloadJSONClick = () => {
    logger(`[TranslationPanel] Downloading translation results...`)
    const json = JSON.stringify(processingContext.translation, null, 2)
    const blob = new Blob([json], { type: "application/json" })
    const url = URL.createObjectURL(blob)

    const a = document.createElement("a")
    a.href = url
    a.download = `${processingContext.originalFile?.name}_translation.json`
    a.click()

    URL.revokeObjectURL(url)
  }

  return (
    <ProcessingPanelLayout
      borderColor="#00aa3c"
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
        sx={{ mb: 2 }}
      >
        {!processingContext.transcription &&
          " No transcript available, transcribe audio first. "}
      </Typography>
      <Box sx={{ display: "flex", flexWrap: "wrap", gap: 1, mb: 2 }}>
        {selectedTargetLanguages.map(code => {
          const lang = Object.values(languageOptions).find(
            l => l.language_code === code,
          )
          return (
            <Chip
              key={code}
              label={lang?.language_name || code}
              onDelete={() => dispatch(toggleLanguage(code))}
            />
          )
        })}
      </Box>
      <IconButton onClick={() => setShowControls(!showControls)}>
        <SettingsIcon />
      </IconButton>
      <Drawer
        anchor="right"
        open={showControls}
        onClose={() => setShowControls(false)}
        sx={{ width: 300, flexShrink: 0 }}
        variant="persistent"
      >
        <Box sx={{ width: 300, p: 2 }}>
          <Typography variant="h6">Language Configuration</Typography>
          <TranslationControls />
        </Box>
      </Drawer>
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
            {processingContext.transcription?.transcript?.text}
          </Typography>
          <Typography variant="h6" sx={{ mb: 1 }}>
            Translations:
          </Typography>
          {Object.entries(processingContext.translation.translations).map(
            ([key, translation], i) => (
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
                <br />

                {!(translation.romanization_method === "NONE") && (
                  <>
                    <Typography
                      variant="body2"
                      sx={{
                        mt: 1,
                        color: extendedPaperbaseTheme.palette.text.secondary,
                        fontSize: "0.875rem",
                      }}
                    >
                      Romanized ({translation.romanization_method}):
                    </Typography>
                    <Typography
                      variant="body2"
                      sx={{
                        color: extendedPaperbaseTheme.palette.text.secondary,
                        fontSize: "0.875rem",
                      }}
                    >
                      {translation.romanized_text}
                    </Typography>
                  </>
                )}
              </Box>
            ),
          )}
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
            Download translation results
          </Button>
        </>
      )}
    </ProcessingPanelLayout>
  )
}

export default TranslationPanel
