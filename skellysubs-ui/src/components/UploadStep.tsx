// UploadStep.tsx
import React from "react"
import { Box, Typography } from "@mui/material"
import FileInput from "./FileInput"
import { processFileUpload } from "../store/slices/processingStagesSlice"
import { useAppDispatch } from "../store/hooks"

const logoUrl = `${window.location.origin}/logo/skellysubs-logo.png`

const UploadStep = ({ onFileUpload }) => {
  const dispatch = useAppDispatch()

  const handleFileChange = (file: File) => {
    dispatch(processFileUpload(file))
  }

  return (
    <>
      <Box
        sx={{
          width: "40vmin",
          height: "40vmin",
          backgroundImage: `url(${logoUrl})`,
          backgroundSize: "contain",
          backgroundRepeat: "no-repeat",
          animation: "App-logo-float infinite 3s ease-in-out",
          "@keyframes App-logo-float": {
            "0%": { transform: "translateY(0)" },
            "50%": { transform: "translateY(10px)" },
            "100%": { transform: "translateY(0px)" },
          },
        }}
      />
      <Typography variant="h2">Welcome to SkellySubs!</Typography>
      <Typography variant="h5" sx={{ mt: 2 }}>
        Upload a video or audio file to get started.
      </Typography>
      <FileInput onFileChange={handleFileChange} />
    </>
  )
}

export default UploadStep
