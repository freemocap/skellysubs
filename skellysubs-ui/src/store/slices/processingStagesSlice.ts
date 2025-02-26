// slices/processingStagesSlice.ts
import { createAsyncThunk, createSlice } from "@reduxjs/toolkit"
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"
import { ffmpegService } from "../../services/FfmpegService/useFfmpeg"

interface ProcessingStagesState {
  currentStage: number
  stages: StageState[]
}
interface ThunkState {
  processingStages: ProcessingStagesState
}
interface FileConversionResult {
  originalFile: {
    name: string
    type: string
    size: number
  }
  convertedAudio: {
    url: string
    size: number
    bitrate: number
    duration?: number
  }
}

interface TranscriptionResult {
  text: string
  language: string
  segments: Array<{
    start: number
    end: number
    text: string
  }>
}

interface TranslationResult {
  translatedText: string
  sourceLang: string
  targetLang: string
}

interface MatchingResult {
  alignedTranscript: string[]
  alignedTranslation: string[]
}

type StageState<T = any> = {
  name: string
  status: "not-ready" | "ready" | "processing" | "completed" | "failed"
  requirements: {
    needsPreviousStage?: boolean
    maxFileSize?: number
    allowedFormats?: string[]
  }
  configuration: {
    audioBitrate?: number
    targetLanguages?: string[]
  }
  input?: T
  output?: T
  error: string | null
}

interface ProcessingStagesState {
  currentStage: number
  stages: [
    StageState<{ file: File | null }, FileConversionResult>,
    StageState<FileConversionResult, TranscriptionResult>,
    StageState<TranscriptionResult, TranslationResult>,
    StageState<TranslationResult, MatchingResult>,
  ]
}

const initialState: ProcessingStagesState = {
  currentStage: 0,
  stages: [
    {
      name: "prepare-file",
      status: "ready",
      requirements: {
        allowedFormats: ["video/*", "audio/*"],
        maxFileSize: 1024 * 1024 * 100, // 100MB
      },
      configuration: {
        audioBitrate: 96, // kbps
      },
      error: null,
    },
    {
      name: "transcribe-audio",
      status: "not-ready",
      requirements: { needsPreviousStage: true },
      configuration: { targetLanguages: ["en"] },
      error: null,
    },
    {
      name: "translate-text",
      status: "not-ready",
      requirements: { needsPreviousStage: true },
      configuration: { targetLanguages: ["es", "fr", "de"] },
      error: null,
    },
    {
      name: "match-words",
      status: "not-ready",
      requirements: { needsPreviousStage: true },
      error: null,
      configuration: {},
    },
  ],
}
const prepareFileThunk = createAsyncThunk(
    "processing/prepareFile",
    async (file: File, { rejectWithValue }) => { // Add file parameter
      try {
        if (!ffmpegService.isLoaded) await ffmpegService.loadFfmpeg()
        const audioBlob = await ffmpegService.convertToMP3(file)

        if (!audioBlob) throw new Error("Audio conversion failed")
        const audioUrl = URL.createObjectURL(audioBlob)

        return {
          originalFile: {
            name: file.name,
            type: file.type,
            size: file.size,
          },
          convertedAudio: {
            url: audioUrl,
            size: audioBlob.size,
            bitrate: 96, // Should calculate this properly
            duration: undefined // Should be populated from FFmpeg
          }
        }
      } catch (error) {
        return rejectWithValue(error instanceof Error ? error.message : 'Unknown error')
      }
    }
)

const transcribeAudioThunk = createAsyncThunk(
    "processing/transcribeAudio",
    async (_, { getState, rejectWithValue }) => {
      try {
        const state = getState() as { processingStages: ProcessingStagesState }
        const audioUrl = state.processingStages.stages[0].output?.convertedAudio?.url

        if (!audioUrl) throw new Error("No audio file found")
        const response = await fetch(`${getApiBaseUrl()}/processing/transcribe`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ audioUrl }),
        })

        if (!response.ok) throw new Error(`HTTP error ${response.status}`)
        return await response.json()
      } catch (error) {
        return rejectWithValue(error instanceof Error ? error.message : 'Unknown error')
      }
    }
    )
export const processingStagesSlice = createSlice({
  name: "processingStages",
  initialState,
  reducers: {
    setCurrentStage: (state, action) => {
      console.log(
        `Setting Processing stage from ${state.currentStage} to ${action.payload}`,
      )
      state.currentStage = action.payload
    },
    resetStages: () => initialState,
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
