// File: src/components/ProcessingPipeline.tsx
import { Box } from "@mui/material"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import FileSelectionBox from "./processing-stages/file-input-stage/FileSelectionBox"
import TranscriptionPanel from "./processing-stages/transcription-stage/TranscriptionPanel"
import TranslationPanel from "./processing-stages/TranslationPanel"

export const ProcessingPanel = () => {
  return (
    <Box
      sx={{
        width: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <FileSelectionBox />
      <TranscriptionPanel />
      <TranslationPanel />
    </Box>
  )
}
