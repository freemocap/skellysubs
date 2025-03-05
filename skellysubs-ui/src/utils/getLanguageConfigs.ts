// skellysubs-ui/src/utils/getLanguageConfigs.ts
import type { LanguageConfig } from "../schemas/languageConfigSchemas"
import { LanguageConfigsSchema } from "../schemas/languageConfigSchemas"
import languageConfigs from "../language_configs.json"

export const getLanguageConfigs = async (): Promise<
  Record<string, LanguageConfig>
> => {
  try {
    return LanguageConfigsSchema.parse(languageConfigs)
  } catch (error) {
    throw new Error("Invalid language configs format: " + error)
  }
}
