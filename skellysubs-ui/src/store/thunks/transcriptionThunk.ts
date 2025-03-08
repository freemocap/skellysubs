import type {
  ProcessingContext,
  TranscriptionVerbose,
} from "../slices/processing-status/processing-status-types"
import { logger } from "../../utils/logger"
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"

import { createProcessingThunk } from "./createProcessingThunk"
import { addAvailableSubtitles } from "../slices/available-subtitles/availableSubtitlesSlice"
import type {
  SubtitleFormat,
  SubtitleVariant,
} from "../slices/available-subtitles/available-subtitles-types"

interface TranscriptionResponse {
  transcript: TranscriptionVerbose
  srt_subtitles_string: string
  formatted_subtitles: {
    [format in SubtitleFormat]?: {
      [variant in SubtitleVariant]?: string
    }
  }
}

export const transcriptionThunk = createProcessingThunk<
  { language?: string; prompt?: string },
  ProcessingContext["transcription"]
>("transcription", async (context, params, thunkAPI) => {
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
      const errorBody = await transcribeResponse.text()
      logger(`Translation error response: ${errorBody}`)
      throw new Error(`HTTP error ${transcribeResponse.status}: ${errorBody}`)
    }

    const result = (await transcribeResponse.json()) as TranscriptionResponse
    logger(`Transcription result: ${JSON.stringify(result, null, 2)}`)
    // Add subtitles to available subtitles slice
    Object.entries(result.formatted_subtitles).forEach(
      ([format, subtitlesByVariant]) => {
        if (subtitlesByVariant) {
          Object.entries(subtitlesByVariant).forEach(([variant, content]) => {
            if (content) {
              thunkAPI?.dispatch(
                addAvailableSubtitles({
                  id: `source_${result.transcript.language}_${variant}_${format}`,
                  name: `${result.transcript.language} (${variant})`,
                  variant: variant as SubtitleVariant,
                  format: format as SubtitleFormat,
                  language: result.transcript.language,
                  content: content,
                }),
              )
            }
          })
        }
      },
    )
    return {
      transcript: result.transcript,
      srt_subtitles_string: result.srt_subtitles_string,
      formatted_subtitles: result.formatted_subtitles,
    } as ProcessingContext["transcription"];
  } catch (error) {
    console.error("Transcription error:", error);
    throw error instanceof Error ? error : new Error("Unknown error");
  }
});
