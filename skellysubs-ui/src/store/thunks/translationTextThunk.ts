// src/thunks/translationThunk.ts
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"
import { logger } from "../../utils/logger"
import type { ProcessingContext } from "../slices/processing-status/processing-status-types"
import { createProcessingThunk } from "./createProcessingThunk"
import { getLanguageConfigs } from "../../utils/getLanguageConfigs"
import type {
  LanguageConfig,
  LanguageConfigSchema,
} from "../slices/translation-config/languageConfigSchemas"
import type { z } from "zod"

export const translationTextThunk = createProcessingThunk<
  {
    text: string
    targetLanguages: Record<string, LanguageConfig>
    originalLanguage: string
  },
  ProcessingContext["translation"]
>("translation", async (context, params) => {
  try {
    if (!context.transcription) throw new Error("No transcription provided")
    if (!params?.targetLanguages) {
      throw new Error("No target languages provided")
    }
    const translationEndpointUrl = `${getApiBaseUrl()}/processing/translate/text?original_language=${params.originalLanguage}`
    const requestBody = JSON.stringify(
      {
        text: params.text,
        target_languages: params.targetLanguages,
      },
      null,
      2,
    )
    logger(`Sending translation request to url: ${translationEndpointUrl}...`)
    const translationResponse = await fetch(translationEndpointUrl, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: requestBody,
    })

    if (!translationResponse.ok) {
      const errorBody = await translationResponse.text()
      logger(`Translation error response: ${errorBody}`)
      throw new Error(`HTTP error ${translationResponse.status}: ${errorBody}`)
    }

    const result = await translationResponse.json()
    logger(`Translation successful!`)
    return result as ProcessingContext["translation"]
  } catch (error) {
    console.error("Translation error:", error)
    throw error instanceof Error ? error : new Error("Translation failed")
  }
})
