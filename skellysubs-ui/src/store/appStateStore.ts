import { configureStore } from "@reduxjs/toolkit"
import { processingStagesSlice } from "./slices/processingStagesSlice"
import { configSlice } from "./slices/configSlice"
import { logsSlice } from "./slices/LogsSlice"

export const AppStateStore = configureStore({
  reducer: {
    processingStagesReducer: processingStagesSlice.reducer,
    logsReducer: logsSlice.reducer,
    configReducer: configSlice.reducer,
  },
  // middleware: (getDefaultMiddleware) =>
  //     getDefaultMiddleware().concat(stageValidation),
})

// Proper type definitions without circular references
export type AppDispatch = typeof AppStateStore.dispatch
export type RootState = ReturnType<typeof AppStateStore.getState>
