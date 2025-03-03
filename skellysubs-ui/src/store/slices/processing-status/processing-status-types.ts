import type { LanguageConfig } from "../../../schemas/languageConfigSchemas"

export interface AudioVisualFile {
  url: string
  name: string
  type: string
  size: number
  bitrate: number
  duration?: number
}

export interface WordSegment {
  word: string
  start: number
  end: number
}

export interface TranscriptionVerbose {
  text: string
  language: string
  duration: number
  words: WordSegment[]
  segments: {
    id: number
    text: string
    start: number
    end: number
  }[]
}

export interface TranscriptionResult {
  transcript: TranscriptionVerbose
  srt_subtitles_string: string
}
interface MatchedTranslatedWord {
  original_language: string
  start_time: number
  end_time: number
  target_language: LanguageConfig
  original_word_text: string
  original_word_index: number
  translated_word_text: string
  translated_word_romanized_text: string | null
  translated_word_index: number
}

interface MatchedTranslatedSegment {
  start: number
  end: number
  target_language_config: LanguageConfig
  original_segment_text: string
  translated_segment_text: string
  romanized_translated_text: string | null
  original_words_list: string[]
  translated_words_list: string[]
  romanized_translated_words_list: string[] | null
  matched_translated_words: MatchedTranslatedWord[]
}

interface TranslatedWhisperWordTimestamp {
  start: number
  end: number
  original_word: string
  matched_words: Record<string, MatchedTranslatedWord>
}

interface TranscriptSegment {
  original_segment_text: string
  original_language: string
  translations: TranslationsCollection
  start: number
  end: number
  matched_translated_segment_by_language: Record<
    string,
    MatchedTranslatedSegment
  > | null
  words: TranslatedWhisperWordTimestamp[]
}

interface TranslationsCollection {
  translations: Record<
    string,
    {
      translated_text: string
      romanized_text: string
      translated_language_name: string
      romanization_method: string
    }
  >
}

export interface TranslatedTranscription {
  original_text: string
  original_language: string
  translations: TranslationsCollection
  segments: TranscriptSegment[]
}

export class MatchingResult {}

export interface ProcessingStage {
  name: string
  requirements: (keyof ProcessingContext)[]
  status: "idle" | "ready" | "processing" | "completed" | "failed"
  produces: keyof ProcessingContext
  error: string | null
}

export interface ProcessingContext {
  originalFile?: AudioVisualFile
  mp3Audio?: AudioVisualFile
  transcription?: TranscriptionResult
  translation?: TranslatedTranscription
}
export interface ProcessingState {
  context: ProcessingContext
  stages: Record<string, ProcessingStage>
}
