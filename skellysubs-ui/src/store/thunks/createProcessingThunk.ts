// createProcessingThunk.ts
import type {
  ProcessingContext,
  ProcessingState,
} from "../slices/processing-status/processing-status-types"
import { createAsyncThunk } from "@reduxjs/toolkit"
import type { AppDispatch, RootState } from "../AppStateStore"

export function createProcessingThunk<InputType, OutputType>(
  stageName: string,
  processor: (
    context: ProcessingContext,
    input?: InputType,
    thunkAPI?: { dispatch: AppDispatch }
  ) => Promise<OutputType>,
) {
  return createAsyncThunk<
    OutputType,
    InputType,
    {
      state: RootState;
      dispatch: AppDispatch;
      extra: undefined;
    }
  >(
    `processing/${stageName}`,
    async (input: InputType, { getState, rejectWithValue, dispatch }) => {
      const state = getState()
      const stage = state.processing.stages[stageName]

      const missingRequirements = stage.requirements.filter(
        req => !state.processing.context[req],
      )

      if (missingRequirements.length > 0) {
        return rejectWithValue(
          `Missing required data: ${missingRequirements.join(", ")}`,
        )
      }

      try {
        return await processor(state.processing.context, input, { dispatch })
      } catch (error) {
        return rejectWithValue(
          error instanceof Error ? error.message : "Unknown error",
        )
      }
    },
  )
}