import type React from "react"
import { createContext } from "react"
import { useRef } from "react"
import { useState } from "react"
import type { ImperativePanelHandle } from "react-resizable-panels"
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels"
import extendedPaperbaseTheme from "./paperbase_theme/paperbase-theme"
import { Box, IconButton } from "@mui/material"
import { LogsTerminal } from "../components/LogsTerminal"
import { RightSideConfigPanel } from "./RightConfigPanel"
import SettingsIcon from "@mui/icons-material/Settings"
export const RightPanelContext = createContext<{
  toggleRightPanel: () => void
}>({
  toggleRightPanel: () => {},
})
export const BasePanelLayout = ({
  children,
}: {
  children: React.ReactNode
}) => {
  const [lastRightPanelSize, setLastRightPanelSize] = useState(20)
  const [isRightPanelCollapsed, setIsRightPanelCollapsed] = useState(false)
  const rightPanelRef = useRef<ImperativePanelHandle>(null)

  const handleRightPanelResize = (size: number) => {
    setIsRightPanelCollapsed(size === 0)
  }

  const toggleRightPanel = () => {
    if (isRightPanelCollapsed) {
      // Expand panel to last known size
      rightPanelRef.current?.resize(lastRightPanelSize)
    } else {
      // Collapse panel and save current size
      setLastRightPanelSize(rightPanelRef.current?.getSize() ?? 20)
      rightPanelRef.current?.resize(0)
    }
  }

  return (
    <RightPanelContext.Provider value={{ toggleRightPanel }}>
      <Box
        sx={{
          display: "flex",
          flexDirection: "column",
          height: "100vh",
          width: "100vw",
        }}
      >
        {/* Persistent Settings Gear */}
        <Box
          sx={{
            position: "fixed",
            top: 16,
            right: isRightPanelCollapsed
              ? 16
              : `calc(${rightPanelRef.current?.getSize() ?? 20}% + 16px)`,
            zIndex: 1500,
            transition: "right 0.2s ease-in-out",
          }}
        >
          <IconButton
            onClick={toggleRightPanel}
            sx={{
              backgroundColor: extendedPaperbaseTheme.palette.background.paper,
              "&:hover": {
                backgroundColor: extendedPaperbaseTheme.palette.action.hover,
              },
              transform: isRightPanelCollapsed
                ? "rotate(0deg)"
                : "rotate(30deg)",
              transition: "transform 0.2s ease-in-out",
            }}
          >
            <SettingsIcon />
          </IconButton>
        </Box>
        <PanelGroup
          direction="horizontal"
          style={{ height: "100%", width: "100%" }}
        >
          {/* Main Content Area */}
          <Panel defaultSize={80} minSize={30}>
            <PanelGroup
              direction="vertical"
              style={{ height: "100%", width: "100%" }}
            >
              <Panel defaultSize={90} minSize={20} style={{ overflow: "auto" }}>
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
                defaultSize={10}
                minSize={10}
                collapsedSize={4}
                style={{
                  backgroundColor: extendedPaperbaseTheme.palette.primary.dark,
                  color: extendedPaperbaseTheme.palette.primary.contrastText,
                }}
              >
                <LogsTerminal />
              </Panel>
            </PanelGroup>
          </Panel>

          {/* Right Panel Resize Handle */}
          <PanelResizeHandle
            style={{
              width: "4px",
              cursor: "col-resize",
              backgroundColor: extendedPaperbaseTheme.palette.primary.light,
            }}
          />

          {/* Right Configuration Panel */}
          <Panel
            ref={rightPanelRef}
            collapsible
            onResize={handleRightPanelResize}
            minSize={15}
            collapsedSize={0}
            style={{
              overflow: isRightPanelCollapsed ? "visible" : "auto",
              backgroundColor: extendedPaperbaseTheme.palette.background.paper,
              borderLeft: `1px solid ${extendedPaperbaseTheme.palette.divider}`,
              position: "relative",
            }}
          >
            <RightSideConfigPanel
              isCollapsed={isRightPanelCollapsed}
              toggleCollapse={toggleRightPanel}
            />
          </Panel>
        </PanelGroup>
      </Box>
    </RightPanelContext.Provider>
  )
}
