import type { PayloadAction } from "@reduxjs/toolkit"
import { createSlice } from "@reduxjs/toolkit"
import { prepareFileThunk, transcribeAudioThunk } from "../thunks"

export interface AudioVisualFile {
  url: string
  name: string
  type: string
  size: number
  bitrate: number
  duration?: number
}

export interface WordSegment {
  word: string
  start: number
  end: number
}

export interface TranscriptionResult {
  text: string
  language: string
  duration: number
  words: WordSegment[]
  segments: {
    id: number
    text: string
    start: number
    end: number
  }[]
}
class TranslationResult {}

class MatchingResult {}

export interface ProcessingContext {
  originalFile?: AudioVisualFile
  mp3Audio?: AudioVisualFile
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

export interface ProcessingState {
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

export const selectIsTranscribeReady = (state: {
  processing: ProcessingState
}) => {
  return !!state.processing.context.mp3Audio
}

export const { injectContextData } = processingSlice.actions
