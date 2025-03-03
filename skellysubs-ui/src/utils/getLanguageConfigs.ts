import { LanguageConfigsSchema } from "../schemas/languageConfigSchemas"
import languageConfigs from "../language_configs.json"

export const getLanguageConfigs = async () => {
  try {
    // Validate the loaded JSON against the schema
      return LanguageConfigsSchema.parse(languageConfigs)
  } catch (error) {
    throw new Error("Invalid language configs format: " + error)
  }
}