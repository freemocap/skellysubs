// src/store/slices/appState/selectors.ts
import { createSelector } from "@reduxjs/toolkit"
import type { RootState } from "../../appStateStore"

export const selectSelectedFile = (state: RootState) =>
  state.appState.selectedFile

export const selectFormattedFrameRate = createSelector(
  [selectSelectedFile],
  selectedFile => {
    if (selectedFile) {
      return selectedFile.name
    }
    return ""
  },
)
