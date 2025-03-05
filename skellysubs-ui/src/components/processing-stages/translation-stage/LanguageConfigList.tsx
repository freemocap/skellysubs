import { useState } from "react"
import {
  List,
  ListItem,
  ListItemText,
  Collapse,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  TextField,
  Checkbox,
  IconButton,
  Box,
} from "@mui/material"
import ExpandMore from "@mui/icons-material/ExpandMore"
import ExpandLess from "@mui/icons-material/ExpandLess"
import type { LanguageConfig } from "../../../schemas/languageConfigSchemas"
import {useAppDispatch, useAppSelector} from "../../../store/hooks";
import {
  addCustomLanguage,
  selectLanguageOptions, selectSelectedTargetLanguages, toggleLanguage,
  updateLanguageConfig
} from "../../../store/slices/processing-configs/translationConfigSlice";

export const LanguageConfigList = () => {
  const dispatch = useAppDispatch();
  const languageOptions = useAppSelector(selectLanguageOptions);
  const selectedTargetLanguages = useAppSelector(selectSelectedTargetLanguages);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [newLanguageOpen, setNewLanguageOpen] = useState(false);
  const [newLanguageName, setNewLanguageName] = useState("");

  const handleToggle = (code: string) => () => {
    dispatch(toggleLanguage(code));
  };


  const handleAddLanguage = () => {
    const newLanguage: LanguageConfig = {
      language_name: newLanguageName,
      language_code: "",
      romanization_method: "NONE",
      background: {
        family_tree: [],
        alphabet: "",
        sample_text: "",
        sample_romanized_text: "",
        wikipedia_links: [],
      },
    };
    dispatch(addCustomLanguage(newLanguage));
    setNewLanguageOpen(false);
    setNewLanguageName("");
  };

  const updateLanguage = (
      key: string,
      field: string,
      value: string | string[],
      nestedField?: keyof LanguageConfig["background"],
  ) => {
    const updatedLang = { ...languageOptions[key] };

    if (nestedField) {
      (updatedLang.background as any)[nestedField] = value;
    } else {
      (updatedLang as any)[field] = value;
    }

    dispatch(updateLanguageConfig({ key, config: updatedLang }));
  };

  return (
    <Box sx={{ width: "100%", mb: 2 }}>
      <List>
        {Object.entries(languageOptions).map(([key, lang]) => (
          <div key={key}>
            <ListItem>
              <Checkbox
                  checked={selectedTargetLanguages.includes(lang.language_code)}
                  onChange={handleToggle(lang.language_code)}
              />
              <ListItemText
                primary={lang.language_name}
                secondary={lang.language_code}
              />
              <IconButton
                onClick={() =>
                  setExpandedId(
                    expandedId === lang.language_code
                      ? null
                      : lang.language_code,
                  )
                }
              >
                {expandedId === lang.language_code ? (
                  <ExpandLess />
                ) : (
                  <ExpandMore />
                )}
              </IconButton>
            </ListItem>
            <Collapse in={expandedId === lang.language_code}>
              <Box
                sx={{ pl: 4, display: "flex", flexDirection: "column", gap: 2 }}
              >
                <TextField
                  label="Language Name"
                  value={lang.language_name}
                  onChange={e =>
                    updateLanguage(key, "language_name", e.target.value)
                  }
                  fullWidth
                />
                <TextField
                  label="Language Code"
                  value={lang.language_code}
                  onChange={e =>
                    updateLanguage(key, "language_code", e.target.value)
                  }
                  fullWidth
                />
                <TextField
                  label="Family Tree (comma-separated)"
                  value={lang.background.family_tree.join(", ")}
                  onChange={e =>
                    updateLanguage(
                      key,
                      "family_tree",
                      e.target.value.split(",").map(s => s.trim()),
                      "family_tree",
                    )
                  }
                  fullWidth
                />
                <TextField
                  label="Alphabet"
                  value={lang.background.alphabet}
                  onChange={e =>
                    updateLanguage(key, "alphabet", e.target.value, "alphabet")
                  }
                  fullWidth
                />
              </Box>
            </Collapse>
          </div>
        ))}
      </List>

      <Button variant="outlined" onClick={() => setNewLanguageOpen(true)}>
        Add Custom Language
      </Button>

      <Dialog open={newLanguageOpen} onClose={() => setNewLanguageOpen(false)}>
        <DialogTitle>Add New Language</DialogTitle>
        <DialogContent>
          <TextField
            autoFocus
            label="Language Name *"
            fullWidth
            value={newLanguageName}
            onChange={e => setNewLanguageName(e.target.value)}
            sx={{ mt: 2 }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setNewLanguageOpen(false)}>Cancel</Button>
          <Button onClick={handleAddLanguage} disabled={!newLanguageName}>
            Add
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  )
}
