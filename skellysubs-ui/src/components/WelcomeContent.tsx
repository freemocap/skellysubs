import type React from "react"
import { Box, TextField, Typography } from "@mui/material"
import ProcessingButtons from "./ProcessingButtons"
import extendedPaperbaseTheme from "../layout/paperbase_theme/paperbase-theme"

// const logoWebUrl =
//   "https://media.githubusercontent.com/media/freemocap/skellysubs/156ec99cc45c99a9388889aa21f7844a65f464ca/skellysubs-ui/src/assets/skellysubs-logo.png"

const logoUrl = `${window.location.origin}/logo/skellysubs-logo.png`

const WelcomeContent: React.FC = () => {
  return (
    <Box
      sx={{
        width: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        p: 4,
        mb: 4,
      }}
    >
      <Box
        sx={{
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          width: "40vmin",
          height: "40vmin",
          pointerEvents: "none",
          backgroundSize: "contain",
          backgroundRepeat: "no-repeat",
          backgroundImage: `url(${logoUrl})`,
          transition: "all 0.3s ease-in-out",
          animation: "App-logo-float infinite 3s ease-in-out",
          "@keyframes App-logo-float": {
            "0%": {
              transform: "translateY(0)",
            },
            "50%": {
              transform: "translateY(10px)",
            },
            "100%": {
              transform: "translateY(0px)",
            },
          },
        }}
      />
      <Typography variant="h2">Welcome to SkellySubs!</Typography>
      <Typography
        variant="body2"
        sx={{
          fontSize: "0.75em",
        }}
      >
        (Hint: Open the browser tools with F12 (Windows) or Cmd+Option+I (macOS)
        and check the console for progress)
      </Typography>
    </Box>
  )
}

export default WelcomeContent
