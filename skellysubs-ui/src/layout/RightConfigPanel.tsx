import type React from "react"
import { useState } from "react"
import { Box, Collapse, List, ListItemButton, Typography } from "@mui/material"
import { KeyboardArrowDown, KeyboardArrowRight } from "@mui/icons-material"
import { TranslationControls } from "../components/processing-stages/translation-stage/TranslationControls"

export const RightSideConfigPanel = ({
  isCollapsed,
  toggleCollapse,
}: {
  isCollapsed: boolean
  toggleCollapse: () => void
}) => {
  const [openSections, setOpenSections] = useState<{ [key: string]: boolean }>({
    translation: true,
  })

  const handleSectionToggle = (section: string) => {
    setOpenSections(prev => ({ ...prev, [section]: !prev[section] }))
  }

  return (
    <Box sx={{ position: "relative", height: "100%"
    }}>
      {/* Collapse Toggle Handle */}

      {!isCollapsed && (
        <Box sx={{ p: 2 }}>
          <Typography variant="h6" gutterBottom>
            Configuration
          </Typography>

          <List component="nav">
            <ConfigSection
              title="Target Languages"
              open={openSections.translation}
              onToggle={() => handleSectionToggle("translation")}
            >
              <TranslationControls />
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
      {open ? (
        <KeyboardArrowDown sx={{ ml: 1 }} />
      ) : (
        <KeyboardArrowRight sx={{ ml: 1 }} />
      )}
      <Typography variant="subtitle2">{title}</Typography>
    </ListItemButton>
    <Collapse in={open}>
      <Box sx={{ display: "flex", flexDirection: "column" }}>{children}</Box>
    </Collapse>
  </>
)
