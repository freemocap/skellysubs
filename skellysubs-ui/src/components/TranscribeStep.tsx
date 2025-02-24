// TranscribeStep.tsx
import { Typography, Container } from "@mui/material"

const TranscribeStep = () => {
  return (
    <Container maxWidth="sm" sx={{ py: 4 }}>
      <Typography variant="h4">Transcribe Audio</Typography>
      <Typography sx={{ mt: 2 }}>
        Transcription processing UI goes here...
      </Typography>
    </Container>
  )
}

export default TranscribeStep
