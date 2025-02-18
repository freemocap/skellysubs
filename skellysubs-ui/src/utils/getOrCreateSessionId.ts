import { v4 as uuidv4 } from "uuid"

export const getOrCreateSessionId = (): string => {
  const storedSessionId = localStorage.getItem("skellysubs_session_id")
  if (storedSessionId) {
    console.log(`Found existing session ID: ${storedSessionId}`)
    return storedSessionId
  }
  const newSessionId = uuidv4()
  localStorage.setItem("skellysubs_session_id", newSessionId)
  console.log(`Created new session ID: ${newSessionId}`)
  return newSessionId
}
