import { configureStore } from "@reduxjs/toolkit"
import processingSlice from "./slices/processingStatusSlice"
import { configSlice } from "./slices/configSlice"
import { logsSlice } from "./slices/LogsSlice"

export const AppStateStore = configureStore({
  reducer: {
    processing: processingSlice, // Key must match what selectors expect
    logs: logsSlice.reducer,
    config: configSlice.reducer,
  },
})