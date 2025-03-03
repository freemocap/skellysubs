import { getApiBaseUrl } from "../../utils/getApiBaseUrl"

import { logger } from "../../utils/logger"
import { ProcessingContext } from "../slices/processing-status/processing-status-types"
import { createProcessingThunk } from "./createProcessingThunk"

export const translateTextThunk = createProcessingThunk<
  {
    targetLanguages?: string[]
    romanize?: boolean
    romanizationMethod?: string
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
          romanize: params?.romanize || false,
          romanization_method: params?.romanizationMethod || "",
        },
      },
      null,
      2,
    )
    logger(
      `Sending translation request to url: ${translationEndpointUrl} -- body: ${requestBody}`,
    )
    const translationResponse = await fetch(translationEndpointUrl, {
      method: "POST",
      body: requestBody,
    })

    if (!translationResponse.ok) {
      throw new Error(`HTTP error ${translationResponse.status}`)
    }

    const result = await translationResponse.json()
    logger(`Translation result: ${JSON.stringify(result, null, 2)}`)
    return result as ProcessingContext["translation"]
  } catch (error) {
    console.error("Translation error:", error)
    throw error instanceof Error ? error : new Error("Unknown error")
  }
})
