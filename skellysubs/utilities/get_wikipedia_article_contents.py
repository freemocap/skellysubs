import json
import logging
import os
import re
from datetime import datetime

import wikipediaapi

from skellysubs.ai_clients.openai_client import get_or_create_openai_client

logger = logging.getLogger(__name__)

WIKIPEDIA_TRANSLATION_SUMMARY_PROMPT = """
You are being provided with a Wikipedia page about a language - 
Extract out all the parts of this Wikipedia article
that will be most useful to someone trying to translate text into this language. 
Be thorough and detailed and SPECIFIC and extensive in your treatment! 
BE SPECIFIC! BE SPECIFIC! BE SPECIFIC!
Especially be sure to include any grammar rules, tables of verb conjugations, and other linguistic grammar etc information
Don't leave out anything that will be useful for the translator."
 
BEGIN WIKIPEDIA ARTICLE TEXT 

URL: {url}
 
{page_text} 

END WIKIPEDIA ARTICLE TEXT -
 
Remember! Your task is to extract out all the parts of this Wikipedia article
that will be most useful to someone trying to translate text into this language. 
Be thorough and detailed and SPECIFIC and extensive in your treatment! 
BE SPECIFIC! BE SPECIFIC! BE SPECIFIC!
Especially be sure to include any grammar rules, tables of verb conjugations, and other linguistic grammar etc information
Don't leave out anything that will be useful for the translator."
"""
CACHE_FILE = 'wikipedia_cache.json'

# Load existing cache or initialize an empty dictionary
if os.path.exists(CACHE_FILE):
    try:
        with open(CACHE_FILE, 'r') as file:
            cache = json.load(file)
    except Exception as e:
        cache = {}
else:
    cache = {}


async def get_wikipedia_texts(urls: list[str]) -> str:
    wiki_wiki = wikipediaapi.Wikipedia(
        user_agent='Skelly FreeMoCap (info@freemocap.org)',
        language='en',
        extract_format=wikipediaapi.ExtractFormat.WIKI
    )

    openai_client = get_or_create_openai_client()
    page_contents = []

    for url in urls:
        try:
            # Check the cache first
            if url in cache:
                logger.info(f'Loading cached content for {url}')
                page_contents.append(cache[url]['ai_summary'])
                continue
            # If not in cache, fetch the page, summarize it, and store in cache
            # Extract the page title from the URL
            match = re.search(r'/([^/]+)$', url)
            logger.info(f'Parsing {url}...')
            if match:
                page_title = match.group(1).replace('_', ' ')
                logger.info(f'Matched page title: `{page_title}`')
                page = wiki_wiki.page(page_title)

                if page.exists():
                    logger.debug(f'Found {page.title} in {url}, length: {len(page.text.split(" "))} words')
                    logger.info(
                        f"Generating translation-specifc AI summary for {url} - will be cached in ({CACHE_FILE}) for future runs")
                    ai_summary = await openai_client.make_text_generation_request(
                        system_prompt=WIKIPEDIA_TRANSLATION_SUMMARY_PROMPT.format(url=url,
                                                                                  page_text=page.text),

                    )
                    # Store in cache
                    cache[url] = {'base_content': page.text, 'ai_summary': ai_summary,
                                  'updated': datetime.now().isoformat()}
                    page_contents.append(ai_summary)
                else:
                    logger.error(f"Page does not exist for URL: {url}")
            else:
                logger.error(f"Invalid URL format: {url}")

        except Exception as e:
            logger.error(f"An error occurred pulling language info links: {e}")

    # Save the updated cache back to the file
    with open(CACHE_FILE, 'w', encoding="utf-8") as file:
        json.dump(cache, file)

    return "\n\n".join(page_contents)
