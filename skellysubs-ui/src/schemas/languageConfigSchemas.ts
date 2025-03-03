import { z } from "zod";

export const LanguageBackgroundSchema = z.object({
    family_tree: z.array(z.string()),
    alphabet: z.string(),
    sample_text: z.string(),
    sample_romanized_text: z.string(),
});

export const LanguageConfigSchema = z.object({
    language_name: z.string(),
    language_code: z.string(),
    romanization_method: z.string(),
    background: LanguageBackgroundSchema,
});

export const LanguageAnnotationConfigSchema = z.object({
    font_file: z.string(),
    font_size_ratio: z.number().default(1).optional(),
    buffer_size: z.number(),
    color: z.tuple([z.number(), z.number(), z.number()]),
});

export const LanguageConfigsSchema = z.object({
    language_configs: z.record(z.string(), LanguageConfigSchema),
    annotation_configs: z.record(z.string(), LanguageAnnotationConfigSchema),
});

export type LanguageConfig = z.infer<typeof LanguageConfigSchema>;
export type LanguageAnnotationConfig = z.infer<typeof LanguageAnnotationConfigSchema>;