import { TextField, Box, Select, MenuItem, InputLabel, FormControl } from "@mui/material";
import languageConfigs from "./language_configs.json";

export const TranscriptionControls = ({
                                          language,
                                          setLanguage,
                                          prompt,
                                          setPrompt,
                                      }: {
    language: string;
    setLanguage: (v: string) => void;
    prompt: string;
    setPrompt: (v: string) => void;
}) => (
    <Box sx={{ display: "flex", gap: 2, mb: 2, width: "100%" }}>
        <FormControl fullWidth>
            <InputLabel>Spoken Language</InputLabel>
            <Select
                value={language || "auto-detect"}
                onChange={(e) => setLanguage(e.target.value)}
                label="Spoken Language"
            >
                <MenuItem value="auto-detect">Auto-detect</MenuItem>
                {Object.values(languageConfigs.language_configs).map((config) => (
                    <MenuItem key={config.language_code} value={config.language_code}>
                        {config.language_name} ({config.language_code})
                    </MenuItem>
                ))}
            </Select>
        </FormControl>
        <TextField
            label="Transcription Prompt"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            fullWidth
            placeholder="None (optional)"
        />
    </Box>
);