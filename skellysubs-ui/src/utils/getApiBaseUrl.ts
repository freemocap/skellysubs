export function getApiBaseUrl() {
  const url = window.location.origin
  // TODO - not this
  return url.replace("5173", "8080")
}
