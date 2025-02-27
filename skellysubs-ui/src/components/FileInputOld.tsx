import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import { Box, Button, Typography } from "@mui/material"
import { useDropzone } from "react-dropzone"
import DriveFileMoveIcon from "@mui/icons-material/DriveFileMove"
import { useAppDispatch } from "../store/hooks"
import { prepareFileThunk } from "../store/slices/processingStatusSlice"

export const FileInputOld = () => {
  const dispatch = useAppDispatch()

  const handleFileSelect = async (file: File) => {
    try {
      console.log(
        `File selected: ${file.name} (${(file.size / 1024) * 1024} MB) - ${file.type} - dispatching 'prepareFileThunk'...`,
      )
      await dispatch(prepareFileThunk(file)).unwrap()
    } catch (error) {
      console.error("File preparation failed:", error)
    }
  }

  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    accept: {
      "audio/*": [],
      "video/*": [],
    },
    multiple: false,
    onDrop: (files: File[]) => files[0] && handleFileSelect(files[0]),
  })

  return (
    <Box
      {...getRootProps()}
      sx={{
        border: `2px dashed ${extendedPaperbaseTheme.palette.primary.contrastText}`,
        borderRadius: 2,
        p: 4,
        textAlign: "center",
        cursor: "pointer",
        backgroundColor: isDragActive
          ? "rgba(255, 255, 255, .5)"
          : "rgba(255, 255, 255, .2)",
        transition: "background-color 0.3s ease",
        width: "100%",
        maxWidth: 500,
        mx: "auto",
      }}
    >
      <input {...getInputProps()} />

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
      </>
    </Box>
  )
}
