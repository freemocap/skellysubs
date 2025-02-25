import { configureStore } from "@reduxjs/toolkit"
import processingStagesSlice from "./slices/processingStages"
import { appStateSlice } from "./slices/appState"

export const AppStateStore = configureStore({
  reducer: {
    //? Linter giving error - TS2339: Property reducer does not exist on type Reducer<ProcessingStagesState>???
    appState: appStateSlice.reducer,
    processingStages: processingStagesSlice.reducer,
  },
})

// Infer the `RootState` and `AppDispatch` types from the store itself
export type RootState = ReturnType<typeof AppStateStore.getState>
// Inferred type: {posts: PostsState, comments: CommentsState, users: UsersState}
export type AppDispatch = typeof AppStateStore.dispatch
