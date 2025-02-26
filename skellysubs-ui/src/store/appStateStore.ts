import { configureStore, ThunkAction, Action } from "@reduxjs/toolkit"
import { processingStagesSlice } from "./slices/processingStagesSlice"
import { appStateSlice } from "./slices/appState"
import { configSlice } from "./slices/configSlice"
import { stageValidation } from "./middleware/stageValidation"
import {logsSlice} from "./slices/LogsSlice";

export const AppStateStore = configureStore({
  reducer: {
    appStateReducer: appStateSlice.reducer,
    processingStagesReducer: processingStagesSlice.reducer,
    logsReducer: logsSlice.reducer,
    configReducer: configSlice.reducer,
  },
  middleware: (getDefaultMiddleware) =>
      getDefaultMiddleware().concat(stageValidation),
})

// Proper type definitions without circular references
export type AppDispatch = typeof AppStateStore.dispatch
export type RootState = ReturnType<typeof AppStateStore.getState>
