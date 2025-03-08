// TranscriptionControls.tsx
import { Box, IconButton, Tooltip, Typography } from "@mui/material"
import { Info } from "@mui/icons-material"
import { useState } from "react"
import { TranscriptionLanguageSelector } from "./TranscriptionLanguageSelector"
import { PromptEditDrawer } from "../../PromptEditDrawer"

export const TranscriptionControls = ({
  language,
  setLanguage,
  prompt,
  setPrompt,
}: {
  language: string
  setLanguage: (v: string) => void
  prompt: string
  setPrompt: (v: string) => void
}) => {
  const [drawerOpen, setDrawerOpen] = useState(false)

  return (
    <Box
      sx={{ display: "flex", flexDirection: "column", gap: 2, width: "100%" }}
    >
      <TranscriptionLanguageSelector value={language} onChange={setLanguage} />

      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        <Typography variant="body2" color="textSecondary">
          Transcription Prompt:
        </Typography>
        <Tooltip title="Click to add optional context for the transcription (e.g., proper names, technical terms)">
          <IconButton
            size="small"
            onClick={() => setDrawerOpen(true)}
            sx={{ border: "1px solid", borderColor: "divider" }}
          >
            <Info fontSize="small" />
          </IconButton>
        </Tooltip>
      </Box>

      <PromptEditDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        prompt={prompt}
        setPrompt={setPrompt}
      />
    </Box>
  )
}
