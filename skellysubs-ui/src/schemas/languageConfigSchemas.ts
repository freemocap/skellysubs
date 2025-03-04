import { z } from "zod"

export const LanguageBackgroundSchema = z.object({
  family_tree: z.array(z.string()),
  alphabet: z.string(),
  sample_text: z.string(),
  sample_romanized_text: z.string().optional(),
})

export const LanguageConfigSchema = z.object({
  language_name: z.string(),
  language_code: z.string(),
  romanization_method: z.string(),
  background: LanguageBackgroundSchema,
})

export const LanguageConfigsSchema = z.record(z.string(), LanguageConfigSchema)

export type LanguageConfig = z.infer<typeof LanguageConfigSchema>
