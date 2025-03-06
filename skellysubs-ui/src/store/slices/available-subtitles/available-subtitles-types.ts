
export const SubtitleFormats = {
    SRT: "srt",
    MD: "md",
    VTT: "vtt",
    SSA: "ssa",
} as const;

export const SubtitleVariants = {
    ORIGINAL_SPOKEN: "original_spoken",
    TRANSLATION_ONLY: "translation_only",
    TRANSLATION_WITH_ROMANIZATION: "translation_with_romanization",
    MULTI_LANGUAGE: "multi_language",
} as const;

// Type utilities
export type SubtitleVariant = typeof SubtitleVariants[keyof typeof SubtitleVariants];
export type SubtitleFormat = typeof SubtitleFormats[keyof typeof SubtitleFormats];
