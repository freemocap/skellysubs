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
  },
})
export const { setSelectedFile } = appStateSlice.actions
export default appStateSlice.reducer
