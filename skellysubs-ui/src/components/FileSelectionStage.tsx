// ExtractStep.tsx (modified from AudioExtractor.tsx)
import React from "react"
import { Box, Typography } from "@mui/material"
import ProcessingButtons from "./ProcessingButtons"

const FileSelectionStage = () => {
  return (
    <Box
      sx={{
        width: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        p: 4,
        mb: 4,
      }}
    >
      <Typography variant="h5">
        Select a video or audio file to get started.
      </Typography>

      <ProcessingButtons />
    </Box>
  )
}

export default FileSelectionStage
