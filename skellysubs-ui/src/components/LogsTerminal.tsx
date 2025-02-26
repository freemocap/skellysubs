import React from "react"
import { useAppSelector } from "../store/hooks"
import { Box } from "@mui/material"
import { Info, Warning, Error, CheckCircle } from "@mui/icons-material"
import {LogSeverity} from "../store/slices/LogsSlice";
import Terminal from "react-terminal-ui";

const SeverityIcon = ({ severity }: { severity: LogSeverity }) => {
    const iconStyle = { fontSize: 16, marginRight: 1 }
    switch (severity) {
        case "success": return <CheckCircle color="success" style={iconStyle} />
        case "warning": return <Warning color="warning" style={iconStyle} />
        case "error": return <Error color="error" style={iconStyle} />
        default: return <Info color="info" style={iconStyle} />
    }
}

export const LogsTerminal = () => {
    const logs = useAppSelector(state => state.logsReducer.entries)

    return (
      <Box
        sx={{
          height: "100%",
          backgroundColor: "#1e1e1e",
          color: "#ffffff",
          p: 2,
          fontFamily: "monospace",
          overflowY: "auto",
        }}
      >
        <Terminal height="100%">
          {logs.map(
            (
              log: {
                severity: string
                timestamp: string | number | Date
                message:
                  | string
                  | number
                  | boolean
                  | React.ReactElement<
                      any,
                      string | React.JSXElementConstructor<any>
                    >
                  | Iterable<React.ReactNode>
                  | React.ReactPortal
                  | null
                  | undefined
              },
              idx: React.Key | null | undefined,
            ) => (
              <div key={idx} style={{ display: "flex", alignItems: "center" }}>
                  // @ts-ignore
                <SeverityIcon severity={log.severity} />
                <span style={{ color: "#888", marginRight: 8 }}>
                  {new Date(log.timestamp).toLocaleTimeString()}
                </span>
                {log.message}
              </div>
            ),
          )}
        </Terminal>
      </Box>
    )
}