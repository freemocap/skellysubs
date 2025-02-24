import React from "react"
import Box from "@mui/material/Box"
import WelcomeContent from "../components/WelcomeContent"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Footnote } from "../components/Footnote"

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
      <Box component="footer" sx={{ p: 1 }}>
        <Footnote />
      </Box>
    </Box>
  )
}
