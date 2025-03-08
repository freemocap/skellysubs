import { Autocomplete, TextField } from "@mui/material"
import { Search } from "@mui/icons-material"
import { useAppSelector } from "../../../../store/hooks"
import { selectAvailableTargetLanguages } from "../../../../store/slices/translation-config/translationConfigSlice"

interface LanguageAutocompleteFieldProps {
  searchQuery: string
  setSearchQuery: (query: string) => void
  placeholder?: string
}

export const LanguageAutocompleteField = ({
  searchQuery,
  setSearchQuery,
  placeholder = "Search languages..."
}: LanguageAutocompleteFieldProps) => {
  const availableLanguages = useAppSelector(selectAvailableTargetLanguages)
  const options = Object.entries(availableLanguages).map(([key, lang]) => ({
    key,
    label: `${lang.language_name} (${lang.background?.sample_text})`,
    ...lang
  }))

  return (
    <Autocomplete
      fullWidth
      options={options}
      getOptionLabel={(option) => option.label}
      filterOptions={(options, { inputValue }) => {
        const searchTerm = inputValue.toLowerCase()
        return options.filter(option =>
          option.label.toLowerCase().includes(searchTerm) ||
          option.background?.sample_text?.toLowerCase().includes(searchTerm) ||
          option.background?.family_tree?.some(family =>
            family.toLowerCase().includes(searchTerm)
          )
        )
      }}
      onInputChange={(_, newValue) => setSearchQuery(newValue)}
      inputValue={searchQuery}
      renderInput={(params) => (
        <TextField
          {...params}
          placeholder={placeholder}
          variant="outlined"
          InputProps={{
            ...params.InputProps,
            startAdornment: <Search sx={{ color: 'action.active', mr: 1 }} />,
          }}
        />
      )}
      sx={{ mb: 2 }}
    />
  )
}
