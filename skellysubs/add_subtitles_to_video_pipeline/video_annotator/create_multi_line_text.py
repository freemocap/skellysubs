import jieba
from PIL import ImageFont


def create_multiline_text_chinese(text: str, font: ImageFont, screen_width: int, buffer: int) -> str:
    """
    Break a long string of Chinese text into multiple lines of text that fit within the screen width.
    Uses jieba for segmentation.
    """
    words = list(jieba.cut(text))
    lines = []
    current_line = ""
    for word in words:
        if font.getlength(current_line + word) + 2 * buffer < screen_width:
            current_line += word
        else:
            lines.append(current_line)
            current_line = word
    lines.append(current_line)
    return '\n'.join(lines)


def create_multiline_text(text: str, font: ImageFont, screen_width: int, buffer: int) -> str:
    """
    Break a long string into multiple lines of text that fit within the screen width by inserting `\n` characters
    at appropriate locations. to ensure the text will fit within the screen width with `buffer` pixels of padding on each side.
    will use `font.getlength('word1 + ' + 'word2' ...) method to determine when to break lines.

    :param text: The text to break into multiple lines
    :param font: The font to use for the text
    :param screen_width: The width of the screen
    :param buffer: The number of pixels of padding to leave on each side of the text
    """
    words = text.split()
    lines = []
    current_line = ""
    for word in words:
        if font.getlength(current_line + ' ' + word) + 2 * buffer < screen_width:
            current_line += ' ' + word
        else:
            lines.append(current_line)
            current_line = word
    lines.append(current_line)
    return '\n'.join(lines)
