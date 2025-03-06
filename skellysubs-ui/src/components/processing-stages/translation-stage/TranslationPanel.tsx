// src/components/processing-panel/translation/TranslationPanel.tsx

import { Box, Button, IconButton, Typography } from "@mui/material"
import AddCircleIcon from "@mui/icons-material/AddCircle"
import {
  ProcessingButton,
  ProcessingPanelLayout,
} from "../ProcessingPanelLayout"
import { useAppDispatch, useAppSelector } from "../../../store/hooks"
import {
  selectIsTranslateReady,
  selectProcessingContext,
} from "../../../store/slices/processing-status/processingStatusSlice"
import { useContext, useEffect } from "react"
import type { LanguageConfig } from "../../../schemas/languageConfigSchemas"
import { logger } from "../../../utils/logger"
import { translationTextThunk } from "../../../store/slices/processing-status/thunks/translationTextThunk"
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme"
import SettingsIcon from "@mui/icons-material/Settings"
import Chip from "@mui/material/Chip"
import {
  fetchLanguageConfigs,
  selectAvailableTargetLanguages,
  selectSelectedTargetLanguages,
  toggleLanguage,
} from "../../../store/slices/processing-configs/translationConfigSlice"
import { RightPanelContext } from "../../../layout/BasePanelLayout"
import { translationTranscriptThunk } from "../../../store/slices/processing-status/thunks/translationTranscriptThunk"

const TranslationPanel: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranslateReady)
  const processingContext = useAppSelector(selectProcessingContext)
  const translationStatus = useAppSelector(
    state => state.processing.stages.translation.status,
  )
  const availableTargetLanguages = useAppSelector(
    selectAvailableTargetLanguages,
  )
  const selectedTargetLanguages = useAppSelector(selectSelectedTargetLanguages)
  const { toggleRightPanel } = useContext(RightPanelContext)

  useEffect(() => {
    dispatch(fetchLanguageConfigs())
  }, [dispatch])

  const handleTranslateClick = () => {
    if (!processingContext.transcription) return

    // Match selected languages by their actual config keys
    const targetLanguagesConfig = selectedTargetLanguages.reduce(
      (acc, configKey) => {
        const language = availableTargetLanguages[configKey]
        if (language) {
          acc[configKey] = language
        }
        return acc
      },
      {} as Record<string, LanguageConfig>,
    )

    const translateThunkArgs = {
      transcript: processingContext.transcription.transcript,
      targetLanguages: targetLanguagesConfig,
    }
    logger(
      `Translate transcript button clicked with parameters - targetLanguages: ${JSON.stringify(translateThunkArgs, null, 2)}`,
    )
    dispatch(translationTranscriptThunk(translateThunkArgs))
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
        position: "relative",
      }}
    >
      <Typography
        variant="body1"
        color={extendedPaperbaseTheme.palette.text.disabled}
        sx={{ m: 2 }}
      >
        {!processingContext.transcription &&
          " No transcript available, transcribe audio first. "}
      </Typography>
      <Box sx={{ display: "flex", flexWrap: "wrap", gap: 1, mb: 2 }}>
        {selectedTargetLanguages.map(configKey => {
          const lang = availableTargetLanguages[configKey]
          return (
            <Chip
              key={configKey}
              label={lang?.language_name || configKey}
              onDelete={() => dispatch(toggleLanguage(configKey))}
              sx={{ backgroundColor: "#005d5d" }}
            />
          )
        })}
        <IconButton onClick={toggleRightPanel}>
          <AddCircleIcon />
        </IconButton>
      </Box>

      <Box
        sx={{
          position: "absolute",
          top: 8,
          right: 8,
          zIndex: 1,
        }}
      >
        <IconButton onClick={toggleRightPanel}>
          <SettingsIcon />
        </IconButton>
      </Box>

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
          {Object.entries(
            processingContext.translation.translated_transcripts,
          ).map(([languageCode, translatedTranscript]) => (
            <Box
              key={languageCode}
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
                {
                  translatedTranscript.translated_full_text
                    .translated_language_name
                }
                :
              </Typography>
              <Typography variant="body1">
                {translatedTranscript.translated_full_text.translated_text}
              </Typography>

              {translatedTranscript.translated_full_text.romanization_method !==
                "NONE" && (
                <>
                  <Typography
                    variant="body2"
                    sx={{
                      mt: 1,
                      color: extendedPaperbaseTheme.palette.text.secondary,
                      fontSize: "0.875rem",
                    }}
                  >
                    Romanized (
                    {
                      translatedTranscript.translated_full_text
                        .romanization_method
                    }
                    ):
                  </Typography>
                  <Typography
                    variant="body2"
                    sx={{
                      color: extendedPaperbaseTheme.palette.text.secondary,
                      fontSize: "0.875rem",
                    }}
                  >
                    {translatedTranscript.translated_full_text.romanized_text}
                  </Typography>
                </>
              )}

              <Typography variant="h6" sx={{ mt: 2, mb: 1 }}>
                Segments:
              </Typography>

              {Object.entries(translatedTranscript.translated_segments).map(
                ([segmentId, segment]) => (
                  <Box
                    key={segmentId}
                    sx={{
                      mb: 2,
                      p: 1,
                      borderLeft: "3px solid #00aa3c",
                    }}
                  >
                    <Typography variant="body2" sx={{ color: "#666" }}>
                      [{segment.start.toFixed(2)}s - {segment.end.toFixed(2)}s]
                    </Typography>
                    <Typography variant="body1" sx={{ mb: 1 }}>
                      {segment.original_segment_text}
                    </Typography>
                    <Typography variant="body1">
                      {segment.translated_text.translated_text}
                    </Typography>

                    {segment.translated_text.romanized_text && (
                      <Typography
                        variant="body2"
                        sx={{
                          color: extendedPaperbaseTheme.palette.text.secondary,
                          fontSize: "0.875rem",
                        }}
                      >
                        ({segment.translated_text.romanized_text})
                      </Typography>
                    )}
                  </Box>
                ),
              )}
            </Box>
          ))}
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
