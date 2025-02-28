import type React from "react"
import { Box } from "@mui/material"
import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  injectContextData,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import FileInput from "./FileInput"
import FileDetails from "./FileDetails"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"
import { prepareFileThunk } from "../../store/thunks"
import { ffmpegService } from "../../services/FfmpegService/useFfmpeg"

const FileSelectionBox: React.FC = () => {
  const dispatch = useAppDispatch()
  const processingContext = useAppSelector(selectProcessingContext)

  const handleFileChange = async (file: File) => {
    if (!ffmpegService.isLoaded) await ffmpegService.loadFfmpeg()

    const { bitrate: ogFileBitrate, duration: ogFileDuration } =
      await ffmpegService.getAvFileDetails(file)
    const fileURl = URL.createObjectURL(file)
    dispatch(
      injectContextData({
        key: "originalFile",
        data: {
          url: fileURl,
          name: file.name,
          type: file.type,
          size: file.size,
          bitrate: ogFileBitrate,
          duration: ogFileDuration,
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
        <FileDetails avFile={processingContext.mp3Audio} />
      )}
    </Box>
  )
}

export default FileSelectionBox
