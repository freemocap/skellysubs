import type React from "react"
import { TextField } from "@mui/material"

interface FileInputProps {
  onFileChange: (file: File | null) => void
}

const FileInput: React.FC<FileInputProps> = ({ onFileChange }) => {
  const handleChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    if (event.target.files && event.target.files[0]) {
      const file = event.target.files[0]
      if (file.type.startsWith("video/") || file.type.startsWith("audio/")) {
        onFileChange(file)
      } else {
        alert("Please select an audio or video file")
        event.target.value = ""
      }
    }
  }

  return (
    <TextField
      label="Select a video or audio file"
      color="secondary"
      type="file"
      margin={"normal"}
      onChange={handleChange}
      inputProps={{ accept: "video/*,audio/*" }}
      focused
    />
  )
}

export default FileInput
