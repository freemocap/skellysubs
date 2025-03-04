import type React from "react"
import { Box, Typography } from "@mui/material"

// const logoUrl = `${window.location.origin}/logo/skellysubs-logo.png`
// const logoUrl = "https://media.githubusercontent.com/media/freemocap/skellysubs/a30fa61cd2dcadfb3c1754548bde263afa5181e9/skellysubs-ui/public/logo/skellysubs-logo.png"
const logoPath = `${window.location.origin}/logo/skellysubs-logo.png`
const WelcomeContent: React.FC = () => {
  return (
    <Box
      sx={{
        width: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
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
          backgroundImage: `url(${logoPath})`,
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
      <br />
      <br />
      <Typography variant="body1" color="#aaa">
        (Hint: Open the browser tools with F12 (Windows) or Cmd+Option+I (macOS)
        and check the console for progress)
      </Typography>
    </Box>
  )
}

export default WelcomeContent
