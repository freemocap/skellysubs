// BaseContent.tsx
import React from "react"
import Box from "@mui/material/Box"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Footnote } from "../components/Footnote"
import { ProcessingPanel } from "../components/ProcessingPanel"
import WelcomeContent from "../components/WelcomeContent"

export const BaseContent = () => {
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
      <WelcomeContent />

      <ProcessingPanel />

      <Box component="footer" sx={{ p: 1, mt: 4 }}>
        <Footnote />
      </Box>
    </Box>
  )
}
