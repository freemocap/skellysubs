import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import { Box, Button, Typography } from "@mui/material"
import { useDropzone } from "react-dropzone"
import DriveFileMoveIcon from "@mui/icons-material/DriveFileMove"
export const FileInput = ({
  onFileSelect,
}: {
  onFileSelect: (file: File) => void
}) => {
  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    accept: {
      "audio/*": [],
      "video/*": [],
    },
    multiple: false,
    onDrop: (files: File[]) => files[0] && onFileSelect(files[0]),
  })

  return (
    <Box
      {...getRootProps()}
      sx={{
        border: `2px dashed ${extendedPaperbaseTheme.palette.divider}`,
        borderRadius: 2,
        p: 4,
        textAlign: "center",
        cursor: "pointer",
        backgroundColor: isDragActive
          ? "rgba(255, 255, 255, 0.1)"
          : "transparent",
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
          color: extendedPaperbaseTheme.palette.text.primary,
          fontSize: "1.1rem",
          textTransform: "none",
          py: 2,
        }}
      >
        Select audio/video file
      </Button>
      <Typography
        variant="body1"
        sx={{ color: extendedPaperbaseTheme.palette.text.secondary, mt: 1 }}
      >
        {isDragActive ? "Drop it here!" : "or drag and drop a file"}
      </Typography>
      <Typography
        variant="caption"
        sx={{
          color: extendedPaperbaseTheme.palette.text.disabled,
          mt: 1,
          display: "block",
        }}
      >
        Supported formats: MP4, MOV, MP3, WAV
      </Typography>
    </Box>
  )
}
