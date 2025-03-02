import { logger } from "./logger"

export function getApiBaseUrl() {
  const ogUrl = window.location.origin
  let url = ogUrl

  // Check if 'localhost' is in the URL and replace the port with 8080
  if (ogUrl.includes("localhost")) {
    url = ogUrl.replace(/:\d+/, ":8080") // This uses a regular expression to replace any port with 8080
  }

  logger(`Base URL: ${url}`)
  return url // Ensure to return the modified URL
}
