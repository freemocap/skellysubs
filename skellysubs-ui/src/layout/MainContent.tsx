// BaseContent.tsx
import React from "react"
import Box from "@mui/material/Box"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Footnote } from "../components/Footnote"
import { ProcessingPanel } from "../components/ProcessingPanel"
import WelcomeContent from "../components/WelcomeContent"
import VideoSubtitleEditor from "../components/VideoSubtitleEditor"

export const MainContent = () => {
  return (
    <Box
      sx={{
        py: 6,
        px: 4,
        flex: 1,
        bgcolor: extendedPaperbaseTheme.palette.primary.main,
        borderColor: extendedPaperbaseTheme.palette.divider,
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        display: "flex",
        overflow: "auto",
      }}
    >
      <WelcomeContent />

      <ProcessingPanel />
      <VideoSubtitleEditor />
      <Footnote />
    </Box>
  )
}
