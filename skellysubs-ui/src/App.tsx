import type React from "react"
import { Box } from "@mui/material"
import WelcomeContent from "./components/WelcomeContent"

const App: React.FC = () => {
  return (
    <Box
      sx={{
        textAlign: "center",
        backgroundColor: "#011415",
        color: "#aaaaaa",
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
  )
}

export default App
