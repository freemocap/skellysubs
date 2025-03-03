import {
  TextField,
  Checkbox,
  FormControlLabel,
  Box,
  Autocomplete,
  CircularProgress,
  Typography,
} from "@mui/material"
import { useEffect, useState } from "react"
import type { LanguageConfig } from "../../../schemas/languageConfigSchemas"
import { getLanguageConfigs } from "../../../utils/getLanguageConfigs"

export const TranslationControls = ({
  targetLanguages,
  setTargetLanguages,
  romanize,
  setRomanize,
  romanizationMethod,
  setRomanizationMethod,
}: {
  targetLanguages: string
  setTargetLanguages: (v: string) => void
  romanize: boolean
  setRomanize: (v: boolean) => void
  romanizationMethod: string
  setRomanizationMethod: (v: string) => void
}) => {
  const [languageOptions, setLanguageOptions] = useState<LanguageConfig[]>([])

  // Load configs on component mount
  useEffect(() => {
    getLanguageConfigs()
      .then(({ language_configs }) => {
        // Convert record to array of LanguageConfig
        const options = Object.values(language_configs)
        setLanguageOptions(options)
      })
      .catch(error => console.error("Failed to load language configs:", error))
  }, [])

  if (languageOptions.length === 0) {
    return (
      <Box
        sx={{
          display: "flex",
          gap: 2,
          mb: 2,
          width: "100%",
          alignItems: "center",
        }}
      >
        <CircularProgress size={24} />
        <Typography variant="body2">Loading language options...</Typography>
      </Box>
    )
  }

  return (
    <Box sx={{ width: "100%", mb: 2 }}>
      <Autocomplete
        multiple
        freeSolo
        options={languageOptions}
        getOptionLabel={option =>
          typeof option === "string"
            ? option
            : `${option.language_name} (${option.language_code})`
        }
        value={targetLanguages.split(",").map(lang => {
          const found = languageOptions.find(
            o => o.language_code === lang.trim(),
          )
          return found || lang
        })}
        onChange={(_, values) =>
          setTargetLanguages(
            values
              .map(v => (typeof v === "string" ? v : v.language_code))
              .join(","),
          )
        }
        renderInput={params => (
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
            onChange={e => setRomanize(e.target.checked)}
          />
        }
        label="Include Romanization"
      />
      {romanize && (
        <TextField
          label="Romanization Method"
          value={romanizationMethod}
          onChange={e => setRomanizationMethod(e.target.value)}
          fullWidth
          sx={{ mt: 1 }}
          placeholder="E.g., PINYIN, ALA_LC"
        />
      )}
    </Box>
  )
}
