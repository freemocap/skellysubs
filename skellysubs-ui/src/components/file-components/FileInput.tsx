import type React from "react"
import { Box, Button, Typography } from "@mui/material"
import { useDropzone } from "react-dropzone"
import DriveFileMoveIcon from "@mui/icons-material/DriveFileMove"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"

interface FileInputProps {
  onFileChange: (file: File) => void
}

const FileInput: React.FC<FileInputProps> = ({ onFileChange }) => {
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
        border: `2px dashed ${extendedPaperbaseTheme.palette.primary.light}`,
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
      <Typography variant="h6" gutterBottom>
        Select an audio or video file to get started!
      </Typography>
      <input {...getInputProps()} />
      <Button
        component="div"
        startIcon={<DriveFileMoveIcon fontSize="large" />}
        sx={{
          fontSize: "1.1rem",
          textTransform: "none",
          py: 2,
          color: extendedPaperbaseTheme.palette.primary.contrastText,
        }}
      >
        Select file...
      </Button>
      <Typography variant="body2" sx={{ mt: 1 }}>
        {isDragActive ? "Drop it here!" : "(or drag and drop here)"}
      </Typography>
    </Box>
  )
}

export default FileInput
