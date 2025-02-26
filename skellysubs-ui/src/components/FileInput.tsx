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
    const { stages } = useAppSelector(state => state.processingStagesReducer)

    // Get file info from the first processing stage output
    const originalFile = stages[0]?.output?.originalFile
    const fileReady = stages[0]?.status === "completed"

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

            {fileReady ? (
                <Box>
                    <Typography variant="h6">{originalFile?.name}</Typography>
                    <Typography>Type: {originalFile?.type}</Typography>
                    <Typography>
                        Size: {Math.round((originalFile?.size || 0) / 1024)} KB
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
                </>
            )}
        </Box>
    )
}