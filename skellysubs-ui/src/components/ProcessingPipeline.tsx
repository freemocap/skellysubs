// ProcessingPipeline.tsx
import React, { useMemo } from "react"
import { Box } from "@mui/material"
import UploadStep from "./UploadStep"
import ExtractStep from "./ExtractStep"
import TranscribeStep from "./TranscribeStep"

const ProcessingPipeline = ({
  currentStepIndex,
  fileType,
  onFileUpload,
  onExtractionComplete,
}) => {
  const steps = useMemo(() => {
    const s = []
    s.push({ component: UploadStep, props: { onFileUpload } })
    if (fileType === "video") {
      s.push({ component: ExtractStep, props: { onExtractionComplete } })
    }
    s.push({ component: TranscribeStep, props: {} })
    return s
  }, [fileType, onFileUpload, onExtractionComplete])

  return (
    <Box sx={{ overflow: "hidden", width: "100%", maxWidth: "800px" }}>
      <Box
        sx={{
          display: "flex",
          transform: `translateX(-${currentStepIndex * 100}%)`,
          transition: "transform 0.5s ease-in-out",
          width: `${steps.length * 100}%`,
        }}
      >
        {steps.map((step, index) => (
          <Box
            key={index}
            sx={{ width: `${100 / steps.length}%`, flexShrink: 0 }}
          >
            <step.component {...step.props} />
          </Box>
        ))}
      </Box>
    </Box>
  )
}

export default ProcessingPipeline
