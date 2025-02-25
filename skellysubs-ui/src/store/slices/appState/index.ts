import { createSlice } from "@reduxjs/toolkit"

type AppState = {
  selectedFile: File | null
}

const initialState: AppState = {
  selectedFile: null,
}

export const appStateSlice = createSlice({
  name: "appState",
  initialState,
  reducers: {
    setSelectedFile: (state, action) => {
      state.selectedFile = action.payload
    },
    resetFile: (state) => {
      state.selectedFile = null
    }
  },
})
export const { setSelectedFile, resetFile } = appStateSlice.actions
export default appStateSlice.reducer
