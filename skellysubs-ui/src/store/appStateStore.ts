import { configureStore } from "@reduxjs/toolkit"
import { processingSlice } from "./slices/processingStatusSlice"
import { logsSlice } from "./slices/LogsSlice"

export const AppStateStore = configureStore({
  reducer: {
    processing: processingSlice.reducer, // Key must match what selectors expect
    logs: logsSlice.reducer,
  },
})
// Export types for use throughout the app
export type RootState = ReturnType<typeof AppStateStore.getState>
export type AppDispatch = typeof AppStateStore.dispatch
