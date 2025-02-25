import { configureStore } from "@reduxjs/toolkit"
import processingStagesSlice from "./slices/processingStagesSlice"

export const AppStateStore = configureStore({
  reducer: {
    processingStages: processingStagesSlice.reducer,
  },
})

// Infer the `RootState` and `AppDispatch` types from the store itself
export type RootState = ReturnType<typeof AppStateStore.getState>
// Inferred type: {posts: PostsState, comments: CommentsState, users: UsersState}
export type AppDispatch = typeof AppStateStore.dispatch
