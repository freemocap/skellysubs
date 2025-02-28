import type React from "react"
import { Box, Button, Typography } from "@mui/material"
import type { AudioVisualFile } from "../../store/slices/processingStatusSlice"
import extendedPaperbaseTheme from "../../layout/paperbase_theme/paperbase-theme"

interface FileDetailsProps {
  originalFile?: AudioVisualFile
  mp3Audio: AudioVisualFile
}

const FileDetails: React.FC<FileDetailsProps> = ({
  originalFile,
  mp3Audio,
}) => (
  <Box sx={{ mt: 3 }}>
    {originalFile && (
      <Box sx={{ mb: 2 }}>
        <Typography variant="h6">Original File Details:</Typography>
        <Typography variant="body2">Name: {originalFile.name}</Typography>
        <Typography variant="body2">Type: {originalFile.type}</Typography>

        <Typography variant="body2">
          Size: {originalFile.size / (1024 * 1024)} MB
        </Typography>
        {originalFile.url && (
          <Button
            variant="contained"
            color="primary"
            component="a"
            href={originalFile.url}
            download={originalFile.name}
            sx={{
              backgroundColor: extendedPaperbaseTheme.palette.primary.light,
              borderColor: "#222222",
              borderStyle: "solid",
              borderWidth: "2px",
            }}
          >
            Download Original File
          </Button>
        )}
      </Box>
    )}

    <Typography variant="h6">Audio Details:</Typography>
    <Typography variant="body2">
      Size: {mp3Audio.size / (1024 * 1024)} MB
    </Typography>
    <Typography variant="body2">Bitrate: {mp3Audio.bitrate} kbps</Typography>
    <Typography variant="body2">
      Duration: {mp3Audio.duration || "N/A"} seconds
    </Typography>
    {mp3Audio.url && (
      <Button
        variant="contained"
        component="a"
        href={mp3Audio.url}
        download={`audio.mp3`}
        sx={{
          backgroundColor: extendedPaperbaseTheme.palette.primary.light,
          borderColor: "#222222",
          borderStyle: "solid",
          borderWidth: "2px",
        }}
      >
        Download MP3 Audio
      </Button>
    )}
  </Box>
)

export default FileDetails
