import { TextField } from "@mui/material"
import { Search } from "@mui/icons-material"

interface LanguageSearchFieldProps {
  searchQuery: string
  setSearchQuery: (query: string) => void
  placeholder?: string
}

export const LanguageSearchField = ({
  searchQuery,
  setSearchQuery,
  placeholder = "Search languages..."
}: LanguageSearchFieldProps) => {
  return (
    <TextField
      fullWidth
      variant="outlined"
      placeholder={placeholder}
      value={searchQuery}
      onChange={(e) => setSearchQuery(e.target.value)}
      InputProps={{
        startAdornment: <Search sx={{ color: 'action.active', mr: 1 }} />,
      }}
      sx={{ mb: 2 }}
    />
  )
}
