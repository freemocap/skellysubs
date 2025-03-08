import { getApiBaseUrl } from "../../utils/getApiBaseUrl"
import { logger } from "../../utils/logger"
import type {
  ProcessingContext,
  TranscriptionVerbose,
} from "../slices/processing-status/processing-status-types"
import { createProcessingThunk } from "./createProcessingThunk"
import type { LanguageConfig } from "../slices/translation-config/languageConfigSchemas"
import {addAvailableSubtitles} from "../slices/available-subtitles/availableSubtitlesSlice";
import type {SubtitleFormat, SubtitleVariant} from "../slices/available-subtitles/available-subtitles-types";

export const translationTranscriptThunk = createProcessingThunk<
  {
    transcript: TranscriptionVerbose
    targetLanguages: Record<string, LanguageConfig>
  },
  ProcessingContext["translation"]
>("translation", async (context, params, thunkAPI) => {
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

    // Add translated subtitles to available subtitles slice
    Object.entries(result.subtitles_by_language).forEach(([language, subtitleFormats]) => {
      Object.entries(subtitleFormats).forEach(([format, subtitles]) => {
        Object.entries(subtitles).forEach(([variant, content]) => {
          thunkAPI?.dispatch(addAvailableSubtitles({
            id: `${language}_${variant}_${format}`,
            name: `${language} (${variant})`,
            variant: variant as SubtitleVariant,
            format: format as SubtitleFormat,
            language: language,
            content: content as string
          }));
        });
      });
    });
    return result as ProcessingContext["translation"]
  } catch (error) {
    console.error("Translation error:", error)
    throw error instanceof Error ? error : new Error("Translation failed")
  }
})
