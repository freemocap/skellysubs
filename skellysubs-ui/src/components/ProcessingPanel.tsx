// File: src/components/ProcessingPipeline.tsx
import { Box } from "@mui/material"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import FileSelectionBox from "./file-input/FileSelectionBox"
import TranscribeAudioPanel from "./processing-stages/TranscribeAudioPanel"
import TranslateTranscriptPanel from "./processing-stages/TranslateTranscriptPanel"

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
      <TranscribeAudioPanel />
      <TranslateTranscriptPanel />
    </Box>
  )
}
