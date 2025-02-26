import type React from "react"
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Box } from "@mui/material"
import {LogsTerminal} from "../components/LogsTerminal";

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
        justifyContent: "center",
        alignItems: "center",
      }}
    >
      <PanelGroup
        direction="vertical"
        style={{ height: "100vh", width: "100vw" }}
      >
        {/* Top section (horizontal panels) - 80% height */}
        {/*Main/Central Content Panel*/}
        <Panel defaultSize={80} minSize={10}>
          {children}
        </Panel>

        {/* Vertical Resize Handle  - like meaning the line is horizontal
            and it lets you resize the vertical size of the panel */}
        <PanelResizeHandle
          style={{
            height: "4px",
            cursor: "row-resize",
            backgroundColor: extendedPaperbaseTheme.palette.primary.light,
          }}
        />

        <Panel collapsible defaultSize={4} minSize={10} collapsedSize={4}>
            <LogsTerminal />
        </Panel>
      </PanelGroup>
    </Box>
  )
}
