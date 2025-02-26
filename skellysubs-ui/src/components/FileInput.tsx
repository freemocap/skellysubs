import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import { Box, Button, Typography } from "@mui/material"
import { useDropzone } from "react-dropzone"
import DriveFileMoveIcon from "@mui/icons-material/DriveFileMove"
import { useAppDispatch, useAppSelector } from "../store/hooks"
import { resetStages } from "../store/slices/processingStagesSlice"

export const FileInput = ({
  onFileSelect,
}: {
  onFileSelect: (file: File) => void
}) => {
  const dispatch = useAppDispatch()

  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    accept: {
      "audio/*": [],
      "video/*": [],
    },
    multiple: false,
    onDrop: (files: File[]) => files[0] && onFileSelect(files[0]),
  })
  const handleReset = () => {
    dispatch(resetStages())
  }

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

      {selectedFile ? (
        <Box>
          <Typography variant="h6">{selectedFile.name}</Typography>
          <Typography>Type: {selectedFile.type}</Typography>
          <Typography>
            Size: {Math.round(selectedFile.size / 1024)} KB
          </Typography>
          <Button
            variant="outlined"
            color="error"
            onClick={handleReset}
            sx={{ mt: 2 }}
          >
            Remove File
          </Button>
        </Box>
      ) : (
        <>
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
          </Typography>
        </>
      )}
    </Box>
  )
}
