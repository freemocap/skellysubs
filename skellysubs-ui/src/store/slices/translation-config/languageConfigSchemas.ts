import { z } from "zod"

export const LanguageConfigSchema = z.object({
  language_name: z.string().min(2, "Language name is required (min 2 characters)"),
  language_code: z.string().optional().nullable(),
  romanization_method: z.string().optional().nullable(),
  background: z.object({
    family_tree: z.array(z.string()).optional().nullable(),
    alphabet: z.string().optional().nullable(),
    sample_text: z.string().optional().nullable(),
    sample_romanized_text: z.string().optional().nullable(),
    wikipedia_links: z.array(z.string().url()).optional().nullable(),
  }).optional().nullable(),
})

export type LanguageConfig = z.infer<typeof LanguageConfigSchema>

export const LanguageConfigsSchema = z.record(LanguageConfigSchema)
