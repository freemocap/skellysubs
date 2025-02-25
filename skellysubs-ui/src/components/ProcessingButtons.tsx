import { Button, CircularProgress, Container, Alert } from "@mui/material"
import FileInput from "./FileInput"
import { useEffect } from "react"
import { useAppSelector, useAppDispatch } from "../store/hooks"
import { setSelectedFile } from "../store/slices/appState"
import { prepareFileThunk, transcribeAudioThunk } from "../store/slices/processingStagesSlice"
import { useFfmpegContext } from "../services/FfmpegService/FfmpegContext"

const ProcessingButtons = () => {
    const dispatch = useAppDispatch()
    const { isLoaded } = useFfmpegContext()
    const selectedFile = useAppSelector(state => state.appState.selectedFile)
    const processingStages = useAppSelector(state => state.processingStages)
    const currentStageStatus = processingStages.stages[1]?.status

    // Automatically start processing when file is selected
    useEffect(() => {
        if (selectedFile) {
            dispatch(prepareFileThunk())
        }
    }, [selectedFile, dispatch])

    const handleTranscribeButtonClicked = () => {
        dispatch(transcribeAudioThunk())
    }

    return (
        <Container maxWidth="sm" className="my-8 rounded-lg bg-gray-50 p-6">
            <FileInput
                onFileChange={(file) => dispatch(setSelectedFile(file))}
                acceptedFormats=".mp3,.wav"
            />

            <Button
                onClick={handleTranscribeButtonClicked}
                variant="contained"
                color="secondary"
                disabled={currentStageStatus !== 'ready' || processingStages.stages[1].status === 'processing'}
                style={{ marginTop: "16px", width: "100%", backgroundColor: "#d75056" }}
            >
                {processingStages.stages[1].status === 'processing' ? (
                    <CircularProgress size={24} color="inherit" />
                ) : (
                    "Transcribe Audio"
                )}
            </Button>

            {processingStages.stages[1].status === 'completed' && (
                <Alert severity="success" sx={{ mt: 2 }}>
                    Transcription completed! Results: {processingStages.stages[1].output?.text}
                </Alert>
            )}

            {processingStages.stages[1].status === 'failed' && (
                <Alert severity="error" sx={{ mt: 2 }}>
                    Error: {processingStages.stages[1].error}
                </Alert>
            )}
        </Container>
    )
}

export default ProcessingButtons