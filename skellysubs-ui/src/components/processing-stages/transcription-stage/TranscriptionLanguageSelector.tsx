// LanguageSelector.tsx
import { Autocomplete, TextField } from "@mui/material";
import iso6391 from 'iso-639-1';
import {useMemo} from "react";

export const TranscriptionLanguageSelector = ({ value, onChange }: {
    value: string;
    onChange: (code: string) => void;
}) => {
    const languageOptions = useMemo(() => {
        const allLanguages = iso6391.getAllCodes().map(code => ({
            code,
            name: iso6391.getName(code)
        }));

        return [
            { code: "auto-detect", name: "Auto-detect" },
            ...allLanguages
        ];
    }, []);

    const selectedLanguage = languageOptions.find(opt => opt.code === value) || languageOptions[0];

    return (
        <Autocomplete
            options={languageOptions}
            getOptionLabel={(option) => `${option.name} (${option.code.toUpperCase()})`}
            filterOptions={(options, { inputValue }) =>
                options.filter(option =>
                    option.name.toLowerCase().includes(inputValue.toLowerCase()) ||
                    option.code.toLowerCase().includes(inputValue.toLowerCase())
                )
            }
            value={selectedLanguage}
            onChange={(_, newValue) => {
                onChange(newValue?.code || "auto-detect");
            }}
            renderInput={(params) => (
                <TextField
                    {...params}
                    label="Spoken Language"
                    placeholder="Search for a language or code"
                    fullWidth
                />
            )}
        />
    );
};