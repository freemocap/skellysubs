// File: src/components/ProcessingStages.tsx
import Slider from "@mui/material/Slider"
import { useAppSelector, useAppDispatch } from "../store/hooks"
import { Box } from "@mui/material"
import { setCurrentStage } from "../store/slices/processingStages"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"
import FileSelectionStage from "./FileSelectionStage"

const marks = [
  { value: 0, label: "Upload" },
  { value: 1, label: "Extract" },
  { value: 2, label: "Transcribe" },
]

export const ProcessingPipeline = ({}) => {
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
      <Slider
        value={currentStage}
        min={0}
        max={2}
        step={1}
        marks={marks}
        valueLabelDisplay="auto"
        sx={{ mb: 4, color: "secondary.main", width: "50%" }}
      />
      <FileSelectionStage />
    </Box>
  )
}
