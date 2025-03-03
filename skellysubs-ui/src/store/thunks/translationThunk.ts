// src/thunks/translationThunk.ts
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"
import { logger } from "../../utils/logger"
import type { ProcessingContext } from "../slices/processing-status/processing-status-types"
import { createProcessingThunk } from "./createProcessingThunk"
import { getLanguageConfigs } from "../../utils/getLanguageConfigs"
import type {
  LanguageConfig,
  LanguageConfigSchema,
} from "../../schemas/languageConfigSchemas"
import type { z } from "zod"

export const translationThunk = createProcessingThunk<
  {
    targetLanguages: Record<string, LanguageConfig>
  },
  ProcessingContext["translation"]
>("translation", async (context, params) => {
  try {
    if (!context.transcription) throw new Error("No transcription provided")
    if (!params?.targetLanguages) {
      throw new Error("No target languages provided")
    }
    const translationEndpointUrl = `${getApiBaseUrl()}/processing/translate/text`
    const requestBody = JSON.stringify(
      {
        text: context.transcription.transcript.text,
        target_languages: params.targetLanguages,
        original_langugage: context.transcription.transcript.language,
      },
      null,
      2,
    )
    logger(
      `Sending translation request to url: ${translationEndpointUrl} with body: ${JSON.stringify(requestBody, null, 2)}`,
    )
    const translationResponse = await fetch(translationEndpointUrl, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: requestBody,
    })

    if (!translationResponse.ok) {
      throw new Error(`HTTP error ${translationResponse.status}`)
    }

    const result = await translationResponse.json()
    logger(
      `Translation successful! Response: ${JSON.stringify(result, null, 2)}`,
    )
    return result as ProcessingContext["translation"]
  } catch (error) {
    console.error("Translation error:", error)
    throw error instanceof Error ? error : new Error("Translation failed")
  }
})
