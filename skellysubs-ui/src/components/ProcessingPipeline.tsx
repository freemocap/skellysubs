// File: src/components/ProcessingPipeline.tsx
import Slider from "@mui/material/Slider"
import { useAppSelector, useAppDispatch } from "../store/hooks"
import { Box, Typography } from "@mui/material"
import { setCurrentStage } from "../store/slices/processingStagesSlice"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import FileSelectionStage from "./FileSelectionStage"
import ProcessingButtons from "./ProcessingButtons"

const marks = [
  { value: 0, label: "Select Audio/Video File" },
  { value: 1, label: "Prepare File" },
  { value: 2, label: "Transcribe Audio" },
  { value: 3, label: "Translate Text" },
  { value: 4, label: "Match Words" },
  { value: 5, label: "Finalize Output" },
]

export const ProcessingPipeline = () => {
  const dispatch = useAppDispatch()
  const { currentStage, stages } = useAppSelector(
    state => state.processingStages,
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
        min={0}
        max={5}
        step={0}
        marks={marks}
        valueLabelDisplay="auto"
        sx={{ mb: 4, color: "secondary.main", width: "50%" }}
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
