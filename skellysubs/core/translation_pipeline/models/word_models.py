import enum
from abc import ABC

from pydantic import BaseModel, Field


# TODO - The word type matching thing is not functional/implemented, but i think its probably pretty close?


class WordTypeABC(BaseModel, ABC):
    word_string: str = Field(description="The text of the word in question")
    word_romanization: str | None = Field(default=None, description="The romanization of the word, if applicable")
    word_type: str = Field(description="The type of word, e.g. noun, verb, etc.")
    slang: bool | None = Field(default=None,
                               description="Whether the word is slang or not, e.g. informal language, etc.")
    definition: str | None = Field(default=None, description="The definition of the word, if applicable")
    onomatopoeia: bool | None = Field(default=None,
                                      description="Whether the word is an onomatopoeia or not, e.g. a word that sounds like the sound it describes, etc.")
    word_definition: str | None = Field(default=None, description="The definition of the word, if applicable")
    linguistic_annotation: str | None = Field(default=None,
                                              description="Any linguistic annotations need to understand the word, if applicable")
    context_annotation: str | None = Field(default=None,
                                           description="Any annotations needed to understand the use of this word in this context, if applicable")


class UnknownWordType(WordTypeABC):
    linguistic_annotation:str = "WORD-NOT-YET-CATEGORIZED"

class OtherWordType(WordTypeABC):
    pass


class NounType(WordTypeABC):
    word_type: str = Field(default="noun", description="The type of word, e.g. noun, verb, etc.", frozen=True)
    proper_name: bool | None = Field(default=None,
                                     description="Whether the word is a proper name or not, e.g. a person's name, a place name, etc.")
    animate_object: bool | None = Field(default=None,
                                        description="Whether the word refers to an animate object, e.g. a person, animal, etc.")
    abstract_object: bool | None = Field(default=None,
                                         description="Whether the word refers to an abstract object, e.g. an idea, concept, etc.")
    countable_object: bool | None = Field(default=None,
                                          description="Whether the word refers to a countable object, e.g. a chair, a book, etc.")
    mass_object: bool | None = Field(default=None,
                                     description="Whether the word refers to a mass object, e.g. water, air, etc.")


class VerbType(WordTypeABC):
    word_type: str = Field(default="verb", description="The type of word, e.g. noun, verb, etc.", frozen=True)
    transitive: bool | None = Field(default=None,
                                    description="Whether the verb is transitive or not, e.g. requires a direct object, etc.")
    tense: str | None = Field(default=None, description="The tense of the verb, e.g. past, present, future, etc.")


class AdjectiveType(WordTypeABC):
    word_type: str = Field(default="adjective", description="The type of word, e.g. noun, verb, etc.", frozen=True)
    comparative: bool | None = Field(default=None,
                                     description="Whether the adjective is comparative or not, e.g. taller, shorter, etc.")
    superlative: bool | None = Field(default=None,
                                     description="Whether the adjective is superlative or not, e.g. tallest, shortest, etc.")


class AdverbType(WordTypeABC):
    word_type: str = Field(default="adverb", description="The type of word, e.g. noun, verb, etc.", frozen=True)
    comparative: bool | None = Field(default=None,
                                     description="Whether the adverb is comparative or not, e.g. more quickly, etc.")
    superlative: bool | None = Field(default=None,
                                     description="Whether the adverb is superlative or not, e.g. most quickly, etc.")


class PronounType(WordTypeABC):
    word_type: str = Field(default="pronoun", description="The type of word, e.g. noun, verb, etc.", frozen=True)
    person: int | None = Field(default=None,
                               description="The person of the pronoun, e.g. first person, second person, third person, etc.")
    number: int | None = Field(default=None, description="The number of the pronoun, e.g. singular, plural etc.")
    formality: str | None = Field(default=None,
                                  description="The formality of the pronoun, e.g. formal, informal, etc (if applicable, else None).")


class WordTypeSchemas(str,enum.Enum):
    NOUN = NounType.model_json_schema()
    VERB = VerbType.model_json_schema()
    ADJECTIVE = AdjectiveType.model_json_schema()
    ADVERB = AdverbType.model_json_schema()
    PRONOUN = PronounType.model_json_schema()
    OTHER = OtherWordType.model_json_schema()
    NOT_PROCESSED = UnknownWordType.model_json_schema()