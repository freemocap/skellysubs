import type React from "react"
import { Button } from "@mui/material"
import { useAppDispatch } from "../../store/hooks"
import { transcribeAudioThunk } from "../../store/slices/processingStatusSlice"

interface TranscribeButtonProps {
  file: File | null
  isReady: boolean
}

const TranscribeButton: React.FC<TranscribeButtonProps> = ({
  file,
  isReady,
}) => {
  const dispatch = useAppDispatch()

  const handleTranscribeClick = () => {
    if (file) {
      dispatch(transcribeAudioThunk(file))
    }
  }

  return (
    <Button
      variant="contained"
      color="primary"
      sx={{ mt: 2 }}
      onClick={handleTranscribeClick}
      disabled={!isReady}
    >
      Transcribe Audio
    </Button>
  )
}

export default TranscribeButton
