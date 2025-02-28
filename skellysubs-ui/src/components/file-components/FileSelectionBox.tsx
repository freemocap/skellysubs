import type React from "react"
import { Box } from "@mui/material"
import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  injectContextData,
  prepareFileThunk,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import FileInput from "./FileInput"
import FileDetails from "./FileDetails"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"

const FileSelectionBox: React.FC = () => {
  const dispatch = useAppDispatch()
  const processingContext = useAppSelector(selectProcessingContext)

  const handleFileChange = (file: File) => {
    const fileURl = URL.createObjectURL(file)
    dispatch(
      injectContextData({
        key: "originalFile",
        data: {
          url: fileURl,
          name: file.name,
          type: file.type,
          size: file.size,
          bitrate: 0,
          duration: 0,
        },
      }),
    )
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

export default FileSelectionBox
