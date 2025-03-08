// SubtitleDownloadSelector.tsx
import type React from "react";
import { useState } from "react"
import {
  Box,
  IconButton,
  Menu,
  MenuItem,
  Typography,
  Checkbox,
  ListItemText,
  Button,
} from "@mui/material"
import FileDownloadIcon from "@mui/icons-material/FileDownload"
import { useAppSelector } from "../../../store/hooks"
import { selectSubtitlesByLanguage } from "../../../store/slices/available-subtitles/availableSubtitlesSlice"
import type { SubtitleVariant } from "../../../store/slices/available-subtitles/available-subtitles-types"
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme";

interface SubtitleDownloadSelectorProps {
  language: string
}

export const SubtitleDownloadSelector: React.FC<SubtitleDownloadSelectorProps> = ({
  language,
}) => {
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null)
  const availableSubtitles = useAppSelector(state =>
    selectSubtitlesByLanguage(state, language)
  )
  const [selectedSubtitles, setSelectedSubtitles] = useState<Set<string>>(new Set())

  const handleClick = (event: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(event.currentTarget)
  }

  const handleClose = () => {
    setAnchorEl(null)
  }

  const handleSelect = (id: string) => {
    setSelectedSubtitles(prev => {
      const newSet = new Set(prev)
      if (newSet.has(id)) {
        newSet.delete(id)
      } else {
        newSet.add(id)
      }
      return newSet
    })
  }

  const handleDownload = () => {
    availableSubtitles.forEach(subtitle => {
      if (selectedSubtitles.has(subtitle.id)) {
        const blob = new Blob([subtitle.content], { type: "text/plain" })
        const url = URL.createObjectURL(blob)
        const a = document.createElement("a")
        a.href = url
        a.download = `${subtitle.name}.${subtitle.format}`
        document.body.appendChild(a)
        a.click()
        document.body.removeChild(a)
        URL.revokeObjectURL(url)
      }
    })
    handleClose()
  }

  // Group subtitles by variant
  const subtitlesByVariant = availableSubtitles.reduce((acc, subtitle) => {
    if (!acc[subtitle.variant]) {
      acc[subtitle.variant] = []
    }
    acc[subtitle.variant].push(subtitle)
    return acc
  }, {} as Record<SubtitleVariant, typeof availableSubtitles>)

  const variantLabels: Record<SubtitleVariant, string> = {
    original_spoken: "Original",
    translation_only: "Translation Only",
    translation_with_romanization: "With Romanization",
    multi_language: "Multi-language",
  }

  return (
    <>
      <Button
          size="small"
          onClick={handleClick}
          sx={{
            ml:2,
            color: 'primary.contrastText',
            backgroundColor: extendedPaperbaseTheme.palette.primary.light,
            '&:hover': {
              backgroundColor: 'primary.dark',
            }
          }}
          startIcon={<FileDownloadIcon />}
      >
        Download
      </Button>
      <Menu
        anchorEl={anchorEl}
        open={Boolean(anchorEl)}
        onClose={handleClose}
        PaperProps={{
          sx: { maxWidth: 300 }
        }}
      >
        {Object.entries(subtitlesByVariant).map(([variant, subtitles]) => (
          <Box key={variant}>
            <Typography variant="caption" sx={{ px: 2, py: 0.5, display: 'block', color: 'text.secondary' }}>
              {variantLabels[variant as SubtitleVariant]}
            </Typography>
            {subtitles.map(subtitle => (
              <MenuItem
                key={subtitle.id}
                dense
                onClick={() => handleSelect(subtitle.id)}
              >
                <Checkbox
                  checked={selectedSubtitles.has(subtitle.id)}
                  size="small"
                />
                <ListItemText
                  primary={subtitle.format.toUpperCase()}
                  sx={{ ml: 1 }}
                />
              </MenuItem>
            ))}
          </Box>
        ))}
        <Box sx={{ p: 1 }}>
          <Button
            fullWidth
            variant="contained"
            size="small"
            onClick={handleDownload}
            disabled={selectedSubtitles.size === 0}
          >
            Download ({selectedSubtitles.size})
          </Button>
        </Box>
      </Menu>
    </>
  )
}
