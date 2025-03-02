import type React from "react"
import { Box, Button, Typography } from "@mui/material"
import { useDropzone } from "react-dropzone"
import DriveFileMoveIcon from "@mui/icons-material/DriveFileMove"
import FileDownloadDoneIcon from "@mui/icons-material/FileDownloadDone"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"
import { useAppSelector } from "../../store/hooks"
import { selectProcessingContext } from "../../store/slices/processingStatusSlice"
import FileDetails from "./FileDetails"

interface FileInputProps {
  onFileChange: (file: File) => void
}

const FileInput: React.FC<FileInputProps> = ({ onFileChange }) => {
  const processingContext = useAppSelector(selectProcessingContext)

  const handleFileSelect = (file: File) => {
    if (file) {
      onFileChange(file)
    }
  }

  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    accept: {
      "audio/*": [],
      "video/*": [],
    },
    multiple: false,
    onDrop: (files: File[]) => handleFileSelect(files[0]),
  })

  return (
    <Box
      {...getRootProps()}
      sx={{
        borderStyle: "dashed",
        borderRadius: 2,
        p: 4,
        textAlign: "center",
        cursor: "pointer",
        backgroundColor: isDragActive
          ? "rgba(70,131,134, .8)"
          : "rgba(70,131,134, .4)",
        transition: "background-color 0.3s ease",
        width: "100%",
        maxWidth: 500,
        mx: "auto",
      }}
    >
      <>
        {processingContext.originalFile ? (
          <>
            <Typography variant="h6" gutterBottom>
              Selected file: <br />
            </Typography>
            <Typography variant="body2" gutterBottom>
              {processingContext.originalFile.name}
            </Typography>
          </>
        ) : (
          <Typography variant="h6" gutterBottom>
            Select an audio or video file to get started!
          </Typography>
        )}
      </>
      <input {...getInputProps()} />

      <Button
        component="div"
        startIcon={
          processingContext.originalFile ? (
            <FileDownloadDoneIcon fontSize="large" />
          ) : (
            <DriveFileMoveIcon fontSize="large" />
          )
        }
        sx={{
          fontSize: ".8rem",
          textTransform: "none",
          color: extendedPaperbaseTheme.palette.primary.contrastText,
        }}
      >
        <>
          {processingContext.originalFile ? (
            <FileDetails avFile={processingContext.originalFile} />
          ) : (
            <Typography variant="body2" gutterBottom>
              Select a file...
            </Typography>
          )}
        </>
      </Button>
      <Typography variant="body2" sx={{ mt: 1 }}>
        {isDragActive ? "Drop it here!" : "(or drag and drop here)"}
      </Typography>
    </Box>
  )
}

export default FileInput
