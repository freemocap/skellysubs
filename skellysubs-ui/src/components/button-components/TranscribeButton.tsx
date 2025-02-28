import { useAppDispatch, useAppSelector } from "../../store/hooks"
import { selectIsTranscribeReady } from "../../store/slices/processingStatusSlice"
import { Button } from "@mui/material"
import { transcribeAudioThunk } from "../../store/thunks"

const TranscribeButton: React.FC = () => {
  const dispatch = useAppDispatch()
  const isReady = useAppSelector(selectIsTranscribeReady)

  const handleTranscribeClick = () => {
    console.log("Transcribe button clicked")
    dispatch(transcribeAudioThunk()) // No argument needed
  }

  return (
    <Button
      variant="contained"
      color="secondary"
      sx={{ mt: 2 }}
      onClick={handleTranscribeClick}
      disabled={!isReady}
    >
      Transcribe Audio
    </Button>
  )
}
export default TranscribeButton
