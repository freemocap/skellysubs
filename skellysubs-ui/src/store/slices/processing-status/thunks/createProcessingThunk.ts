// Generic stage thunk creator
import type {
  ProcessingContext,
  ProcessingState,
} from "../processing-status-types"
import { createAsyncThunk } from "@reduxjs/toolkit"

export function createProcessingThunk<InputType, OutputType>(
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
