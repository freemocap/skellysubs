// src/store/slices/appState/selectors.ts
import { createSelector } from "@reduxjs/toolkit"
import type { RootState } from "../../appStateStore"

export const selectNextReadyStage = createSelector(
  (state: RootState) => state.processingStages,
  stagesState => {
    return stagesState.stages.findIndex(
      (stage: any, index: any) =>
        stage.status === "ready" && index > stagesState.currentStage,
    )
  },
)
