// slices/processingStages.ts
import { createAsyncThunk, createSlice } from "@reduxjs/toolkit"
import { getApiBaseUrl } from "../../../utils/getApiBaseUrl"

export interface StageState {
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
    {
      name: "prepare-file",
      status: "ready",
      output: null,
      error: null,
    },
    {
      name: "transcribe-audio",
      status: "not-ready",
      output: null,
      error: null,
    },
    {
      name: "translate-text",
      status: "not-ready",
      output: null,
      error: null,
    },
    {
      name: "match-words",
      status: "not-ready",
      output: null,
      error: null,
    },
  ],
}
const transcribeAudioThunk = createAsyncThunk(
  "processing/transcribeAudio",
  async (_, { getState }) => {
    const state = getState() as { processingStages: ProcessingStagesState }
    const audioFile = state.processingStages.stages[0].output?.path
    if (!audioFile) throw new Error("No audio file found")
    const url = `${getApiBaseUrl()}/processing/transcribe`
    console.log(`Sending request to ${url} with videoPath: ${audioFile}`)
    const response = await fetch(`${getApiBaseUrl()}/processing/transcribe`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ audioFile }),
    })

    if (!response.ok) throw new Error("Audio extraction failed")
    return response.json()
  },
)

export const index = createSlice({
  name: "processingStages",
  initialState,
  reducers: {
    setCurrentStage: (state, action) => {
      state.currentStage = action.payload
    },
  },
  extraReducers: builder => {
    // TranscribeAudio Handling
    builder.addCase(transcribeAudioThunk.pending, state => {
      state.stages[0].status = "processing"
    })
    builder.addCase(transcribeAudioThunk.fulfilled, (state, action) => {
      state.stages[0].status = "completed"
      state.stages[0].output = action.payload
      state.stages[1].status = "ready"
      state.currentStage = 1
    })
    builder.addCase(transcribeAudioThunk.rejected, (state, action) => {
      state.stages[0].status = "failed"
      state.stages[0].error = action.error.message || "Unknown error"
    })
  },
})

export const { setCurrentStage } = index.actions
export default index.reducer
