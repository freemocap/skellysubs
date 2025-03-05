import { Box } from "@mui/material"
import { LanguageConfigList } from "./LanguageConfigList"

export const TranslationControls = () => {
  return (
    <Box sx={{ width: "100%", mb: 2 }}>
      <LanguageConfigList />
    </Box>
  )
}