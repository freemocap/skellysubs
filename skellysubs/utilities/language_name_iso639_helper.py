from iso639 import Lang
import  logging
logger = logging.getLogger(__name__)
def language_name_from_iso_639_code(code:str) -> str:
    language = Lang(code)
    logger.trace(f"Language name from ISO 639 code: {code} is {language.name}")
    return language.name