// File: src/components/ProcessingPipeline.tsx
import { Box } from "@mui/material"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import FileSelectionBox from "./file-components/FileSelectionBox"
import TranscribeAudioButton from "./button-components/TranscribeAudioButton"
import TranslateTextButton from "./button-components/TranslateTextButton"

export const ProcessingPanel = () => {
  return (
    <Box
      sx={{
        width: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        bgcolor: extendedPaperbaseTheme.palette.primary.main,
        color: extendedPaperbaseTheme.palette.primary.contrastText,
        p: 4,
        mb: 4,
      }}
    >
      <FileSelectionBox />
      <TranscribeAudioButton />
      <TranslateTextButton />
    </Box>
  )
}
