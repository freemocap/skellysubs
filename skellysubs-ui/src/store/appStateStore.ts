import { configureStore } from "@reduxjs/toolkit"
import { appStateSlice } from "./slices/appState"

export const AppStateStore = configureStore({
  reducer: {
    appState: appStateSlice.reducer,
  },
})

// Infer the `RootState` and `AppDispatch` types from the store itself
export type RootState = ReturnType<typeof AppStateStore.getState>
// Inferred type: {posts: PostsState, comments: CommentsState, users: UsersState}
export type AppDispatch = typeof AppStateStore.dispatch
