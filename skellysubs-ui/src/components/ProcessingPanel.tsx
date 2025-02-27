// File: src/components/ProcessingPipeline.tsx
import { Box, Typography } from "@mui/material"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import TranscribeAudioButton from "./TranscribeAudioButton"
import { FileInputOld } from "./FileInputOld"

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
      <FileInputOld />
      <TranscribeAudioButton />
    </Box>
  )
}
