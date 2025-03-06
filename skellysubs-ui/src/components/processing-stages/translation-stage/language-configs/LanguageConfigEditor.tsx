import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  TextField,
  Grid,
} from "@mui/material"
import type { LanguageConfig } from "../../../../store/slices/translation-config/languageConfigSchemas"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { LanguageConfigSchema } from "../../../../store/slices/translation-config/languageConfigSchemas"

interface LanguageConfigEditorProps {
  open: boolean
  onClose: () => void
  onSubmit: (config: LanguageConfig) => void
  initialConfig?: LanguageConfig
}

export const LanguageConfigEditor = ({
  open,
  onClose,
  onSubmit,
  initialConfig,
}: LanguageConfigEditorProps) => {
  const {
    register,
    handleSubmit,
    formState: { errors },
    reset,
  } = useForm<LanguageConfig>({
    resolver: zodResolver(LanguageConfigSchema),
    defaultValues: initialConfig || {
      language_name: "",
      language_code: "",
      romanization_method: "NONE",
      background: {
        family_tree: [],
        alphabet: "",
        sample_text: "",
        sample_romanized_text: "",
        wikipedia_links: [],
      },
    },
  })

  const handleFormSubmit = (data: LanguageConfig) => {
    onSubmit(data)
    reset()
    onClose()
  }

  return (
    <Dialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <DialogTitle>
        {initialConfig ? "Edit Language" : "Add New Language"}
      </DialogTitle>
      <form onSubmit={handleSubmit(handleFormSubmit)}>
        <DialogContent>
          <Grid container spacing={2}>
            <Grid item xs={12} sm={6}>
              <TextField
                label="Language Name"
                fullWidth
                margin="dense"
                {...register("language_name")}
                error={!!errors.language_name}
                helperText={errors.language_name?.message}
              />
            </Grid>
            <Grid item xs={12} sm={6}>
              <TextField
                label="Language Code"
                fullWidth
                margin="dense"
                {...register("language_code")}
                error={!!errors.language_code}
                helperText={errors.language_code?.message}
              />
            </Grid>
            <Grid item xs={12}>
              <TextField
                label="Family Tree (comma-separated)"
                fullWidth
                margin="dense"
                {...register("background.family_tree")}
                error={!!errors.background?.family_tree}
                helperText={errors.background?.family_tree?.message}
              />
            </Grid>
            <Grid item xs={12} sm={6}>
              <TextField
                label="Alphabet"
                fullWidth
                margin="dense"
                {...register("background.alphabet")}
                error={!!errors.background?.alphabet}
                helperText={errors.background?.alphabet?.message}
              />
            </Grid>
            <Grid item xs={12} sm={6}>
              <TextField
                label="Sample Text"
                fullWidth
                margin="dense"
                {...register("background.sample_text")}
                error={!!errors.background?.sample_text}
                helperText={errors.background?.sample_text?.message}
              />
            </Grid>
          </Grid>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="contained">
            {initialConfig ? "Save Changes" : "Add Language"}
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  )
}
