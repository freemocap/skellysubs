import type { PayloadAction } from "@reduxjs/toolkit"
import { createAsyncThunk, createSlice } from "@reduxjs/toolkit"
import { ffmpegService } from "../../services/FfmpegService/useFfmpeg"
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"

class TranscriptionResult {}

class TranslationResult {}

class MatchingResult {}

interface ProcessingContext {
  originalFile?: File
  mp3Audio?: {
    url: string
    size: number
    bitrate: number
    duration?: number
  }
  transcription?: TranscriptionResult
  translation?: TranslationResult
  alignment?: MatchingResult
}

interface ProcessingStage {
  name: string
  requirements: (keyof ProcessingContext)[]
  status: "idle" | "ready" | "processing" | "completed" | "failed"
  produces: keyof ProcessingContext
  error: string | null
}

interface ProcessingState {
  context: ProcessingContext
  stages: Record<string, ProcessingStage>
}

// Updated initial state
const initialState: ProcessingState = {
  context: {},
  stages: {
    filePreparation: {
      name: "filePreparation",
      requirements: [],
      status: "ready",
      produces: "mp3Audio",
      error: null,
    },
    transcription: {
      name: "transcription",
      requirements: ["mp3Audio"],
      status: "idle",
      produces: "transcription",
      error: null,
    },
    translation: {
      name: "translation",
      requirements: ["transcription"],
      status: "idle",
      produces: "translation",
      error: null,
    },
    alignment: {
      name: "alignment",
      requirements: ["transcription", "translation"],
      status: "idle",
      produces: "alignment",
      error: null,
    },
  },
}

// Generic stage thunk creator
function createProcessingThunk<InputType, OutputType>(
  stageName: string,
  processor: (
    context: ProcessingContext,
    input?: InputType,
  ) => Promise<OutputType>,
) {
  return createAsyncThunk(
    `processing/${stageName}`,
    async (input: InputType, { getState, rejectWithValue }) => {
      const state = getState() as { processing: ProcessingState }
      const stage = state.processing.stages[stageName]

      // Check requirements
      const missingRequirements = stage.requirements.filter(
        req => !state.processing.context[req],
      )

      if (missingRequirements.length > 0) {
        return rejectWithValue(
          `Missing required data: ${missingRequirements.join(", ")}`,
        )
      }

      try {
        return await processor(state.processing.context, input)
      } catch (error) {
        return rejectWithValue(
          error instanceof Error ? error.message : "Unknown error",
        )
      }
    },
  )
}

// Example thunk for file preparation
export const prepareFileThunk = createProcessingThunk<
  File,
  ProcessingContext["mp3Audio"]
>("filePreparation", async (context, file) => {
  if (!file) throw new Error("No file provided")
  if (!ffmpegService.isLoaded) await ffmpegService.loadFfmpeg()

  const { audioBlob, bitrate, duration } =
    await ffmpegService.convertToMP3(file)
  if (!audioBlob) throw new Error("Audio conversion failed")

  return {
    url: URL.createObjectURL(audioBlob),
    size: audioBlob.size,
    bitrate,
    duration,
  }
})
export const transcribeAudioThunk = createProcessingThunk<
  void, // No input needed
  ProcessingContext["transcription"]
>("transcription", async context => {
  if (!context.mp3Audio?.url) throw new Error("No audio URL provided")
  // Fetch the MP3 blob from the URL
  const response = await fetch(context.mp3Audio.url)
  const mp3Blob = await response.blob()
  const formData = new FormData()
  formData.append("file", mp3Blob, "audio.mp3")

  const transcribeResponse = await fetch(
    `${getApiBaseUrl()}/processing/transcribe`,
    {
      method: "POST",
      body: formData, // Send as FormData
    },
  )

  if (!response.ok) throw new Error(`HTTP error ${response.status}`)
  return await response.json()
})

// Updated slice with data injection capability
export const processingSlice = createSlice({
  name: "processing",
  initialState,
  reducers: {
    injectContextData: (
      state,
      action: PayloadAction<{ key: keyof ProcessingContext; data: any }>,
    ) => {
      state.context[action.payload.key] = action.payload.data

      // Update stage statuses based on new data
      Object.values(state.stages).forEach(stage => {
        if (stage.status === "idle") {
          const hasRequirements = stage.requirements.every(
            req => req in state.context,
          )
          if (hasRequirements) stage.status = "ready"
        }
      })
    },
    resetProcessing: () => initialState,
    updateStageStatus: (
      state,
      action: PayloadAction<{
        stage: string
        status: ProcessingStage["status"]
      }>,
    ) => {
      const stage = state.stages[action.payload.stage]
      if (stage) stage.status = action.payload.status
    },
  },
  extraReducers: builder => {
    builder
      .addCase(prepareFileThunk.pending, state => {
        state.stages.filePreparation.status = "processing"
      })
      .addCase(prepareFileThunk.fulfilled, (state, action) => {
        state.context.mp3Audio = action.payload
        state.stages.filePreparation.status = "completed"
        state.stages.transcription.status = "ready"
      })
      .addCase(prepareFileThunk.rejected, (state, action) => {
        state.stages.filePreparation.status = "failed"
        state.stages.filePreparation.error = action.payload as string
      })
      .addCase(transcribeAudioThunk.pending, state => {
        state.stages.transcription.status = "processing"
      })
      .addCase(transcribeAudioThunk.fulfilled, (state, action) => {
        state.context.transcription = action.payload
        state.stages.transcription.status = "completed"
        state.stages.translation.status = "ready" // Activate next stage
      })
      .addCase(transcribeAudioThunk.rejected, (state, action) => {
        state.stages.transcription.status = "failed"
        state.stages.transcription.error = action.payload as string
      })
  },
})

// Utility selectors
export const selectProcessingContext = (state: {
  processing: ProcessingState
}) => state.processing.context

export const selectStage =
  (stageName: string) => (state: { processing: ProcessingState }) =>
    state.processing.stages[stageName]

export const { injectContextData, resetProcessing, updateStageStatus } =
  processingSlice.actions
export default processingSlice.reducer
