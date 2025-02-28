import { useAppDispatch, useAppSelector } from "../../store/hooks"
import {
  selectIsTranscribeReady,
  selectProcessingContext,
} from "../../store/slices/processingStatusSlice"
import { Box, Button, Typography } from "@mui/material"
import { transcribeAudioThunk } from "../../store/thunks"

const TranscribeButton: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranscribeReady)
  const processingContext = useAppSelector(selectProcessingContext)

  const handleTranscribeClick = () => {
    console.log("Transcribe button clicked")
    dispatch(transcribeAudioThunk()) // No argument needed
  }

  return (
    <Box>
      <Button
        variant="contained"
        color="secondary"
        sx={{ mt: 2 }}
        onClick={handleTranscribeClick}
        disabled={!isReady}
      >
        Transcribe Audio
      </Button>

      {!processingContext.transcription && (
        <Typography>
          {JSON.stringify(processingContext.transcription, null, 2)}
        </Typography>
      )}
    </Box>
  )
}
export default TranscribeButton
