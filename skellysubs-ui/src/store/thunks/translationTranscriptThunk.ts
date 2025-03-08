// translationTranscriptThunk.ts
import { getApiBaseUrl } from "../../utils/getApiBaseUrl"
import { logger } from "../../utils/logger"
import type {
  ProcessingContext,
  TranscriptionVerbose, TranslatedTranscript,
} from "../slices/processing-status/processing-status-types"
import { createProcessingThunk } from "./createProcessingThunk"
import type { LanguageConfig } from "../slices/translation-config/languageConfigSchemas"
import { addAvailableSubtitles } from "../slices/available-subtitles/availableSubtitlesSlice"
import type { SubtitleFormat, SubtitleVariant } from "../slices/available-subtitles/available-subtitles-types"


interface TranslationResponse {
  translated_transcripts: Record<string, TranslatedTranscript>;
  segment_prompts_by_language: Record<string, string[]>;
  translated_srt_subtitles: Record<string, Record<string, string>>;
  subtitles_by_language: {
    [language: string]: {
      [format: string]: {
        [variant: string]: string;
      };
    };
  };
}



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

    const result = (await translationResponse.json()) as TranslationResponse
    logger(`Translation successful!`)

    Object.entries(result.subtitles_by_language).forEach(([language, subtitleFormats]) => {
      Object.entries(subtitleFormats).forEach(([format, subtitles]) => {
        Object.entries(subtitles).forEach(([variant, content]) => {
          thunkAPI?.dispatch(
            addAvailableSubtitles({
              id: `${language}_${variant}_${format}`,
              name: `${language} (${variant})`,
              variant: variant as SubtitleVariant,
              format: format as SubtitleFormat,
              language: language,
              content: content,
            })
          )
        })
      })
    })


return {
  translated_transcripts: result.translated_transcripts,
  segment_prompts_by_language: result.segment_prompts_by_language,
  translated_srt_subtitles: result.translated_srt_subtitles
} as ProcessingContext["translation"];
  } catch (error) {
    console.error("Translation error:", error)
    throw error instanceof Error ? error : new Error("Translation failed")
  }
})