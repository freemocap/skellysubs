import { Middleware } from "@reduxjs/toolkit"
import { RootState } from "../appStateStore"
import {addLog} from "../slices/LogsSlice";

export const stageValidation: Middleware<{}, RootState> = store => next => action => {
    const currentState = store.getState()

    if (action.type === "processingStages/setCurrentStage") {
        const currentStage = currentState.processingStagesReducer.currentStage
        const targetStage = action.payload

        if (targetStage > currentStage + 1) {
            store.dispatch(addLog({
                message: "Cannot skip stages! Complete current stage first.",
                severity: "error"
            }))
            return
        }
    }

    return next(action)
}