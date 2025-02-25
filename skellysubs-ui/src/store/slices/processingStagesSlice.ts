// slices/processingStages.ts
import { createAsyncThunk, createSlice } from "@reduxjs/toolkit"
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"

interface StageState {
  name: string
  status: "not-ready" | "ready" | "processing" | "completed" | "failed"
  output: any
  error: string | null
}

interface ProcessingStagesState {
  currentStage: number
  stages: StageState[]
}

const initialState: ProcessingStagesState = {
  currentStage: 0,
  stages: [
    { name: "select-file", status: "ready", output: null, error: null },
    { name: "scrape-audio", status: "not-ready", output: null, error: null },
    { name: "transcribe", status: "not-ready", output: null, error: null },
    { name: "translate-text", status: "not-ready", output: null, error: null },
    { name: "match-words", status: "not-ready", output: null, error: null },
  ],
}

export const processFileUpload = createAsyncThunk(
  "processing/uploadFile",
  async (file: File, { dispatch }) => {
    const formData = new FormData()
    formData.append("file", file)

    const response = await fetch(`${getApiBaseUrl()}/upload`, {
      method: "POST",
      body: formData,
    })

    if (!response.ok) throw new Error("Upload failed")
    return response.json()
  },
)

export const processScrapeAudio = createAsyncThunk(
  "processing/scrapeAudio",
  async (_, { getState }) => {
    const state = getState() as { processingStages: ProcessingStagesState }
    const videoPath = state.processingStages.stages[0].output?.path

    const response = await fetch(`${getApiBaseUrl()}/scrape-audio`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ videoPath }),
    })

    if (!response.ok) throw new Error("Audio extraction failed")
    return response.json()
  },
)

const processingStagesSlice = createSlice({
  name: "processingStages",
  initialState,
  reducers: {
    setCurrentStage: (state, action) => {
      state.currentStage = action.payload
    },
  },
  extraReducers: builder => {
    // File Upload Handling
    builder.addCase(processFileUpload.pending, state => {
      state.stages[0].status = "processing"
    })
    builder.addCase(processFileUpload.fulfilled, (state, action) => {
      state.stages[0].status = "completed"
      state.stages[0].output = action.payload
      state.stages[1].status = "ready"
      state.currentStage = 1
    })
    builder.addCase(processFileUpload.rejected, (state, action) => {
      state.stages[0].status = "failed"
      state.stages[0].error = action.error.message || "Unknown error"
    })

    // Audio Scraping Handling
    builder.addCase(processScrapeAudio.pending, state => {
      state.stages[1].status = "processing"
    })
    builder.addCase(processScrapeAudio.fulfilled, (state, action) => {
      state.stages[1].status = "completed"
      state.stages[1].output = action.payload
      state.stages[2].status = "ready"
      state.currentStage = 2
    })
    builder.addCase(processScrapeAudio.rejected, (state, action) => {
      state.stages[1].status = "failed"
      state.stages[1].error = action.error.message || "Unknown error"
    })
  },
})

export const { setCurrentStage } = processingStagesSlice.actions
export default processingStagesSlice.reducer
