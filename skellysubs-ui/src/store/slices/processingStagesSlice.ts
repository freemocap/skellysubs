// slices/processingStagesSlice.ts
import { createAsyncThunk, createSlice } from "@reduxjs/toolkit"
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"
import {ffmpegService} from "../../services/FfmpegService/useFfmpeg";

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
interface ThunkState {
  appState: {
    selectedFile: File | null
  }
  processingStages: ProcessingStagesState
}

// ???
// // interface PrepareFileStageState {
//     inputs: {   originalFile: File | null },
// outputs: {   audioBlob: File | null,
//     mp3File: File | null}


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
const prepareFileThunk = createAsyncThunk(
    "processing/prepareFile",
    async (_, { getState }) => {
        const state = getState() as ThunkState
        const selectedFile = state.appState.selectedFile

        if (!selectedFile) throw new Error("No file selected")
        if (!ffmpegService.isLoaded) await ffmpegService.loadFfmpeg()

        const audioBlob = await ffmpegService.extractAudioFromVideo(selectedFile)
        const audioUrl = URL.createObjectURL(audioBlob)

        return {
            originalFile: {
                name: selectedFile.name,
                type: selectedFile.type,
                size: selectedFile.size,
            },
            convertedAudioUrl: audioUrl
        }
    }
)

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

export const processingStagesSlice = createSlice({
  name: "processingStages",
  initialState,
  reducers: {
    setCurrentStage: (state, action) => {
      console.log(`Setting Processing stage from ${state.currentStage} to ${action.payload}`)
      state.currentStage = action.payload
    },
      resetStages: () => initialState
  },
  extraReducers: builder => {
    // Prepare-file Handling
    builder.addCase(prepareFileThunk.pending, state => {
      state.stages[0].status = "processing"
    })
    builder.addCase(prepareFileThunk.fulfilled, (state, action) => {
      state.stages[0].status = "completed"
      state.stages[0].output = action.payload
      state.stages[1].status = "ready" // Unlock next stage
      state.currentStage = 1
    })
    builder.addCase(prepareFileThunk.rejected, (state, action) => {
      state.stages[0].status = "failed"
      state.stages[0].error = action.error.message || "File prep failed"
    })

    // Transcribe-audio Handling (now using index 1)
    builder.addCase(transcribeAudioThunk.pending, state => {
      state.stages[1].status = "processing"
    })
    builder.addCase(transcribeAudioThunk.fulfilled, (state, action) => {
      state.stages[1].status = "completed"
      state.stages[1].output = action.payload
      state.stages[2].status = "ready" // Unlock translate-text
      state.currentStage = 2
    })
    builder.addCase(transcribeAudioThunk.rejected, (state, action) => {
      state.stages[1].status = "failed"
      state.stages[1].error = action.error.message || "Transcription failed"
    })
  },
})

// Export the new thunk
export { prepareFileThunk, transcribeAudioThunk }

export const { setCurrentStage, resetStages } = processingStagesSlice.actions
export default processingStagesSlice.reducer
