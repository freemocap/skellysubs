// LanguageConfigList.tsx
import { useState } from "react"
import {
  Box,
  Button,
  Checkbox,
  Collapse,
  List,
  ListItem,
  ListItemText,
  Paper,
  Typography,
  IconButton,
  TextField,
} from "@mui/material"
import { ExpandMore, ExpandLess, Search } from "@mui/icons-material"
import type { LanguageConfig } from "../../../../store/slices/translation-config/languageConfigSchemas"
import { useAppDispatch, useAppSelector } from "../../../../store/hooks"
import {
  addCustomLanguage,
  selectAvailableTargetLanguages,
  selectSelectedTargetLanguages,
  toggleLanguage,
  updateLanguageConfig,
} from "../../../../store/slices/translation-config/translationConfigSlice"
import { LanguageConfigEditor } from "./LanguageConfigEditor"
import extendedPaperbaseTheme from "../../../../layout/paperbase_theme/paperbase-theme"

export const LanguageConfigList = () => {
  const dispatch = useAppDispatch()
  const languageOptions = useAppSelector(selectAvailableTargetLanguages)
  const selectedTargetLanguages = useAppSelector(selectSelectedTargetLanguages)
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [editModalOpen, setEditModalOpen] = useState(false)
  const [editingLanguage, setEditingLanguage] = useState<{
    key: string
    config: LanguageConfig
  } | null>(null)
  const [searchQuery, setSearchQuery] = useState("")

  const handleToggle = (code: string) => () => {
    dispatch(toggleLanguage(code))
  }

  const handleRowClick = (code: string) => () => {
    setExpandedId(expandedId === code ? null : code)
  }

  const handleEdit = (key: string, config: LanguageConfig) => {
    setEditingLanguage({ key, config })
    setEditModalOpen(true)
  }

  const handleSave = (updatedConfig: LanguageConfig) => {
    if (editingLanguage) {
      dispatch(
        updateLanguageConfig({
          key: editingLanguage.key,
          config: updatedConfig,
        })
      )
    } else {
      dispatch(addCustomLanguage(updatedConfig))
    }
    setEditModalOpen(false)
    setEditingLanguage(null)
  }
  const filteredLanguages = Object.entries(languageOptions).filter(([_, lang]) => {
    const searchTerm = searchQuery.toLowerCase()
    return (
        lang.language_name.toLowerCase().includes(searchTerm) ||
        lang.language_code.toLowerCase().includes(searchTerm) ||
        lang.background.sample_text.toLowerCase().includes(searchTerm) ||
        lang.background.family_tree.some(family =>
            family.toLowerCase().includes(searchTerm)
        )
    )
  })

  return (
      <Box
          sx={{
            width: "100%",
            backgroundColor: extendedPaperbaseTheme.palette.primary.light,
            color: extendedPaperbaseTheme.palette.text.primary,
              borderRadius:2
          }}
      >
        <TextField
            fullWidth
            variant="outlined"
            placeholder="Search languages by name, code, or sample text..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            InputProps={{
              startAdornment: <Search sx={{ color: 'action.active', mr: 1 }} />,
            }}
            sx={{ mb: 2 }}
        />

        <List dense>
          {filteredLanguages.map(([key, lang]) => (
              <Paper key={key} sx={{ mb: 1 }}>
                <ListItem sx={{ pr: 1 }}>
                  <Checkbox
                      checked={selectedTargetLanguages.includes(key)}
                      onChange={handleToggle(key)}
                      sx={{ mr: 1 }}
                  />
                  <ListItemText
                      primary={
                        <Typography>
                          {lang.language_name} ({lang.language_code})
                        </Typography>
                      }
                      secondary={
                        <Typography
                            variant="body2"
                            sx={{
                              fontStyle: 'italic',
                              color: 'text.secondary',
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              display: '-webkit-box',
                              WebkitLineClamp: 2,
                              WebkitBoxOrient: 'vertical',
                            }}
                        >
                          {lang.background.sample_text}
                          {lang.romanization_method !== "NONE" && (
                              <>
                                <br />
                                {lang.background.sample_romanized_text}
                              </>
                          )}
                        </Typography>
                      }
                      onClick={handleRowClick(key)}
                      sx={{ cursor: "pointer", flex: 1 }}
                  />
                  <IconButton onClick={handleRowClick(key)}>
                    {expandedId === key ? <ExpandLess /> : <ExpandMore />}
                  </IconButton>
                </ListItem>

                <Collapse in={expandedId === key}>
                  <Box sx={{ p: 2, pt: 0 }}>
                    <Paper
                        variant="outlined"
                        sx={{ p: 2, cursor: "pointer" }}
                        onDoubleClick={() => handleEdit(key, lang)}
                    >
                      <Typography variant="subtitle2">Language Details</Typography>
                      <Typography variant="body2">
                        Family Tree: {lang.background.family_tree.join(" → ")}
                      </Typography>
                      <Typography variant="body2">
                        Alphabet: {lang.background.alphabet}
                      </Typography>
                      <Typography variant="body2">
                        Sample Text: {lang.background.sample_text}
                      </Typography>
                      {lang.romanization_method !== "NONE" && (
                          <>
                            <Typography variant="body2">
                              Romanization Method: {lang.romanization_method}
                            </Typography>
                            <Typography variant="body2">
                              Romanization Sample: {lang.background.sample_romanized_text}
                            </Typography>
                          </>
                      )}
                      <Button
                          size="small"
                          onClick={() => handleEdit(key, lang)}
                          sx={{ mt: 1 }}
                      >
                        Edit
                      </Button>
                    </Paper>
                  </Box>
                </Collapse>
              </Paper>
          ))}
        </List>

        <Button
            variant="contained"
            onClick={() => {
              setEditingLanguage(null)
              setEditModalOpen(true)
            }}
            sx={{ mt: 2 }}
        >
          Add Custom Language
        </Button>

        <LanguageConfigEditor
            open={editModalOpen}
            onClose={() => setEditModalOpen(false)}
            onSubmit={handleSave}
            initialConfig={editingLanguage?.config}
        />
      </Box>
  )
}
