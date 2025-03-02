import type React from "react"
import { Box, Button, Typography } from "@mui/material"
import type { AudioVisualFile } from "../../store/slices/processingStatusSlice"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"

interface FileDetailsProps {
  avFile: AudioVisualFile
}

const FileDetails: React.FC<FileDetailsProps> = ({ avFile }) => (
  <Box sx={{ mt: 3 }}>
    <Typography variant="body2">
      Size: {avFile.size / (1024 * 1024)} MB
    </Typography>
    <Typography variant="body2">Bitrate: {avFile.bitrate} kbps</Typography>
    <Typography variant="body2">
      Duration: {avFile.duration || "N/A"} seconds
    </Typography>
    {avFile.url && (
      <Button
        variant="contained"
        component="a"
        href={avFile.url}
        download={avFile.name}
        sx={{
          backgroundColor: extendedPaperbaseTheme.palette.primary.light,
          borderColor: "#222222",
          borderStyle: "solid",
          borderWidth: "1px",
        }}
      >
        Download
      </Button>
    )}
  </Box>
)

export default FileDetails
