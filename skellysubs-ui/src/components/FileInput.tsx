// File: src/components/FileInput.tsx
import { Button, Typography } from "@mui/material"
import { ChangeEvent } from "react"

interface FileInputProps {
  onFileChange: (file: File) => void
  acceptedFormats?: string
}

const FileInput = ({ onFileChange, acceptedFormats = "*" }: FileInputProps) => {
  const handleFileChange = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) {
      onFileChange(file)
    }
  }

  return (
      <div>
        <input
            accept={acceptedFormats}
            style={{ display: 'none' }}
            id="contained-button-file"
            type="file"
            onChange={handleFileChange}
        />
        <label htmlFor="contained-button-file">
          <Button
              variant="contained"
              component="span"
              color="primary"
              sx={{ width: '100%' }}
          >
            Select File
          </Button>
        </label>
        <Typography variant="caption" display="block" sx={{ mt: 1 }}>
          Supported formats: {acceptedFormats}
        </Typography>
      </div>
  )
}

export default FileInput