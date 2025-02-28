import type React from "react"
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Box } from "@mui/material"

export const BasePanelLayout = ({
  children,
}: {
  children: React.ReactNode
}) => {
  return (
    <Box
      sx={{
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        width: "100vw",
        justifyContent: "center",
        alignItems: "center",
      }}
    >
      <PanelGroup
        direction="vertical"
        style={{ height: "100%", width: "100%" }}
      >
        {/* Top section (horizontal panels) - Fixed height */}
        <Panel defaultSize={80} minSize={10}>
          {children}
        </Panel>

        <PanelResizeHandle
          style={{
            height: "4px",
            cursor: "row-resize",
            backgroundColor: extendedPaperbaseTheme.palette.primary.light,
          }}
        />

        <Panel
          collapsible
          defaultSize={20}
          minSize={10}
          collapsedSize={4}
          style={{
            backgroundColor: extendedPaperbaseTheme.palette.primary.dark,
            color: extendedPaperbaseTheme.palette.primary.contrastText,
            display: "flex",
            flexDirection: "column",
            justifyContent: "left",
            alignItems: "left",
            padding: "1rem",
          }}
        >
          {/*<LogsTerminal />*/}
          woweee it's logs
        </Panel>
      </PanelGroup>
    </Box>
  )
}
