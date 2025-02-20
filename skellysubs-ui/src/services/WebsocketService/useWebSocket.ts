import { useCallback, useEffect, useState } from "react"
import { z } from "zod"

const MAX_RECONNECT_ATTEMPTS = 20

function getOrCreateSessionId() {
  // Check if sessionId is already in localStorage and make a new uuid if not
  const existingSessionId = localStorage.getItem("sessionId")
  if (existingSessionId) {
    return existingSessionId
  } else {
    const newSessionId = crypto.randomUUID()
    localStorage.setItem("sessionId", newSessionId)
    return newSessionId
  }
}

const wsUrl: string =
  window.location.origin.replace("http", "ws") +
  `/websocket/connect/${getOrCreateSessionId()}`

export const useWebSocket = () => {
  const [isConnected, setIsConnected] = useState(false)
  const [websocket, setWebSocket] = useState<WebSocket | null>(null)
  const [connectAttempt, setConnectAttempt] = useState(0)

  const connect = useCallback(() => {
    const handleIncomingMessage = (data: Blob | string) => {
      if (typeof data === "string") {
        parseAndValidateMessage(data)
      } else if (data instanceof Blob) {
        // If data is a Blob, convert it to text
        data
          .text()
          .then(text => {
            parseAndValidateMessage(text)
          })
          .catch(error => {
            console.error("Error reading Blob data:", error)
          })
      }
    }

    const parseAndValidateMessage = (data: string) => {
      try {
        const parsedData = JSON.parse(data)
        console.log("Parsed websocket data:", parsedData)
      } catch (e) {
        if (e instanceof z.ZodError) {
          console.error(
            "Validation failed with errors:",
            JSON.stringify(e.errors, null, 2),
          )
        } else {
          console.log(`Websocket message: ${data}`)
        }
      }
    }

    if (websocket && websocket.readyState !== WebSocket.CLOSED) {
      return
    }
    if (connectAttempt >= MAX_RECONNECT_ATTEMPTS) {
      console.error(
        `Max reconnection attempts reached. Could not connect to ${wsUrl}`,
      )
      return
    }
    const ws = new WebSocket(wsUrl)

    ws.onopen = () => {
      setIsConnected(true)
      setConnectAttempt(0)
      console.log(`Websocket is connected to url: ${wsUrl}`)
    }

    ws.onclose = () => {
      setIsConnected(false)
      setConnectAttempt(prev => prev + 1)
    }

    ws.onmessage = event => {
      // console.log('Websocket message received with length: ', event.data.length);
      handleIncomingMessage(event.data)
    }

    ws.onerror = error => {
      console.error("Websocket error:", error)
    }

    setWebSocket(ws)
  }, [websocket, connectAttempt])

  const disconnect = useCallback(() => {
    if (websocket) {
      websocket.close()
      setWebSocket(null)
    }
  }, [websocket])

  useEffect(() => {
    const timeout = setTimeout(
      () => {
        console.log(
          `Connecting (attempt #${connectAttempt + 1} of ${MAX_RECONNECT_ATTEMPTS}) to websocket at url: ${wsUrl}`,
        )
        connect()
      },
      Math.min(1000 * Math.pow(2, connectAttempt), 30000),
    ) // exponential backoff

    return () => {
      clearTimeout(timeout)
    }
  }, [connect, connectAttempt])

  return {
    isConnected,
    connect,
    disconnect,
  }
}
