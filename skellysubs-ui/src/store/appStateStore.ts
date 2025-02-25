// File: src/store/appStateStore.ts
import { configureStore } from "@reduxjs/toolkit"
import { processingStagesSlice } from "./slices/processingStagesSlice"
import { appStateSlice } from "./slices/appState"

export const AppStateStore = configureStore({
  reducer: {
    appState: appStateSlice.reducer,
    processingStages: processingStagesSlice.reducer,
  },
})

export type RootState = ReturnType<typeof AppStateStore.getState>
export type AppDispatch = typeof AppStateStore.dispatch