import type React from "react"
import { useState } from "react"
import { Panel } from "react-resizable-panels"
import {
  Box,
  Collapse,
  List,
  ListItemButton,
  ListItemText,
  Typography,
} from "@mui/material"
import {
  ChevronLeft,
  ChevronRight,
  ExpandLess,
  ExpandMore,
} from "@mui/icons-material"

export const RightSideConfigPanel = ({
  isCollapsed,
  toggleCollapse,
}: {
  isCollapsed: boolean
  toggleCollapse: () => void
}) => {
  const [openSections, setOpenSections] = useState<{ [key: string]: boolean }>({
    general: true,
    appearance: false,
    advanced: false,
  })

  const handleSectionToggle = (section: string) => {
    setOpenSections(prev => ({ ...prev, [section]: !prev[section] }))
  }

  return (
    <Box sx={{ position: "relative", height: "100%" }}>
      {/* Collapse Toggle Handle */}
      <Box
        sx={{
          position: "absolute",
          left: -28,
          top: "50%",
          transform: "translateY(-50%)",
          cursor: "pointer",
          zIndex: 1000,
          backgroundColor: "background.paper",
          borderRadius: "50%",
          boxShadow: 1,
          "&:hover": { backgroundColor: "action.hover" },
        }}
        onClick={toggleCollapse}
      >
        {isCollapsed ? (
          <ChevronLeft fontSize="large" />
        ) : (
          <ChevronRight fontSize="large" />
        )}
      </Box>

      {!isCollapsed && (
        <Box sx={{ p: 2 }}>
          <Typography variant="h6" gutterBottom>
            Configuration
          </Typography>

          <List component="nav">
            <ConfigSection
              title="General Settings"
              open={openSections.general}
              onToggle={() => handleSectionToggle("general")}
            >
              <Typography variant="body2">Theme selection</Typography>
              <Typography variant="body2">Default values</Typography>
              <Typography variant="body2">User preferences</Typography>
            </ConfigSection>

            <ConfigSection
              title="Appearance"
              open={openSections.appearance}
              onToggle={() => handleSectionToggle("appearance")}
            >
              <Typography variant="body2">Dark/Light mode</Typography>
              <Typography variant="body2">UI density</Typography>
              <Typography variant="body2">Font sizes</Typography>
            </ConfigSection>

            <ConfigSection
              title="Advanced"
              open={openSections.advanced}
              onToggle={() => handleSectionToggle("advanced")}
            >
              <Typography variant="body2">API configurations</Typography>
              <Typography variant="body2">Debug mode</Typography>
              <Typography variant="body2">Experimental features</Typography>
            </ConfigSection>
          </List>
        </Box>
      )}
    </Box>
  )
}

const ConfigSection = ({
  title,
  open,
  onToggle,
  children,
}: {
  title: string
  open: boolean
  onToggle: () => void
  children: React.ReactNode
}) => (
  <>
    <ListItemButton onClick={onToggle} sx={{ px: 0 }}>
      <Typography variant="subtitle2">{title}</Typography>
      {open ? <ExpandLess sx={{ ml: 1 }} /> : <ExpandMore sx={{ ml: 1 }} />}
    </ListItemButton>
    <Collapse in={open}>
      <Box sx={{ pl: 2, display: "flex", flexDirection: "column", gap: 1 }}>
        {children}
      </Box>
    </Collapse>
  </>
)
// Dummy components for sub-sections
const GeneralSettings = () => (
  <div>
    <Typography variant="body2">Theme selection</Typography>
    <Typography variant="body2">Default values</Typography>
    <Typography variant="body2">User preferences</Typography>
  </div>
)

const AppearanceSettings = () => (
  <div>
    <Typography variant="body2">Dark/Light mode</Typography>
    <Typography variant="body2">UI density</Typography>
    <Typography variant="body2">Font sizes</Typography>
  </div>
)

const AdvancedOptions = () => (
  <div>
    <Typography variant="body2">API configurations</Typography>
    <Typography variant="body2">Debug mode</Typography>
    <Typography variant="body2">Experimental features</Typography>
  </div>
)
