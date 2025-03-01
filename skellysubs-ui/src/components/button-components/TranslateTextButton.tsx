import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  selectIsTranslateReady,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import { Box, Button, Typography } from "@mui/material"
import { transcribeAudioThunk, translateTextThunk } from "../../store/thunks"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"
import type React from "react"

const TranslateTextButton: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranslateReady)
  const processingContext = useAppSelector(selectProcessingContext)

  const handleTranslateClick = () => {
    console.log("Translate button clicked")
    dispatch(translateTextThunk()) // No argument needed
  }

  const handleDownloadClick = () => {
    const json = JSON.stringify(processingContext.translation, null, 2)
    const blob = new Blob([json], { type: "application/json" })
    const url = URL.createObjectURL(blob)

    const a = document.createElement("a")
    a.href = url
    a.download = `${processingContext.originalFile?.name}_translation.json`
    a.click()

    // Clean up the URL object
    URL.revokeObjectURL(url)
  }

  return (
    <Box
      sx={{
        m: 3,
        p: 3,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        borderStyle: "solid",
        borderColor: "#00aa3c",
        borderWidth: "1px",
        borderRadius: 2,
      }}
    >
      <Button
        variant="contained"
        color="secondary"
        sx={{ m: 2 }}
        onClick={handleTranslateClick}
        disabled={!isReady}
      >
        Translate Text
      </Button>

      {processingContext.translation && (
        <>
          <Typography>
            {JSON.stringify(processingContext.translation)}
          </Typography>
          <Button
            variant="contained"
            onClick={handleDownloadClick}
            sx={{
              backgroundColor: extendedPaperbaseTheme.palette.primary.light,
              borderColor: "#222222",
              borderStyle: "solid",
              borderWidth: "1px",
            }}
          >
            Download translation results
          </Button>
        </>
      )}
    </Box>
  )
}
export default TranslateTextButton
