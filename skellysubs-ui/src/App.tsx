import type React from "react"
import { Box } from "@mui/material"
import WelcomeContent from "./components/WelcomeContent"
import { WebSocketContextProvider } from "./services/WebsocketService/WebSocketContext"

const App: React.FC = () => {
  return (
    <WebSocketContextProvider>
      <Box
        sx={{
          textAlign: "center",
          backgroundColor: "#011415",
          color: "#fafafa",
          minHeight: "100vh",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          fontSize: "calc(10px + 2vmin)",
        }}
      >
        <WelcomeContent />
      </Box>
    </WebSocketContextProvider>
  )
}

export default App
