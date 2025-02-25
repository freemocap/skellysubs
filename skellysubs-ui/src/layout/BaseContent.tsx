// BaseContent.tsx
import React, { useState } from "react"
import { useDropzone } from "react-dropzone"
import Box from "@mui/material/Box"
import Button from "@mui/material/Button"
import Typography from "@mui/material/Typography"
import UploadFileIcon from "@mui/icons-material/UploadFile"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Footnote } from "../components/Footnote"
import { useAppDispatch, useAppSelector } from "../store/hooks"
import { setSelectedFile } from "../store/slices/appState"
import { ProcessingPipeline } from "../components/ProcessingPipeline"

const FileUpload = ({ onFileSelect }: { onFileSelect: (file: File) => void }) => {
    const { getRootProps, getInputProps, isDragActive } = useDropzone({
        accept: {
            "audio/*": [".mp3", ".wav"],
            "video/*": [".mp4", ".mov"],
        },
        multiple: false,
        onDrop: files => files[0] && onFileSelect(files[0]),
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
                backgroundColor: isDragActive ? "rgba(255, 255, 255, 0.1)" : "transparent",
                transition: "background-color 0.3s ease",
                width: "100%",
                maxWidth: 500,
                mx: "auto",
            }}
        >
            <input {...getInputProps()} />
            <Button
                component="div"
                startIcon={<UploadFileIcon fontSize="large" />}
                sx={{
                    color: extendedPaperbaseTheme.palette.text.primary,
                    fontSize: "1.1rem",
                    textTransform: "none",
                    py: 2,
                }}
            >
                Select audio/video file
            </Button>
            <Typography variant="body1" sx={{ color: extendedPaperbaseTheme.palette.text.secondary, mt: 1 }}>
                {isDragActive ? "Drop it here!" : "or drag and drop a file"}
            </Typography>
            <Typography variant="caption" sx={{ color: extendedPaperbaseTheme.palette.text.disabled, mt: 1, display: "block" }}>
                Supported formats: MP4, MOV, MP3, WAV
            </Typography>
        </Box>
    )
}

export const BaseContent = () => {
    const dispatch = useAppDispatch()
    const currentStage = useAppSelector(state => state.processingStages.currentStage)
    const [fileType, setFileType] = useState<"audio" | "video" | null>(null)

    const handleFileSelect = (file: File) => {
        const type = file.type.startsWith("video/") ? "video" : "audio"
        dispatch(setSelectedFile(file))
        setFileType(type)
    }

    return (
        <Box
            sx={{
                py: 6,
                px: 4,
                flex: 1,
                height: "100%",
                bgcolor: extendedPaperbaseTheme.palette.primary.main,
                borderColor: extendedPaperbaseTheme.palette.divider,
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                display: "flex",
            }}
        >
            <Box sx={{ mb: 4, textAlign: "center" }}>
                <Typography
                    variant="h4"
                    sx={{
                        color: extendedPaperbaseTheme.palette.text.primary,
                        mb: 2,
                        fontWeight: 500,
                    }}
                >
                    Audio Processing Pipeline
                </Typography>
                <Typography
                    variant="body1"
                    sx={{
                        color: extendedPaperbaseTheme.palette.text.secondary,
                        mb: 4,
                        maxWidth: 600,
                        mx: "auto",
                    }}
                >
                    Upload your audio or video file to begin processing. We'll handle transcription, translation, and word matching automatically.
                </Typography>
                <FileUpload onFileSelect={handleFileSelect} />
            </Box>

            <ProcessingPipeline />

            <Box component="footer" sx={{ p: 1, mt: 4 }}>
                <Footnote />
            </Box>
        </Box>
    )
}