import enum

SubtitleFormattedString = str
SrtFormattedString = SubtitleFormattedString
VttFormattedString = SubtitleFormattedString
SsaFormattedString = SubtitleFormattedString
AssFormattedString = SubtitleFormattedString  # lol ((
SccFormattedString = SubtitleFormattedString  # YouTube's preference -https://support.google.com/youtube/answer/2734698?hl=en
TtmlFormattedString = SubtitleFormattedString
MdFormattedString = SubtitleFormattedString


class SubtitleFormats(enum.Enum):
    SRT: SrtFormattedString = "srt"
    VTT: VttFormattedString = "vtt"
    SSA: SsaFormattedString = "ssa"
    ASS: AssFormattedString = "ass"
    SCC: SccFormattedString = "scc"
    TTML: TtmlFormattedString = "ttml"
    MD: MdFormattedString = "md"


class SubtitleVariant(enum.Enum):
    original_spoken = "original_spoken"
    translation_only = "translation_only"
    translation_with_romanization = "translation_with_romanization"
    multi_language = "multi_language"


FormattedSubtitleStringsByVariant = dict[SubtitleVariant, SubtitleFormattedString]
SubtitleStringsByFormatByVariant = dict[SubtitleFormats, FormattedSubtitleStringsByVariant]
