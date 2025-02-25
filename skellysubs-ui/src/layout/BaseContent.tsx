// BaseContent.tsx
import React, { useState } from "react"
import Box from "@mui/material/Box"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Footnote } from "../components/Footnote"
import { ProcessingPipeline } from "../components/ProcessingStages"
import {useAppDispatch, useAppSelector} from "../store/hooks";

export const BaseContent = () => {
    const dispatch = useAppDispatch()
    const currentStage = useAppSelector(state => state.processingStages.currentStage)
    const [fileType, setFileType] = useState<"audio" | "video" | null>(null)

    const handleFileUpload = (file: File) => {
        const type = file.type.startsWith("video/") ? "video" : "audio"
        // CHANGED: Dispatch to Redux store
        dispatch(setSelectedFile(file))
        setFileType(type)
    }



  return (
    <Box
      sx={{
        py: 6,
        px: 4,
        flex: 1,
        height: "100%",
        bgcolor: extendedPaperbaseTheme.palette.primary.main,
        borderColor: extendedPaperbaseTheme.palette.divider,
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        display: "flex",
      }}
    >
      <ProcessingPipeline
        fileType={fileType}
        onFileUpload={handleFileUpload}
        onExtractionComplete={handleExtractionComplete}
      />
      <Box component="footer" sx={{ p: 1 }}>
        <Footnote />
      </Box>
    </Box>
  )
}
