import string


def strip_punctuation_and_whitespace(text:str) -> str:
    # Define additional punctuation for foreign languages
    additional_punctuation = (
        '，。、？！：；（）《》【】「」『』・〜·'
        '،؛؟'
        ';'
        '—«»'
        '–—‐'
        '…′″'
        '।՝։ฯๆ'
    )

    # Combine all punctuation
    all_punctuation = string.punctuation + additional_punctuation

    # Create translation table to remove punctuation
    translator = str.maketrans('', '', all_punctuation)

    # Strip leading and trailing whitespace and remove punctuation
    return text.strip().translate(translator)

if __name__ == '__main__':
    # Example usage
    text = "  Hello, world! 你好，世界！  مرحبا، بالعالم!  "
    cleaned_text = strip_punctuation_and_whitespace(text)
    print(cleaned_text)  # Output: "Hello world 你好 世界 مرحبا بالعالم"