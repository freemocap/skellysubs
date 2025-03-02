// src/utils/logger.ts
import { addLog } from "../store/slices/LogsSlice" // Adjust the path as needed
import type { LogSeverity } from "../store/slices/LogsSlice"
import type { AppStateStore } from "../store/appStateStore"

let isStoreInitialized = false
let pendingLogs: Array<{ message: string; severity: LogSeverity }> = []

export const initializeLogger = (store: typeof AppStateStore) => {
  isStoreInitialized = true
  pendingLogs.forEach(({ message, severity }) => {
    store.dispatch(addLog({ message, severity }))
  })
  pendingLogs = []
}

export const logger = (message: string, severity: LogSeverity = "info") => {
  console.log(`[${severity.toUpperCase()}] ${message}`)
  if (isStoreInitialized) {
    // Dispatch immediately if the store is initialized
    import("../store/appStateStore").then(({ AppStateStore }) => {
      AppStateStore.dispatch(addLog({ message, severity }))
    })
  } else {
    // Store pending logs until the store is initialized
    pendingLogs.push({ message, severity })
  }
}
