import { configureStore } from "@reduxjs/toolkit"
import { processingSlice } from "./slices/processing-status/processingStatusSlice"
import { logsSlice } from "./slices/LogsSlice"
import {subtitleOptionsSlice} from "./slices/subtitleOptionsSlice";
import {translationConfigSlice} from "./slices/translation-config/translationConfigSlice";

export const AppStateStore = configureStore({
  reducer: {
    processing: processingSlice.reducer,
    logs: logsSlice.reducer,
    translationConfig: translationConfigSlice.reducer,
    subtitleOptions: subtitleOptionsSlice.reducer,
  },
})
// (JSM) - Export types, to be repacked into more App specific form in `hooks.ts`, I think?
export type RootState = ReturnType<typeof AppStateStore.getState>
export type AppDispatch = typeof AppStateStore.dispatch
