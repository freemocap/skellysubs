// File: src/components/ProcessingPipeline.tsx
import Slider from "@mui/material/Slider"
import { useAppSelector, useAppDispatch } from "../store/hooks"
import { Box, Typography } from "@mui/material"
import { setCurrentStage } from "../store/slices/processingStagesSlice"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import FileSelectionStage from "./FileSelectionStage"
import ProcessingButtons from "./ProcessingButtons"

const marks = [
    { value: 0, label: "Prepare File" },
    { value: 1, label: "Transcribe Audio" },
    { value: 2, label: "Translate Text" },
    { value: 3, label: "Match Words" },
]

export const ProcessingPipeline = () => {
  const dispatch = useAppDispatch()
  const { currentStage, stages } = useAppSelector(
    state => state.processingStagesReducer,
  )

  return (
    <Box
      sx={{
        width: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        bgcolor: extendedPaperbaseTheme.palette.primary.main,
        color: extendedPaperbaseTheme.palette.primary.contrastText,
        p: 4,
        mb: 4,
      }}
    >
      <ProcessingButtons />

        <Slider
            value={currentStage}
            marks={marks}
            min={0}
            max={marks.length - 1}
            sx={{
                mb: 4,
                width: "90%",
                '& .MuiSlider-markLabel': {
                    fontSize: { xs: '0.6rem', sm: '0.8rem', md: '1rem' },
                    whiteSpace: 'nowrap'
                }
            }}
            slotProps={{
                markLabel: { style: { transform: 'rotate(-45deg) translateX(-20%)' } }
            }}
        />

      {/* Add additional stage outputs here */}
      {stages[1].status === "completed" && (
        <Box sx={{ mt: 2, p: 2, bgcolor: "background.paper", width: "100%" }}>
          <Typography variant="h6">Transcription Results:</Typography>
          <pre>{JSON.stringify(stages[1].output, null, 2)}</pre>
        </Box>
      )}
    </Box>
  )
}
