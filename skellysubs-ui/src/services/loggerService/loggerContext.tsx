import type React from "react"
import { createContext, useContext, useCallback } from "react"
import type { LogSeverity } from "../../store/slices/LogsSlice"
import { addLog } from "../../store/slices/LogsSlice"
import { useDispatch } from "react-redux"
import { useAppDispatch } from "../../store/hooks"

interface LoggerContextType {
  log: (message: string, severity: LogSeverity) => void
}

const LoggerContext = createContext<LoggerContextType | undefined>(undefined)

export const LoggerProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const dispatch = useAppDispatch()

  const log = useCallback(
    (message: string, severity?: LogSeverity) => {
      if (!severity) {
        severity = "info"
      }
      logger(`[${severity.toUpperCase()}] ${message}`)
      dispatch(addLog({ message, severity }))
    },
    [dispatch],
  )

  return (
    <LoggerContext.Provider value={{ log }}>{children}</LoggerContext.Provider>
  )
}

export const useLogger = () => {
  const context = useContext(LoggerContext)
  if (!context) {
    throw new Error("useLogger must be used within a LoggerProvider")
  }
  return context.log
}
