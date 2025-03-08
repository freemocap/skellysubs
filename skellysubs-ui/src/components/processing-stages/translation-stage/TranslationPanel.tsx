import {
  Box,
  IconButton,
  Typography,
  TextField,
  Button,
  Tooltip,
  Stack,
} from "@mui/material"
import {
  ProcessingButton,
  ProcessingPanelLayout,
} from "../ProcessingPanelLayout"
import { useAppDispatch, useAppSelector } from "../../../store/hooks"
import {
  selectIsTranslateReady,
  selectProcessingContext,
} from "../../../store/slices/processing-status/processingStatusSlice"
import { useContext, useEffect, useState } from "react"  // Added useState
import type { LanguageConfig } from "../../../store/slices/translation-config/languageConfigSchemas"
import { logger } from "../../../utils/logger"
import SettingsIcon from "@mui/icons-material/Settings"
import RefreshIcon from '@mui/icons-material/Refresh'
import { Search } from "@mui/icons-material"
import {
  fetchLanguageConfigs,
  selectAvailableTargetLanguages,
  selectSelectedTargetLanguages,
  toggleLanguage,
} from "../../../store/slices/translation-config/translationConfigSlice"
import { RightPanelContext } from "../../../layout/BasePanelLayout"
import { translationTranscriptThunk } from "../../../store/thunks/translationTranscriptThunk"
import { LanguageChipsPanel } from "./LanguageChipsPanel"
import TranslatedTranscriptView from "./TranslatedTranscriptView"
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme"
import {LanguageSearchField} from "./language-configs/LanguageSearchField";
import {LanguageAutocompleteField} from "./language-configs/LanguageAutocompleteField";

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
  const [searchQuery, setSearchQuery] = useState("")

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

  const handleRandomSelection = () => {
    // Clear current selection
    selectedTargetLanguages.forEach(lang => {
      dispatch(toggleLanguage(lang))
    })

    // Get all available languages
    const availableLanguages = Object.keys(availableTargetLanguages)

    // Randomly select 3 unique languages
    const randomLanguages = []
    while (randomLanguages.length < 3 && availableLanguages.length > 0) {
      const randomIndex = Math.floor(Math.random() * availableLanguages.length)
      const selectedLang = availableLanguages.splice(randomIndex, 1)[0]
      randomLanguages.push(selectedLang)
      dispatch(toggleLanguage(selectedLang))
    }
  }

  const filteredTranslations = processingContext.translation
    ? Object.entries(processingContext.translation.translated_transcripts)
        .filter(([languageName]) =>
          languageName.toLowerCase().includes(searchQuery.toLowerCase())
        )
    : []


  return (
    <ProcessingPanelLayout
      sx={{
        m: 3,
        p: 3,
        display: "flex",
        flexDirection: "column",
        alignItems: "stretch",
        borderStyle: "solid",
        borderColor: "#00aa22",
        borderWidth: "1px",
        borderRadius: 2,
      }}
    >
      {/* Header Section */}
      <Box
        sx={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          mb: 3,
          position: "relative",
        }}
      >
        <Typography
          variant="h5"
          sx={{
            color: theme => theme.palette.primary.contrastText,
            fontWeight: "bold",
            mr: 2,
          }}
        >
          Translation Panel
        </Typography>

        <Stack direction="row" spacing={2} alignItems="center">
          <Tooltip title="Randomly select 3 languages">
            <Button
              variant="outlined"
              startIcon={<RefreshIcon />}
              onClick={handleRandomSelection}
              size="small"
              sx={{ color: theme => theme.palette.text.secondary,
                  border: theme => `1px solid ${theme.palette.text.secondary}`
            }}

            >
              Random Selection
            </Button>
          </Tooltip>

          <Tooltip title="Configure languages">
            <Button
              variant="outlined"
              startIcon={<SettingsIcon />}
              onClick={toggleRightPanel}
              size="small"
              sx={{  color: theme => theme.palette.text.secondary,
                      border: theme => `1px solid ${theme.palette.text.secondary}`
                  }}
            >
              Open Language Panel
            </Button>
          </Tooltip>
        </Stack>
      </Box>

      <LanguageAutocompleteField
        searchQuery={searchQuery}
        setSearchQuery={setSearchQuery}
        placeholder="Search available languages..."
      />

      <LanguageChipsPanel />
      <Typography variant="body1" color="text.disabled" sx={{ mb: 2 }}>
        {!processingContext.transcription &&
          "No transcript available, transcribe audio first."}
      </Typography>
      <ProcessingButton
        status={translationStatus}
        isReady={isReady}
        label="Translate Text"
        onClick={handleTranslateClick}
      />

      {processingContext.translation && (
        <>
          <Box
            sx={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              mt: 4,
              mb: 2,
            }}
          >
            <Typography variant="h6" color="primary">
              Translated Transcript(s)
            </Typography>
          </Box>

          {filteredTranslations.map(([languageName, translation]) => (
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
