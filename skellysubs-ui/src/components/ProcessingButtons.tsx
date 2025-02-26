import { Alert, Button, CircularProgress, Container } from "@mui/material"
import { useEffect } from "react"
import { useAppDispatch, useAppSelector } from "../store/hooks"
import {
  prepareFileThunk,
  transcribeAudioThunk,
} from "../store/slices/processingStagesSlice"
import { FileInput } from "./FileInput"
const ProcessingButtons = () => {
  const dispatch = useAppDispatch()
  const processingStages = useAppSelector(state => state.processingStagesReducer)
  const transcribeStage = processingStages.stages[1]

  const handleFileSelect = async (file: File) => {
    try {
      await dispatch(prepareFileThunk(file)).unwrap()
    } catch (error) {
      console.error("File preparation failed:", error)
    }
  }

  const handleTranscribe = () => {
    dispatch(transcribeAudioThunk())
  }

  return (
      <Container maxWidth="sm" className="my-8 rounded-lg bg-gray-50 p-6">
        <FileInput onFileSelect={handleFileSelect} />

        <Button
            onClick={handleTranscribe}
            variant="contained"
            color="secondary"
            disabled={transcribeStage.status !== 'ready'}
            style={{ marginTop: '16px', width: '100%', backgroundColor: '#a150d7' }}
        >
          {transcribeStage.status === 'processing' ? (
              <CircularProgress size={24} color="inherit" />
          ) : (
              'Transcribe Audio'
          )}
        </Button>

      {processingStages.stages[1].status === "completed" && (
        <Alert severity="success" sx={{ mt: 2 }}>
          Transcription completed! Results:{" "}
          {processingStages.stages[1].output?.text}
        </Alert>
      )}

      {processingStages.stages[1].status === "failed" && (
        <Alert severity="error" sx={{ mt: 2 }}>
          Error: {processingStages.stages[1].error}
        </Alert>
      )}
    </Container>
  )
}

export default ProcessingButtons
