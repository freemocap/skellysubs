import type React from "react"
import { Box, Button, Typography } from "@mui/material"
import { useDropzone } from "react-dropzone"
import DriveFileMoveIcon from "@mui/icons-material/DriveFileMove"

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
    },
    multiple: false,
    onDrop: (files: File[]) => handleFileSelect(files[0]),
  })

  return (
    <Box
      {...getRootProps()}
      sx={{
        border: "2px dashed #fff",
        borderRadius: 2,
        p: 4,
        textAlign: "center",
        cursor: "pointer",
        backgroundColor: isDragActive
          ? "rgba(255, 255, 255, 0.5)"
          : "rgba(255, 255, 255, 0.2)",
        transition: "background-color 0.3s ease",
        width: "100%",
        maxWidth: 500,
        mx: "auto",
      }}
    >
      <input {...getInputProps()} />
      <Button
        component="div"
        startIcon={<DriveFileMoveIcon fontSize="large" />}
        sx={{
          color: "#fff",
          fontSize: "1.1rem",
          textTransform: "none",
          py: 2,
        }}
      >
        Select audio file
      </Button>
      <Typography variant="body1" sx={{ color: "#ccc", mt: 1 }}>
        {isDragActive ? "Drop it here!" : "or drag and drop a file"}
      </Typography>
    </Box>
  )
}

export default FileInput
