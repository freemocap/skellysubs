// ProcessingPipeline.tsx
import Slider from "@mui/material/Slider"
import { useAppSelector, useAppDispatch } from "../store/hooks"
import { Box } from "@mui/material"
import { setCurrentStage } from "../store/slices/processingStages"

const marks = [
  { value: 0, label: "Upload" },
  { value: 1, label: "Extract" },
  { value: 2, label: "Transcribe" },
]

export const ProcessingPipeline = ({
  fileType,
  onFileUpload,
  onExtractionComplete,
}) => {
  const dispatch = useAppDispatch()
  const { currentStage, stages } = useAppSelector(
    state => state.processingStages,
  )

  return (
    <Box sx={{ width: "100%" }}>
      <Slider
        value={currentStage}
        min={0}
        max={2}
        step={1}
        marks={marks}
        valueLabelDisplay="auto"
        sx={{ mb: 4 }}
        onChange={(_, value) => dispatch(setCurrentStage(value))}
      />

      {/* Existing step rendering */}
    </Box>
  )
}
