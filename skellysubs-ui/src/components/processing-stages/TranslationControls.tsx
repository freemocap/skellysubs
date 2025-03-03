import { TextField, Checkbox, FormControlLabel, Box, Autocomplete } from "@mui/material";
import languageConfigs from "./language_configs.json";

export const TranslationControls = ({
                                        targetLanguages,
                                        setTargetLanguages,
                                        romanize,
                                        setRomanize,
                                        romanizationMethod,
                                        setRomanizationMethod,
                                    }: {
    targetLanguages: string;
    setTargetLanguages: (v: string) => void;
    romanize: boolean;
    setRomanize: (v: boolean) => void;
    romanizationMethod: string;
    setRomanizationMethod: (v: string) => void;
}) => {
    const languageOptions = Object.values(languageConfigs.language_configs).map(
        (config) => ({
            code: config.language_code,
            name: config.language_name,
        })
    );

    return (
        <Box sx={{ width: "100%", mb: 2 }}>
            <Autocomplete
                multiple
                freeSolo
                options={languageOptions}
                getOptionLabel={(option) =>
                    typeof option === "string" ? option : `${option.name} (${option.code})`
                }
                value={targetLanguages.split(",").map(lang => {
                    const found = languageOptions.find(o => o.code === lang.trim());
                    return found || lang;
                })}
                onChange={(_, values) =>
                    setTargetLanguages(values.map(v => typeof v === "string" ? v : v.code).join(","))
                }
                renderInput={(params) => (
                    <TextField
                        {...params}
                        label="Target Languages (comma-separated)"
                        placeholder="Enter language codes or names"
                    />
                )}
            />
            <FormControlLabel
                control={
                    <Checkbox
                        checked={romanize}
                        onChange={(e) => setRomanize(e.target.checked)}
                    />
                }
                label="Include Romanization"
            />
            {romanize && (
                <TextField
                    label="Romanization Method"
                    value={romanizationMethod}
                    onChange={(e) => setRomanizationMethod(e.target.value)}
                    fullWidth
                    sx={{ mt: 1 }}
                    placeholder="E.g., PINYIN, ALA_LC"
                />
            )}
        </Box>
    );
};