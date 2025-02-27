import type React from "react"
import { useState } from "react"
import { Box, Paper, Typography } from "@mui/material"
import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  prepareFileThunk,
  selectProcessingContext,
  selectStage,
} from "../../store/slices/processingStatusSlice"
import FileInput from "./FileInput"
import FileDetails from "./FileDetails"
import TranscribeButton from "../button-components/TranscribeButton"

const FileUploader: React.FC = () => {
  const dispatch = useAppDispatch()
  const [selectedFile, setSelectedFile] = useState<File | null>(null)

  const processingContext = useAppSelector(selectProcessingContext)
  const mp3Audio = processingContext.mp3Audio
  const originalFile = processingContext.originalFile
  const filePreparationStage = useAppSelector(selectStage("filePreparation"))

  const handleFileChange = (file: File) => {
    setSelectedFile(file)
    dispatch(prepareFileThunk(file))
  }

  return (
    <Box
      sx={{
        bgcolor: "background.default",
        color: "text.primary",
        p: 3,
        textAlign: "center",
      }}
    >
      <Paper elevation={3} sx={{ p: 3, bgcolor: "background.paper" }}>
        <Typography variant="h5" gutterBottom>
          Audio File Uploader
        </Typography>
        <FileInput onFileChange={handleFileChange} />
        {mp3Audio && <FileDetails mp3Audio={mp3Audio} />}
        <TranscribeButton
          file={processingContext.mp3Audio}
          isReady={filePreparationStage.status === "completed"}
        />
      </Paper>
    </Box>
  )
}

export default FileUploader
