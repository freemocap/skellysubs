import { getApiBaseUrl } from "../../../../utils/getApiBaseUrl"
import { logger } from "../../../../utils/logger"
import type {
  ProcessingContext,
  TranscriptionVerbose,
} from "../processing-status-types"
import { createProcessingThunk } from "./createProcessingThunk"
import type { LanguageConfig } from "../../../../schemas/languageConfigSchemas"

export const translationTranscriptThunk = createProcessingThunk<
  {
    transcript: TranscriptionVerbose
    targetLanguages: Record<string, LanguageConfig>
  },
  ProcessingContext["translation"]
>("translation", async (context, params) => {
  try {
    if (!context.transcription) throw new Error("No transcription provided")
    if (!params?.targetLanguages) {
      throw new Error("No target languages provided")
    }
    const translationEndpointUrl = `${getApiBaseUrl()}/processing/translate/transcript`
    const requestBody = JSON.stringify(
      {
        transcript: params.transcript,
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
