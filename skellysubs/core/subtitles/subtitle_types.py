import enum

from skellysubs.core.translation.models.translation_typehints import LanguageNameString

SubtitleFormattedString = str
SrtFormattedString = SubtitleFormattedString
VttFormattedString = SubtitleFormattedString
SsaFormattedString = SubtitleFormattedString
MdFormattedString = SubtitleFormattedString


class SubtitleFormats(enum.Enum):
    SRT: SrtFormattedString = "srt"
    VTT: VttFormattedString = "vtt"
    SSA: SsaFormattedString = "ssa"
    MD: MdFormattedString = "md"

class SubtitleTypes(enum.Enum):
    original_spoken = "original_spoken"
    translation_only = "translated_script_only"
    translation_with_romanization = "translated_with_romanization"
    multi_language = "multi_language"


FormattedSubtitleStringsByType = dict[SubtitleTypes, SubtitleFormattedString]
SubtitleStringsByFormatByType = dict[SubtitleFormats, FormattedSubtitleStringsByType]
