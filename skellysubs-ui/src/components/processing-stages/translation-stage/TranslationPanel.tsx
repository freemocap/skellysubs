import {IconButton, Paper, Typography} from "@mui/material"
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
import { translationTranscriptThunk } from "../../../store/slices/processing-status/thunks/translationTranscriptThunk"
import { LanguageChipsPanel } from "./LanguageChipsPanel"
import { DownloadTranslationButton } from "./DownloadTranslationButton"
import { RichTreeView } from "@mui/x-tree-view"
import TranslatedTranscriptView from "./TranslatedTranscriptView";
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme";

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
              Object.entries(processingContext.translation.translated_transcripts).map(
                  ([languageName, translation]) => (
                      <Paper
                        key={languageName}
                        sx={{
                            width: "100%",
                            m: 2,
                            display: "flex",
                            flexDirection: "column",
                            alignItems: "flex-start",
                          borderStyle: "solid",
                            borderColor: extendedPaperbaseTheme.palette.primary.light,
                            backgroundColor : extendedPaperbaseTheme.palette.primary.dark,
                        }
                        }>
                      <TranslatedTranscriptView
                          key={languageName}
                          laguageName={languageName}
                          translation={translation}
                      />
                        </Paper>
                  )
              )}

          <DownloadTranslationButton onClick={handleDownloadJSONClick} />
        </>
      )}
    </ProcessingPanelLayout>
  )
}
export default TranslationPanel
