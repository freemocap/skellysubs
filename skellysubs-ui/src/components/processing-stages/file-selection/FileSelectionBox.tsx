import type React from "react"
import { Box, Typography } from "@mui/material"
import { useAppDispatch, useAppSelector } from "../../../store/hooks"
import {
  injectContextData,
  selectProcessingContext,
} from "../../../store/slices/processing-status/processingStatusSlice"
import FileSelectionPanel from "./FileSelectionPanel"
import FileDetails from "./FileDetails"
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme"
import { ffmpegService } from "../../../services/FfmpegService/useFfmpeg"
import { prepareFileThunk } from "../../../store/thunks/prepareFileThunk"

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
      <FileSelectionPanel onFileChange={handleFileChange} />
      {processingContext.mp3Audio && (
        <Box
          sx={{
            mt: 3,
            p: 2,
            border: "1px solid",
            borderColor: extendedPaperbaseTheme.palette.primary.light,
            borderRadius: 2,
          }}
        >
          <Box>
            <Typography variant="h6">
              Extracted Audio File: <br />{" "}
            </Typography>
            <Typography variant="body2">
              {processingContext.mp3Audio.name}{" "}
            </Typography>
          </Box>
          <FileDetails avFile={processingContext.mp3Audio} />
        </Box>
      )}
    </Box>
  )
}

export default FileSelectionBox
