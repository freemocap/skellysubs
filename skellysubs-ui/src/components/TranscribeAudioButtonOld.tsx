import {
  Alert,
  Button,
  CircularProgress,
  Container,
  Typography,
} from "@mui/material"
import { useAppDispatch, useAppSelector } from "../store/hooks"
import { transcribeAudioThunk } from "../store/slices/processingStagesSlice"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"

const TranscribeAudioButton = () => {
  const dispatch = useAppDispatch()
  const processingStages = useAppSelector(
    state => state.processingStagesReducer,
  )
  const transcribeStage = processingStages.stages[1]

  const handleTranscribe = () => {
    dispatch(transcribeAudioThunk())
  }

  return (
    <Container
      maxWidth="sm"
      className="my-8 rounded-lg bg-gray-50 p-6 shadow-md"
      style={{
        justifyContent: "center",
        alignItems: "center",
        display: "flex",
      }}
    >
      <Button
        onClick={handleTranscribe}
        variant="contained"
        color="secondary"
        // disabled={transcribeStage.status !== "ready"}
        style={{
          margin: "1em",
          color: extendedPaperbaseTheme.palette.primary.contrastText,
          width: "50%",
          height: "8em",
          textSizeAdjust: "300%",
          borderColor: extendedPaperbaseTheme.palette.primary.contrastText,
        }}
      >
        {transcribeStage.status === "processing" ? (
          <CircularProgress size={24} color="inherit" />
        ) : (
          <Typography>Transcribe Audio</Typography>
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

export default TranscribeAudioButton
