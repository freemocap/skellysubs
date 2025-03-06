// skellysubs-ui/src/utils/getLanguageConfigs.ts
import type { LanguageConfig } from "../store/slices/translation-config/languageConfigSchemas"
import { LanguageConfigsSchema } from "../store/slices/translation-config/languageConfigSchemas"
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
