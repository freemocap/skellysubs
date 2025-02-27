import type React from "react"
import { Box, Paper, Typography } from "@mui/material"
import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  injectContextData,
  prepareFileThunk,
  selectProcessingContext,
  selectStage,
} from "../../store/slices/processingStatusSlice"
import FileInput from "./FileInput"
import FileDetails from "./FileDetails"
import TranscribeButton from "../button-components/TranscribeButton"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"

const FileInputBox: React.FC = () => {
  const dispatch = useAppDispatch()
  const processingContext = useAppSelector(selectProcessingContext)

  const handleFileChange = (file: File) => {
    dispatch(injectContextData({ key: "originalFile", data: file }))
    dispatch(prepareFileThunk(file))
  }

  return (
    <Box
      sx={{
        p: 3,
        textAlign: "center",
        bgcolor: extendedPaperbaseTheme.palette.primary.main,
      }}
    >
      <FileInput onFileChange={handleFileChange} />
      {processingContext.mp3Audio && (
        <FileDetails
          mp3Audio={processingContext.mp3Audio}
          originalFile={processingContext.originalFile}
        />
      )}
    </Box>
  )
}

export default FileInputBox
