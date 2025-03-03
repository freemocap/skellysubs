// src/thunks/translationThunk.ts
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"
import { logger } from "../../utils/logger"
import type { ProcessingContext } from "../slices/processing-status/processing-status-types"
import { createProcessingThunk } from "./createProcessingThunk"

export const translationThunk = createProcessingThunk<
    {
      targetLanguages?: string[]
    },
    ProcessingContext["translation"]
>("translation", async (context, params) => {
  try {
    if (!context.transcription) throw new Error("No transcription provided")

    const translationEndpointUrl = `${getApiBaseUrl()}/processing/translate`
    const requestBody = JSON.stringify(
        {
          ...context.transcription,
          translation_config: {
            target_languages: params?.targetLanguages || [],
          },
        },
        null,
        2,
    )

    logger(`Sending translation request to url: ${translationEndpointUrl}`)
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
    logger(`Translation successful for ${params?.targetLanguages?.join(", ") || "no languages"}`)
    return result as ProcessingContext["translation"]
  } catch (error) {
    console.error("Translation error:", error)
    throw error instanceof Error ? error : new Error("Translation failed")
  }
})