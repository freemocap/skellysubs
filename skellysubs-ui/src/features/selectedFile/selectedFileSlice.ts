import { createAsyncThunk } from "@reduxjs/toolkit"
import { createAppSlice } from "../../app/createAppSlice"
import type { RootState } from "../../app/store"
import AudioVideoFileHandler from "./AudioVideoFileHandler"

interface UploadedFileState {
  fileType: "video" | "audio" | null
  fileName: string | null
  audioBlobUrl: string | null
  videoBlobUrl: string | null
  status: "idle" | "preparing" | "processing" | "done" | "error"
}

const initialState: UploadedFileState = {
  fileType: null,
  fileName: null,
  audioBlobUrl: null,
  videoBlobUrl: null,
  status: "idle",
}

export const processFile = createAsyncThunk(
  "uploadedFile/processFile",
  async ({ selectedFile }: { selectedFile: File }) => {
    // Check file type
    let videoBlobUrl: string | null = null
    let audioBlobUrl: string | null = null
    let fileType: "video" | "audio" | null = null
    console.log(`Processing file: ${selectedFile.name}`)
    const fileHandler = new AudioVideoFileHandler()
    if (selectedFile.type.startsWith("video")) {
      fileType = "video"
      videoBlobUrl = URL.createObjectURL(selectedFile)

      // Extract audio from video
      const audioBlob = await fileHandler.extractAudio(selectedFile)
      audioBlobUrl = URL.createObjectURL(audioBlob)
    } else if (selectedFile.type.startsWith("audio")) {
      fileType = "audio"
      audioBlobUrl = URL.createObjectURL(selectedFile)
    } else {
      throw new Error(`Unsupported file type: ${selectedFile.type}`)
    }
    console.log(
      `Recieved file type: ${fileType}, audioBlobUrl: ${audioBlobUrl}, videoBlobUrl: ${videoBlobUrl}`,
    )
    return { fileType, audioBlobUrl, videoBlobUrl }
  },
)

export const selectedFileSlice = createAppSlice({
  name: "uploadedFile",
  initialState,
  reducers: {
    reset: state => {
      state.fileType = null
      state.fileName = null
      state.audioBlobUrl = null
      state.videoBlobUrl = null
      state.status = "idle"
    },
  },
  extraReducers: builder => {
    builder.addCase(processFile.pending, state => {
      state.status = "preparing"
    })
    builder.addCase(processFile.fulfilled, (state, action) => {
      state.fileType = action.payload.fileType
      state.audioBlobUrl = action.payload.audioBlobUrl
      state.videoBlobUrl = action.payload.videoBlobUrl
      state.status = "done"
    })
    builder.addCase(processFile.rejected, state => {
      state.status = "error"
    })
  },
})

export const { reset } = selectedFileSlice.actions

export const selectFileType = (state: RootState) => state.uploadedFile.fileType
export const selectFileName = (state: RootState) => state.uploadedFile.fileName
export const selectAudioBlobUrl = (state: RootState) =>
  state.uploadedFile.audioBlobUrl
export const selectVideoBlobUrl = (state: RootState) =>
  state.uploadedFile.videoBlobUrl
export const selectStatus = (state: RootState) => state.uploadedFile.status

export default selectedFileSlice.reducer
