import { Button } from "@mui/material"
import extendedPaperbaseTheme from "../../../layout/paperbase_theme/paperbase-theme";

interface DownloadTranslationButtonProps {
  onClick: () => void
}

export const DownloadTranslationButton = ({ onClick }: DownloadTranslationButtonProps) => (
  <Button
    variant="contained"
    onClick={onClick}
    sx={{
      backgroundColor: extendedPaperbaseTheme.palette.primary.light,
      borderColor: "#222222",
      borderStyle: "solid",
      borderWidth: "1px",
    }}
  >
    Download translation results
  </Button>
)
