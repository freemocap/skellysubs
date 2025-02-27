import type React from "react"
import { Box, Typography } from "@mui/material"

interface AudioVisualFile {
    url: string
    size: number
    bitrate: number
    duration?: number
}

interface FileDetailsProps {
    originalFile?: File;
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
                <Typography variant="body2">Size: {originalFile.size} bytes</Typography>
            </Box>
        )}

        <Typography variant="h6">Audio Details:</Typography>
        <Typography variant="body2">URL: {mp3Audio.url}</Typography>
        <Typography variant="body2">Size: {mp3Audio.size} bytes</Typography>
        <Typography variant="body2">Bitrate: {mp3Audio.bitrate} kbps</Typography>
        <Typography variant="body2">
            Duration: {mp3Audio.duration || "N/A"} seconds
        </Typography>
    </Box>
)

export default FileDetails