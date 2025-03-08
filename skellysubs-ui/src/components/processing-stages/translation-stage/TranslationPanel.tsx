import { Box, IconButton, Typography } from "@mui/material"
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
import type { LanguageConfig } from "../../../store/slices/translation-config/languageConfigSchemas"
import { logger } from "../../../utils/logger"
import SettingsIcon from "@mui/icons-material/Settings"
import {
  fetchLanguageConfigs,
  selectAvailableTargetLanguages,
  selectSelectedTargetLanguages,
} from "../../../store/slices/translation-config/translationConfigSlice"
import { RightPanelContext } from "../../../layout/BasePanelLayout"
import { translationTranscriptThunk } from "../../../store/thunks/translationTranscriptThunk"
import { LanguageChipsPanel } from "./LanguageChipsPanel"
import TranslatedTranscriptView from "./TranslatedTranscriptView"
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme"

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

  return (
    <ProcessingPanelLayout
      sx={{
        m: 3,
        p: 3,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        borderStyle: "solid",
        borderColor: "#00aa22",
        borderWidth: "1px",
        borderRadius: 2,
      }}
    >
      <Typography variant="body1" color="text.disabled" sx={{ m: 2 }}>
        {!processingContext.transcription &&
          "No transcript available, transcribe audio first."}
      </Typography>

      <LanguageChipsPanel />

      <IconButton
        onClick={toggleRightPanel}
        sx={{ position: "absolute", top: 8, right: 8, zIndex: 1 }}
      >
        <SettingsIcon />
      </IconButton>

      <ProcessingButton
        status={translationStatus}
        isReady={isReady}
        label="Translate Text"
        onClick={handleTranslateClick}
      />

      {processingContext.translation && (
        <>
          {processingContext.translation &&
            Object.entries(
              processingContext.translation.translated_transcripts,
            ).map(([languageName, translation]) => (
              <Box
                key={languageName}
                sx={{
                  width: "100%",
                  border: 1,
                  borderColor: extendedPaperbaseTheme.palette.primary.light,
                  borderRadius: 1,
                  p: 1,
                  my: 1,
                }}
              >
                <TranslatedTranscriptView
                  key={languageName}
                  languageName={languageName}
                  translation={translation}
                />
              </Box>
            ))}
        </>
      )}
    </ProcessingPanelLayout>
  )
}
export default TranslationPanel
