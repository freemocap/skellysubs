import type React from "react"
import { useEffect, useRef } from "react"
import { useAppSelector } from "../store/hooks"
import { Box } from "@mui/material"
import { CheckCircle, Error, Info, Warning } from "@mui/icons-material"
import type { LogSeverity } from "../store/slices/LogsSlice"

const SeverityIcon = ({ severity }: { severity: LogSeverity }) => {
  const iconStyle = { fontSize: 16, marginRight: 1 }
  switch (severity) {
    case "success":
      return <CheckCircle color="success" style={iconStyle} />
    case "warning":
      return <Warning color="warning" style={iconStyle} />
    case "error":
      return <Error color="error" style={iconStyle} />
    default:
      return <Info color="info" style={iconStyle} />
  }
}

export const LogsTerminal = () => {
  const logs = useAppSelector(state => state.logs.entries)
  const terminalRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const terminal = terminalRef.current
    if (terminal) {
      terminal.scrollTop = terminal.scrollHeight
    }
  }, [logs])

  return (
    <Box
      ref={terminalRef}
      sx={{
        height: "100%",
        backgroundColor: "#1e1e1e",
        color: "#ffffff",
        fontFamily: "monospace",
        overflowY: "auto",
        overflowX: "auto",
        textWrap: "nowrap",
        transition: "all 0.3s ease-in-out",
      }}
    >
      Hey look, its logs!
      {logs.map((log, idx) => (
        <div key={idx} style={{ display: "flex", alignItems: "center" }}>
          <SeverityIcon severity={log.severity} />
          <span style={{ color: "#888", marginRight: 8 }}>
            {new Date(log.timestamp).toLocaleTimeString()}
          </span>
          {log.message}
        </div>
      ))}
    </Box>
  )
}
