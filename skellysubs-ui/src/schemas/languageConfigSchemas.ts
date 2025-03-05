// skellysubs-ui/src/schemas/languageConfigSchemas.ts
import { z } from "zod"

export const LanguageConfigSchema = z.object({
  language_name: z.string(),
  language_code: z.string(),
  romanization_method: z.string(),
  background: z.object({
    family_tree: z.array(z.string()),
    alphabet: z.string(),
    sample_text: z.string(),
    sample_romanized_text: z.string(),
    wikipedia_links: z.array(z.string().url()),
  }),
})

export type LanguageConfig = z.infer<typeof LanguageConfigSchema>

export const LanguageConfigsSchema = z.record(LanguageConfigSchema)
