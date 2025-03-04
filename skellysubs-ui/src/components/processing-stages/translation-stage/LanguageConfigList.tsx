// src/components/processing-panel/translation/LanguageConfigList/index.tsx
import { useState } from "react";
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
    Box
} from "@mui/material";
import ExpandMore from "@mui/icons-material/ExpandMore";
import ExpandLess from "@mui/icons-material/ExpandLess";
import {LanguageConfig} from "../../../schemas/languageConfigSchemas";

export const LanguageConfigList = ({
                                       languageOptions,
                                       setLanguageOptions,
                                       targetLanguages,
                                       setTargetLanguages,
                                   }: {
    languageOptions: LanguageConfig[];
    setLanguageOptions: (configs: LanguageConfig[]) => void;
    targetLanguages: string;
    setTargetLanguages: (v: string) => void;
}) => {
    const [expandedId, setExpandedId] = useState<string | null>(null);
    const [newLanguageOpen, setNewLanguageOpen] = useState(false);
    const [newLanguageName, setNewLanguageName] = useState("");

    const handleToggle = (code: string) => () => {
        const codes = targetLanguages.split(",").filter((c) => c);
        const newCodes = codes.includes(code)
            ? codes.filter((c) => c !== code)
            : [...codes, code];
        setTargetLanguages(newCodes.join(","));
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
            },
        };
        setLanguageOptions([...languageOptions, newLanguage]);
        setNewLanguageOpen(false);
        setNewLanguageName("");
    };

    const updateLanguage = (
        index: number,
        field: string,
        value: string | string[],
        nestedField?: keyof LanguageConfig["background"]
    ) => {
        const updated = [...languageOptions];
        if (nestedField) {
            (updated[index].background as any)[nestedField] = value;
        } else {
            (updated[index] as any)[field] = value;
        }
        setLanguageOptions(updated);
    };

    return (
        <Box sx={{ width: "100%", mb: 2 }}>
            <List>
                {languageOptions.map((lang, index) => (
                    <div key={lang.language_code || lang.language_name}>
                        <ListItem>
                            <Checkbox
                                checked={targetLanguages
                                    .split(",")
                                    .includes(lang.language_code)}
                                onChange={handleToggle(lang.language_code)}
                            />
                            <ListItemText
                                primary={lang.language_name}
                                secondary={lang.language_code}
                            />
                            <IconButton
                                onClick={() =>
                                    setExpandedId(
                                        expandedId === lang.language_code ? null : lang.language_code
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
                            <Box sx={{ pl: 4, display: "flex", flexDirection: "column", gap: 2 }}>
                                <TextField
                                    label="Language Name"
                                    value={lang.language_name}
                                    onChange={(e) =>
                                        updateLanguage(index, "language_name", e.target.value)
                                    }
                                    fullWidth
                                />
                                <TextField
                                    label="Language Code"
                                    value={lang.language_code}
                                    onChange={(e) =>
                                        updateLanguage(index, "language_code", e.target.value)
                                    }
                                    fullWidth
                                />
                                <TextField
                                    label="Family Tree (comma-separated)"
                                    value={lang.background.family_tree.join(", ")}
                                    onChange={(e) =>
                                        updateLanguage(
                                            index,
                                            "family_tree",
                                            e.target.value.split(",").map((s) => s.trim()),
                                            "family_tree"
                                        )
                                    }
                                    fullWidth
                                />
                                <TextField
                                    label="Alphabet"
                                    value={lang.background.alphabet}
                                    onChange={(e) =>
                                        updateLanguage(index, "alphabet", e.target.value, "alphabet")
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
                        onChange={(e) => setNewLanguageName(e.target.value)}
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
    );
};