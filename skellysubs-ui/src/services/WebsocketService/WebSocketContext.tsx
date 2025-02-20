import type React from "react"
import type { ReactNode } from "react"
import { createContext, useContext } from "react"
import { useWebSocket } from "./useWebSocket"

interface WebSocketContextProps {
  isConnected: boolean
  connect: () => void
  disconnect: () => void
}

interface WebSocketProviderProps {
  children: ReactNode
}

const WebSocketContext = createContext<WebSocketContextProps | undefined>(
  undefined,
)

export const WebSocketContextProvider: React.FC<WebSocketProviderProps> = ({
  children,
}) => {
  const { isConnected, connect, disconnect } = useWebSocket()

  return (
    <WebSocketContext.Provider value={{ isConnected, connect, disconnect }}>
      {children}
    </WebSocketContext.Provider>
  )
}

export const useWebSocketContext = () => {
  const context = useContext(WebSocketContext)
  if (!context) {
    throw new Error(
      "useWebSocketContext must be used within a WebSocketProvider",
    )
  }
  return context
}
