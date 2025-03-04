import type { ProcessingContext } from "../slices/processing-status/processing-status-types"
import { logger } from "../../utils/logger"
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"

import { createProcessingThunk } from "./createProcessingThunk"

export const transcriptionThunk = createProcessingThunk<
  { language?: string; prompt?: string },
  ProcessingContext["transcription"]
>("transcription", async (context, params) => {
  // params now available
  try {
    if (!context.mp3Audio?.url) throw new Error("No audio URL provided")

    logger(`Fetching MP3 from URL: ${context.mp3Audio.url}`)
    const response = await fetch(context.mp3Audio.url)
    if (!response.ok) throw new Error("Failed to fetch MP3")

    const mp3Blob = await response.blob()
    logger(`MP3 Blob fetched successfully -- size: ${mp3Blob.size}`)

    const formData = new FormData()
    formData.append("audio_file", mp3Blob, "audio.mp3")
    if (params?.language) formData.append("language", params.language)
    if (params?.prompt) formData.append("prompt", params.prompt)

    const transcribeEndpointUrl = `${getApiBaseUrl()}/processing/transcribe`
    logger(
      `Sending formData to url: ${transcribeEndpointUrl} -- formData: ${JSON.stringify(formData, null, 2)}`,
    )
    const transcribeResponse = await fetch(transcribeEndpointUrl, {
      method: "POST",
      body: formData,
    })

    if (!transcribeResponse.ok) {
      throw new Error(`HTTP error ${transcribeResponse.status}`)
    }

    const result = await transcribeResponse.json()
    logger(`Transcription result: ${JSON.stringify(result, null, 2)}`)
    return result as ProcessingContext["transcription"]
  } catch (error) {
    console.error("Transcription error:", error)
    throw error instanceof Error ? error : new Error("Unknown error")
  }
})
