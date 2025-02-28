// File: src/components/ProcessingPipeline.tsx
import { Box } from "@mui/material"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import FileSelectionBox from "./file-components/FileSelectionBox"
import TranscribeButton from "./button-components/TranscribeButton"
import type React from "react"
import { useAppSelector } from "../store/hooks"
import { selectStage } from "../store/slices/processingStatusSlice"

export const ProcessingPanel = () => {
  const filePreparationStage = useAppSelector(selectStage("filePreparation"))

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
      <TranscribeButton isReady={filePreparationStage.status === "completed"} />
    </Box>
  )
}
