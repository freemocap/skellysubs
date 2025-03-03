import type { PayloadAction } from "@reduxjs/toolkit"
import { createSlice } from "@reduxjs/toolkit"
import {
  translationThunk,
} from "../../thunks/translationThunk"
import {
  AudioVisualFile, ProcessingContext,
  ProcessingStage, ProcessingState,
  TranscriptionResult,
  TranslatedTranscription
} from "./processing-status-types";
import {prepareFileThunk} from "../../thunks/prepareFileThunk";
import {transcriptionThunk} from "../../thunks/transcriptionThunk";

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
      .addCase(transcriptionThunk.pending, state => {
        state.stages.transcription.status = "processing"
      })
      .addCase(transcriptionThunk.fulfilled, (state, action) => {
        state.context.transcription = action.payload
        state.stages.transcription.status = "completed"
        state.stages.translation.status = "ready" // Activate next stage
      })
      .addCase(transcriptionThunk.rejected, (state, action) => {
        state.stages.transcription.status = "failed"
        state.stages.transcription.error = action.payload as string
      })
      .addCase(translationThunk.pending, state => {
        state.stages.translation.status = "processing"
      })
      .addCase(translationThunk.fulfilled, (state, action) => {
        state.context.translation = action.payload
        state.stages.translation.status = "completed"
      })
      .addCase(translationThunk.rejected, (state, action) => {
        state.stages.translation.status = "failed"
        state.stages.translation.error = action.payload as string
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
export const selectIsTranslateReady = (state: {
  processing: ProcessingState
}) => {
  return !!state.processing.context.transcription
}

export const { injectContextData } = processingSlice.actions
